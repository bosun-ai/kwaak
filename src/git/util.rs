use std::path::Path;

use anyhow::Result;
use swiftide_core::{Command, ToolExecutor};

use crate::util::accept_non_zero_exit;

/// Get the diff from a tool executor
pub async fn diff(executor: &dyn ToolExecutor, base_sha: &str, color: bool) -> Result<String> {
    let color = if color { "--color=always" } else { "" };
    let cmd = Command::shell(format!("git diff {color} {base_sha}",));

    let mut output = accept_non_zero_exit(executor.exec_cmd(&cmd).await)?.output;

    if output.is_empty() {
        output = "No changes".to_string();
    }

    Ok(output)
}

/// Gets the main branch sync
///
/// WARN: Not to be used outside init and tests
pub fn main_branch(workdir: impl AsRef<Path>) -> String {
    const DEFAULT_BRANCH: &str = "main";
    // Tries to get it from the ref if present, otherwise sets it, the uses rev-parse to get the branch
    // The ref can be missing if the repo was never cloned (i.e. author and pushed to github directly)
    const MAIN_BRANCH_CMD: &str =
    "(git symbolic-ref refs/remotes/origin/HEAD >/dev/null 2>&1 || git remote set-head origin --auto >/dev/null 2>&1) && git rev-parse --abbrev-ref origin/HEAD";

    let Ok(output) = std::process::Command::new("sh")
        .arg("-c")
        .arg(MAIN_BRANCH_CMD)
        .current_dir(workdir.as_ref())
        .output()
    else {
        return DEFAULT_BRANCH.to_string();
    };

    let parsed = std::str::from_utf8(&output.stdout)
        .unwrap_or(DEFAULT_BRANCH)
        .trim_start_matches("origin/")
        .trim();

    if parsed.is_empty() {
        DEFAULT_BRANCH.to_string()
    } else {
        parsed.to_string()
    }
}

/// Checks if the current branch is dirty or contains untracked files
///
/// If for some reason the command fails, it will return true
pub async fn is_dirty(workdir: impl AsRef<Path>) -> bool {
    tokio::process::Command::new("git")
        .arg("diff-index")
        .arg("--quiet")
        .arg("HEAD")
        .current_dir(workdir.as_ref())
        .output()
        .await
        .map(|output| !output.status.success())
        .unwrap_or(true)
}
