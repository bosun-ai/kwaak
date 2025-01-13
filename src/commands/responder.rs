use std::sync::Arc;

#[cfg(test)]
use mockall::automock;
use swiftide::chat_completion;
use uuid::Uuid;

/// Uuid here refers to the identifier of the command
///
/// TODO: Remove the UUID here, the responder is expected to know the uuid
/// of the command, and it confuses with the uuid to identify chats (same value only for convience,
/// not the same 'thing')
#[derive(Debug, Clone)]
pub enum CommandResponse {
    Chat(Uuid, chat_completion::ChatMessage),
    ActivityUpdate(Uuid, String),
    RenameChat(Uuid, String),
    Completed(Uuid),
}

impl CommandResponse {
    #[must_use]
    pub fn with_uuid(self, uuid: Uuid) -> Self {
        match self {
            CommandResponse::Chat(uuid, msg) => CommandResponse::Chat(uuid, msg),
            CommandResponse::ActivityUpdate(_, state) => {
                CommandResponse::ActivityUpdate(uuid, state)
            }
            CommandResponse::RenameChat(_, name) => CommandResponse::RenameChat(uuid, name),
            CommandResponse::Completed(_) => CommandResponse::Completed(uuid),
        }
    }
}

/// A responder reacts to updates from commands
///
/// Backend defines the interface, frontend can define ways to handle the responses
///
/// TODO: Consider, perhaps with the new structure, less concrete methods are needed
/// and the frontend just uses a oneoff handler for each command
#[cfg_attr(test, automock)]
pub trait Responder: std::fmt::Debug + Send + Sync {
    /// Generic handler for command responses
    fn handle(&self, response: CommandResponse);

    /// Messages from an agent
    fn agent_message(&self, message: chat_completion::ChatMessage);

    /// System messages from the backend
    fn system_message(&self, message: &str);

    /// State updates with a message from the backend
    fn update(&self, state: &str);

    /// Response to a rename request
    fn rename(&self, name: &str);
}

impl Responder for tokio::sync::mpsc::UnboundedSender<CommandResponse> {
    fn handle(&self, response: CommandResponse) {
        let _ = self.send(response);
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        let _ = self.send(CommandResponse::Chat(Uuid::default(), message));
    }

    fn system_message(&self, message: &str) {
        let _ = self.send(CommandResponse::ActivityUpdate(
            Uuid::default(),
            message.to_string(),
        ));
    }

    fn update(&self, state: &str) {
        let _ = self.send(CommandResponse::ActivityUpdate(
            Uuid::default(),
            state.to_string(),
        ));
    }

    fn rename(&self, name: &str) {
        let _ = self.send(CommandResponse::RenameChat(
            Uuid::default(),
            name.to_string(),
        ));
    }
}

impl Responder for Arc<dyn Responder> {
    fn handle(&self, response: CommandResponse) {
        self.as_ref().handle(response);
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        self.as_ref().agent_message(message);
    }

    fn system_message(&self, message: &str) {
        self.as_ref().system_message(message);
    }

    fn update(&self, state: &str) {
        self.as_ref().update(state);
    }

    fn rename(&self, name: &str) {
        self.as_ref().rename(name);
    }
}
