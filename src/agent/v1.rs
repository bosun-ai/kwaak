use std::sync::Arc;

use anyhow::Result;
use swiftide::{
    agents::{
        system_prompt::SystemPrompt, tools::local_executor::LocalExecutor, Agent, DefaultContext,
    },
    chat_completion::{self, ChatCompletion, Tool},
    traits::{SimplePrompt, ToolExecutor},
};
use tavily::Tavily;

use crate::{
    commands::CommandResponder, config::SupportedToolExecutors, git::github::GithubSession,
    indexing, repository::Repository,
};

use super::{
    docker_tool_executor::DockerExecutor, env_setup::EnvSetup, tool_summarizer::ToolSummarizer,
    tools,
};

async fn generate_initial_context(
    repository: &Repository,
    query: &str,
    tools: &[Box<dyn Tool>],
) -> Result<String> {
    let context_query = indoc::formatdoc! {r#"
        What is the purpose of the {project_name} that is written in {lang}? Provide a detailed answer to help me understand the context.
        Also consider what else might be helpful to accomplish the following:
        `{query}`
        This context is provided for an ai agent that has to accomplish the above. Additionally, the agent has access to the following tools:
        `{tools}`
        Do not make assumptions, instruct to investigate instead.
        "#,
        project_name = repository.config().project_name,
        lang = repository.config().language,
        tools = tools.iter().map(Tool::name).collect::<Vec<_>>().join(", "),
    };
    let retrieved_context = indexing::query(repository, &context_query).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

fn configure_tools(
    repository: &Repository,
    github_session: &Arc<GithubSession>,
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
        tools::CreatePullRequest::new(github_session).boxed(),
        tools::RunTests::new(&repository.config().commands.test).boxed(),
        tools::RunCoverage::new(&repository.config().commands.coverage).boxed(),
    ];

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

    let repository = Arc::new(repository.clone());
    let github_session = Arc::new(GithubSession::from_repository(&repository)?);
    let tools = configure_tools(&repository, &github_session)?;

    // Run executor and initial context in parallel
    let (executor, initial_context) = tokio::try_join!(
        start_tool_executor(&repository),
        generate_initial_context(&repository, query, &tools)
    )?;

    // Run a series of commands inside the executor so that everything is available
    let env_setup = EnvSetup::new(&repository, &github_session, &*executor);
    env_setup.exec_setup_commands().await?;

    let context = DefaultContext::from_executor(executor);

    let command_responder = Arc::new(command_responder);
    // Maybe I'm just too tired but feels off.
    let tx_2 = command_responder.clone();
    let tx_3 = command_responder.clone();
    let tx_4 = command_responder.clone();

    let system_prompt =
    SystemPrompt::builder()
        .role("You are an atonomous ai agent tasked with helping a user with a code project. You can solve coding problems yourself and should try to always work towards a full solution.")
        .constraints([
            "Research your solution before providing it",
            "When writing files, ensure you write and implement everything, everytime. Do NOT leave anything out. Writing a file overwrites the entire file, so it must include everything",
            "Tool calls are in parallel. You can run multiple tool calls at the same time, but they must not rely on eachother",
            "Your first response to ANY user message, must ALWAYS be your thoughts on how to solve the problem",
            "When writing code or tests, make sure this is ideomatic for the language",
            "When writing tests, verify that test coverage has changed. If it hasn't, the tests are not doing anything. This means you _must_ run coverage after creating a new test.",
            "When writing tests, make sure you cover all edge cases",
            "When writing code, make sure the code runs and is included in the build",
            "If you create a pull request, make sure the tests pass",
            "Do NOT rely on your own knowledge, always research and verify!",
            "Try to solve the problem yourself first, only if you cannot solve it, ask for help",
            "If you just want to run the tests, prefer running the tests over running coverage, as running tests is faster",
            "Verify assumptions you make about the code by researching the actual code first",
            "If you are stuck, consider using git to undo your changes"
        ]).build()?;

    // NOTE: Kinda inefficient, copying over tools for the summarizer
    let tool_summarizer =
        ToolSummarizer::new(fast_query_provider, &["run_tests", "run_coverage"], &tools);

    let agent = Agent::builder()
        .context(context)
        .system_prompt(system_prompt)
        .tools(tools)
        .before_all(move |context| {
            let initial_context = initial_context.clone();

            Box::pin(async move {
                context
                    .add_message(&chat_completion::ChatMessage::User(initial_context))
                    .await;

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
        // before each, update that we're running completions
        .before_each(move |_| {
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
        .llm(&query_provider)
        .build()?;

    Ok(agent)
}
