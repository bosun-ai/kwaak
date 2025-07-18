//! The delegate agent is meant to delegate tasks to other agents
//!
//! Additionally, it can answer simple questions

use std::sync::Arc;

use anyhow::Result;
use swiftide::{
    agents::{Agent, AgentBuilder, DefaultContext, system_prompt::SystemPrompt},
    chat_completion::{self, ChatCompletion, Tool},
    prompt::Prompt,
    traits::{AgentContext, Command, SimplePrompt, ToolBox, ToolExecutor},
};

use crate::{
    agent::{
        conversation_summarizer::ConversationSummarizer,
        git_agent_environment::GitAgentEnvironment, running_agent::RunningAgent, session::Session,
        tool_summarizer::ToolSummarizer,
    },
    commands::Responder,
    repository::Repository,
};

pub async fn start(
    session: &Session,
    executor: &Arc<dyn ToolExecutor>,
    tools: &[Box<dyn Tool>],
    tool_boxes: &[Box<dyn ToolBox>],
    agent_env: &GitAgentEnvironment,
    initial_context: &str,
) -> Result<RunningAgent> {
    let agent = build(
        &session.repository,
        &session.default_responder,
        executor,
        tools,
        tool_boxes,
        agent_env,
        Some(&initial_context),
    )
    .await?
    .build()?;

    Ok(agent.into())
}

pub async fn build(
    repository: &Repository,
    responder: &Arc<dyn Responder>,
    executor: &Arc<dyn ToolExecutor>,
    tools: &[Box<dyn Tool>],
    tool_boxes: &[Box<dyn ToolBox>],
    agent_env: &GitAgentEnvironment,
    initial_context: Option<&str>,
) -> Result<AgentBuilder> {
    let config = repository.config();
    let backoff = config.backoff;
    let query_provider: Box<dyn ChatCompletion> =
        config.query_provider().get_chat_completion_model(backoff)?;
    let fast_query_provider: Box<dyn SimplePrompt> = config
        .indexing_provider()
        .get_simple_prompt_model(backoff)?;

    let system_prompt = build_system_prompt(&repository)?;

    let mut context = DefaultContext::from_executor(Arc::clone(&executor));

    let top_level_project_overview = context
        .executor()
        .exec_cmd(&Command::shell("fd -iH -d2 -E '.git/'"))
        .await?
        .output;
    tracing::debug!(top_level_project_overview = ?top_level_project_overview, "Top level project overview");

    if config.endless_mode {
        context.with_stop_on_assistant(false);
    }

    let responder = Arc::clone(&responder);
    let tx_2 = responder.clone();
    let tx_3 = responder.clone();
    let tx_4 = responder.clone();

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
        config.num_completions_for_summary,
    );

    let initial_context = initial_context.map(std::string::ToString::to_string);
    let context = Arc::new(context);
    let mut builder = Agent::builder()
        .context(Arc::clone(&context) as Arc<dyn AgentContext>)
        .system_prompt(system_prompt)
        .tools(tools.to_vec())
        .before_all(move |agent| {
            let initial_context = initial_context.clone();

            Box::pin(async move {
                if let Some(initial_context) = initial_context {
                    agent.context().add_message(chat_completion::ChatMessage::new_user(initial_context)).await?;
                }

                let top_level_project_overview = agent.context().executor().exec_cmd(&Command::shell("fd -iH -d2 -E '.git/*'")).await?.output;
                agent.context().add_message(chat_completion::ChatMessage::new_user(format!("The following is a max depth 2, high level overview of the directory structure of the project: \n ```{top_level_project_overview}```"))).await?;

                Ok(())
            })
        })
        .on_new_message(move |agent, message| {
            let command_responder = tx_2.clone();
            let message = message.clone();

            Box::pin(async move {
                command_responder.agent_message(agent,message).await;

                Ok(())
            })
        })
        .before_completion(move |_, _| {
            let command_responder = tx_3.clone();
            Box::pin(async move {
                command_responder.update("running completions").await;
                Ok(())
            })
        })
        .before_tool(move |_, tool| {
            let command_responder = tx_4.clone();
            let tool = tool.clone();
            Box::pin(async move {
                command_responder.update(&format!("running tool {}", tool.name())).await;
                Ok(())
            })
        })
        .after_tool(tool_summarizer.summarize_hook())
        .after_each(conversation_summarizer.summarize_hook())
        .llm(&query_provider).to_owned();

    for tool_box in tool_boxes {
        builder.add_toolbox(tool_box.clone());
    }

    Ok(builder)
}

pub fn build_system_prompt(repository: &Repository) -> Result<Prompt> {
    let mut constraints: Vec<String> = vec![
        // General
        "Thoroughly research your solution before providing it",
        "Tool calls are in parallel. You can run multiple tool calls at the same time, but they must not rely on each other",
        "Your first response to ANY user message, must ALWAYS be your thoughts on how to solve the problem",
        "Keep a neutral tone, refrain from using superlatives and unnecessary adjectives",
        "Your response must always include your observation, your reasoning for the next step you are going to take, and the next step you are going to take",
        "Think step by step",

        // Knowledge
        "Do NOT rely on your own knowledge, always research and verify!",
        "Verify assumptions you make about the code by researching the actual code first",
        "Do not leave tasks incomplete. If you lack information, use the available tools to find the correct information",
        "Make sure you understand the project layout in terms of files and directories",
        "Research the project structure and the codebase before providing a plan",
        "Always reference full paths to files",
        "Always provide references to the codebase when answering questions or otherwise stating knowledge",

        // Tool usage
        "If you just want to run the tests, prefer running the tests over running coverage, as running tests is faster",
        "When delegating to an agent, research thoroughly how to solve the problem",
        "When delegating to an agent, provide a clear description of the task, requirements, and constraints",
        "When delegating to an agent, reference your instructions with full paths to the files involved",
        "When delegating to an agent, provide a clear definition of done",
        "When delegating to an agent, clearly state edge cases",
        "After every tool use, include your observations, reasoning, and the next step",

        // Workflow
        "Focus on completing the task fully as requested by the user",
        "Do not repeat your answers, if they are exactly the same you should probably stop",
    ].into_iter().map(Into::into).collect();

    if repository.config().endless_mode {
        constraints
            .push("You cannot ask for feedback and have to try to complete the given task".into());
    } else {
        constraints.push(
            "Try to solve the problem yourself first, only if you cannot solve it, ask for help"
                .into(),
        );
        constraints.push(
            "Before delegating to an agent, you MUST ask the user if they agree to your plan"
                .into(),
        );
    }

    if let Some(agent_custom_constraints) = repository.config().agent_custom_constraints.as_ref() {
        constraints.extend(agent_custom_constraints.iter().cloned());
    }

    let prompt = SystemPrompt::builder()
        .role(format!("You are an autonomous ai agent tasked with helping a user with a code project. You can solve coding problems yourself and should try to always work towards a full solution. The project is called {} and is written in {}", repository.config().project_name, repository.config().languages.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join(", ")))
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

        assert!(
            prompt
                .render()
                .unwrap()
                .contains("You cannot ask for feedback and have to try to complete the given task")
        );
    }

    #[tokio::test]
    async fn test_build_system_prompt_custom_constraints() {
        let custom_constraints = vec![
            "Custom constraint 1".to_string(),
            "Custom constraint 2".to_string(),
        ];

        let (mut repository, _guard) = test_repository();
        repository.config_mut().agent_custom_constraints = Some(custom_constraints);

        let prompt = build_system_prompt(&repository).unwrap().render().unwrap();
        assert!(prompt.contains("Custom constraint 1"));
        assert!(prompt.contains("Custom constraint 2"));
    }
}
