//! Series of commands to run before each agent starts inside the docker container
//!
//!
//!

use anyhow::bail;
use anyhow::Result;
use secrecy::ExposeSecret;
use swiftide::traits::Command;
use swiftide::traits::CommandOutput;
use swiftide::traits::ToolExecutor as _;

use crate::git::github::GithubSession;
use crate::repository::Repository;

use super::docker_tool_executor::RunningDockerExecutor;

pub struct EnvSetup<'a> {
    #[allow(dead_code)]
    repository: &'a Repository,
    github_session: &'a GithubSession,
    executor: &'a RunningDockerExecutor,
}

impl EnvSetup<'_> {
    pub fn new<'a>(
        repository: &'a Repository,
        github_session: &'a GithubSession,
        executor: &'a RunningDockerExecutor,
    ) -> EnvSetup<'a> {
        EnvSetup {
            repository,
            github_session,
            executor,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn exec_setup_commands(&self) -> Result<()> {
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

        self.executor
            .exec_cmd(&Command::shell(format!(
                "git remote set-url origin {}",
                url_with_token.expose_secret()
            )))
            .await?;

        Ok(())
    }
}
