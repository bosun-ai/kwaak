use std::sync::Arc;

use anyhow::{Context as _, Result};
use swiftide::{
    agents::{
        system_prompt::SystemPrompt, tools::local_executor::LocalExecutor, Agent, DefaultContext,
    },
    chat_completion::{self, ChatCompletion, Tool},
    prompt::Prompt,
    traits::{Command, SimplePrompt, ToolExecutor},
};
use tavily::Tavily;
use uuid::Uuid;

use super::{
    conversation_summarizer::ConversationSummarizer, env_setup::EnvSetup,
    tool_summarizer::ToolSummarizer, tools,
};
use crate::{
    commands::CommandResponder, config::SupportedToolExecutors, git::github::GithubSession,
    indexing, repository::Repository, util::accept_non_zero_exit,
};
use swiftide_docker_executor::DockerExecutor;

async fn generate_initial_context(repository: &Repository, query: &str) -> Result<String> {
    let retrieved_context = indexing::query(repository, &query).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

pub fn available_tools(
    repository: &Repository,
    github_session: Option<&Arc<GithubSession>>,
) -> Result<Vec<Box<dyn Tool>>> {
    let query_pipeline = indexing::build_query_pipeline(repository)?;

    let mut tools = vec![
        tools::read_file(),
        tools::write_file(),
        tools::search_file(),
        tools::git(),
        tools::shell_command(),
        tools::search_code(),
        tools::fetch_url(),
        tools::ExplainCode::new(query_pipeline).boxed(),
        tools::RunTests::new(&repository.config().commands.test).boxed(),
        tools::RunCoverage::new(&repository.config().commands.coverage).boxed(),
    ];

    if let Some(github_session) = github_session {
        tools.push(tools::CreateOrUpdatePullRequest::new(github_session).boxed());
        tools.push(tools::GithubSearchCode::new(github_session).boxed());
    }

    if let Some(tavily_api_key) = &repository.config().tavily_api_key {
        let tavily = Tavily::builder(tavily_api_key.expose_secret()).build()?;
        tools.push(tools::SearchWeb::new(tavily, tavily_api_key.clone()).boxed());
    };

    Ok(tools)
}

async fn start_tool_executor(uuid: Uuid, repository: &Repository) -> Result<Box<dyn ToolExecutor>> {
    let boxed = match repository.config().tool_executor {
        SupportedToolExecutors::Docker => {
            let mut executor = DockerExecutor::default();
            let dockerfile = &repository.config().docker.dockerfile;

            if std::fs::metadata(dockerfile).is_err() {
                tracing::error!("Dockerfile not found at {}", dockerfile.display());
                // TODO: Clean me up
                panic!("Running in docker requires a Dockerfile");
            }
            let running_executor = executor
                .with_context_path(&repository.config().docker.context)
                .with_image_name(&repository.config().project_name)
                .with_dockerfile(dockerfile)
                .with_container_uuid(uuid)
                .to_owned()
                .start()
                .await?;

            Box::new(running_executor) as Box<dyn ToolExecutor>
        }
        SupportedToolExecutors::Local => Box::new(LocalExecutor::new(".")) as Box<dyn ToolExecutor>,
    };

    Ok(boxed)
}

#[tracing::instrument(skip(repository, command_responder))]
pub async fn build_agent(
    uuid: Uuid,
    repository: &Repository,
    query: &str,
    command_responder: CommandResponder,
) -> Result<Agent> {
    command_responder.send_update("starting up agent for the first time, this might take a while");

    let query_provider: Box<dyn ChatCompletion> =
        repository.config().query_provider().try_into()?;
    let fast_query_provider: Box<dyn SimplePrompt> =
        repository.config().indexing_provider().try_into()?;

    let github_session = match repository.config().github.token {
        Some(_) => Some(Arc::new(GithubSession::from_repository(&repository)?)),
        None => None,
    };

    let tools = available_tools(&repository, github_session.as_ref())?;

    let system_prompt = build_system_prompt(&repository)?;
    let ((), executor, initial_context) = tokio::try_join!(
        rename_chat(&query, &fast_query_provider, &command_responder),
        start_tool_executor(uuid, &repository),
        generate_initial_context(&repository, query)
    )?;

    let env_setup = EnvSetup::new(uuid, &repository, github_session.as_deref(), &*executor);
    env_setup.exec_setup_commands().await?;

    let mut context = DefaultContext::from_executor(executor);

    if repository.config().endless_mode {
        context.with_stop_on_assistant(false);
    }

    let command_responder = Arc::new(command_responder);
    let tx_2 = command_responder.clone();
    let tx_3 = command_responder.clone();
    let tx_4 = command_responder.clone();

    let tool_summarizer =
        ToolSummarizer::new(fast_query_provider, &["run_tests", "run_coverage"], &tools);
    let conversation_summarizer = ConversationSummarizer::new(query_provider.clone(), &tools);
    let maybe_lint_fix_command = repository.config().commands.lint_and_fix.clone();

    let agent = Agent::builder()
        .context(context)
        .system_prompt(system_prompt)
        .tools(tools)
        .before_all(move |context| {
            let initial_context = initial_context.clone();

            Box::pin(async move {
                context
                    .add_message(chat_completion::ChatMessage::new_user(initial_context))
                    .await;

                let top_level_project_overview = context.exec_cmd(&Command::shell("fd -d2")).await?.output;
                context.add_message(chat_completion::ChatMessage::new_user(format!("The following is a max depth 2, high level overview of the directory structure of the project: \n ```{top_level_project_overview}```"))).await;

                Ok(())
            })
        })
        .on_new_message(move |_, message| {
            let command_responder = tx_2.clone();
            let message = message.clone();

            Box::pin(async move {
                command_responder.send_message(message);

                Ok(())
            })
        })
        .before_completion(move |_, _| {
            let command_responder = tx_3.clone();
            Box::pin(async move {
                command_responder.send_update("running completions");
                Ok(())
            })
        })
        .before_tool(move |_, tool| {
            let command_responder = tx_4.clone();
            let tool = tool.clone();
            Box::pin(async move {
                command_responder.send_update(format!("running tool {}", tool.name()));
                Ok(())
            })
        })
        .after_tool(tool_summarizer.summarize_hook())
        .after_each(move |context| {
            let maybe_lint_fix_command = maybe_lint_fix_command.clone();
            let command_responder = command_responder.clone();
            Box::pin(async move {
                if accept_non_zero_exit(
                    context
                        .exec_cmd(&Command::shell("git status --porcelain"))
                        .await,
                )
                .context("Could not determine git status")?
                .is_empty()
                {
                    tracing::info!("No changes to commit, skipping commit");

                    return Ok(());
                }

                if let Some(lint_fix_command) = &maybe_lint_fix_command {
                    command_responder.send_update("running lint and fix");
                    accept_non_zero_exit(context.exec_cmd(&Command::shell(lint_fix_command)).await)
                        .context("Could not run lint and fix")?;
                };

                accept_non_zero_exit(context.exec_cmd(&Command::shell("git add .")).await)
                    .context("Could not add files to git")?;

                accept_non_zero_exit(
                    context
                        .exec_cmd(&Command::shell(
                            "git commit -m \"[kwaak]: Committed changes after completion\"",
                        ))
                        .await,
                )
                .context("Could not commit files to git")?;

                accept_non_zero_exit(context.exec_cmd(&Command::shell("git push")).await)
                    .context("Could not push changes to git")?;

                Ok(())
            })
        })
        .after_each(conversation_summarizer.summarize_hook())
        .llm(&query_provider)
        .build()?;

    Ok(agent)
}

fn build_system_prompt(repository: &Repository) -> Result<Prompt> {
    let mut constraints = vec![
        "Research your solution before providing it",
        "When writing files, ensure you write and implement everything, everytime. Do NOT leave anything out. Writing a file overwrites the entire file, so it MUST include the full, completed contents of the file. Do not make changes other than the ones requested.",
        "Tool calls are in parallel. You can run multiple tool calls at the same time, but they must not rely on eachother",
        "Your first response to ANY user message, must ALWAYS be your thoughts on how to solve the problem",
        "When writing code or tests, make sure this is ideomatic for the language",
        "When writing tests, verify that test coverage has changed. If it hasn't, the tests are not doing anything. This means you _must_ run coverage after creating a new test.",
        "When writing tests, make sure you cover all edge cases",
        "When writing tests, if a specific test continues to be troublesome, think out of the box and try to solve the problem in a different way, or reset and focus on other tests first",
        "When writing code, make sure the code runs, tests pass, and is included in the build",
        "When writing code, make sure all public facing functions, methods, modules, etc are documented ideomatically",
        "Your changes are automatically added to git, there is no need to commit files yourself",
        "If you create a pull request, you must ensure the tests pass",
        "Do NOT rely on your own knowledge, always research and verify!",
        "If you just want to run the tests, prefer running the tests over running coverage, as running tests is faster",
        "Verify assumptions you make about the code by researching the actual code first",
        "If you are stuck, consider using git to undo your changes",
        "Focus on completing the task fully as requested by the user",
        "Do not repeat your answers, if they are exactly the same you should probably stop",
        "Make sure you understand the project layout in terms of files and directories",
        "Keep a neutral tone, refrain from using superlatives and unnecessary adjectives",
        "Do NOT remove any existing comments",
        "ALWAYS consider existing functionality and code when writing new code. Functionality must remain the same unless explicitly instructed otherwise.",
        "If after changing code, the code is no longer used, you can safely remove it"
    ];

    if repository.config().endless_mode {
        constraints.push("You cannot ask for feedback and have to try to complete the given task");
    } else {
        constraints.push(
            "Try to solve the problem yourself first, only if you cannot solve it, ask for help",
        );
    }

    let prompt = SystemPrompt::builder()
        .role(format!("You are an autonomous ai agent tasked with helping a user with a code project. You can solve coding problems yourself and should try to always work towards a full solution. The project is called {} and is written in {}", repository.config().project_name, repository.config().language))
        .constraints(constraints).build()?.into();

    Ok(prompt)
}

async fn rename_chat(
    query: &str,
    fast_query_provider: &dyn SimplePrompt,
    command_responder: &CommandResponder,
) -> Result<()> {
    let chat_name = fast_query_provider
        .prompt(
            format!("Give a good, short, max 60 chars title for the following query. Only respond with the title.:\n{query}")
                .into(),
        )
        .await
        .context("Could not get chat name")?
        .trim_matches('"')
        .chars()
        .take(30) 
        .collect::<String>();

    command_responder.send_rename(chat_name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use swiftide_core::MockSimplePrompt;

    use crate::commands::CommandResponse;

    use super::*;

    #[tokio::test]
    async fn test_rename_chat() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("Excellent title".to_string()));

        let mut command_responder = CommandResponder::default();

        rename_chat(&query, &llm_mock as &dyn SimplePrompt, &command_responder)
            .await
            .unwrap();

        let message = command_responder.recv().await.unwrap();

        match message {
            CommandResponse::RenameChat(_, msg) => assert_eq!(msg, "Excellent title"),
            _ => panic!("Expected RenameChat"),
        }
    }

    #[tokio::test]
    async fn test_rename_chat_limits_60() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("Excellent title".repeat(100).to_string()));

        let mut command_responder = CommandResponder::default();

        rename_chat(&query, &llm_mock as &dyn SimplePrompt, &command_responder)
            .await
            .unwrap();

        let message = command_responder.recv().await.unwrap();

        match message {
            CommandResponse::RenameChat(_, msg) => assert_eq!(msg.len(), 30),
            _ => panic!("Expected RenameChat"),
        }
    }
}
