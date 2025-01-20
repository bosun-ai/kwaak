//! Series of commands to run before each agent starts inside the docker container

use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::ToolExecutor;

use crate::config::SupportedToolExecutors;
use crate::git::github::GithubSession;
use crate::repository::Repository;

/// Configures and sets up a git (and github if enabled) environment for the agent to run in
pub struct EnvSetup<'a> {
    repository: &'a Repository,
    github_session: Option<&'a GithubSession>,
    executor: &'a dyn ToolExecutor,
}

/// Returned after setting up the environment
#[derive(Default, Debug, Clone)]
pub struct AgentEnvironment {
    #[allow(dead_code)]
    pub branch_name: String,
    pub start_ref: String,
    pub remote_enabled: bool,
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

    #[tracing::instrument(skip_all, err)]
    pub async fn exec_setup_commands(&self, branch_name: String) -> Result<AgentEnvironment> {
        // Only run these commands if we are running inside a docker container
        if self.repository.config().tool_executor != SupportedToolExecutors::Docker {
            return Ok(AgentEnvironment {
                branch_name: self.get_current_branch().await?,
                start_ref: self.get_current_ref().await?,
                remote_enabled: false,
            });
        }

        let mut remote_enabled = true;
        if let Err(e) = self.setup_github_auth().await {
            tracing::warn!(error = ?e, "Failed to setup github auth");
            remote_enabled = false;
        }

        self.configure_git_user().await?;
        self.switch_to_work_branch(branch_name).await?;

        Ok(AgentEnvironment {
            branch_name: self.get_current_branch().await?,
            start_ref: self.get_current_ref().await?,
            remote_enabled,
        })
    }

    async fn setup_github_auth(&self) -> Result<()> {
        let Some(github_session) = self.github_session else {
            anyhow::bail!("Github session is required to setup github auth");
        };

        let Ok(origin_url) = self
            .executor
            .exec_cmd(&Command::shell("git remote get-url origin"))
            .await
            .map(|t| t.output)
        else {
            anyhow::bail!("Could not get origin url; does the repository have a remote of origin enabled? Github integration will be disabled");
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

    async fn switch_to_work_branch(&self, branch_name: String) -> Result<()> {
        let cmd = Command::Shell(format!("git checkout -b {branch_name}"));
        self.executor.exec_cmd(&cmd).await?;
        Ok(())
    }

    async fn get_current_ref(&self) -> Result<String> {
        let cmd = Command::shell("git rev-parse HEAD");
        let output = self.executor.exec_cmd(&cmd).await?;
        tracing::debug!("agent starting from ref: {}", output.output.trim());
        Ok(output.output.trim().to_string())
    }

    async fn get_current_branch(&self) -> Result<String> {
        let cmd = Command::shell("git rev-parse --abbrev-ref HEAD");
        let output = self.executor.exec_cmd(&cmd).await?;
        tracing::debug!("agent starting from branch: {}", output.output.trim());
        Ok(output.output.trim().to_string())
    }
}
