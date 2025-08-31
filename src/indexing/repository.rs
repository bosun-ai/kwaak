use std::sync::Arc;

use crate::commands::Responder;
use crate::repository::Repository;
use anyhow::Result;
use swiftide::indexing::TextNode;
use swiftide::indexing::loaders;
use swiftide::indexing::transformers;
use swiftide::traits::EmbeddingModel;
use swiftide::traits::{NodeCache, Persist, SimplePrompt};

#[cfg(feature = "duckdb")]
use super::garbage_collection::GarbageCollector;
use super::multi_language_chunker::MultiLanguageChunker;
use super::progress_updater::ProgressUpdater;

const CODE_CHUNK_RANGE: std::ops::Range<usize> = 100..512;
const MARKDOWN_CHUNK_RANGE: std::ops::Range<usize> = 100..512;

#[tracing::instrument(skip_all)]
pub async fn index_repository<S>(
    repository: &Repository,
    storage: &S,
    responder: Option<Arc<dyn Responder>>,
) -> Result<()>
where
    S: Persist<Input = String, Output = String> + NodeCache<Input = String> + Clone + 'static,
{
    let mut updater = ProgressUpdater::from(responder);

    // The updater forwards formatted progress updates to the connected frontend
    let _handle = updater.spawn();

    garbage_collect(&updater, &repository).await?;

    updater.send_update("Starting to index your code ...");
    let mut extensions = repository.config().language_extensions();
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

    let (mut markdown, mut code) = swiftide::indexing::Pipeline::from_loader(loader)
        .with_concurrency(repository.config().indexing_concurrency())
        .with_default_llm_client(indexing_provider)
        .filter_cached(storage.clone())
        .split_by(|node| {
            let Ok(node) = node else { return true };

            node.path.extension().is_none_or(|ext| ext == "md")
        });

    code = code
        .then_chunk(MultiLanguageChunker::try_for_languages_and_chunk_size(
            &repository.config().languages,
            CODE_CHUNK_RANGE,
        )?)
        .then(updater.count_total_fn())
        .then(transformers::MetadataQACode::default());

    markdown = markdown
        .then_chunk(transformers::ChunkMarkdown::from_chunk_range(
            MARKDOWN_CHUNK_RANGE,
        ))
        .then(updater.count_total_fn())
        .then(transformers::MetadataQAText::default());

    let batch_size = repository.config().indexing_batch_size();
    code.merge(markdown)
        .log_errors()
        .filter_errors()
        .then_in_batch(transformers::Embed::new(embedding_provider).with_batch_size(batch_size))
        .then(|mut chunk: TextNode| {
            chunk
                .metadata
                .insert("path", chunk.path.display().to_string());

            Ok(chunk)
        })
        .then(updater.count_processed_fn())
        .then_store_with(storage.clone())
        .run()
        .await?;

    Ok(())
}

#[cfg(feature = "duckdb")]
async fn garbage_collect(updater: &ProgressUpdater, repository: &Repository) -> Result<()> {
    updater.send_update("Cleaning up the index ...");
    let garbage_collector = GarbageCollector::from_repository(repository);
    garbage_collector.clean_up().await
}

#[cfg(not(feature = "duckdb"))]
async fn garbage_collect(_updater: &ProgressUpdater, _repository: &Repository) -> Result<()> {
    Ok(())
}
