use anyhow::Result;
use std::sync::Arc;
use swiftide::agents::{Agent, DefaultContext};
use swiftide::chat_completion::{ChatCompletion, Tool};
use swiftide::traits::AgentContext;

use crate::agent::agents;
use crate::agent::conversation_summarizer::ConversationSummarizer;
use crate::agent::running_agent::RunningAgent;
use crate::commands::Responder;
use crate::repository::Repository;

// Note that this uses a local executor
pub async fn start_tool_evaluation_agent(
    repository: &Repository,
    responder: Arc<dyn Responder>,
    tools: Vec<Box<dyn Tool>>,
    initial_query: &str,
) -> Result<RunningAgent> {
    // Create agent with simplified tools
    let system_prompt = agents::coding::build_system_prompt(repository)?;
    let agent_context: Arc<dyn AgentContext> = Arc::new(
        DefaultContext::default()
            .with_stop_on_assistant(false)
            .to_owned(),
    ) as Arc<dyn AgentContext>;

    let backoff = repository.config().backoff;

    let query_provider: Box<dyn ChatCompletion> = repository
        .config()
        .query_provider()
        .get_chat_completion_model(backoff)?;

    let conversation_summarizer = ConversationSummarizer::new(
        query_provider.clone(),
        &tools,
        "HEAD",
        repository.config().num_completions_for_summary,
        initial_query,
    );

    let responder_for_messages = responder.clone();
    let responder_for_tools = responder.clone();

    let agent = Agent::builder()
        .tools(tools)
        .system_prompt(system_prompt)
        .context(agent_context.clone())
        .llm(&query_provider)
        .on_new_message(move |_, message| {
            let responder = responder_for_messages.clone();
            let message = message.clone();
            Box::pin(async move {
                responder.agent_message(message).await;
                Ok(())
            })
        })
        .before_tool(move |_, tool| {
            let responder = responder_for_tools.clone();
            let tool = tool.clone();
            Box::pin(async move {
                responder
                    .update(&format!("running tool {}", tool.name()))
                    .await;
                Ok(())
            })
        })
        .after_each(conversation_summarizer.summarize_hook())
        .build()?;

    let agent = RunningAgent::builder()
        .agent(agent)
        .agent_context(agent_context)
        .build()?;

    Ok(agent)
}
