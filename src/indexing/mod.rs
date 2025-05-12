mod progress_updater;
mod query;
mod repository;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
pub use query::build_query_pipeline;
pub use query::query;
pub use repository::index_repository;

use crate::commands::Responder;
use crate::repository::Repository;

#[cfg(feature = "duckdb")]
pub mod duckdb_index;

/// Garbage collection is specific for duckdb
#[cfg(feature = "duckdb")]
mod garbage_collection;

/// Interface that wraps storage providers
///
/// Implementors of index are expected to be owned and cheap to clone
#[async_trait]
pub trait Index: Send + Sync + std::fmt::Debug + Clone + 'static {
    async fn query_repository(
        &self,
        repository: &Repository,
        query: impl AsRef<str> + Send,
    ) -> Result<String>;

    async fn index_repository(
        &self,
        repository: &Repository,
        responder: Option<Arc<dyn Responder>>,
    ) -> Result<()>;
}

#[async_trait]
impl<I: Index> Index for Arc<I> {
    async fn query_repository(
        &self,
        repository: &Repository,
        query: impl AsRef<str> + Send,
    ) -> Result<String> {
        (**self).query_repository(repository, query).await
    }

    async fn index_repository(
        &self,
        repository: &Repository,
        responder: Option<Arc<dyn Responder>>,
    ) -> Result<()> {
        (**self).index_repository(repository, responder).await
    }
}
