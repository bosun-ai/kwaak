//! Series of commands to run before each agent starts inside the docker container

use anyhow::bail;
use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::CommandOutput;
use swiftide::traits::ToolExecutor;
use swiftide::traits::ToolExecutor as _;

use crate::config::SupportedToolExecutors;
use crate::git::github::GithubSession;
use crate::repository::Repository;


pub struct EnvSetup<'a> {
    #[allow(dead_code)]
    repository: &'a Repository,
    github_session: &'a GithubSession,
    executor: &'a dyn ToolExecutor,
}

impl EnvSetup<'_> {
    pub fn new<'a>(
        repository: &'a Repository,
        github_session: &'a GithubSession,
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
        let CommandOutput::Shell {
            stdout: origin_url, ..
        } = self
            .executor
            .exec_cmd(&Command::shell("git remote get-url origin"))
            .await?
        else {
            bail!("Could not get origin url")
        };

        let url_with_token = self.github_session.add_token_to_url(&origin_url)?;

        for cmd in &[
            Command::shell(format!(
                "git remote set-url origin {}",
                url_with_token.expose_secret()
            )),
            Command::shell("git config --global user.email \"kwaak@bosun.ai\""),
            Command::shell("git config --global user.name \"kwaak\""),
        ] {
            self.executor.exec_cmd(cmd).await?;
        }

        Ok(())
    }
}
