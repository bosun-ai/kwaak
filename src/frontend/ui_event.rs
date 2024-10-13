use crossterm::event::KeyEvent;

use crate::{
    chat_message::{ChatMessage, ChatMessageBuilder},
    commands::Command,
};

use super::{app::AppMode, UserInputCommand};

// Event handling
#[derive(Debug, Clone, strum::Display)]
#[allow(dead_code)]
pub enum UIEvent {
    Input(KeyEvent),
    Tick,
    Command(Command),
    ChatMessage(ChatMessage),
    NewChat,
    NextChat,
    ChangeMode(AppMode),
}

impl From<ChatMessage> for UIEvent {
    fn from(msg: ChatMessage) -> Self {
        Self::ChatMessage(msg)
    }
}

impl From<ChatMessageBuilder> for UIEvent {
    fn from(mut builder: ChatMessageBuilder) -> Self {
        Self::ChatMessage(builder.build())
    }
}

impl From<&mut ChatMessageBuilder> for UIEvent {
    fn from(builder: &mut ChatMessageBuilder) -> Self {
        Self::ChatMessage(builder.build().clone())
    }
}
impl From<Command> for UIEvent {
    fn from(cmd: Command) -> Self {
        Self::Command(cmd)
    }
}

impl From<KeyEvent> for UIEvent {
    fn from(key: KeyEvent) -> Self {
        Self::Input(key)
    }
}

impl TryFrom<UserInputCommand> for UIEvent {
    type Error = anyhow::Error;

    fn try_from(value: UserInputCommand) -> Result<Self, Self::Error> {
        match value {
            UserInputCommand::NextChat => Ok(Self::NextChat),
            UserInputCommand::NewChat => Ok(Self::NewChat),
            _ => anyhow::bail!("Cannot convert {value} to UIEvent"),
        }
    }
}
