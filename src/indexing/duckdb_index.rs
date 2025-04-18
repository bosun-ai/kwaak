use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use swiftide_integrations::duckdb::Duckdb;

use crate::{commands::Responder, repository::Repository};

use super::{index_repository, query, Index};

use std::sync::OnceLock;

use anyhow::Context;
use swiftide::indexing::EmbeddedField;

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
#[derive(Clone, Debug, Default)]
pub struct DuckdbIndex {}

impl DuckdbIndex {
    #[allow(clippy::unused_self)]
    fn get_duckdb(&self, repository: &Repository) -> Duckdb {
        get_duckdb(repository)
    }
}

#[async_trait]
impl Index for DuckdbIndex {
    async fn query_repository(
        &self,
        repository: &Repository,
        query: impl AsRef<str> + Send,
    ) -> Result<String> {
        let storage = self.get_duckdb(repository);
        query::query(repository, &storage, query).await
    }

    async fn index_repository(
        &self,
        repository: &Repository,
        responder: Option<Arc<dyn Responder>>,
    ) -> Result<()> {
        let storage = self.get_duckdb(repository);
        index_repository(repository, &storage, responder).await
    }
}
