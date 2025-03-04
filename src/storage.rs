//! Builds various storage providers for kwaak
//!
//! Handled as statics to avoid multiple instances of the same storage provider

use std::sync::OnceLock;

use anyhow::{Context, Result};
use swiftide::{indexing::EmbeddedField, integrations::duckdb::Duckdb};

use crate::repository::Repository;

static DUCK_DB: OnceLock<Duckdb> = OnceLock::new();

/// Retrieves a static duckdb
///
/// # Panics
///
/// Panics if it cannot setup duckdb
pub fn get_duckdb(repository: &Repository) -> Duckdb {
    DUCK_DB
        .get_or_init(|| build_duckdb(repository).expect("Failed to build duckdb"))
        .to_owned()
}

// Probably should just be on the repository/config, cloned from there.
// This sucks in tests
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
            embedding_provider.vector_size().try_into()?,
        )
        .table_name(normalize_table_name(&config.project_name))
        .cache_table(format!(
            "cache_{}",
            normalize_table_name(&config.project_name)
        ))
        .build()
        .context("Failed to build Duckdb")
}

// Is this enough?
fn normalize_table_name(name: &str) -> String {
    name.replace('-', "_")
}
