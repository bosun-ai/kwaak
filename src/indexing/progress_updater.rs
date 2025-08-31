use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::mpsc::UnboundedSender;

use crate::commands::Responder;
use swiftide::traits::{Transformer, WithIndexingDefaults};
use tokio_util::task::AbortOnDropHandle;

// Just a simple wrapper so we can avoid having to Option check all the time
// We need something sync as swiftide pipelines are not (yet) fully async
//
// Returns `Transformer` hooks so they can be dropped into the pipeline
#[derive(Debug, Clone)]
pub struct ProgressUpdater {
    /// Sends messages to the connected frontend
    responder: Option<Arc<dyn Responder>>,

    /// Inbetween channel to deal with pipelines being sync
    updater: Option<UnboundedSender<String>>,

    total_chunks: Arc<AtomicU64>,
    processed_chunks: Arc<AtomicU64>,
}

impl From<Option<Arc<dyn Responder>>> for ProgressUpdater {
    fn from(responder: Option<Arc<dyn Responder>>) -> Self {
        Self {
            responder,
            updater: None,
            total_chunks: Arc::new(AtomicU64::new(0)),
            processed_chunks: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl ProgressUpdater {
    pub fn spawn(&mut self) -> AbortOnDropHandle<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        self.updater = Some(sender);
        let responder = self.responder.take();

        tracing::debug!("Spawning progress updater");
        AbortOnDropHandle::new(tokio::spawn(async move {
            tracing::debug!("Starting progress updater");
            while let Some(update) = receiver.recv().await {
                tracing::trace!("Sending update: {}", update);

                // If there's nothing to send updates to, that's fine
                if let Some(responder) = &responder {
                    responder.update(&update).await;
                }
            }
        }))
    }

    pub fn count_processed_fn(
        &self,
    ) -> impl Transformer<Input = String, Output = String> + WithIndexingDefaults + 'static {
        let processed_chunks = Arc::clone(&self.processed_chunks);
        let total_chunks = Arc::clone(&self.total_chunks);
        let updater = self
            .updater
            .as_ref()
            .expect("Progress updater not initialized")
            .clone();

        move |node| {
            let current = processed_chunks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

            tracing::debug!("Processed chunks: {}", current);

            let _ = updater.send(format!(
                "Indexing a bit of code {}/{}",
                current,
                total_chunks.load(std::sync::atomic::Ordering::Relaxed)
            ));

            Ok(node)
        }
    }

    pub fn count_total_fn(
        &self,
    ) -> impl Transformer<Input = String, Output = String> + WithIndexingDefaults + 'static {
        let processed_chunks = Arc::clone(&self.processed_chunks);
        let total_chunks = Arc::clone(&self.total_chunks);
        let updater = self
            .updater
            .as_ref()
            .expect("Progress updater not initialized")
            .clone();

        move |node| {
            let total_chunks = total_chunks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

            tracing::debug!("Total chunks: {}", total_chunks);

            let _ = updater.send(format!(
                "Indexing a bit of code {}/{}",
                processed_chunks.load(std::sync::atomic::Ordering::Relaxed),
                total_chunks
            ));

            Ok(node)
        }
    }

    pub fn send_update(&self, update: impl Into<String>) {
        if let Some(updater) = &self.updater {
            let _ = updater.send(update.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate;
    use swiftide::indexing::TextNode;

    use crate::commands::{MockResponder, Response};

    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn test_spawn() {
        let mut responder = MockResponder::default();
        responder
            .expect_send()
            .with(predicate::eq(Response::Activity("Test update".to_string())))
            .once();

        let responder = Arc::new(responder);
        let mut updater = ProgressUpdater::from(Some(responder.clone() as Arc<dyn Responder>));

        let _handle = updater.spawn();

        updater.send_update("Test update");
    }

    #[test_log::test(tokio::test)]
    async fn test_count_processed_fn() {
        let mut responder = MockResponder::new();
        responder
            .expect_update()
            .with(predicate::eq("Indexing a bit of code 1/0"))
            .returning(|_| ())
            .once();

        let responder = Arc::new(responder);
        let mut updater = ProgressUpdater::from(Some(responder.clone() as Arc<dyn Responder>));
        let _handle = updater.spawn();

        let transformer = updater.count_processed_fn();

        let node = TextNode::default();
        let result = transformer.transform_node(node).await.unwrap();

        // Give the task some time to process
        tokio::task::yield_now().await;

        assert_eq!(updater.processed_chunks.load(Ordering::Relaxed), 1);
        assert_eq!(result, TextNode::default());
    }

    #[tokio::test]
    async fn test_count_total_fn() {
        let mut responder = MockResponder::default();
        responder
            .expect_update()
            .with(predicate::eq("Indexing a bit of code 0/1"))
            .returning(|_| ())
            .once();
        let responder = Arc::new(responder);
        let mut updater = ProgressUpdater::from(Some(responder.clone() as Arc<dyn Responder>));
        let _handle = updater.spawn();
        let transformer = updater.count_total_fn();

        let node = TextNode::default();
        let result = transformer.transform_node(node).await.unwrap();

        // Give the task some time to process
        tokio::task::yield_now().await;

        assert_eq!(updater.total_chunks.load(Ordering::Relaxed), 1);
        assert_eq!(result, TextNode::default());
    }
}
