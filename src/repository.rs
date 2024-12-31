use anyhow::Result;
use std::{path::PathBuf, str::FromStr as _};

use tokio::fs;

use crate::{config::Config, runtime_settings::RuntimeSettings};

#[derive(Debug, Clone)]
pub struct Repository {
    pub(crate) config: Config,
    pub(crate) path: PathBuf,
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

    pub fn config(&self) -> &Config {
        &self.config
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_repository;
    use std::fs as sync_fs;
    use tokio::runtime::Runtime; // Synchronous filesystem operations for test setup

    #[test]
    fn test_from_config() {
        let repo = test_repository();
        assert_eq!(repo.path(), &PathBuf::from("."));
    }

    #[test]
    fn test_path() {
        let repo = test_repository();
        assert_eq!(repo.path(), &PathBuf::from("."));
    }

    #[test]
    fn test_config() {
        let repo = test_repository();
        // Assuming Config implements PartialEq;
        assert_eq!(repo.config(), &repo.config);
    }

    #[test]
    fn test_into_repository() {
        let repo = test_repository();
        let _: Repository = (&repo).into(); // Check trait implementation
    }

    #[test]
    fn test_clear_cache() {
        let repo = test_repository();
        let rt = Runtime::new().unwrap();

        // Ensure the cache directory exists before attempting to clear it
        let cache_dir = repo.config.cache_dir();
        if !cache_dir.exists() {
            sync_fs::create_dir_all(cache_dir).expect("Failed to create cache directory for test");
        }

        // Run `clear_cache` asynchronously within the test runtime
        rt.block_on(async {
            assert!(repo.clear_cache().await.is_ok());
        });
    }
}
