//! This module provides a github session wrapping octocrab
//!
//! It is responsible for providing tooling and interaction with github
use anyhow::{Context, Result};
use octocrab::{models::pulls::PullRequest, Octocrab};
use secrecy::SecretString;

use crate::{config::ApiKey, repository::Repository};

#[derive(Debug)]
pub struct GithubSession {
    token: ApiKey,
    octocrab: Octocrab,
    repository: Repository,
}
impl GithubSession {
    pub fn from_repository(repository: &Repository) -> Result<Self> {
        let token = repository
            .config()
            .github
            .token
            .clone()
            .ok_or(anyhow::anyhow!("No github token found in config"))?;

        let octocrab = Octocrab::builder()
            .personal_token(token.expose_secret())
            .build()?;

        Ok(Self {
            token,
            octocrab,
            repository: repository.to_owned(),
        })
    }

    /// Adds the github token to the repository url
    ///
    /// Used to overwrite the origin remote so that the agent can interact with git
    #[tracing::instrument(skip_all)]
    pub fn add_token_to_url(&self, repo_url: impl AsRef<str>) -> Result<SecretString> {
        if !repo_url.as_ref().starts_with("https://") {
            anyhow::bail!("Only https urls are supported")
        }

        let mut parsed = url::Url::parse(repo_url.as_ref()).context("Failed to parse url")?;

        parsed
            .set_username("x-access-token")
            .and_then(|()| parsed.set_password(Some(self.token.expose_secret())))
            .expect("Infallible");

        Ok(SecretString::from(parsed.to_string()))
    }

    pub fn main_branch(&self) -> &str {
        &self.repository.config().github.main_branch
    }

    #[tracing::instrument(skip_all)]
    pub async fn create_pull_request(
        &self,
        branch_name: impl AsRef<str>,
        base_branch_name: impl AsRef<str>,
        title: impl AsRef<str>,
        description: impl AsRef<str>,
    ) -> Result<PullRequest> {
        let owner = &self.repository.config().github.owner;
        let repo = &self.repository.config().github.repository;

        tracing::debug!(
            "Creating pull request for {}/{} from branch {} onto {}",
            owner,
            repo,
            branch_name.as_ref(),
            base_branch_name.as_ref()
        );

        self.octocrab
            .pulls(owner, repo)
            .create(
                title.as_ref(),
                branch_name.as_ref(),
                base_branch_name.as_ref(),
            )
            .body(description.as_ref())
            .send()
            .await
            .map_err(anyhow::Error::from)
    }
}
