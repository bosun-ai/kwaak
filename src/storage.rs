//! Builds various storage providers for kwaak
//!
//! Handled as statics to avoid multiple instances of the same storage provider

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use swiftide::indexing::{transformers, EmbeddedField};
use swiftide::integrations::lancedb::{LanceDB, LanceDBBuilder};
use swiftide::integrations::redb::{Redb, RedbBuilder};

use crate::repository::Repository;

static LANCE_DB: OnceLock<Arc<LanceDB>> = OnceLock::new();
static REDB: OnceLock<Arc<Redb>> = OnceLock::new();

pub fn get_lancedb(repository: &Repository) -> Arc<LanceDB> {
    Arc::clone(LANCE_DB.get_or_init(|| {
        Arc::new(
            build_lancedb(repository)
                .expect("Failed to build LanceDB")
                .build()
                .expect("Failed to build LanceDB"),
        )
    }))
}

pub fn get_redb(repository: &Repository) -> Arc<Redb> {
    Arc::clone(REDB.get_or_init(|| {
        Arc::new(
            build_redb(repository)
                .expect("Failed to build Redb")
                .build()
                .expect("Failed to build Redb"),
        )
    }))
}

fn build_lancedb(repository: &Repository) -> Result<LanceDBBuilder> {
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
        .with_metadata("path")
        .with_metadata(transformers::metadata_qa_code::NAME)
        .with_metadata(transformers::metadata_qa_text::NAME)
        .to_owned())
}

#[allow(clippy::unnecessary_wraps)]
fn build_redb(repository: &Repository) -> Result<RedbBuilder> {
    let config = repository.config();
    let mut cache_dir = config.cache_dir().to_owned();
    cache_dir.push("redb");

    let redb_builder = Redb::builder()
        .database_path(cache_dir)
        .table_name(&config.project_name)
        .to_owned();

    Ok(redb_builder)
}
