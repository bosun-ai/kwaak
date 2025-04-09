use anyhow::{Context as _, Result};
use derive_builder::Builder;
use std::sync::Arc;
use swiftide::traits::AgentContext;

use swiftide::agents::Agent;
use tokio::sync::Mutex;

/// Defines any agent that is running
#[derive(Clone, Builder)]
#[builder(build_fn(error = anyhow::Error))]
pub struct RunningAgent {
    /// The agent that is running
    #[builder(setter(custom))]
    pub agent: Arc<Mutex<Agent>>,
    /// The content the agent is running with
    #[builder(setter(into))]
    pub agent_context: Arc<dyn AgentContext>,
}

impl RunningAgent {
    #[must_use]
    pub fn builder() -> RunningAgentBuilder {
        RunningAgentBuilder::default()
    }

    pub async fn query(&self, query: &str) -> Result<()> {
        self.agent
            .lock()
            .await
            .query(query)
            .await
            .context("Failed to query agent")
    }

    pub async fn run(&self) -> Result<()> {
        self.agent
            .lock()
            .await
            .run()
            .await
            .context("Failed to run agent")
    }

    pub async fn stop(&self) {
        self.agent.lock().await.stop("Stopped from kwaak").await;
    }
}

impl RunningAgentBuilder {
    pub fn agent(&mut self, agent: Agent) -> &mut Self {
        self.agent = Some(Arc::new(Mutex::new(agent)));
        self
    }
}
