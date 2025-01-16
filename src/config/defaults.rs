use std::{path::PathBuf, process::Command};

use regex::Regex;

/// The default project name based on the current directory
///
/// # Panics
///
/// Panics if the current directory is not available
#[must_use]
pub fn default_project_name() -> String {
    // Infer from the current directory
    std::env::current_dir()
        .expect("Failed to get current directory")
        .file_name()
        .expect("Failed to get current directory name")
        .to_string_lossy()
        .to_string()
}

pub(super) fn default_cache_dir() -> PathBuf {
    let mut path = dirs::cache_dir().expect("Failed to get cache directory");
    path.push("kwaak");
    path
}

pub(super) fn default_log_dir() -> PathBuf {
    let mut path = dirs::cache_dir().expect("Failed to get cache directory");
    path.push("kwaak");
    path.push("logs");

    path
}

#[must_use]
pub fn default_dockerfile() -> PathBuf {
    "./Dockerfile".into()
}

#[must_use]
pub fn default_docker_context() -> PathBuf {
    ".".into()
}

/// Determines the default branch
///
/// # Panics
///
/// Panics if no git repository, no remote or no main/master branch
#[must_use]
pub fn default_main_branch() -> String {
    const DEFAULT_BRANCH: &str = "main";
    // Tries to get it from the ref if present, otherwise sets it, the uses rev-parse to get the branch
    // The ref can be missing if the repo was never cloned (i.e. author and pushed to github directly)
    const MAIN_BRANCH_CMD: &str =
    "(git symbolic-ref refs/remotes/origin/HEAD >/dev/null 2>&1 || git remote set-head origin --auto >/dev/null 2>&1) && git rev-parse --abbrev-ref origin/HEAD";

    let Ok(output) = Command::new("sh").arg("-c").arg(MAIN_BRANCH_CMD).output() else {
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

/// Extracts the owner and repo from the git remote url
///
/// # Panics
///
/// Panics if the git remote url is not available
#[must_use]
pub fn default_owner_and_repo() -> (String, String) {
    let url = std::string::String::from_utf8(
        Command::new("git")
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .output()
            .expect("Failed to get git remote url")
            .stdout,
    )
    .unwrap();

    extract_owner_and_repo(&url)
}

fn extract_owner_and_repo(url: &str) -> (String, String) {
    let re = Regex::new(r"^(?:https://|git@|ssh://|git://|http://)?(?:[^@/]+@)?(?:[^/:]+[/:])?([^/]+)/([^/.]+)(?:\.git)?$").unwrap();

    re.captures(&url.trim())
        .and_then(|caps| {
            let owner = caps.get(1)?.as_str().to_string();
            let repo = caps.get(2)?.as_str().to_string();
            Some((owner, repo))
        })
        .expect("Failed to extract owner and repo from git remote url")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extract_owner_and_repo() {
        let (owner, repo) = default_owner_and_repo();

        assert_eq!(owner, "bosun-ai");
        assert_eq!(repo, "kwaak");
    }

    #[test]
    fn test_extract_owner_and_repo_from_url() {
        let urls = vec![
            "https://github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo",
            "git://github.com/owner/repo.git",
            "http://github.com/owner/repo",
            "https://user:password@github.com/owner/repo.git",
        ];

        for url in urls {
            let (owner, repo) = extract_owner_and_repo(url);
            assert_eq!(owner, "owner");
            assert_eq!(repo, "repo");
        }
    }

    #[test]
    fn test_default_main_branch() {
        let branch = default_main_branch();
        assert_eq!(branch, "master");
    }
}
