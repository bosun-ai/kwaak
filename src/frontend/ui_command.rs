// TODO: Rename to slash commands for clarity?
//
use uuid::Uuid;

use crate::commands::Command;
use anyhow::Result;

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
}

impl UserInputCommand {
    pub fn to_command(&self, uuid: Uuid) -> Option<Command> {
        match self {
            UserInputCommand::Quit => Some(Command::Quit { uuid }),
            UserInputCommand::ShowConfig => Some(Command::ShowConfig { uuid }),
            UserInputCommand::IndexRepository => Some(Command::IndexRepository { uuid }),
            _ => None,
        }
    }

    pub fn parse_from_input(input: &str) -> Result<UserInputCommand> {
        assert!(input.starts_with('/'));

        let cmd_parts = input.split_whitespace().collect::<Vec<_>>();

        let raw_cmd = cmd_parts.first().unwrap();
        let _args = cmd_parts[1..].join(" ");

        let cmd = raw_cmd[1..].parse::<UserInputCommand>()?;
        Ok(cmd)
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
        ];

        for (input, expected_command) in test_cases {
            let parsed_command = UserInputCommand::parse_from_input(input).unwrap();
            assert_eq!(parsed_command, expected_command);
        }
    }
}
