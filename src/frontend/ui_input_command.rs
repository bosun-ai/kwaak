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
    PartialEq,
)]
#[strum(serialize_all = "snake_case")]
pub enum UserInputCommand {
    Quit,
    ShowConfig,
    IndexRepository,
    NextChat,
    NewChat,
    DeleteChat,
    Copy,
    #[strum(disabled)] // <-- Different PR
    Diff(DiffVariant),
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
    #[default]
    Show,
    Apply,
    Download,
}

impl UserInputCommand {
    /// Convenience method to turn a `UserInputCommand` into a `Command`
    ///
    /// Not all user input commands can be turned into a `Command`
    #[must_use]
    pub fn to_command(&self) -> Option<Command> {
        match self {
            UserInputCommand::Quit => Some(Command::Quit),
            UserInputCommand::ShowConfig => Some(Command::ShowConfig),
            UserInputCommand::IndexRepository => Some(Command::IndexRepository),
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

    #[ignore = "future pull request"]
    #[test]
    fn test_parse_diff_input() {
        let test_cases = vec![
            ("/diff", UserInputCommand::Diff(DiffVariant::Show)),
            ("/diff show", UserInputCommand::Diff(DiffVariant::Show)),
            ("/diff apply", UserInputCommand::Diff(DiffVariant::Apply)),
            (
                "/diff download",
                UserInputCommand::Diff(DiffVariant::Download),
            ),
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
