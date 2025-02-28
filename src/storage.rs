//! Builds various storage providers for kwaak
//!
//! Handled as statics to avoid multiple instances of the same storage provider

use std::sync::OnceLock;

use anyhow::{Context, Result};
use swiftide::{
    indexing::{EmbeddedField, transformers},
    integrations::{lancedb::LanceDB, redb::Redb},
};

use crate::repository::Repository;

static LANCE_DB: OnceLock<LanceDB> = OnceLock::new();
static REDB: OnceLock<Redb> = OnceLock::new();

/// Retrieves a static lancedb
///
/// # Panics
///
/// Panics if it cannot setup lancedb
pub fn get_lancedb(repository: &Repository) -> LanceDB {
    LANCE_DB
        .get_or_init(|| build_lancedb(repository).expect("Failed to build LanceDB"))
        .to_owned()
}

/// Retrieves a static redb
///
/// # Panics
///
/// Panic if it cannot setup redb, i.e. its already open
pub fn get_redb(repository: &Repository) -> Redb {
    REDB.get_or_init(|| build_redb(repository).expect("Failed to build Redb"))
        .to_owned()
}

pub(crate) fn build_lancedb(repository: &Repository) -> Result<LanceDB> {
    let config = repository.config();
    let cache_dir = config.cache_dir().join("lancedb");

    tracing::debug!("Building LanceDB with cache dir: {}", cache_dir.display());

    let embedding_provider = config.embedding_provider();

    let uri = cache_dir
        .to_str()
        .context("Failed to convert path to string")?;
    LanceDB::builder()
        .uri(uri)
        .with_vector(EmbeddedField::Combined)
        .vector_size(embedding_provider.vector_size())
        .table_name(&config.project_name)
        .with_metadata("path")
        .with_metadata(transformers::metadata_qa_code::NAME)
        .with_metadata(transformers::metadata_qa_text::NAME)
        .build()
}

pub(crate) fn build_redb(repository: &Repository) -> Result<Redb> {
    let config = repository.config();
    let cache_dir = config.cache_dir().join("redb");

    tracing::debug!("Building Redb with cache dir: {}", cache_dir.display());

    Redb::builder()
        .database_path(cache_dir)
        .table_name(&config.project_name)
        .build()
}
