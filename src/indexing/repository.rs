use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use crate::commands::Responder;
use crate::repository::Repository;
use crate::storage;
use anyhow::Result;
use swiftide::indexing::loaders;
use swiftide::indexing::transformers;
use swiftide::indexing::Node;
use swiftide::traits::EmbeddingModel;
use swiftide::traits::SimplePrompt;
use swiftide::traits::Transformer;

use super::garbage_collection::GarbageCollector;
use super::progress_updater::ProgressUpdater;

const CODE_CHUNK_RANGE: std::ops::Range<usize> = 100..2048;
const MARKDOWN_CHUNK_RANGE: std::ops::Range<usize> = 100..1024;

// NOTE: Indexing in parallel guarantees a bad time

#[tracing::instrument(skip_all)]
pub async fn index_repository(
    repository: &Repository,
    responder: Option<Arc<dyn Responder>>,
) -> Result<()> {
    let mut updater = ProgressUpdater::from(responder);

    // The updater forwards formatted progress updates to the connected frontend
    let _handle = updater.spawn();

    updater.send_update("Cleaning up the index ...");
    let garbage_collector = GarbageCollector::from_repository(repository);
    garbage_collector.clean_up().await?;

    updater.send_update("Starting to index your code ...");
    let mut extensions = repository.config().language.file_extensions().to_vec();
    extensions.push("md");

    let loader = loaders::FileLoader::new(repository.path()).with_extensions(&extensions);

    let backoff = repository.config().backoff;

    let indexing_provider: Box<dyn SimplePrompt> = repository
        .config()
        .indexing_provider()
        .get_simple_prompt_model(backoff)?;
    let embedding_provider: Box<dyn EmbeddingModel> = repository
        .config()
        .embedding_provider()
        .get_embedding_model(backoff)?;

    let duckdb = storage::get_duckdb(repository);

    let (mut markdown, mut code) = swiftide::indexing::Pipeline::from_loader(loader)
        .with_concurrency(repository.config().indexing_concurrency())
        .with_default_llm_client(indexing_provider)
        .filter_cached(duckdb.clone())
        .split_by(|node| {
            let Ok(node) = node else { return true };

            node.path.extension().is_none_or(|ext| ext == "md")
        });

    code = code
        .then_chunk(transformers::ChunkCode::try_for_language_and_chunk_size(
            repository.config().language,
            CODE_CHUNK_RANGE,
        )?)
        .then(updater.count_processed_fn())
        .then(transformers::MetadataQACode::default());

    markdown = markdown
        .then_chunk(transformers::ChunkMarkdown::from_chunk_range(
            MARKDOWN_CHUNK_RANGE,
        ))
        .then(updater.count_processed_fn())
        .then(transformers::MetadataQAText::default());

    let batch_size = repository.config().indexing_batch_size();
    code.merge(markdown)
        .log_errors()
        .filter_errors()
        .then_in_batch(transformers::Embed::new(embedding_provider).with_batch_size(batch_size))
        .then(|mut chunk: Node| {
            chunk
                .metadata
                .insert("path", chunk.path.display().to_string());

            Ok(chunk)
        })
        .then(updater.count_total_fn())
        .then_store_with(duckdb)
        .run()
        .await?;

    Ok(())
}
