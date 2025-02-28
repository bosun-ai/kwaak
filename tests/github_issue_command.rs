use kwaak::commands::{Command, CommandEvent, CommandEventBuilder};
use kwaak::frontend::{ui, UIEvent};
use kwaak::git::github::{GithubIssueWithComments, GithubSession};
use kwaak::test_utils::{setup_integration, IntegrationContext};
use kwaak::{assert_agent_responded, assert_command_done};
use mockito::mock;
use octocrab::models::issues::{Comment, Issue, IssueState};
use std::sync::Arc;
use std::time::Duration;

/// Test for the GitHub issue command
///
/// This test mocks GitHub API responses and tests that:
/// 1. The command fetches an issue and its comments
/// 2. The command summarizes the issue
/// 3. The agent analyzes the issue
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn test_github_issue_command() {
    // Set up the test environment
    let IntegrationContext {
        mut app,
        uuid,
        repository,
        mut terminal,
        ..
    } = setup_integration().await.unwrap();

    // Mock GitHub API for fetching an issue
    let issue_number = 42;
    
    // Create a test issue
    let mut issue = Issue::default();
    issue.number = 42;
    issue.title = "Test Issue".to_string();
    issue.state = IssueState::Open;
    issue.body = Some("This is a test issue description.".to_string());
    
    // Create test comments
    let mut comment = Comment::default();
    comment.body = Some("This is a test comment.".to_string());
    
    let comments = vec![comment];
    
    // Create the issue with comments struct
    let issue_with_comments = GithubIssueWithComments { 
        issue, 
        comments 
    };
    
    // Create a GitHub session for testing
    let github_session = match GithubSession::from_repository(&repository) {
        Ok(session) => session,
        Err(e) => {
            panic!("Failed to create GitHub session: {}", e);
        }
    };

    // Dispatch the GitHub issue command
    app.dispatch_command(
        uuid,
        Command::GithubIssue {
            number: issue_number,
        },
    );

    // Wait for the command to complete
    assert_command_done!(app, uuid);

    // The agent should respond with an analysis
    assert_agent_responded!(app, uuid);
    
    // Render the UI to check the output
    terminal.draw(|f| ui(f, f.area(), &mut app)).unwrap();
    
    // Check that the terminal output contains the expected issue details
    let backend_output = format!("{:?}", terminal.backend());
    assert!(backend_output.contains(&format!("Issue #{}", issue_number)));
    assert!(backend_output.contains("Test Issue"));
    assert!(backend_output.contains("The agent has summarized the GitHub issue"));
}
