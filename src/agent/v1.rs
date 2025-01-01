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

use crate::{
    commands::CommandResponder, config::SupportedToolExecutors, git::github::GithubSession,
    indexing, repository::Repository, templates::Templates, util::accept_non_zero_exit,
};

use super::{
    conversation_summarizer::ConversationSummarizer, docker_tool_executor::DockerExecutor,
    env_setup::EnvSetup, tool_summarizer::ToolSummarizer, tools,
};

async fn generate_initial_context(
    repository: &Repository,
    query: &str,
    original_system_prompt: &str,
    tools: &[Box<dyn Tool>],
) -> Result<String> {
    let available_tools = tools
        .iter()
        .map(|tool| format!("- **{}**: {}", tool.name(), tool.tool_spec().description))
        .collect::<Vec<String>>()
        .join("\n");


    // TODO: Move this to the _answer_ transformer
    let mut template_context = tera::Context::new();
    template_context.insert("project_name", &repository.config().project_name);
    template_context.insert("lang", &repository.config().language);
    template_context.insert("original_system_prompt", original_system_prompt);
    template_context.insert("query", query);
    template_context.insert("available_tools", &available_tools);

    let initial_context_prompt = Templates::render("v1_initial_context.md", &template_context)?;
    let retrieved_context = indexing::query(repository, &initial_context_prompt).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

fn configure_tools(
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
        tools::ExplainCode::new(query_pipeline).boxed(),
        tools::RunTests::new(&repository.config().commands.test).boxed(),
        tools::RunCoverage::new(&repository.config().commands.coverage).boxed(),
    ];

    if let Some(github_session) = github_session {
        tools.push(tools::CreateOrUpdatePullRequest::new(github_session).boxed());
    }

    if let Some(tavily_api_key) = &repository.config().tavily_api_key {
        // Client is a bit weird that it needs the api key twice
        // Maybe roll our own? It's just a rest api
        let tavily = Tavily::new(tavily_api_key.expose_secret());
        tools.push(tools::SearchWeb::new(tavily, tavily_api_key.clone()).boxed());
    };

    Ok(tools)
}

async fn start_tool_executor(repository: &Repository) -> Result<Box<dyn ToolExecutor>> {
    let boxed = match repository.config().tool_executor {
        SupportedToolExecutors::Docker => {
            Box::new(DockerExecutor::from_repository(repository).start().await?)
                as Box<dyn ToolExecutor>
        }
        SupportedToolExecutors::Local => Box::new(LocalExecutor::new(".")) as Box<dyn ToolExecutor>,
    };

    Ok(boxed)
}

#[tracing::instrument(skip(repository, command_responder))]
pub async fn build_agent(
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

    let tools = configure_tools(&repository, github_session.as_ref())?;

    let system_prompt: Prompt =
    SystemPrompt::builder()
        .role(format!("You are an autonomous ai agent tasked with helping a user with a code project. You can solve coding problems yourself and should try to always work towards a full solution. The project is called {} and is written in {}", repository.config().project_name, repository.config().language))
        .constraints([
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
            "Try to solve the problem yourself first, only if you cannot solve it, ask for help",
            "If you just want to run the tests, prefer running the tests over running coverage, as running tests is faster",
            "Verify assumptions you make about the code by researching the actual code first",
            "If you are stuck, consider using git to undo your changes",
            "Focus on completing the task fully as requested by the user",
            "Make sure you understand the project layout in terms of files and directories",
            "Keep a neutral tone, refrain from using superlatives and unnecessary adjectives",
        ]).build()?.into();

    // Run executor and initial context in parallel
    let rendered_system_prompt = system_prompt.render().await?;
    let (executor, initial_context) = tokio::try_join!(
        start_tool_executor(&repository),
        generate_initial_context(&repository, query, &rendered_system_prompt, &tools)
    )?;

    // Run a series of commands inside the executor so that everything is available
    let env_setup = EnvSetup::new(&repository, github_session.as_deref(), &*executor);
    env_setup.exec_setup_commands().await?;

    let context = DefaultContext::from_executor(executor);

    let command_responder = Arc::new(command_responder);
    // Maybe I'm just too tired but feels off.
    let tx_2 = command_responder.clone();
    let tx_3 = command_responder.clone();
    let tx_4 = command_responder.clone();

    let tool_summarizer =
        ToolSummarizer::new(fast_query_provider, &["run_tests", "run_coverage"], &tools);

    // Would be nice if the summarizer also captured the initial query
    let conversation_summarizer = ConversationSummarizer::new(query_provider.clone(), &tools);
    let maybe_lint_fix_command = repository.config().commands.lint_and_fix.clone();

    let agent = Agent::builder()
        .context(context)
        .system_prompt(system_prompt)
        .tools(tools)
        .before_all(move |context| {
            let initial_context = initial_context.clone();

            Box::pin(async move {
                // Add initial context
                context
                    .add_message(chat_completion::ChatMessage::new_user(initial_context))
                    .await;

                // Add a high level overview of the project
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
        // After each completion, lint and fix and commit
        .after_each(move |context| {
            let maybe_lint_fix_command = maybe_lint_fix_command.clone();
            let command_responder = command_responder.clone();
            Box::pin(async move {
                // TODO: Refactor to a separate tool so it can be tested in isolation and is less
                // messy
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

                // Then commit the changes
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
