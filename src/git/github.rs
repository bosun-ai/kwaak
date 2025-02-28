//! This module provides a github session wrapping octocrab
//!
//! It is responsible for providing tooling and interaction with github
use std::sync::Mutex;

use anyhow::{Context, Result};
use octocrab::{models::pulls::PullRequest, Octocrab, Page};
use reqwest::header::{HeaderMap, ACCEPT};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use swiftide::chat_completion::ChatMessage;
use url::Url;

use crate::{config::ApiKey, repository::Repository, templates::Templates};

#[derive(Debug)]
pub struct GithubSession {
    token: ApiKey,
    octocrab: Octocrab,
    repository: Repository,
    active_pull_request: Mutex<Option<PullRequest>>,
}
impl GithubSession {
    pub fn from_repository(repository: &Repository) -> Result<Self> {
        let token = repository
            .config()
            .github_api_key
            .clone()
            .ok_or(anyhow::anyhow!("No github token found in config"))?;

        if !&repository.config().is_github_enabled() {
            return Err(anyhow::anyhow!(
                "Github is not enabled; make it is properly configured."
            ));
        }

        let octocrab = Octocrab::builder()
            .personal_token(token.expose_secret())
            .build()?;

        Ok(Self {
            token,
            octocrab,
            repository: repository.to_owned(),
            active_pull_request: Mutex::new(None),
        })
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

    pub fn main_branch(&self) -> &str {
        &self.repository.config().git.main_branch
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
        if !self.repository.config().is_github_enabled() {
            return Err(anyhow::anyhow!("Github is not enabled"));
        }
        // Above checks make the unwrap infallible
        let owner = self.repository.config().git.owner.as_deref().unwrap();
        let repo = self.repository.config().git.repository.as_deref().unwrap();

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
                .pulls(owner, repo)
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
            .pulls(owner, repo)
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
}
/// A struct to hold a GitHub issue and its comments
#[derive(Debug, Clone)]
pub struct GithubIssueWithComments {
    /// The issue
    pub issue: octocrab::models::issues::Issue,
    /// The comments on the issue
    pub comments: Vec<octocrab::models::issues::Comment>,
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
        if !self.repository.config().is_github_enabled() {
            return Err(anyhow::anyhow!("Github is not enabled"));
        }
        
        // Above checks make the unwrap infallible
        let owner = self.repository.config().git.owner.as_deref().unwrap();
        let repo = self.repository.config().git.repository.as_deref().unwrap();

        let issue = self
            .octocrab
            .issues(owner, repo)
            .get(issue_number)
            .await
            .context("Failed to fetch issue")?;

        let comments = self
            .octocrab
            .issues(owner, repo)
            .list_comments(issue_number)
            .send()
            .await
            .context("Failed to fetch issue comments")?
            .items;

        Ok(GithubIssueWithComments { issue, comments })
    }

    /// Generates a summary of a GitHub issue
    ///
    /// # Arguments
    ///
    /// * `issue_with_comments` - The issue and its comments to summarize
    ///
    /// # Returns
    ///
    /// A summary of the issue
    pub fn summarize_issue(&self, issue_with_comments: &GithubIssueWithComments) -> String {
        let GithubIssueWithComments { issue, comments } = issue_with_comments;
        
        let mut summary = format!(
            "# Issue #{}: {}\n\n",
            issue.number, issue.title
        );
        
        // Add issue state (open/closed)
        summary.push_str(&format!("**State**: {}\n", issue.state));
        
        // Add issue author if available
        if let Some(user) = &issue.user {
            summary.push_str(&format!("**Author**: {}\n", user.login));
        }
        
        // Add issue creation date
        if let Some(created_at) = &issue.created_at {
            summary.push_str(&format!("**Created**: {}\n", created_at));
        }
        
        // Add labels if any
        if !issue.labels.is_empty() {
            summary.push_str("\n**Labels**: ");
            for (i, label) in issue.labels.iter().enumerate() {
                if i > 0 {
                    summary.push_str(", ");
                }
                summary.push_str(&label.name);
            }
            summary.push('\n');
        }
        
        // Add assignees if any
        if let Some(assignees) = &issue.assignees {
            if !assignees.is_empty() {
                summary.push_str("\n**Assignees**: ");
                for (i, assignee) in assignees.iter().enumerate() {
                    if i > 0 {
                        summary.push_str(", ");
                    }
                    summary.push_str(&assignee.login);
                }
                summary.push('\n');
            }
        }
        
        // Add issue body
        if let Some(body) = &issue.body {
            summary.push_str("\n## Issue Description\n\n");
            summary.push_str(body);
            summary.push_str("\n\n");
        }
        
        // Add comments if any
        if !comments.is_empty() {
            summary.push_str("## Comments\n\n");
            for (i, comment) in comments.iter().enumerate() {
                if let Some(user) = &comment.user {
                    summary.push_str(&format!("### Comment #{} by {}\n\n", i + 1, user.login));
                } else {
                    summary.push_str(&format!("### Comment #{}\n\n", i + 1));
                }
                
                if let Some(body) = &comment.body {
                    summary.push_str(body);
                    summary.push_str("\n\n");
                }
            }
        }
        
        // Add a section describing what needs to be done
        summary.push_str("## Task Analysis\n\n");
        summary.push_str("Based on the issue and comments, the following needs to be done:\n\n");
        summary.push_str("1. [Analysis will be completed by the agent]\n");
        
        summary
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
