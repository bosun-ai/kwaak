use anyhow::Result;
use std::{path::PathBuf, str::FromStr as _, sync::Arc};
use swiftide::query::search_strategies;
use swiftide::traits::{NodeCache, Persist, Retrieve};
use swiftide_integrations::duckdb::Duckdb;

use tokio::fs;

use crate::{config::Config, runtime_settings::RuntimeSettings, storage};

#[derive(Debug, Clone)]
pub struct Repository<S> {
    config: Config,
    path: PathBuf,
    storage: Arc<S>,
}

impl Repository<Duckdb> {
    /// Creates a new repository from a configuration
    ///
    /// # Panics
    ///
    /// Panics if the current directory cannot be converted to a path
    #[must_use]
    pub fn from_config(config: impl Into<Config>) -> Repository<Duckdb> {
        let config = config.into();
        let storage = Arc::new(storage::get_duckdb(&config));

        Self {
            config,
            path: PathBuf::from_str(".").expect("Failed to create path from current directory"),
            storage,
        }
    }

    // These only work with duckdb
    #[must_use]
    pub fn runtime_settings(&self) -> RuntimeSettings {
        RuntimeSettings::from_repository(&self)
    }
}

impl<S> Repository<S> {
    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn storage_cloned(&self) -> Arc<S> {
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
}
