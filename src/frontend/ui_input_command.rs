/// When a user types an input command (/ prefix) it is parsed into a `UserInputCommand`
/// and then bubbled up to an `UIEvent::UserInputCommand`
///
/// The event handler in the app will then try to convert the `UserInputCommand` into a `Command`
/// or an `UIEvent` depending on which is implemented
///
/// Alternative solution could be to have a thing that executes user input commands directly,
/// removing the need for the `UIEvent` roundtrip
use crate::commands::Command;
use anyhow::{Context as _, Result};

use super::ui_event::UIEvent;

#[derive(
    Debug,
    Clone,
    strum_macros::Display,
    strum_macros::EnumIs,
    strum_macros::AsRefStr,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::EnumMessage,
    PartialEq,
)]
#[strum(serialize_all = "snake_case")]
pub enum UserInputCommand {
    /// Stop the application
    Quit,
    /// Show the current configuration
    ShowConfig,
    /// Force a re-index of the repository
    IndexRepository,
    /// Switch to the next chat
    NextChat,
    /// Start a new chat
    NewChat,
    /// Delete the current chat
    DeleteChat,
    /// Copy the last message from an agent
    Copy,
    /// Show or pull changes made by an agent
    /// Defaults to `show` if no argument is given
    ///
    /// Usage:
    ///     /diff show - Shows the diff in the chat
    ///     /diff pull - Pulls the diff into a new branch
    Diff(DiffVariant),
    /// Retries the last chat with the agent.
    Retry,
    /// Print help
    Help,
    /// Fetch and summarize a GitHub issue
    ///
    /// Usage:
    ///     /gh_issue 123 - Fetches the issue #123 and displays it
    GithubIssue(u64),
}

#[derive(
    Default,
    Debug,
    Clone,
    strum_macros::Display,
    strum_macros::EnumIs,
    strum_macros::AsRefStr,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    PartialEq,
)]
#[strum(serialize_all = "snake_case")]
pub enum DiffVariant {
    /// Print the current changes
    #[default]
    Show,
    /// Pulls the current changes into the same branch as the agent is working in
    Pull,
}

impl UserInputCommand {
    /// Convenience method to turn a `UserInputCommand` into a `Command`
    ///
    /// Not all user input commands can be turned into a `Command`
    #[must_use]
    pub fn to_command(&self) -> Option<Command> {
        match self {
            UserInputCommand::ShowConfig => Some(Command::ShowConfig),
            UserInputCommand::IndexRepository => Some(Command::IndexRepository),
            UserInputCommand::Retry => Some(Command::RetryChat),
            UserInputCommand::GithubIssue(number) => Some(Command::GithubIssue { number: *number }),
            _ => None,
        }
    }

    /// Convenience method to turn a `UserInputCommand` into a `UIEvent`
    ///
    /// Not all user input commands can be turned into a `UIEvent`
    #[must_use]
    pub fn to_ui_event(&self) -> Option<UIEvent> {
        match self {
            UserInputCommand::NextChat => Some(UIEvent::NextChat),
            UserInputCommand::NewChat => Some(UIEvent::NewChat),
            UserInputCommand::Copy => Some(UIEvent::CopyLastMessage),
            UserInputCommand::DeleteChat => Some(UIEvent::DeleteChat),
            UserInputCommand::Help => Some(UIEvent::Help),
            UserInputCommand::Quit => Some(UIEvent::Quit),
            UserInputCommand::Diff(diff_variant) => match diff_variant {
                DiffVariant::Show => Some(UIEvent::DiffShow),
                DiffVariant::Pull => Some(UIEvent::DiffPull),
            },
            _ => None,
        }
    }

    /// Parses a `UserInputCommand` from a user input string
    ///
    /// # Panics
    ///
    /// Panics if the input string does not start with a '/' or the string is empty
    pub fn parse_from_input(input: &str) -> Result<UserInputCommand> {
        debug_assert!(input.starts_with('/'));

        let cmd_parts = input.split_whitespace().collect::<Vec<_>>();

        let input = cmd_parts.first().unwrap();
        let subcommand = cmd_parts.get(1);

        let input_cmd = input[1..]
            .parse::<UserInputCommand>()
            .with_context(|| format!("failed to parse input command {input}"))?;

        match input_cmd {
            UserInputCommand::Diff(_) => {
                let Some(subcommand) = subcommand else {
                    return Ok(UserInputCommand::Diff(DiffVariant::default()));
                };
                let diff_variant = subcommand
                    .parse()
                    .with_context(|| format!("failed to parse diff subcommand {subcommand}"))?;
                Ok(UserInputCommand::Diff(diff_variant))
            }
            UserInputCommand::GithubIssue(_) => {
                let Some(issue_number) = subcommand else {
                    return Err(anyhow::anyhow!("GitHub issue number is required"));
                };
                
                let issue_number = issue_number
                    .parse::<u64>()
                    .with_context(|| format!("failed to parse GitHub issue number {issue_number}"))?;
                    
                Ok(UserInputCommand::GithubIssue(issue_number))
            }
            _ => Ok(input_cmd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_from_input() {
        let test_cases = vec![
            ("/quit", UserInputCommand::Quit),
            ("/show_config", UserInputCommand::ShowConfig),
            ("/index_repository", UserInputCommand::IndexRepository),
            ("/next_chat", UserInputCommand::NextChat),
            ("/new_chat", UserInputCommand::NewChat),
            ("/delete_chat", UserInputCommand::DeleteChat),
            ("/copy", UserInputCommand::Copy), // New test case for Copy command
        ];

        for (input, expected_command) in test_cases {
            let parsed_command = UserInputCommand::parse_from_input(input).unwrap();
            assert_eq!(parsed_command, expected_command);
        }
    }

    #[test]
    fn test_parse_diff_input() {
        let test_cases = vec![
            ("/diff", UserInputCommand::Diff(DiffVariant::Show)),
            ("/diff show", UserInputCommand::Diff(DiffVariant::Show)),
            ("/diff pull", UserInputCommand::Diff(DiffVariant::Pull)),
        ];

        for (input, expected_command) in test_cases {
            let parsed_command = UserInputCommand::parse_from_input(input).unwrap();
            assert_eq!(
                parsed_command, expected_command,
                "expected: {expected_command:?} for: {input:?}",
            );
        }
    }
}
