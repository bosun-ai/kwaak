use anyhow::Result;
use std::{
    path::PathBuf,
    str::FromStr as _,
    sync::{Arc, Mutex},
};
use swiftide::agents::tools::local_executor::LocalExecutor;
use swiftide::traits::ToolExecutor;
use swiftide_docker_executor::DockerExecutor;
use uuid::Uuid;

use tokio::{fs, sync::OnceCell};

#[cfg(feature = "duckdb")]
use crate::runtime_settings::RuntimeSettings;
use crate::{
    config::{Config, SupportedToolExecutors},
    git::github::GithubSession,
};

#[derive(Debug, Clone)]
pub struct Repository {
    config: Config,
    path: PathBuf,

    /// Lazy loaded github session if the repository is configured to use github
    github_session: Arc<OnceCell<Option<GithubSession>>>,
}

impl Repository {
    /// Creates a new repository from a configuration
    ///
    /// # Panics
    ///
    /// Panics if the current directory cannot be converted to a path
    #[must_use]
    pub fn from_config(config: impl Into<Config>) -> Repository {
        let config = config.into();
        let repository = Self {
            config,
            path: PathBuf::from_str(".")
                .expect("Failed to create path from current directory")
                .into(),
            github_session: Arc::new(OnceCell::new()),
        };

        repository
    }

    pub fn with_github_session(&mut self, session: &GithubSession) -> &mut Self {
        self.github_session = Arc::new(OnceCell::from(Some(session.clone())));

        self
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

    // TODO: Properly handle this
    #[cfg(feature = "duckdb")]
    #[must_use]
    pub fn runtime_settings(&self) -> RuntimeSettings {
        RuntimeSettings::from_repository(self)
    }

    /// Starts a new tool executor for this repository based on the configuration
    #[tracing::instrument(skip(self), err)]
    pub async fn start_tool_executor(
        &self,
        container_uuid: Option<Uuid>,
    ) -> Result<Arc<dyn ToolExecutor>> {
        let boxed = match self.config().tool_executor {
            SupportedToolExecutors::Docker => {
                let mut executor = DockerExecutor::default();
                let dockerfile = &self.config().docker.dockerfile;

                if std::fs::metadata(dockerfile).is_err() {
                    tracing::error!("Dockerfile not found at {}", dockerfile.display());
                    return Err(anyhow::anyhow!("Dockerfile not found"));
                }
                let running_executor = executor
                    .with_context_path(&self.config().docker.context)
                    .with_image_name(self.config().project_name.to_lowercase())
                    .with_dockerfile(dockerfile)
                    .with_container_uuid(container_uuid.unwrap_or_else(Uuid::new_v4))
                    .to_owned()
                    .start()
                    .await?;

                Arc::new(running_executor) as Arc<dyn ToolExecutor>
            }
            SupportedToolExecutors::Local => {
                Arc::new(LocalExecutor::new(".")) as Arc<dyn ToolExecutor>
            }
        };

        Ok(boxed)
    }

    /// Gets a github session for this repository if it is enabled
    ///
    /// Initializes the session lazily, so it will only be created when first accessed.
    pub async fn github_session(&self) -> Result<Option<&GithubSession>> {
        self.github_session
            .get_or_try_init(|| async {
                if self.config().is_github_enabled() {
                    return Some(GithubSession::from_repository(self)).transpose();
                }
                Ok(None)
            })
            .await
            .map(Option::as_ref)
    }
}
