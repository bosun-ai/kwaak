use anyhow::{Context as _, Result};
use swiftide::agents::hooks::AfterEachFn;
use swiftide::traits::{Command, SimplePrompt};

use crate::git;
use crate::{repository::Repository, util::accept_non_zero_exit};

use super::env_setup::AgentEnvironment;

#[derive(Debug)]
pub struct CommitAndPush {
    auto_commit_enabled: bool,
    push_to_remote_enabled: bool,
    llm: Box<dyn SimplePrompt>,
}

impl CommitAndPush {
    pub fn try_new(repository: &Repository, agent_env: &AgentEnvironment) -> Result<Self> {
        let auto_commit_enabled = !repository.config().git.auto_commit_disabled;
        let push_to_remote_enabled =
            agent_env.remote_enabled && repository.config().git.auto_push_remote;
        let llm = repository
            .config()
            .indexing_provider()
            .get_simple_prompt_model(repository.config().backoff)
            .context("could not get simple prompt provider for commit and push")?;

        Ok(Self {
            auto_commit_enabled,
            push_to_remote_enabled,
            llm,
        })
    }

    pub fn hook(self) -> impl AfterEachFn {
        let llm = self.llm;
        move |agent| {
            let auto_commit_enabled = self.auto_commit_enabled;
            let push_to_remote_enabled = self.push_to_remote_enabled;
            let llm = llm.clone();

            Box::pin(async move {
                if auto_commit_enabled {
                    if accept_non_zero_exit(
                        agent
                            .context()
                            .exec_cmd(&Command::shell("git status --porcelain"))
                            .await,
                    )
                    .context("Could not determine git status")?
                    .is_empty()
                    {
                        tracing::info!("No changes to commit, skipping commit");

                        return Ok(());
                    }

                    accept_non_zero_exit(
                        agent.context().exec_cmd(&Command::shell("git add .")).await,
                    )
                    .context("Could not add files to git")?;

                    let diff = accept_non_zero_exit(
                        agent
                            .context()
                            .exec_cmd(&Command::shell("git diff --color=never --staged"))
                            .await,
                    )?;

                    let commit_message = llm.prompt(format!("Please generate a conventional commit message for the following changes:\n\n{}", diff.output).into())
                        .await
                        .context("Could not prompt for commit message")?;

                    accept_non_zero_exit(
                        agent
                            .context()
                            .exec_cmd(&Command::shell(format!(
                                "git commit -m \"{commit_message}\""
                            )))
                            .await,
                    )
                    .context("Could not commit files to git")?;
                }

                if push_to_remote_enabled {
                    accept_non_zero_exit(
                        agent.context().exec_cmd(&Command::shell("git push")).await,
                    )
                    .context("Could not push changes to git")?;
                }
                Ok(())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::process::Command;

    use crate::test_utils::{test_agent_for_repository, test_repository};

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_auto_commit() {
        let (repository, _guard) = test_repository();
        let commit_and_push =
            CommitAndPush::try_new(&repository, &AgentEnvironment::default()).unwrap();

        std::fs::write(repository.path().join("test.txt"), "test").unwrap();

        let mut agent = test_agent_for_repository(&repository);
        commit_and_push.hook()(&mut agent).await.unwrap();

        // verify commit, check if the the commit message is correct and no uncommitted changes
        let commit = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .current_dir(repository.path())
            .output()
            .await
            .unwrap();

        assert_eq!(std::str::from_utf8(&commit.stdout).unwrap(), "Kwek\n\n");

        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repository.path())
            .output()
            .await
            .unwrap();

        assert!(status.stdout.is_empty());
    }

    #[test_log::test(tokio::test)]
    async fn test_skips_commit_if_no_changes() {
        let (repository, _guard) = test_repository();
        let commit_and_push =
            CommitAndPush::try_new(&repository, &AgentEnvironment::default()).unwrap();

        let mut agent = test_agent_for_repository(&repository);
        commit_and_push.hook()(&mut agent).await.unwrap();

        // verify commit, check if the the commit message is correct and no uncommitted changes
        let commit = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .current_dir(repository.path())
            .output()
            .await
            .unwrap();
        let commit = std::str::from_utf8(&commit.stdout).unwrap();

        dbg!(&commit);
        assert!(commit.contains("Initial commit"));

        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repository.path())
            .output()
            .await
            .unwrap();

        assert!(status.stdout.is_empty());
    }
}
