use std::sync::Arc;

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
pub async fn index_repository(repository: &Repository) -> Result<()> {
    let garbage_collector = GarbageCollector::from_repository(repository);
    garbage_collector.clean_up().await;

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

    swiftide::indexing::Pipeline::from_loader(loader)
        .filter_cached(redb)
        .then_chunk(transformers::ChunkCode::try_for_language_and_chunk_size(
            repository.config().language,
            chunk_size,
        )?)
        .then(transformers::MetadataQACode::new(indexing_provider))
        .then_in_batch(transformers::Embed::new(embedding_provider))
        .then(|mut chunk: Node| {
            chunk
                .metadata
                .insert("path", chunk.path.display().to_string());

            Ok(chunk)
        })
        .then_store_with(lancedb)
        .run()
        .await
}
