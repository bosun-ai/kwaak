use anyhow::Result;
use std::sync::Arc;
use swiftide::agents::{Agent, DefaultContext};
use swiftide::chat_completion::{ChatCompletion, Tool};
use swiftide::traits::AgentContext;
use uuid::Uuid;

use crate::agent::{v1, RunningAgent, env_setup::AgentEnvironment};
use crate::commands::Responder;
use crate::repository::Repository;

pub async fn start_tool_evaluation_agent(
    _uuid: Uuid,
    repository: &Repository,
    _query: &str,
    responder: Arc<dyn Responder>,
    tools: Vec<Box<dyn Tool>>,
) -> Result<RunningAgent> {
    // Create agent with simplified tools
    let system_prompt = v1::build_system_prompt(repository)?;
    let agent_context: Arc<dyn AgentContext> = Arc::new(DefaultContext::default());
    let executor = Arc::new(swiftide::agents::tools::local_executor::LocalExecutor::default());
    let query_provider: Box<dyn ChatCompletion> =
        repository.config().query_provider().try_into()?;

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
                responder.agent_message(message);
                Ok(())
            })
        })
        .before_tool(move |_, tool| {
            let responder = responder_for_tools.clone();
            let tool = tool.clone();
            Box::pin(async move {
                responder.update(&format!("running tool {}", tool.name()));
                Ok(())
            })
        })
        .build()?;

    let agent = RunningAgent::builder()
        .agent(agent)
        .executor(executor)
        .agent_context(agent_context)
        .agent_environment(Arc::new(AgentEnvironment::default()))
        .build()?;

    Ok(agent)
}
