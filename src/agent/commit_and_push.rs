use anyhow::Context as _;
use swiftide::agents::hooks::AfterEachFn;
use swiftide::traits::Command;

use crate::{repository::Repository, util::accept_non_zero_exit};

use super::env_setup::AgentEnvironment;

#[derive(Debug)]
pub struct CommitAndPush {
    auto_commit_enabled: bool,
    push_to_remote_enabled: bool,
}

impl CommitAndPush {
    pub fn new(repository: &Repository, agent_env: &AgentEnvironment) -> Self {
        let auto_commit_enabled = !repository.config().git.auto_commit_disabled;
        let push_to_remote_enabled =
            agent_env.remote_enabled && repository.config().git.auto_push_remote;

        Self {
            auto_commit_enabled,
            push_to_remote_enabled,
        }
    }

    pub fn hook(self) -> impl AfterEachFn {
        move |agent| {
            let auto_commit_enabled = self.auto_commit_enabled;
            let push_to_remote_enabled = self.push_to_remote_enabled;

            Box::pin(async move {
                if auto_commit_enabled {
                    accept_non_zero_exit(
                        agent.context().exec_cmd(&Command::shell("git add .")).await,
                    )
                    .context("Could not add files to git")?;

                    accept_non_zero_exit(
                        agent
                            .context()
                            .exec_cmd(&Command::shell(
                                "git commit -m \"[kwaak]: Committed changes after completion\"",
                            ))
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
        let commit_and_push = CommitAndPush::new(&repository, &AgentEnvironment::default());

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

        assert_eq!(
            std::str::from_utf8(&commit.stdout).unwrap(),
            "[kwaak]: Committed changes after completion\n\n"
        );

        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repository.path())
            .output()
            .await
            .unwrap();

        assert!(status.stdout.is_empty());
    }
}
