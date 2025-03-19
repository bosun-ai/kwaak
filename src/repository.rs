use anyhow::Result;
use std::{path::PathBuf, str::FromStr as _, sync::Arc};
use swiftide::query::search_strategies;
use swiftide::traits::{NodeCache, Persist, Retrieve};

use tokio::fs;

use crate::{config::Config, runtime_settings::RuntimeSettings, storage};

/// Wrapper trait for storage; this is what kwaak minimally expects from a storage provider
trait KwaakStorage:
    Persist + Retrieve<search_strategies::SimilaritySingleEmbedding> + NodeCache
{
}
impl<T> KwaakStorage for T where
    T: Persist + Retrieve<search_strategies::SimilaritySingleEmbedding> + NodeCache
{
}

#[derive(Debug, Clone)]
pub struct Repository {
    config: Config,
    path: PathBuf,
    storage: Arc<dyn KwaakStorage>,
}

impl Repository {
    /// Creates a new repository from a configuration
    ///
    /// # Panics
    ///
    /// Panics if the current directory cannot be converted to a path
    #[must_use]
    pub fn from_config(config: impl Into<Config>) -> Repository {
        let config: Config = config.into();
        let storage = Arc::new(storage::get_duckdb(&config));
        Self {
            config,
            path: PathBuf::from_str(".").expect("Failed to create path from current directory"),
            storage,
        }
    }

    pub fn storage(&self) -> Arc<dyn KwaakStorage> {
        self.storage.clone()
    }

    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    pub async fn clear_cache(&self) -> Result<()> {
        fs::remove_dir_all(self.config.cache_dir()).await?;
        Ok(())
    }

    #[must_use]
    pub fn runtime_settings(&self) -> RuntimeSettings {
        RuntimeSettings::from_repository(self)
    }
}

#[allow(clippy::from_over_into)]
impl Into<Repository> for &Repository {
    fn into(self) -> Repository {
        self.clone()
    }
}
