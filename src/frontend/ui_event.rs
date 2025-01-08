use crossterm::event::KeyEvent;
use uuid::Uuid;

use crate::chat_message::{ChatMessage, ChatMessageBuilder};

use super::{app::AppMode, UserInputCommand};

// Event handling
#[derive(Debug, Clone, strum::Display)]
#[allow(dead_code)]
pub enum UIEvent {
    /// A key is pressed
    Input(KeyEvent),
    /// A frontend tick event to trigger updates, etc
    Tick,
    /// A chat message is received
    ChatMessage(ChatMessage),
    /// Start a new chat
    NewChat,
    /// Switch to the next chat
    NextChat,
    /// Rename a chat
    RenameChat(Uuid, String),
    /// Change the view mode of the frontend
    ChangeMode(AppMode),
    /// Command finished
    CommandDone(Uuid),
    /// Agent has an update (for showing intermediate progress)
    ActivityUpdate(Uuid, String),
    /// Quit from the frontend
    Quit,
    /// Deletes the current chat
    DeleteChat,
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
            UserInputCommand::DeleteChat => Ok(Self::DeleteChat),
            _ => anyhow::bail!("Cannot convert {value} to UIEvent"),
        }
    }
}
