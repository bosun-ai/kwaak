use kwaak::commands::Command;
use kwaak::frontend::{ui, UIEvent};
use kwaak::test_utils::{setup_integration, IntegrationContext};
use kwaak::{assert_agent_responded, assert_command_done};
use std::sync::Arc;
use std::time::Duration;

/// Test for the GitHub issue command
///
/// This test verifies that:
/// 1. The command is properly registered
/// 2. The command handler returns appropriate error messages when GitHub is not configured
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn test_github_issue_command_not_configured() {
    // Set up the test environment
    let IntegrationContext {
        mut app,
        uuid,
        mut terminal,
        handler_guard,
        repository_guard: _repository_guard,
        ..
    } = setup_integration().await.unwrap();

    // Dispatch the GitHub issue command with a test issue number
    app.dispatch_command(
        uuid,
        Command::GithubIssue {
            number: 42,
        },
    );

    // Wait for the command to complete
    assert_command_done!(app, uuid);

    // Render the UI to check the output
    terminal.draw(|f| ui(f, f.area(), &mut app)).unwrap();
    
    // Since GitHub is not properly configured in the test environment,
    // we expect an error message
    let backend_output = format!("{:?}", terminal.backend());
    assert!(backend_output.contains("Failed to create GitHub session") ||
            backend_output.contains("Github is not enabled") ||
            backend_output.contains("Failed to fetch GitHub issue"));

    drop(handler_guard);
}
