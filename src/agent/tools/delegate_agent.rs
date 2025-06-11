use std::{borrow::Cow, sync::Arc};

use async_trait::async_trait;
use derive_builder::Builder;
use serde::Deserialize;
use swiftide::{
    chat_completion::{self, Tool, ToolCall, ToolOutput, ToolSpec, errors::ToolError},
    traits::AgentContext,
};

use crate::agent::{running_agent::RunningAgent, session::Session};

/// A tool that delegates to an agent
///
/// For convenience, its assumed the agent is already set up (a `RunningAgent`).
///
/// The tool takes a tool spec, agent and session during creating, so that it can be utilized to
/// delegate to any agent.
///
/// After delegation, the agent invoking the tool is stopped, but not destroyed.
#[derive(Clone, Builder)]
pub struct DelegateAgent {
    session: Arc<Session>,
    agent: RunningAgent,

    tool_spec: ToolSpec,
}

impl DelegateAgent {
    #[must_use]
    pub fn builder() -> DelegateAgentBuilder {
        DelegateAgentBuilder::default()
    }

    pub async fn delegate_agent(
        &self,
        _context: &dyn AgentContext,
        task: &str,
    ) -> Result<ToolOutput, ToolError> {
        self.session.swap_agent(self.agent.clone())?;
        self.agent.query(task).await?;

        tracing::info!("Delegated task to agent");
        Ok(ToolOutput::Stop)
    }
}

#[derive(Deserialize)]
struct DelegateArgs {
    task: String,
}

#[async_trait]
impl Tool for DelegateAgent {
    async fn invoke(
        &self,
        agent_context: &dyn AgentContext,
        tool_call: &ToolCall,
    ) -> Result<ToolOutput, ToolError> {
        let Some(args) = tool_call.args() else {
            return Err(ToolError::MissingArguments(format!(
                "No arguments provided for {}",
                self.name()
            )));
        };

        let args: DelegateArgs = serde_json::from_str(&args)?;
        return self.delegate_agent(agent_context, &args.task).await;
    }

    fn tool_spec(&self) -> chat_completion::ToolSpec {
        self.tool_spec.clone()
    }

    fn name(&self) -> Cow<'_, str> {
        self.tool_spec().name.into()
    }
}
