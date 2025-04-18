use anyhow::{Context as _, Result};
use std::sync::Arc;

use swiftide::agents::Agent;
use tokio::sync::Mutex;

/// Defines any agent that is running
///
/// Internally wraps an agent with an arc mutex so it can be shared
#[derive(Clone)]
pub struct RunningAgent {
    /// The agent that is running
    pub agent: Arc<Mutex<Agent>>,
}

impl From<Agent> for RunningAgent {
    fn from(agent: Agent) -> Self {
        RunningAgent {
            agent: Arc::new(Mutex::new(agent)),
        }
    }
}

impl RunningAgent {
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
