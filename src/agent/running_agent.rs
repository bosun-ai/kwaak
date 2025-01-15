use anyhow::Result;
use derive_builder::Builder;
use std::sync::Arc;
use swiftide_core::ToolExecutor;

use swiftide::agents::Agent;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::env_setup::AgentEnvironment;

/// Defines any agent that is running
#[derive(Clone, Builder)]
#[builder(build_fn(error = anyhow::Error))]
pub struct RunningAgent {
    /// The agent that is running
    #[builder(setter(custom))]
    pub agent: Arc<Mutex<Agent>>,
    /// A copy of the running tool executor the agent is using
    pub executor: Arc<dyn ToolExecutor>,
    /// Used to kill the agent
    #[builder(default)]
    pub cancel_token: CancellationToken,
    /// Information about the environment the agent is running in
    #[builder(setter(into))]
    pub agent_environment: Arc<AgentEnvironment>,
}

impl RunningAgent {
    #[must_use]
    pub fn builder() -> RunningAgentBuilder {
        RunningAgentBuilder::default()
    }

    pub async fn query(&self, query: &str) -> Result<()> {
        self.agent.lock().await.query(query).await
    }

    pub async fn stop(&self) {
        self.cancel_token.cancel();
        self.agent.lock().await.stop();
    }
}

impl RunningAgentBuilder {
    pub fn agent(&mut self, agent: Agent) -> &mut Self {
        self.agent = Some(Arc::new(Mutex::new(agent)));
        self
    }
}
