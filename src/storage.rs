//! Builds various storage providers for kwaak

use anyhow::Result;
use swiftide::indexing::EmbeddedField;
use swiftide::integrations::lancedb::{LanceDB, LanceDBBuilder};
use swiftide::integrations::redb::{Redb, RedbBuilder};

use crate::repository::Repository;

pub fn build_lancedb(repository: &Repository) -> Result<LanceDBBuilder> {
    let config = repository.config();
    let mut cache_dir = config.cache_dir().to_owned();
    cache_dir.push("lancedb");

    let embedding_provider = config.embedding_provider();

    Ok(LanceDB::builder()
        .uri(
            cache_dir
                .to_str()
                .ok_or(anyhow::anyhow!("Failed to convert path to string"))?,
        )
        .with_vector(EmbeddedField::Combined)
        .vector_size(embedding_provider.vector_size())
        .table_name(&config.project_name)
        .to_owned())
}

#[allow(clippy::unnecessary_wraps)]
pub fn build_redb(repository: &Repository) -> Result<RedbBuilder> {
    let config = repository.config();
    let mut cache_dir = config.cache_dir().to_owned();
    cache_dir.push("redb");

    let redb_builder = Redb::builder()
        .database_path(cache_dir)
        .table_name(&config.project_name)
        .to_owned();

    Ok(redb_builder)
}
