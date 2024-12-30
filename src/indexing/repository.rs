use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::commands::CommandResponder;
use crate::repository::Repository;
use crate::storage;
use anyhow::Result;
use swiftide::indexing::loaders;
use swiftide::indexing::transformers;
use swiftide::indexing::Node;
use swiftide::traits::EmbeddingModel;
use swiftide::traits::NodeCache;
use swiftide::traits::Persist;
use swiftide::traits::SimplePrompt;

use super::garbage_collection::GarbageCollector;

// NOTE: Indexing in parallel guarantees a bad time

#[tracing::instrument(skip_all)]
pub async fn index_repository(
    repository: &Repository,
    responder: Option<CommandResponder>,
) -> Result<()> {
    let updater = UiUpdater::from(responder);

    updater.send_update("Cleaning up the index ...");
    let garbage_collector = GarbageCollector::from_repository(repository);
    garbage_collector.clean_up().await?;

    updater.send_update("Starting to index your code ...");
    let extensions = repository.config().language.file_extensions();
    let loader = loaders::FileLoader::new(repository.path()).with_extensions(extensions);
    // NOTE: Parameter to optimize on
    let chunk_size = 100..2048;

    let indexing_provider: Box<dyn SimplePrompt> =
        repository.config().indexing_provider().try_into()?;
    let embedding_provider: Box<dyn EmbeddingModel> =
        repository.config().embedding_provider().try_into()?;

    let lancedb = storage::get_lancedb(repository) as Arc<dyn Persist>;
    let redb = storage::get_redb(repository) as Arc<dyn NodeCache>;

    let total_chunks = Arc::new(AtomicU64::new(0));
    let processed_chunks = Arc::new(AtomicU64::new(0));

    swiftide::indexing::Pipeline::from_loader(loader)
        .with_concurrency(repository.config().indexing_concurrency)
        .filter_cached(redb)
        .then_chunk(transformers::ChunkCode::try_for_language_and_chunk_size(
            repository.config().language,
            chunk_size,
        )?)
        .then({
            let total_chunks = Arc::clone(&total_chunks);
            let processed_chunks = Arc::clone(&processed_chunks);
            let updater = updater.clone();

            move |node| {
                let total_chunks = total_chunks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                updater.send_update(format!(
                    "Indexing a bit of code {}/{}",
                    processed_chunks
                        .clone()
                        .load(std::sync::atomic::Ordering::Relaxed),
                    total_chunks
                ));

                Ok(node)
            }
        })
        .then(transformers::MetadataQACode::new(indexing_provider))
        // Since OpenAI is IO bound, making many small embedding requests in parallel is faster
        .then_in_batch(transformers::Embed::new(embedding_provider).with_batch_size(12))
        .then(|mut chunk: Node| {
            chunk
                .metadata
                .insert("path", chunk.path.display().to_string());

            Ok(chunk)
        })
        .then(move |node| {
            let current = processed_chunks
                .clone()
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            updater.send_update(format!(
                "Indexing a bit of code {}/{}",
                current,
                total_chunks
                    .clone()
                    .load(std::sync::atomic::Ordering::Relaxed)
            ));

            Ok(node)
        })
        .then_store_with(lancedb)
        .run()
        .await
}

// Just a simple wrapper so we can avoid having to Option check all the time
#[derive(Debug, Clone)]
struct UiUpdater(Option<CommandResponder>);

impl UiUpdater {
    fn send_update(&self, state: impl Into<String>) {
        let Some(responder) = &self.0 else { return };
        responder.send_update(state);
    }
}

impl From<Option<CommandResponder>> for UiUpdater {
    fn from(responder: Option<CommandResponder>) -> Self {
        Self(responder)
    }
}
