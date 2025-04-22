//! Sets up the git environment for the agent
//!
//! Only sets up auth if the docker executor is used

use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::ToolExecutor;

use crate::config::SupportedToolExecutors;
use crate::repository::Repository;

/// Returned after setting up the environment
#[derive(Default, Debug, Clone)]
pub struct GitAgentEnvironment {
    pub branch_name: String,
    pub start_ref: String,
    pub remote_enabled: bool,
}

impl GitAgentEnvironment {
    #[tracing::instrument(skip_all, err)]
    pub async fn setup(
        repository: &Repository,
        executor: &dyn ToolExecutor,
        branch_name: &str,
    ) -> Result<Self> {
        // Only run these commands if we are running inside a docker container
        if repository.config().tool_executor != SupportedToolExecutors::Docker {
            return Ok(GitAgentEnvironment {
                branch_name: Self::get_current_branch(executor).await?,
                start_ref: Self::get_current_ref(executor).await?,
                remote_enabled: false,
            });
        }

        let mut remote_enabled = true;
        if let Err(e) = Self::setup_github_auth(repository, executor).await {
            tracing::warn!(error = ?e, "Failed to setup github auth");
            remote_enabled = false;
        }

        Self::configure_git_user(repository, executor).await?;

        Self::switch_to_work_branch(executor, branch_name).await?;
        Ok(GitAgentEnvironment {
            branch_name: Self::get_current_branch(executor).await?,
            start_ref: Self::get_current_ref(executor).await?,
            remote_enabled,
        })
    }

    async fn setup_github_auth(repository: &Repository, executor: &dyn ToolExecutor) -> Result<()> {
        let Some(github_session) = repository.github_session() else {
            anyhow::bail!("Github session is required to setup github auth");
        };

        let Ok(origin_url) = executor
            .exec_cmd(&Command::shell("git remote get-url origin"))
            .await
            .map(|t| t.output)
        else {
            anyhow::bail!(
                "Could not get origin url; does the repository have a remote of origin enabled? Github integration will be disabled"
            );
        };

        let url_with_token = github_session.add_token_to_url(&origin_url)?;

        let cmd = Command::shell(format!(
            "git remote set-url origin {}",
            url_with_token.expose_secret()
        ));
        executor.exec_cmd(&cmd).await?;

        Ok(())
    }

    async fn configure_git_user(
        repository: &Repository,
        executor: &dyn ToolExecutor,
    ) -> Result<()> {
        let name = &repository.config().git.agent_user_name;
        let email = &repository.config().git.agent_user_email;
        for cmd in &[
            Command::shell(format!("git config --global user.name \"{name}\"")),
            Command::shell(format!("git config --global user.email \"{email}\"")),
            Command::shell("git config --global push.autoSetupRemote true"),
        ] {
            executor.exec_cmd(cmd).await?;
        }

        Ok(())
    }

    async fn switch_to_work_branch(executor: &dyn ToolExecutor, branch_name: &str) -> Result<()> {
        let cmd = Command::Shell(format!("git checkout -b {branch_name}"));
        executor.exec_cmd(&cmd).await?;
        Ok(())
    }

    async fn get_current_ref(executor: &dyn ToolExecutor) -> Result<String> {
        let cmd = Command::shell("git rev-parse HEAD");
        let output = executor.exec_cmd(&cmd).await?;
        tracing::debug!("agent starting from ref: {}", output.output.trim());
        Ok(output.output.trim().to_string())
    }

    async fn get_current_branch(executor: &dyn ToolExecutor) -> Result<String> {
        let cmd = Command::shell("git rev-parse --abbrev-ref HEAD");
        let output = executor.exec_cmd(&cmd).await?;
        tracing::debug!("agent starting from branch: {}", output.output.trim());
        Ok(output.output.trim().to_string())
    }
}
