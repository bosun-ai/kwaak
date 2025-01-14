use crossterm::event::KeyEvent;
use uuid::Uuid;

use crate::chat_message::ChatMessage;

use super::{app::AppMode, ui_input_command::UserInputCommand};

// Event handling
#[derive(Debug, Clone, strum::Display)]
#[allow(dead_code)]
pub enum UIEvent {
    /// A key is pressed
    Input(KeyEvent),
    /// A frontend tick event to trigger updates, etc
    Tick,
    /// A chat message is received
    ChatMessage(Uuid, ChatMessage),
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
    /// Copy last message from current chat to clipboard
    CopyLastMessage,
    /// Deletes the current chat
    DeleteChat,
    /// Received a user command (prefixed with '/') from the user
    UserInputCommand(Uuid, UserInputCommand),
}

impl From<KeyEvent> for UIEvent {
    fn from(key: KeyEvent) -> Self {
        Self::Input(key)
    }
}
