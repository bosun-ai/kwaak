//! This module provides a github session wrapping octocrab
//!
//! It is responsible for providing tooling and interaction with github
//!
//! A github session is cheap to clone, it is expected to have one per repository
use std::sync::Mutex;
use std::{fmt::Write as _, sync::Arc};

use anyhow::{Context, Result};
use jsonwebtoken::EncodingKey;
use octocrab::{
    Octocrab, Page,
    models::{pulls::PullRequest, repos::Content},
    params::apps::CreateInstallationAccessToken,
};
use reqwest::header::{ACCEPT, HeaderMap};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swiftide::chat_completion::ChatMessage;
use tokio::sync::OnceCell;
use url::Url;

use crate::{
    config::{ApiKey, Config, defaults::extract_owner_and_repo},
    repository::Repository,
    templates::Templates,
};

#[derive(Debug, Clone)]
pub struct GithubSession {
    token: Arc<ApiKey>,
    octocrab: Arc<Octocrab>,
    active_pull_request: Arc<Mutex<Option<PullRequest>>>,

    git_main_branch: Arc<String>,
    git_owner: Arc<String>,
    git_repository: Arc<String>,

    octocrab_repo: Arc<OnceCell<octocrab::models::Repository>>,
}
impl GithubSession {
    #[tracing::instrument(skip_all, err)]
    pub async fn new_for_installation(
        app_id: u64,
        private_key: &SecretString,
        repository_url: &Url,
    ) -> Result<Self> {
        let jwt = generate_jwt(&private_key)?;

        // First authenticate our app with GitHub using the JWT
        tracing::debug!("Authenticating GitHub App with JWT");
        let octocrab = Octocrab::builder()
            .app(app_id.into(), jwt)
            .build()
            .context("Failed to build octocrab")?;

        let (git_owner, git_repository) = extract_owner_and_repo(repository_url.as_str())
            .context("Failed to extract owner and repo")?;

        tracing::debug!(
            "Retrieving installation for repository {}/{}",
            git_owner,
            git_repository
        );
        let installation = octocrab
            .apps()
            .get_repository_installation(&git_owner, &git_repository)
            .await?;

        tracing::debug!(
            "Retrieving installation access token for {}",
            installation.id
        );
        let create_access_token = CreateInstallationAccessToken::default();
        let access_token_url = Url::parse(
            installation
                .access_tokens_url
                .as_ref()
                .context("infallible; installation access tokens url should always be present")?,
        )?;

        let token = octocrab
            .post(access_token_url.path(), Some(&create_access_token))
            .await?;

        // We now have an octocrab instance authenticated as the app for the specified
        // installation.
        tracing::debug!("Creating octocrab installation for {}", installation.id);
        let octocrab: Octocrab = octocrab
            .installation(installation.id)
            .context("Failed to create octocrab installation")?;

        tracing::debug!(
            "Successfully authenticated as GitHub App for repository {}/{}",
            git_owner,
            git_repository
        );
        tracing::debug!(
            "Retrieving repository information {}/{}",
            git_owner,
            git_repository
        );
        // Retrieve the default branch of the repository
        let octocrab_repo = octocrab.repos(&git_owner, &git_repository).get().await?;

        let git_main_branch = octocrab_repo
            .default_branch
            .as_deref()
            .unwrap_or("main")
            .to_string()
            .into();

        Ok(Self {
            token,
            octocrab: octocrab.into(),
            git_main_branch,
            active_pull_request: Arc::new(Mutex::new(None)),
            git_owner: git_owner.into(),
            git_repository: git_repository.into(),
            octocrab_repo: Arc::new(OnceCell::from(octocrab_repo)),
        })
    }

    #[tracing::instrument(skip_all, err)]
    pub fn from_repository(repository: &Repository) -> Result<Self> {
        if !repository.config().is_github_enabled() {
            return Err(anyhow::anyhow!(
                "Github is not enabled; make sure it is properly configured."
            ));
        }

        let token = repository
            .config()
            .github_api_key
            .clone()
            .ok_or(anyhow::anyhow!("No github token found in config"))?;

        let octocrab = Octocrab::builder()
            .personal_token(token.expose_secret())
            .build()?;

        Ok(Self {
            token: token.into(),
            octocrab: octocrab.into(),
            active_pull_request: Arc::new(Mutex::new(None)),

            git_main_branch: Arc::new(repository.config().git.main_branch.to_string()),
            git_owner: repository
                .config()
                .git
                .owner
                .as_deref()
                .context("Expected git owner; infallible")?
                .to_string()
                .into(),
            git_repository: repository
                .config()
                .git
                .repository
                .as_deref()
                .context("Expected repository; infallible")?
                .to_string()
                .into(),
            octocrab_repo: Arc::new(OnceCell::new()),
        })
    }

    /// Returns a cloneable URL for the repository with the token included
    pub async fn clone_url(&self) -> Result<SecretString> {
        let repo_url = self
            .octocrab_repo()
            .await?
            .clone_url
            .as_ref()
            .context("No clone URL found")?;

        self.add_token_to_url(repo_url)
            .context("Failed to add token to clone URL")
    }

    /// Retrieves the `kwaak.toml` configuration file from the repository
    ///
    /// TODO: Some values are inferred on parse. These are incorrect and need to be adjusted based
    /// on the repository.
    ///
    /// NOTE: Git will be configured to always checkout the main branch when retrieving the config
    /// this way.
    pub async fn get_config(&self) -> Result<Config> {
        let mut config: Config = self.get_file("kwaak.toml").await?.parse()?;

        config.git.clone_repository_on_start = true;

        Ok(config)
    }

    /// Retrieves a file from the repository
    pub async fn get_file(&self, path: &str) -> Result<String> {
        self.octocrab
            .repos(&*self.git_owner, &*self.git_repository)
            .get_content()
            .path(path)
            .r#ref(&*self.git_main_branch)
            .send()
            .await
            .context("Failed to get file from repository")?
            .take_items()
            .first()
            .and_then(Content::decoded_content)
            .with_context(|| format!("Could not find file {path} in repository"))
    }

    /// Adds the github token to the repository url
    ///
    /// Used to overwrite the origin remote so that the agent can interact with git
    #[tracing::instrument(skip_all)]
    pub fn add_token_to_url(&self, repo_url: impl AsRef<str>) -> Result<SecretString> {
        let mut repo_url = repo_url.as_ref().to_string();

        if repo_url.starts_with("git@") {
            let converted = repo_url.replace(':', "/").replace("git@", "https://");
            let _ = std::mem::replace(&mut repo_url, converted);
        }

        let mut parsed = url::Url::parse(repo_url.as_ref()).context("Failed to parse url")?;

        parsed
            .set_username("x-access-token")
            .and_then(|()| parsed.set_password(Some(self.token.expose_secret())))
            .expect("Infallible");

        Ok(SecretString::from(parsed.to_string()))
    }

    #[must_use]
    pub fn main_branch(&self) -> &str {
        &self.git_main_branch
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn search_code(&self, query: &str) -> Result<Page<CodeWithMatches>> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, "application/vnd.github.text-match+json".parse()?);

        self.octocrab
            .get_with_headers(
                "/search/code",
                Some(&json!({
                "q": query,
                })),
                Some(headers),
            )
            .await
            .context("Failed to search code")
    }

    #[tracing::instrument(skip_all)]
    pub async fn create_or_update_pull_request(
        &self,
        branch_name: impl AsRef<str>,
        base_branch_name: impl AsRef<str>,
        title: impl AsRef<str>,
        description: impl AsRef<str>,
        messages: &[ChatMessage],
    ) -> Result<PullRequest> {
        // Above checks make the unwrap infallible
        let owner = &self.git_owner;
        let repo = &self.git_repository;

        tracing::debug!(messages = ?messages,
            "Creating pull request for {}/{} from branch {} onto {}",
            owner,
            repo,
            branch_name.as_ref(),
            base_branch_name.as_ref()
        );

        // Messages in pull request are disabled for now. They quickly get too large.
        // "messages": messages.iter().map(format_message).collect::<Vec<_>>(),
        let context = tera::Context::from_serialize(serde_json::json!({
            "owner": owner,
            "repo": repo,
            "branch_name": branch_name.as_ref(),
            "base_branch_name": base_branch_name.as_ref(),
            "title": title.as_ref(),
            "description": description.as_ref(),
            "messages": []
        }))?;

        let body = Templates::render("pull_request.md", &context)?;

        let maybe_pull = { self.active_pull_request.lock().unwrap().clone() };

        if let Some(pull_request) = maybe_pull {
            let pull_request = self
                .octocrab
                .pulls(owner.as_str(), repo.as_str())
                .update(pull_request.number)
                .title(title.as_ref())
                .body(&body)
                .send()
                .await?;

            self.active_pull_request
                .lock()
                .unwrap()
                .replace(pull_request.clone());

            return Ok(pull_request);
        }

        let pull_request = self
            .octocrab
            .pulls(owner.as_str(), repo.as_str())
            .create(
                title.as_ref(),
                branch_name.as_ref(),
                base_branch_name.as_ref(),
            )
            .body(&body)
            .send()
            .await?;

        self.active_pull_request
            .lock()
            .unwrap()
            .replace(pull_request.clone());

        Ok(pull_request)
    }

    /// Lazy loads the repository information from GitHub
    async fn octocrab_repo(&self) -> Result<&octocrab::models::Repository> {
        self.octocrab_repo
            .get_or_try_init(|| async {
                tracing::debug!("Retrieving repository information from GitHub");
                self.octocrab
                    .repos(&*self.git_owner, &*self.git_repository)
                    .get()
                    .await
                    .map_err(anyhow::Error::from)
            })
            .await
    }
}

/// Generates a JWT key for the GitHub App
fn generate_jwt(app_private_key: &SecretString) -> Result<EncodingKey> {
    tracing::debug!("Generating JWT for GitHub App");
    jsonwebtoken::EncodingKey::from_rsa_pem(app_private_key.expose_secret().as_bytes())
        .context("Could not generate jwt token")
}

/// A struct to hold a GitHub issue and its comments
#[derive(Debug, Clone)]
pub struct GithubIssueWithComments {
    /// The issue
    pub issue: octocrab::models::issues::Issue,
    /// The comments on the issue
    pub comments: Vec<octocrab::models::issues::Comment>,
}

impl GithubIssueWithComments {
    /// Generates a summary of a GitHub issue in markdown format.
    #[must_use]
    pub fn markdown(&self) -> String {
        let GithubIssueWithComments { issue, comments } = self;

        let mut summary = format!("# Issue #{}: {}\n\n", issue.number, issue.title);

        let _ = writeln!(&mut summary, "**State**: {:?}", issue.state); // (open/closed)
        let _ = writeln!(&mut summary, "**Author**: {}", issue.user.login);
        let _ = writeln!(&mut summary, "**Created**: {}", issue.created_at);

        // add labels if any
        if !issue.labels.is_empty() {
            summary.push_str("\n**Labels**: ");
            summary.push_str(
                &issue
                    .labels
                    .iter()
                    .map(|label| label.name.as_str())
                    .collect::<Vec<&str>>()
                    .join(", "),
            );
            summary.push('\n');
        }

        // add issue body
        if let Some(body) = &issue.body {
            summary.push_str("\n## Issue Description\n\n");
            summary.push_str(body);
            summary.push_str("\n\n");
        }

        // add comments if any
        if !comments.is_empty() {
            summary.push_str("## Comments\n\n");
            for (i, comment) in comments.iter().enumerate() {
                let _ = writeln!(
                    &mut summary,
                    "### Comment #{} by {}\n",
                    i + 1,
                    comment.user.login
                );

                if let Some(body) = &comment.body {
                    summary.push_str(body);
                    summary.push('\n');
                }
            }
        }
        summary
    }
}

impl GithubSession {
    /// Fetches a GitHub issue and its comments
    ///
    /// # Arguments
    ///
    /// * `issue_number` - The number of the issue to fetch
    ///
    /// # Returns
    ///
    /// The issue and its comments
    #[tracing::instrument(skip(self), err)]
    pub async fn fetch_issue(&self, issue_number: u64) -> Result<GithubIssueWithComments> {
        // Above checks make the unwrap infallible
        let owner = &self.git_owner;
        let repo = &self.git_repository;

        let issue = self
            .octocrab
            .issues(owner.as_str(), repo.as_str())
            .get(issue_number)
            .await
            .context("Failed to fetch issue")?;

        let comments = self
            .octocrab
            .issues(owner.as_str(), repo.as_str())
            .list_comments(issue_number)
            .send()
            .await
            .context("Failed to fetch issue comments")?
            .items;

        Ok(GithubIssueWithComments { issue, comments })
    }
}

// Temporarily disabled, if messages get too large the PR can't be created.
//
// Need a better solution, i.e. github content api
#[allow(dead_code)]
const MAX_TOOL_CALL_LENGTH: usize = 250;
#[allow(dead_code)]
const MAX_TOOL_RESPONSE_LENGTH: usize = 2048;

#[allow(dead_code)]
fn format_message(message: &ChatMessage) -> serde_json::Value {
    let role = match message {
        ChatMessage::User(_) => "▶ User",
        ChatMessage::System(_) => "ℹ System",
        // Add a nice uncoloured glyph for the summary
        ChatMessage::Summary(_) => ">> Summary",
        ChatMessage::Assistant(..) => "✦ Assistant",
        ChatMessage::ToolOutput(..) => "⚙ Tool Output",
    };
    let content = match message {
        ChatMessage::User(msg) | ChatMessage::System(msg) | ChatMessage::Summary(msg) => {
            msg.to_string()
        }
        ChatMessage::Assistant(msg, tool_calls) => {
            let mut msg = msg.as_deref().unwrap_or_default().to_string();

            if let Some(tool_calls) = tool_calls {
                msg.push_str("\nTool calls: \n");
                for tool_call in tool_calls {
                    let mut tool_call = format!("{tool_call}\n");
                    tool_call.truncate(MAX_TOOL_CALL_LENGTH);
                    msg.push_str(&tool_call);
                }
            }

            msg
        }
        ChatMessage::ToolOutput(tool_call, tool_output) => {
            let mut msg = format!("{tool_call} => {tool_output}");
            msg.truncate(MAX_TOOL_RESPONSE_LENGTH);
            msg
        }
    };

    serde_json::json!({
        "role": role,
        "content": content,
    })
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret as _;

    use crate::test_utils;

    use super::*;

    #[test]
    fn test_template_render() {
        let chat_messages = vec![
            ChatMessage::new_user("user message"),
            ChatMessage::new_system("system message"),
            ChatMessage::new_assistant(Some("assistant message"), None),
            ChatMessage::new_summary("summary message"),
        ];

        let mut context = tera::Context::from_serialize(serde_json::json!({
            "owner": "owner",
            "repo": "repo",
            "branch_name": "branch_name",
            "base_branch_name": "base_branch_name",
            "title": "title",
            "description": "description",
            "messages": chat_messages.iter().map(format_message).collect::<Vec<_>>(),


        }))
        .unwrap();
        let rendered = Templates::render("pull_request.md", &context).unwrap();

        insta::assert_snapshot!(rendered);

        context.insert("messages", &serde_json::json!([]));

        let rendered_no_messages = Templates::render("pull_request.md", &context).unwrap();
        insta::assert_snapshot!(rendered_no_messages);

        // and without messages
    }

    #[tokio::test]
    async fn test_add_token_to_url() {
        let (mut repository, _) = test_utils::test_repository(); // Assuming you have a default implementation for Repository
        let config_mut = repository.config_mut();
        config_mut.github_api_key = Some("token".into());
        let github_session = GithubSession::from_repository(&repository).unwrap();

        let repo_url = "https://github.com/owner/repo";
        let tokenized_url = github_session.add_token_to_url(repo_url).unwrap();

        assert_eq!(
            tokenized_url.expose_secret(),
            format!(
                "https://x-access-token:{}@github.com/owner/repo",
                repository
                    .config()
                    .github_api_key
                    .as_ref()
                    .unwrap()
                    .expose_secret()
            )
        );
    }

    #[tokio::test]
    async fn test_add_token_to_git_url() {
        let (mut repository, _) = test_utils::test_repository(); // Assuming you have a default implementation for Repository
        let config_mut = repository.config_mut();
        config_mut.github_api_key = Some("token".into());
        let github_session = GithubSession::from_repository(&repository).unwrap();

        let repo_url = "git@github.com:user/repo.git";
        let tokenized_url = github_session.add_token_to_url(repo_url).unwrap();

        assert_eq!(
            tokenized_url.expose_secret(),
            format!(
                "https://x-access-token:{}@github.com/user/repo.git",
                repository
                    .config()
                    .github_api_key
                    .as_ref()
                    .unwrap()
                    .expose_secret()
            )
        );
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CodeWithMatches {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub url: Url,
    pub git_url: Url,
    pub html_url: Url,
    pub repository: octocrab::models::Repository,
    pub text_matches: Vec<TextMatches>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextMatches {
    object_url: Url,
    object_type: String,
    property: String,
    fragment: String,
    // matches: Vec<Match>,
}
