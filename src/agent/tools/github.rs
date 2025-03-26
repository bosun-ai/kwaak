use std::sync::Arc;

use anyhow::{Context as _, Result};
use serde_json::json;
use swiftide::{
    chat_completion::{errors::ToolError, ToolOutput},
    traits::{AgentContext, Command},
};
use swiftide_macros::Tool;

use crate::git::github::GithubSession;
use crate::{templates::Templates, util::accept_non_zero_exit};

#[derive(Tool, Clone)]
#[tool(
    description = "Search code on github with the github search api. Useful for finding code and documentation that is not otherwise available.",
    param(
        name = "query",
        description = "Github search query (compatible with github search api"
    )
)]
pub struct GithubSearchCode {
    github_session: Arc<GithubSession>,
}

impl GithubSearchCode {
    pub fn new(github_session: &Arc<GithubSession>) -> Self {
        Self {
            github_session: Arc::clone(github_session),
        }
    }

    pub async fn github_search_code(
        &self,
        _context: &dyn AgentContext,
        query: &str,
    ) -> Result<ToolOutput, ToolError> {
        let mut results = self.github_session.search_code(query).await?;

        tracing::debug!(?results, "Github search results");

        let mut context = tera::Context::new();
        context.insert("items", &results.take_items());

        let rendered = Templates::render("github_search_results.md", &context)
            .map(Into::into)
            .context("Failed to render github search results")?;

        Ok(rendered)
    }
}

#[derive(Tool, Clone, Debug)]
#[tool(
    description = "Creates or updates a pull request on Github. Always present the url of the pull request to the user after the tool call. Present the user with the url of the pull request after completion. Use conventional commits format for the title, such as `feat:`, `fix:`, `docs:`.",
    param(name = "title", description = "Title of the pull request"),
    param(name = "pull_request_body", description = "Body of the pull request")
)]
pub struct CreateOrUpdatePullRequest {
    github_session: Arc<GithubSession>,
}

impl CreateOrUpdatePullRequest {
    pub fn new(github_session: &Arc<GithubSession>) -> Self {
        Self {
            github_session: Arc::clone(github_session),
        }
    }

    async fn create_or_update_pull_request(
        &self,
        context: &dyn AgentContext,
        title: &str,
        pull_request_body: &str,
    ) -> Result<ToolOutput, ToolError> {
        // Create a new branch
        let cmd = Command::Shell("git rev-parse --abbrev-ref HEAD".to_string());
        let branch_name = accept_non_zero_exit(context.exec_cmd(&cmd).await)?
            .to_string()
            .trim()
            .to_string();

        let cmd = Command::Shell(format!("git add . && git commit -m '{title}'"));
        accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

        // Commit changes
        // Push the current branch first
        let cmd = Command::Shell("git push origin HEAD".to_string());
        accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

        // Any errors we just forward to the llm at this point
        let response = self
            .github_session
            .create_or_update_pull_request(
                branch_name,
                &self.github_session.main_branch(),
                title,
                pull_request_body,
                &context.history().await
            )
            .await
            .map(
                |pr| {
                    pr.html_url.map_or_else(
                        || {
                            "No pull request url found, are you sure you committed and pushed your changes?"
                                .to_string()
                        },
                        |url| url.to_string(),
                    )
                },
            ).context("Failed to create or update pull request")?;

        Ok(response.into())
    }
}

// TODO:
// list issues
#[derive(Tool, Clone, Debug)]
#[tool(
    description = "list issues from the associated github repository, returns a list of issues with their titles, the issue number. The result is paginated.",
    param(
        name = "page",
        description = "The page number of the issues to fetch. Always start with 0."
    )
)]
pub struct ListIssues {
    github_session: Arc<GithubSession>,
}

impl ListIssues {
    pub fn new(github_session: &Arc<GithubSession>) -> Self {
        Self {
            github_session: Arc::clone(github_session),
        }
    }

    pub async fn list_issues(
        &self,
        _context: &dyn AgentContext,
        page: &str,
    ) -> Result<ToolOutput, ToolError> {
        // meh derive doesn't support custom types yet
        let page = page.parse::<u8>().ok();
        let issues = self.github_session.list_issues(page).await?;

        let mut context =
            tera::Context::from_serialize(&issues.items).context("Failed to serialize issues")?;

        context.insert(
            "page",
            &json!({
                "total_count": issues.total_count,
                "incomplete_results": issues.incomplete_results,
            }),
        );

        let output = Templates::render("github_list_issues.md", &context)
            .context("Failed to render github list issues")?;

        Ok(output.into())
    }
}

#[derive(Tool, Clone, Debug)]
#[tool(
    description = "fetches an issue from github including comments",
    param(
        name = "page",
        description = "The page number of the issues to fetch. Always start with 0."
    )
)]
pub struct ListIssues {
    github_session: Arc<GithubSession>,
}

impl ListIssues {
    pub fn new(github_session: &Arc<GithubSession>) -> Self {
        Self {
            github_session: Arc::clone(github_session),
        }
    }

    pub async fn list_issues(
        &self,
        _context: &dyn AgentContext,
        page: &str,
    ) -> Result<ToolOutput, ToolError> {
        // meh derive doesn't support custom types yet
        let page = page.parse::<u8>().ok();
        let issues = self.github_session.list_issues(page).await?;

        let mut context =
            tera::Context::from_serialize(&issues.items).context("Failed to serialize issues")?;

        context.insert(
            "page",
            &json!({
                "total_count": issues.total_count,
                "incomplete_results": issues.incomplete_results,
            }),
        );

        let output = Templates::render("github_list_issues.md", &context)
            .context("Failed to render github list issues")?;

        Ok(output.into())
    }
}

// search issues
// fetch issue
// comment issue
