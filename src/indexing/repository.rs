use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::commands::Responder;
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

const CODE_CHUNK_RANGE: std::ops::Range<usize> = 100..2048;
const MARKDOWN_CHUNK_RANGE: std::ops::Range<usize> = 100..1024;

// NOTE: Indexing in parallel guarantees a bad time

#[tracing::instrument(skip_all)]
pub async fn index_repository(
    repository: &Repository,
    responder: Option<Arc<dyn Responder>>,
) -> Result<()> {
    let updater = UiUpdater::from(responder);

    updater.send_update("Cleaning up the index ...");
    let garbage_collector = GarbageCollector::from_repository(repository);
    garbage_collector.clean_up().await?;

    updater.send_update("Starting to index your code ...");
    let mut extensions = repository.config().language.file_extensions().to_vec();
    extensions.push("md");

    let loader = loaders::FileLoader::new(repository.path()).with_extensions(&extensions);

    let indexing_provider: Box<dyn SimplePrompt> =
        repository.config().indexing_provider().try_into()?;
    let embedding_provider: Box<dyn EmbeddingModel> =
        repository.config().embedding_provider().try_into()?;

    let lancedb = storage::get_lancedb(repository);
    let redb = storage::get_redb(repository) as Arc<dyn NodeCache>;

    let total_chunks = Arc::new(AtomicU64::new(0));
    let processed_chunks = Arc::new(AtomicU64::new(0));

    let (mut markdown, mut code) = swiftide::indexing::Pipeline::from_loader(loader)
        .with_concurrency(repository.config().indexing_concurrency())
        .with_default_llm_client(indexing_provider)
        .filter_cached(redb)
        .split_by(|node| {
            let Ok(node) = node else { return true };

            node.path.extension().map_or(true, |ext| ext == "md")
        });

    code = code
        .then_chunk(transformers::ChunkCode::try_for_language_and_chunk_size(
            repository.config().language,
            CODE_CHUNK_RANGE,
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
        .then(transformers::MetadataQACode::default());

    markdown = markdown
        .then_chunk(transformers::ChunkMarkdown::from_chunk_range(
            MARKDOWN_CHUNK_RANGE,
        ))
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
        .then(transformers::MetadataQAText::default());

    let batch_size = repository.config().indexing_batch_size();
    code.merge(markdown)
        .then_in_batch(transformers::Embed::new(embedding_provider).with_batch_size(batch_size))
        .then(|mut chunk: Node| {
            chunk
                .metadata
                .insert("path", chunk.path.display().to_string());

            Ok(chunk)
        })
        .then({
            let updater = updater.clone();

            move |node| {
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
            }
        })
        .then_store_with(Arc::clone(&lancedb) as Arc<dyn Persist>)
        .run()
        .await?;

    updater.send_update("Creating column indices ...");
    // NOTE: Disable indexing for now, it uses ANN, which kinda sucks at small scale when
    // performance is fine already
    //
    // let table = lancedb.open_table().await?;
    // let column_name = format!("vector_{}", EmbeddedField::Combined.field_name());
    //
    // table
    //     .create_index(&[&column_name], lancedb::index::Index::Auto)
    //     .execute()
    //     .await?;

    Ok(())
}

// Just a simple wrapper so we can avoid having to Option check all the time
#[derive(Debug, Clone)]
struct UiUpdater(Option<Arc<dyn Responder>>);

impl UiUpdater {
    fn send_update(&self, state: impl AsRef<str>) {
        let Some(responder) = &self.0 else { return };
        responder.update(state.as_ref());
    }
}

impl From<Option<Arc<dyn Responder>>> for UiUpdater {
    fn from(responder: Option<Arc<dyn Responder>>) -> Self {
        Self(responder)
    }
}
