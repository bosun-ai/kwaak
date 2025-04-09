mod garbage_collection;
mod progress_updater;
mod query;
mod repository;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
pub use query::build_query_pipeline;
pub use query::query;
pub use repository::index_repository;
use swiftide::integrations::duckdb::Duckdb;
use swiftide::query::{states, Pipeline};

use crate::commands::Responder;
use crate::repository::Repository;
use crate::storage::get_duckdb;

/// Interface that wraps storage providers
#[async_trait]
pub trait Index: Send + Sync + std::fmt::Debug {
    type SearchStrategy: swiftide::query::SearchStrategy + 'static;

    fn build_query_pipeline<'b>(
        &self,
        repository: &Repository,
    ) -> Result<Pipeline<'b, Self::SearchStrategy, states::Answered>>;

    async fn query_repository<'b>(
        &self,
        repository: &Repository,
        query: impl AsRef<str> + Send,
    ) -> Result<String>;

    async fn index_repository<'b>(
        &self,
        repository: &Repository,
        responder: Option<Arc<dyn Responder>>,
    ) -> Result<()>;
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
    type SearchStrategy = swiftide::query::search_strategies::SimilaritySingleEmbedding<()>;

    fn build_query_pipeline<'b>(
        &self,
        repository: &Repository,
    ) -> Result<Pipeline<'b, Self::SearchStrategy, states::Answered>> {
        let storage = self.get_duckdb(repository);
        query::build_query_pipeline(repository, &storage, None)
    }

    async fn query_repository<'b>(
        &self,
        repository: &Repository,
        query: impl AsRef<str> + Send,
    ) -> Result<String> {
        let storage = self.get_duckdb(repository);
        query::query(repository, &storage, query).await
    }

    async fn index_repository<'b>(
        &self,
        repository: &Repository,
        responder: Option<Arc<dyn Responder>>,
    ) -> Result<()> {
        let storage = self.get_duckdb(repository);
        index_repository(repository, &storage, responder).await
    }
}
