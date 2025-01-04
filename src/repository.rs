use anyhow::Result;
use std::{path::PathBuf, str::FromStr as _};

use tokio::fs;

use crate::{config::Config, runtime_settings::RuntimeSettings};

#[derive(Debug, Clone)]
pub struct Repository {
    config: Config,
    path: PathBuf,
}

impl Repository {
    pub fn from_config(config: impl Into<Config>) -> Repository {
        Self {
            config: config.into(),
            path: PathBuf::from_str(".").expect("Failed to create path from current directory"),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }

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
