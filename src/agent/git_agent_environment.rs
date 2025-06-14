//! Sets up the git environment for the agent
//!
//! Only sets up auth if the docker executor is used

use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::CommandOutput;
use swiftide::traits::ToolExecutor;

use crate::config::SupportedToolExecutors;
use crate::repository::Repository;
use crate::util::accept_non_zero_exit;

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
        // Only run these command if the executor is not local
        if repository.config().tool_executor == SupportedToolExecutors::Local {
            tracing::debug!("Local executor detected, skipping git setup");

            return Ok(GitAgentEnvironment {
                branch_name: Self::get_current_branch(executor).await?,
                start_ref: Self::get_current_ref(executor).await?,
                remote_enabled: false,
            });
        }

        tracing::debug!("Configuring git user");
        Self::configure_git_user(repository, executor).await?;

        let mut remote_enabled = true;
        // If enabled, we clone the repository or pull the latest changes on the main branch,
        // before switching to the work branch
        if repository.config().git.clone_repository_on_start {
            tracing::debug!("Cloning or pulling main branch");
            Self::clone_or_pull_main(repository, executor).await?;
        } else {
            tracing::debug!("Skipping clone/pull of main branch");
            tracing::debug!("Adding credentials to remote");
            if let Err(e) = Self::setup_github_auth(repository, executor).await {
                tracing::warn!(error = ?e, "Failed to setup github auth");
                remote_enabled = false;
            }
        }

        tracing::debug!("Switching to work branch: {}", branch_name);
        Self::switch_to_work_branch(executor, branch_name).await?;

        Ok(GitAgentEnvironment {
            branch_name: Self::get_current_branch(executor).await?,
            start_ref: Self::get_current_ref(executor).await?,
            remote_enabled,
        })
    }

    async fn setup_github_auth(repository: &Repository, executor: &dyn ToolExecutor) -> Result<()> {
        let Ok(Some(github_session)) = repository.github_session().await else {
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

    async fn clone_or_pull_main(
        repository: &Repository,
        executor: &dyn ToolExecutor,
    ) -> Result<()> {
        // If the current directory is a git repository, we pull the latest changes
        // switching to the main branch
        if let Ok(output) = executor
            .exec_cmd(&Command::shell("git rev-parse --is-inside-work-tree"))
            .await
        {
            if output.as_ref() == "true" {
                Self::setup_github_auth(repository, executor).await?;

                // Stash any changes before pulling, just to be safe
                let _ = executor
                    .exec_cmd(&Command::shell("git stash --include-untracked"))
                    .await;

                // Check-out the main branch
                executor
                    .exec_cmd(&Command::shell(format!(
                        "git checkout {}",
                        repository.config().git.main_branch
                    )))
                    .await?;

                return Ok(());
            }
        }

        // Otherwise, we clone the repository

        let Ok(Some(github_session)) = repository.github_session().await else {
            anyhow::bail!("Github session is required to setup github auth");
        };

        let cmd = Command::Shell(format!(
            "git clone {} .",
            github_session.clone_url().await?.expose_secret()
        ));
        executor.exec_cmd(&cmd).await?;
        Ok(())
    }

    async fn switch_to_work_branch(executor: &dyn ToolExecutor, branch_name: &str) -> Result<()> {
        let cmd = Command::Shell(format!("git checkout -B {branch_name}"));
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
