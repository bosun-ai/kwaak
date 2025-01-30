#![allow(dead_code)]
use std::sync::Arc;
use swiftide::traits::CommandError;

use anyhow::{Context as _, Result};
use swiftide::{
    chat_completion::{errors::ToolError, ToolOutput},
    query::{search_strategies, states},
    traits::{AgentContext, Command},
};
use swiftide_macros::{tool, Tool};
use tavily::Tavily;
use tokio::sync::Mutex;

use crate::{
    config::ApiKey,
    git::github::GithubSession,
    templates::Templates,
    util::{self, accept_non_zero_exit},
};

static MAIN_BRANCH_CMD: &str = "git remote show origin | sed -n '/HEAD branch/s/.*: //p'";

/// WARN: Experimental
#[tool(
    description = "Run any shell command in the current project, use this if other tools are not enough.",
    param(
        name = "cmd",
        description = "The shell command, including any arguments if needed, to run"
    )
)]
pub async fn shell_command(context: &dyn AgentContext, cmd: &str) -> Result<ToolOutput, ToolError> {
    if util::is_git_branch_change(cmd) {
        return Ok(
            "You cannot change branches, you are already on a branch created specifically for you."
                .into(),
        );
    }
    let output = accept_non_zero_exit(context.exec_cmd(&Command::Shell(cmd.into())).await)?;
    Ok(output.into())
}

#[tool(
    description = "Reads file content",
    param(name = "file_name", description = "Full path of the file")
)]
pub async fn read_file(
    context: &dyn AgentContext,
    file_name: &str,
) -> Result<ToolOutput, ToolError> {
    let cmd = Command::ReadFile(file_name.into());

    // i.e. if the file doesn't exist, just forward that message
    let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

    Ok(output.into())
}

// TODO: Better to have a single read_file tool with an optional line number flag
#[tool(
    description = "Reads file content, including line numbers. You MUST use this tool to retrieve line numbers before making an edit with edit_file",
    param(name = "file_name", description = "Full path of the file")
)]
pub async fn read_file_with_line_numbers(
    context: &dyn AgentContext,
    file_name: &str,
) -> Result<ToolOutput, ToolError> {
    let cmd = Command::ReadFile(file_name.into());

    // i.e. if the file doesn't exist, just forward that message
    let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

    let lines = output
        .output
        .lines()
        .enumerate()
        .map(|(i, l)| format!("{line_num}|{l}", line_num = i + 1));

    Ok(lines.collect::<Vec<_>>().join("\n").into())
}

#[tool(
    description = "Write to a file. You MUST ALWAYS include the full file content, including what you did not change, as it overwrites the full file. Only make changes that pertain to your task.",
    param(name = "file_name", description = "Full path of the file"),
    param(name = "content", description = "FULL Content to write to the file")
)]
pub async fn write_file(
    context: &dyn AgentContext,
    file_name: &str,
    content: &str,
) -> Result<ToolOutput, ToolError> {
    println!("Writing file {file_name} with {content_len} characters", content_len = content.len());
    let cmd = Command::WriteFile(file_name.into(), content.into());

    context.exec_cmd(&cmd).await?;

    let success_message = format!("File written successfully to {file_name}");

    Ok(success_message.into())
}

#[tool(
    description = "Searches for a file inside the current project, leave the argument empty to list all files. Uses `find`.",
    param(name = "file_name", description = "Partial or full name of the file")
)]
pub async fn search_file(
    context: &dyn AgentContext,
    file_name: &str,
) -> Result<ToolOutput, ToolError> {
    let cmd = Command::Shell(format!("fd -E '.git/*' -iH --full-path '{file_name}'"));
    let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

    Ok(output.into())
}

#[tool(
    description = "Invoke a git command on the current repository",
    param(name = "command", description = "Git sub-command to run")
)]
pub async fn git(context: &dyn AgentContext, command: &str) -> Result<ToolOutput, ToolError> {
    let cmd = format!("git {command}");
    if util::is_git_branch_change(&cmd) {
        return Ok(
            "You cannot change branches, you are already on a branch created specifically for you."
                .into(),
        );
    }
    let cmd = Command::Shell(cmd);
    let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

    Ok(output.into())
}

#[derive(Tool, Clone)]
#[tool(
    description = "Reset changes you have made to a file. If you have made changes to a file and need to reset them, use this tool.",
    param(name = "file_name", description = "Full path of the file")
)]
pub struct ResetFile {
    start_ref: String,
}

impl ResetFile {
    pub fn new(start_ref: impl AsRef<str>) -> Self {
        Self {
            start_ref: start_ref.as_ref().to_string(),
        }
    }
    pub async fn reset_file(
        &self,
        context: &dyn AgentContext,
        file_name: &str,
    ) -> Result<ToolOutput, ToolError> {
        let cmd = Command::Shell(format!(
            "git checkout {start_ref} -- {file_name}",
            start_ref = self.start_ref
        ));

        let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

        Ok(output.into())
    }
}

#[tool(
    description = "Search code in the project with ripgrep. Only searches within the current project. For searching code outside the project, use other tools instead.",
    param(
        name = "query",
        description = "Code you would like to find in the repository. Best used for exact search in the code. Uses `ripgrep`."
    )
)]
pub async fn search_code(context: &dyn AgentContext, query: &str) -> Result<ToolOutput, ToolError> {
    let cmd = Command::Shell(format!("rg -g '!.git' -i. -F '{query}'"));
    let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;
    Ok(output.into())
}

#[derive(Tool, Clone)]
#[tool(
    description = "Search code and documentation in human language in the project. Only searches within the current project. If you need help on code outside the project, use other tools.",
    param(
        name = "query",
        description = "A description, question, or literal code you want to know more about. Uses a semantic similarly search."
    )
)]
pub struct ExplainCode<'a> {
    query_pipeline: Arc<
        Mutex<
            swiftide::query::Pipeline<
                'a,
                search_strategies::SimilaritySingleEmbedding,
                states::Answered,
            >,
        >,
    >,
}

impl<'a> ExplainCode<'a> {
    #[must_use]
    pub fn new(
        query_pipeline: swiftide::query::Pipeline<
            'a,
            search_strategies::SimilaritySingleEmbedding,
            states::Answered,
        >,
    ) -> Self {
        Self {
            query_pipeline: Arc::new(Mutex::new(query_pipeline)),
        }
    }
    async fn explain_code(
        &self,
        _context: &dyn AgentContext,
        query: &str,
    ) -> Result<ToolOutput, ToolError> {
        let results = self
            .query_pipeline
            .lock()
            .await
            .query_mut(query)
            .await?
            .answer()
            .to_string();
        Ok(results.into())
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

#[derive(Tool, Clone, Debug)]
#[tool(
    description = "Runs tests in the current project. Run this in favour of coverage, as it is typically faster."
)]
pub struct RunTests {
    pub test_command: String,
}

impl RunTests {
    pub fn new(test_command: impl AsRef<str>) -> Self {
        Self {
            test_command: test_command.as_ref().to_string(),
        }
    }

    async fn run_tests(&self, context: &dyn AgentContext) -> Result<ToolOutput, ToolError> {
        let cmd = Command::Shell(self.test_command.clone());
        let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

        Ok(output.into())
    }
}

#[derive(Tool, Clone, Debug)]
#[tool(
    description = "Get coverage of tests, this also runs the tests. Only run this in favour of just the tests if you need coverage, as it is typically slower than running tests."
)]
pub struct RunCoverage {
    pub coverage_command: String,
}

impl RunCoverage {
    pub fn new(coverage_command: impl AsRef<str>) -> Self {
        Self {
            coverage_command: coverage_command.as_ref().to_string(),
        }
    }

    async fn run_coverage(&self, context: &dyn AgentContext) -> Result<ToolOutput, ToolError> {
        let cmd = Command::Shell(self.coverage_command.clone());
        let output = accept_non_zero_exit(context.exec_cmd(&cmd).await)?;

        Ok(output.into())
    }
}

#[derive(Tool, Clone)]
#[tool(
    description = "Search the web to answer a question. If you encounter an issue that cannot be resolved, use this tool to help getting an answer.",
    param(name = "query", description = "Search query")
)]
pub struct SearchWeb {
    tavily_client: Arc<Tavily>,
    api_key: ApiKey,
}

impl SearchWeb {
    #[must_use]
    pub fn new(tavily_client: Tavily, api_key: ApiKey) -> Self {
        Self {
            tavily_client: Arc::new(tavily_client),
            api_key,
        }
    }
    async fn search_web(
        &self,
        _context: &dyn AgentContext,
        query: &str,
    ) -> Result<ToolOutput, ToolError> {
        let request = tavily::SearchRequest::new(self.api_key.expose_secret(), query)
            .search_depth("advanced")
            .include_answer(true)
            .include_images(false)
            .include_raw_content(false)
            .max_results(5);

        let results = self
            .tavily_client
            .call(&request)
            .await
            .map_err(anyhow::Error::from)?;

        tracing::debug!(results = ?results, "Search results from tavily");

        let mut context = tera::Context::new();

        context.insert("answer", &results.answer);
        context.insert(
            "results",
            &results
                .results
                .iter()
                .filter(|r| r.score >= 0.5)
                .map(|r| {
                    serde_json::json!({
                        "title": r.title,
                        "content": r.content,
                        "url": r.url,
                    })
                })
                .collect::<Vec<_>>(),
        );
        context.insert("follow_up_questions", &results.follow_up_questions);

        let rendered = Templates::render("tavily_search_results.md", &context)
            .context("Failed to render search web results")?;

        Ok(rendered.into())
    }
}

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
        let mut results = self
            .github_session
            .search_code(query)
            .await
            .map_err(anyhow::Error::from)?;

        tracing::debug!(?results, "Github search results");

        let mut context = tera::Context::new();
        context.insert("items", &results.take_items());

        let rendered = Templates::render("github_search_results.md", &context)
            .map(Into::into)
            .context("Failed to render github search results")?;

        Ok(rendered)
    }
}

#[tool(
    description = "Fetch a url and present it as markdown. Useful for fetching content from the web like documentation, code, snippetes, etc. Will also include links and can be used to deeply explore a subject that otherwise cannot be explored.",
    param(name = "url", description = "The url to fetch")
)]
pub async fn fetch_url(_context: &dyn AgentContext, url: &str) -> Result<ToolOutput, ToolError> {
    let url_content = match reqwest::get(url).await {
        Ok(response) if response.status().is_success() => response.text().await.unwrap(),

        // Assuming 9/10 parsing/network errors for now always return it to the llm
        Err(e) => return Ok(format!("Failed to fetch url: {e:#}").into()),
        Ok(response) => return Ok(format!("Failed to fetch url: {}", response.status()).into()),
    };

    htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "img", "video", "audio", "embed"])
        .build()
        .convert(&url_content)
        .or_else(|e| {
            tracing::warn!("Error converting markdown {e:#}");
            Ok(url_content)
        })
        .map(Into::into)
}

const REPLACE_LINES_DESCRIPTION: &str = "Replace lines in a file.

You MUST read the file with line numbers first BEFORE EVERY EDIT, to know the start and end line numbers of the block you want to replace.
After editing, you MUST read the file again to get the new line numbers.

Prefix the content with some of context before the lines you want to edit and add some lines of context after the lines you want to edit.

Do not include the line numbers in the content.

If you want to add lines, use `add_lines` instead.

Example:

Given a file `test.txt`:
```
1|A
2|B
3|C
4|D
5|E
6|F
7|G
8|H
```

To replace line 4 and 5 with:
```
XD
XX
XE
```

Call the tool with:

start_line=2,end_line=7,content=B\nC\nXD\nXX\nXE\nF\nG

Then the result will be:
```
1|A
2|B
3|C
4|XD
5|X^
6|XE
7|F
8|G
9|H
```
";
#[tool(
    description = REPLACE_LINES_DESCRIPTION,
    param(name = "file_name", description = "Full path of the file"),
    param(
        name = "start_line",
        description = "Start line number of the lines to replace"
    ),
    param(
        name = "end_line",
        description = "Last line number of the block to replace."
    ),
    param(name = "content", description = "Replacement content including context before and after")
)]
pub async fn replace_lines(
    context: &dyn AgentContext,
    file_name: &str,
    start_line: &str,
    end_line: &str,
    content: &str,
) -> Result<ToolOutput, ToolError> {
    // Read the file content
    let cmd = Command::ReadFile(file_name.into());

    let file_content = match context.exec_cmd(&cmd).await {
        Ok(output) => output.output,
        Err(CommandError::NonZeroExit(output, ..)) => {
            return Ok(output.into());
        }
        Err(e) => return Err(e.into()),
    };

    let mut lines = file_content.lines().collect::<Vec<_>>();

    let Ok(start_line) = start_line.parse::<usize>() else {
        return Ok("Invalid start line number, must be a valid number greater than 0".into());
    };

    let Ok(end_line) = end_line.parse::<usize>() else {
        return Ok("Invalid end line number, must be a valid number 0 or greater".into());
    };

    let lines_len = lines.len();

    if start_line > lines_len || end_line > lines_len {
        return Ok("Start or end line number is out of bounds".into());
    }

    if end_line > 0 && start_line > end_line {
        return Ok("Start line number must be less than or equal to end line number".into());
    }

    if start_line == 0 {
        return Ok("Start line number must be greater than 0".into());
    }

    println!("Going to replace lines {start_line}-{end_line} with content: {content}");

    // LLM often messes up the first line, so assume it included at least one line of context, and drop it
    let content = content
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");

    let start_line = start_line + 1;

    // Input is 1 indexed, lines are 0 indexed
    if end_line == 0 {
        lines.insert(start_line, &content);
    } else {
        lines.splice(
            start_line.saturating_sub(1)..=end_line.saturating_sub(1),
            content.lines(),
        );
    }

    let write_cmd = Command::WriteFile(file_name.into(), lines.join("\n"));
    context.exec_cmd(&write_cmd).await?;

    Ok(format!("Successfully replaced content in {file_name}. Before making new edits, you MUST read the file again, as the line numbers WILL have changed.").into())
}

#[tool(
    description = "Add new lines after a specific line number. You MUST read the file with line numbers first BEFORE EVERY EDIT, to know after what line number to add. After adding lines, you MUST read the file again to get the new line numbers.",
    param(name = "file_name", description = "Full path of the file"),
    param(
        name = "start_line",
        description = "The line number to insert the content after"
    ),
    param(name = "content", description = "New content")
)]
pub async fn add_lines(
    context: &dyn AgentContext,
    file_name: &str,
    start_line: &str,
    content: &str,
) -> Result<ToolOutput, ToolError> {
    // Read the file content
    let cmd = Command::ReadFile(file_name.into());

    let file_content = match context.exec_cmd(&cmd).await {
        Ok(output) => output.output,
        Err(CommandError::NonZeroExit(output, ..)) => {
            return Ok(output.into());
        }
        Err(e) => return Err(e.into()),
    };

    let mut lines = file_content.lines().collect::<Vec<_>>();

    let Ok(start_line) = start_line.parse::<usize>() else {
        return Ok("Invalid start line number, must be a valid number greater than 0".into());
    };

    let lines_len = lines.len();

    if start_line > lines_len {
        return Ok("Start or end line number is out of bounds".into());
    }

    if start_line == 0 {
        return Ok("Start line number must be greater than 0".into());
    }

    // Input is 1 indexed, lines are 0 indexed
    lines.insert(start_line, content);

    let write_cmd = Command::WriteFile(file_name.into(), lines.join("\n"));
    context.exec_cmd(&write_cmd).await?;

    Ok(format!("Successfully added content to {file_name} at line {start_line}. Before making new edits, you MUST read the file again, as the line numbers WILL have changed.").into())
}
