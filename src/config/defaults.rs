use std::{path::PathBuf, process::Command};

use regex::Regex;

use crate::git;

/// The default project name based on the current directory
///
/// # Panics
///
/// Panics if the current directory is not available
#[must_use]
pub fn default_project_name() -> String {
    // Infer from the current directory
    default_owner_and_repo().map_or_else(
        || {
            let current_dir = std::env::current_dir().expect("Failed to get current directory");
            current_dir
                .file_name()
                .expect("Failed to get current directory name")
                .to_string_lossy()
                .to_string()
        },
        |(_, repo)| repo,
    )
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
/// Returns `main` if the default branch cannot be determined
#[must_use]
pub fn default_main_branch() -> String {
    git::util::main_branch(".")
}

#[must_use]
pub fn default_auto_push_remote() -> bool {
    true
}

/// Extracts the owner and repo from the git remote url
///
/// # Panics
///
/// Panics if the git remote url is not available
#[must_use]
pub fn default_owner_and_repo() -> Option<(String, String)> {
    let url = std::string::String::from_utf8(
        Command::new("git")
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .output()
            .ok()?
            .stdout,
    )
    .ok()?;

    extract_owner_and_repo(&url)
}

fn extract_owner_and_repo(url: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^(?:https://|git@|ssh://|git://|http://)?(?:[^@/]+@)?(?:[^/:]+[/:])?([^/]+)/([^/.]+)(?:\.git)?$").unwrap();

    re.captures(&url.trim()).and_then(|caps| {
        let owner = caps.get(1)?.as_str().to_string();
        let repo = caps.get(2)?.as_str().to_string();
        Some((owner, repo))
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extract_owner_and_repo() {
        let (owner, repo) = default_owner_and_repo().unwrap();

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
            let (owner, repo) = extract_owner_and_repo(url).unwrap();
            assert_eq!(owner, "owner");
            assert_eq!(repo, "repo");
        }
    }

    #[test]
    fn test_default_main_branch() {
        let branch = default_main_branch();
        assert_eq!(branch, "master");
    }

    #[test]
    fn test_default_project_name() {
        // At least we got half of this beauty
        let name = default_project_name();
        assert_eq!(name, "kwaak");
    }
}
