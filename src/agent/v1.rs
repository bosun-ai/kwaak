use std::sync::Arc;

use anyhow::{Context as _, Result};
use swiftide::{
    agents::{
        system_prompt::SystemPrompt, tools::local_executor::LocalExecutor, Agent, DefaultContext,
    },
    chat_completion::{self, ChatCompletion, Tool},
    prompt::Prompt,
    traits::{AgentContext, Command, SimplePrompt, ToolExecutor},
};
use tavily::Tavily;
use uuid::Uuid;

use super::{
    conversation_summarizer::ConversationSummarizer,
    env_setup::{self, EnvSetup},
    tool_summarizer::ToolSummarizer,
    tools, RunningAgent,
};
use crate::{
    agent::{
        commit_and_push::{self, CommitAndPush},
        util,
    },
    commands::Responder,
    config::{AgentEditMode, SupportedToolExecutors},
    git::github::GithubSession,
    indexing,
    repository::Repository,
    util::accept_non_zero_exit,
};
use swiftide_docker_executor::DockerExecutor;

async fn generate_initial_context(repository: &Repository, query: &str) -> Result<String> {
    let retrieved_context = indexing::query(repository, &query).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

// Maybe extract this into a toolbox?
pub fn available_tools(
    repository: &Repository,
    github_session: Option<&Arc<GithubSession>>,
    agent_env: Option<&env_setup::AgentEnvironment>,
) -> Result<Vec<Box<dyn Tool>>> {
    let query_pipeline = indexing::build_query_pipeline(repository)?;

    let mut tools = vec![
        tools::write_file(),
        tools::search_file(),
        tools::git(),
        tools::shell_command(),
        tools::search_code(),
        tools::fetch_url(),
        tools::ExplainCode::new(query_pipeline).boxed(),
    ];

    match repository.config().agent_edit_mode {
        AgentEditMode::Whole => {
            tools.push(tools::write_file());
            tools.push(tools::read_file());
        }
        AgentEditMode::Line => {
            tools.push(tools::read_file_with_line_numbers());
            tools.push(tools::replace_lines());
            tools.push(tools::add_lines());
        }
    }

    if let Some(github_session) = github_session {
        if !repository.config().disabled_tools.pull_request {
            tools.push(tools::CreateOrUpdatePullRequest::new(github_session).boxed());
        }
        tools.push(tools::GithubSearchCode::new(github_session).boxed());
    }

    if let Some(tavily_api_key) = &repository.config().tavily_api_key {
        let tavily = Tavily::builder(tavily_api_key.expose_secret()).build()?;
        tools.push(tools::SearchWeb::new(tavily, tavily_api_key.clone()).boxed());
    };

    if let Some(test_command) = &repository.config().commands.test {
        tools.push(tools::RunTests::new(test_command).boxed());
    }

    if let Some(coverage_command) = &repository.config().commands.coverage {
        tools.push(tools::RunCoverage::new(coverage_command).boxed());
    }

    if let Some(env) = agent_env {
        tools.push(tools::ResetFile::new(&env.start_ref).boxed());
    }

    Ok(tools)
}

async fn start_tool_executor(uuid: Uuid, repository: &Repository) -> Result<Arc<dyn ToolExecutor>> {
    let boxed = match repository.config().tool_executor {
        SupportedToolExecutors::Docker => {
            let mut executor = DockerExecutor::default();
            let dockerfile = &repository.config().docker.dockerfile;

            if std::fs::metadata(dockerfile).is_err() {
                tracing::error!("Dockerfile not found at {}", dockerfile.display());
                panic!("Running in docker requires a Dockerfile");
            }
            let running_executor = executor
                .with_context_path(&repository.config().docker.context)
                .with_image_name(repository.config().project_name.to_lowercase())
                .with_dockerfile(dockerfile)
                .with_container_uuid(uuid)
                .to_owned()
                .start()
                .await?;

            Arc::new(running_executor) as Arc<dyn ToolExecutor>
        }
        SupportedToolExecutors::Local => Arc::new(LocalExecutor::new(".")) as Arc<dyn ToolExecutor>,
    };

    Ok(boxed)
}

pub async fn start(
    query: &str,
    uuid: Uuid,
    repository: &Repository,
    command_responder: Arc<dyn Responder>,
) -> Result<RunningAgent> {
    let backoff = repository.config().backoff;
    let query_provider: Box<dyn ChatCompletion> = repository
        .config()
        .query_provider()
        .get_chat_completion_model(backoff)?;
    let fast_query_provider: Box<dyn SimplePrompt> = repository
        .config()
        .indexing_provider()
        .get_simple_prompt_model(backoff)?;

    let github_session = match repository.config().github_api_key {
        Some(_) => Some(Arc::new(GithubSession::from_repository(&repository)?)),
        None => None,
    };

    // TODO: Feels a bit off to have EnvSetup return an Env, just to pass it to tool creation to
    // get the ref/branch name
    //
    // Probably nicer to have a `ChatSession` or `AgentSession` that encapsulates all the
    // complexity
    let ((), branch_name, executor, initial_context) = tokio::try_join!(
        util::rename_chat(&query, &fast_query_provider, &command_responder),
        util::create_branch_name(&query, &uuid, &fast_query_provider, &command_responder),
        start_tool_executor(uuid, &repository),
        generate_initial_context(&repository, query)
    )?;
    let env_setup = EnvSetup::new(&repository, github_session.as_deref(), &*executor);
    let agent_env = env_setup.exec_setup_commands(branch_name).await?;
    let system_prompt = build_system_prompt(&repository)?;

    let tools = available_tools(&repository, github_session.as_ref(), Some(&agent_env))?;

    let mut context = DefaultContext::from_executor(Arc::clone(&executor));

    let top_level_project_overview = context
        .exec_cmd(&Command::shell("fd -iH -d2 -E '.git/'"))
        .await?
        .output;
    tracing::debug!(top_level_project_overview = ?top_level_project_overview, "Top level project overview");

    if repository.config().endless_mode {
        context.with_stop_on_assistant(false);
    }

    let command_responder = Arc::new(command_responder);
    let tx_2 = command_responder.clone();
    let tx_3 = command_responder.clone();
    let tx_4 = command_responder.clone();

    let tool_summarizer = ToolSummarizer::new(
        fast_query_provider,
        &["run_tests", "run_coverage"],
        &tools,
        &agent_env.start_ref,
    );
    let conversation_summarizer = ConversationSummarizer::new(
        query_provider.clone(),
        &tools,
        &agent_env.start_ref,
        repository.config().num_completions_for_summary,
    );
    let commit_and_push = CommitAndPush::new(repository, &agent_env);

    let maybe_lint_fix_command = repository.config().commands.lint_and_fix.clone();

    let context = Arc::new(context);
    let agent = Agent::builder()
        .context(Arc::clone(&context) as Arc<dyn AgentContext>)
        .system_prompt(system_prompt)
        .tools(tools)
        .before_all(move |agent| {
            let initial_context = initial_context.clone();

            Box::pin(async move {
                agent.context()
                    .add_message(chat_completion::ChatMessage::new_user(initial_context))
                    .await;

                let top_level_project_overview = agent.context().exec_cmd(&Command::shell("fd -iH -d2 -E '.git/*'")).await?.output;
                agent.context().add_message(chat_completion::ChatMessage::new_user(format!("The following is a max depth 2, high level overview of the directory structure of the project: \n ```{top_level_project_overview}```"))).await;

                Ok(())
            })
        })
        .on_new_message(move |_, message| {
            let command_responder = tx_2.clone();
            let message = message.clone();

            Box::pin(async move {
                command_responder.agent_message(message);

                Ok(())
            })
        })
        .before_completion(move |_, _| {
            let command_responder = tx_3.clone();
            Box::pin(async move {
                command_responder.update("running completions");
                Ok(())
            })
        })
        .before_tool(move |_, tool| {
            let command_responder = tx_4.clone();
            let tool = tool.clone();
            Box::pin(async move {
                command_responder.update(&format!("running tool {}", tool.name()));
                Ok(())
            })
        })
        .after_tool(tool_summarizer.summarize_hook())
        .after_each(move |agent| {
            let maybe_lint_fix_command = maybe_lint_fix_command.clone();
            let command_responder = command_responder.clone();
            Box::pin(async move {
                if accept_non_zero_exit(
                    agent.context()
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
                    command_responder.update("running lint and fix");
                    accept_non_zero_exit(agent.context().exec_cmd(&Command::shell(lint_fix_command)).await)
                        .context("Could not run lint and fix")?;
                };

                Ok(())
            })
        })
        .after_each(commit_and_push.hook())
        .after_each(conversation_summarizer.summarize_hook())
        .llm(&query_provider)
        .build()?;

    RunningAgent::builder()
        .agent(agent)
        .executor(executor)
        .agent_environment(agent_env)
        .agent_context(context as Arc<dyn AgentContext>)
        .build()
}

pub fn build_system_prompt(repository: &Repository) -> Result<Prompt> {
    let mut constraints: Vec<String> = vec![
        // General
        "Research your solution before providing it",
        "Tool calls are in parallel. You can run multiple tool calls at the same time, but they must not rely on each other",
        "Your first response to ANY user message, must ALWAYS be your thoughts on how to solve the problem",
        "Keep a neutral tone, refrain from using superlatives and unnecessary adjectives",

        // Knowledge
        "Do NOT rely on your own knowledge, always research and verify!",
        "Verify assumptions you make about the code by researching the actual code first",
        "Do not leave tasks incomplete. If you lack information, use the available tools to find the correct information",
        "Make sure you understand the project layout in terms of files and directories",
        "Research the project structure and the codebase before providing a plan",

        // Tool usage
        "When writing files, ensure you write and implement everything, everytime. Do NOT leave anything out. Writing a file overwrites the entire file, so it MUST include the full, completed contents of the file. Do not make changes other than the ones requested.",
        "If you create a pull request, you must ensure the tests pass",
        "If you just want to run the tests, prefer running the tests over running coverage, as running tests is faster",
        "NEVER write or edit a file before having read it",

        // Code writing
        "When writing code or tests, make sure this is idiomatic for the language",
        "When writing code, make sure you account for edge cases",
        "When writing tests, verify that test coverage has changed. If it hasn't, the tests are not doing anything. This means you _must_ run coverage after creating a new test.",
        "When writing tests, make sure you cover all edge cases",
        "When writing tests, if a specific test continues to be troublesome, think out of the box and try to solve the problem in a different way, or reset and focus on other tests first",
        "When writing code, make sure the code runs, tests pass, and is included in the build",
        "When writing code, make sure all public facing functions, methods, modules, etc are documented idiomatically",
        "Do NOT remove any existing comments",
        "ALWAYS consider existing functionality and code when writing new code. Functionality must remain the same unless explicitly instructed otherwise.",
        "When writing code, make sure you understand the existing architecture and its intend. Use tools to explore the project.",
        "If after changing code, the code is no longer used, you can safely remove it",

        // Workflow
        "Your changes are automatically added to git, there is no need to commit files yourself",
        "You are already operating on a git branch specific to this task. You do not need to create a new branch",
        "If you are stuck, consider using reset_file to undo your changes",
        "Focus on completing the task fully as requested by the user",
        "Do not repeat your answers, if they are exactly the same you should probably stop",
    ].into_iter().map(Into::into).collect();

    if repository.config().agent_edit_mode.is_line() {
        constraints.extend( [
        "Prefer editing files with `replace_lines` and `add_lines` over `write_file`, if possible. This is faster and less error prone. You can only make ONE `replace_lines` or `add_lines` call at the time. After each you MUST call `read_file_with_line_numbers` again, as the linenumbers WILL have changed.",
        "If you are only adding NEW lines, you MUST use `add_lines`",
        "Before every call to `replace_lines` or `add_lines`, you MUST read the file content with the line numbers. You are not allowed to count lines yourself.",

        ].into_iter().map(Into::into));
    }

    if repository.config().endless_mode {
        constraints
            .push("You cannot ask for feedback and have to try to complete the given task".into());
    } else {
        constraints.push(
            "Try to solve the problem yourself first, only if you cannot solve it, ask for help"
                .into(),
        );
    }

    if let Some(agent_custom_constraints) = repository.config().agent_custom_constraints.as_ref() {
        constraints.extend(agent_custom_constraints.iter().cloned());
    }

    let prompt = SystemPrompt::builder()
        .role(format!("You are an autonomous ai agent tasked with helping a user with a code project. You can solve coding problems yourself and should try to always work towards a full solution. The project is called {} and is written in {}", repository.config().project_name, repository.config().language))
        .constraints(constraints).build()?.into();

    Ok(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_repository;

    #[tokio::test]
    async fn test_build_system_prompt_endless_mode() {
        let (mut repository, _guard) = test_repository();
        repository.config_mut().endless_mode = true;
        let prompt = build_system_prompt(&repository).unwrap();

        assert!(prompt
            .render()
            .await
            .unwrap()
            .contains("You cannot ask for feedback and have to try to complete the given task"));
    }

    #[tokio::test]
    async fn test_build_system_prompt_custom_constraints() {
        let custom_constraints = vec![
            "Custom constraint 1".to_string(),
            "Custom constraint 2".to_string(),
        ];

        let (mut repository, _guard) = test_repository();
        repository.config_mut().agent_custom_constraints = Some(custom_constraints);

        let prompt = build_system_prompt(&repository)
            .unwrap()
            .render()
            .await
            .unwrap();
        assert!(prompt.contains("Custom constraint 1"));
        assert!(prompt.contains("Custom constraint 2"));
    }
}
