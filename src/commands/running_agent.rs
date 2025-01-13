use anyhow::Result;
use std::sync::Arc;

use swiftide::agents::Agent;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct RunningAgent {
    pub agent: Arc<Mutex<Agent>>,

    #[allow(dead_code)]
    pub response_handle: Arc<tokio::task::JoinHandle<()>>,

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
