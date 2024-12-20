//! Series of commands to run before each agent starts inside the docker container

use anyhow::bail;
use anyhow::Context as _;
use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::ToolExecutor;

use crate::config::SupportedToolExecutors;
use crate::git::github::GithubSession;
use crate::repository::Repository;

pub struct EnvSetup<'a> {
    #[allow(dead_code)]
    repository: &'a Repository,
    github_session: Option<&'a GithubSession>,
    executor: &'a dyn ToolExecutor,
}

impl EnvSetup<'_> {
    pub fn new<'a>(
        repository: &'a Repository,
        github_session: Option<&'a GithubSession>,
        executor: &'a dyn ToolExecutor,
    ) -> EnvSetup<'a> {
        EnvSetup {
            repository,
            github_session,
            executor,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn exec_setup_commands(&self) -> Result<()> {
        // Only run these commands if we are running inside a docker container
        if self.repository.config().tool_executor != SupportedToolExecutors::Docker {
            return Ok(());
        }

        self.setup_github_auth().await?;
        self.configure_git_user().await?;
        self.switch_to_work_branch().await?;

        Ok(())
    }

    async fn setup_github_auth(&self) -> Result<()> {
        let origin_url = self
            .executor
            .exec_cmd(&Command::shell("git remote get-url origin"))
            .await
            .context("Could not get origin url")?
            .output;

        let Some(github_session) = self.github_session else {
            bail!("When running inside docker, a valid github token is required")
        };

        let url_with_token = github_session.add_token_to_url(&origin_url)?;

        let cmd = Command::shell(format!(
            "git remote set-url origin {}",
            url_with_token.expose_secret()
        ));
        self.executor.exec_cmd(&cmd).await?;

        Ok(())
    }

    async fn configure_git_user(&self) -> Result<()> {
        for cmd in &[
            Command::shell("git config --global user.email \"kwaak@bosun.ai\""),
            Command::shell("git config --global user.name \"kwaak\""),
            Command::shell("git config --global push.autoSetupRemote true"),
        ] {
            self.executor.exec_cmd(cmd).await?;
        }

        Ok(())
    }

    async fn switch_to_work_branch(&self) -> Result<()> {
        let branch_name = format!("kwaak-{}", uuid::Uuid::new_v4());
        let cmd = Command::Shell(format!("git checkout -b {branch_name}"));
        self.executor.exec_cmd(&cmd).await?;

        Ok(())
    }
}
