//! Builds various storage providers for kwaak
//!
//! Handled as statics to avoid multiple instances of the same storage provider

use std::sync::OnceLock;

use anyhow::{Context, Result};
use swiftide::{
    indexing::{transformers, EmbeddedField},
    integrations::{duckdb::Duckdb, redb::Redb},
};

use crate::repository::Repository;

static LANCE_DB: OnceLock<Duckdb> = OnceLock::new();
static REDB: OnceLock<Redb> = OnceLock::new();

/// Retrieves a static duckdb
///
/// # Panics
///
/// Panics if it cannot setup duckdb
pub fn get_duckdb(repository: &Repository) -> Duckdb {
    LANCE_DB
        .get_or_init(|| build_duckdb(repository).expect("Failed to build duckdb"))
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

pub(crate) fn build_duckdb(repository: &Repository) -> Result<Duckdb> {
    let config = repository.config();
    let path = config.cache_dir().join("duck.db3");

    tracing::debug!("Building Duckdb: {}", path.display());

    let embedding_provider = config.embedding_provider();

    let connection =
        duckdb::Connection::open(&path).context("Failed to open connection to duckdb")?;
    Duckdb::builder()
        .connection(connection)
        .with_vector(
            EmbeddedField::Combined,
            embedding_provider.vector_size() as usize,
        )
        .table_name(&config.project_name)
        .build()
        .context("Failed to build Duckdb")
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
