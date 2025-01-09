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

pub(super) fn default_indexing_concurrency() -> usize {
    // Assume majority is IO bound, so beef it up
    num_cpus::get() * 4
}
#[must_use]
pub fn default_dockerfile() -> PathBuf {
    "./Dockerfile".into()
}

#[must_use]
pub fn default_docker_context() -> PathBuf {
    ".".into()
}

static MAIN_BRANCH_CMD: &str = "git remote show origin | sed -n '/HEAD branch/s/.*: //p'";
/// Determines the default branch
///
/// # Panics
///
/// Panics if no git repository, no remote or no main/master branch
#[must_use]
pub fn default_main_branch() -> String {
    // "main".to_string()
    std::string::String::from_utf8(
        Command::new("sh")
            .arg("-c")
            .arg(MAIN_BRANCH_CMD)
            .output()
            .expect("Failed to get main branch")
            .stdout
            .split(|c| c == &b'\n' || c == &b'\r')
            .next()
            .expect("Failed to get main branch")
            .to_owned(),
    )
    .expect("Failed to get main branch")
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
