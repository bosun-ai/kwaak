use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use swiftide_integrations::duckdb::Duckdb;

use crate::{commands::Responder, duckdb::get_duckdb, repository::Repository};

use super::{index_repository, query, Index};

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
