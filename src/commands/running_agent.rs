use anyhow::Result;
use std::sync::Arc;
use swiftide_core::ToolExecutor;

use swiftide::agents::Agent;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct RunningAgent {
    /// The agent that is running
    pub agent: Arc<Mutex<Agent>>,
    /// A copy of the running tool executor the agent is using
    pub executor: Arc<dyn ToolExecutor>,
    /// Used to kill the agent
    pub cancel_token: CancellationToken,
}

impl RunningAgent {
    pub async fn query(&self, query: &str) -> Result<()> {
        self.agent.lock().await.query(query).await
    }

    pub async fn stop(&self) {
        self.cancel_token.cancel();
        self.agent.lock().await.stop();
    }
}
