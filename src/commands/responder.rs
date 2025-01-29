use std::sync::Arc;

#[cfg(test)]
use mockall::automock;
use swiftide::chat_completion;
use uuid::Uuid;

/// Uuid here refers to the identifier of the command
///
/// TODO: Remove the UUID here, the responder is expected to know the uuid
/// of the command, and it confuses with the uuid to identify chats (same value only for convenience,
/// not the same 'thing') OR have only 3 generic types.
#[derive(Debug, Clone)]
pub enum CommandResponse {
    /// Messages coming from an agent
    Chat(Uuid, chat_completion::ChatMessage),
    /// Short activity updates
    Activity(Uuid, String),
    /// A chat has been renamed
    RenameChat(Uuid, String),
    /// A chat branch has been renamed
    RenameBranch(Uuid, String),
    /// Backend system messages (kwaak currently just renders these as system chat like messages)
    BackendMessage(Uuid, String),
    /// A command has been completed
    Completed(Uuid),
}

impl CommandResponse {
    #[must_use]
    pub fn with_uuid(self, uuid: Uuid) -> Self {
        match self {
            CommandResponse::Chat(uuid, msg) => CommandResponse::Chat(uuid, msg),
            CommandResponse::Activity(_, state) => CommandResponse::Activity(uuid, state),
            CommandResponse::RenameChat(_, name) => CommandResponse::RenameChat(uuid, name),
            CommandResponse::RenameBranch(_, name) => CommandResponse::RenameBranch(uuid, name),
            CommandResponse::BackendMessage(_, msg) => CommandResponse::BackendMessage(uuid, msg),
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
    fn send(&self, response: CommandResponse);

    /// Messages from an agent
    fn agent_message(&self, message: chat_completion::ChatMessage);

    /// System messages from the backend
    fn system_message(&self, message: &str);

    /// State updates with a message from the backend
    fn update(&self, state: &str);

    /// Response to a rename request
    fn rename_chat(&self, name: &str);

    /// Response to a branch rename request
    fn rename_branch(&self, name: &str);
}

// TODO: Naming should be identical to command response
impl Responder for tokio::sync::mpsc::UnboundedSender<CommandResponse> {
    fn send(&self, response: CommandResponse) {
        let _ = self.send(response);
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        let _ = self.send(CommandResponse::Chat(Uuid::default(), message));
    }

    // TODO: These should not be swiftide messages, they should be backend messages
    fn system_message(&self, message: &str) {
        let _ = self.send(CommandResponse::BackendMessage(
            Uuid::default(),
            message.to_string(),
        ));
    }

    fn update(&self, state: &str) {
        let _ = self.send(CommandResponse::Activity(
            Uuid::default(),
            state.to_string(),
        ));
    }

    fn rename_chat(&self, name: &str) {
        let _ = self.send(CommandResponse::RenameChat(
            Uuid::default(),
            name.to_string(),
        ));
    }

    fn rename_branch(&self, branch_name: &str) {
        let _ = self.send(CommandResponse::RenameBranch(
            Uuid::default(),
            branch_name.to_string(),
        ));
    }
}

impl Responder for Arc<dyn Responder> {
    fn send(&self, response: CommandResponse) {
        self.as_ref().send(response);
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

    fn rename_chat(&self, name: &str) {
        self.as_ref().rename_chat(name);
    }

    fn rename_branch(&self, name: &str) {
        self.as_ref().rename_branch(name);
    }
}

// Debug responder that logs all messages to stderr
#[derive(Debug)]
pub struct DebugResponder;

impl Responder for DebugResponder {
    fn send(&self, response: CommandResponse) {
        eprintln!("DEBUG: Response: {response:?}");
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        eprintln!("DEBUG: Agent message: {message:?}");
    }

    fn system_message(&self, message: &str) {
        eprintln!("DEBUG: System message: {message}");
    }

    fn update(&self, state: &str) {
        eprintln!("DEBUG: State update: {state}");
    }

    fn rename_chat(&self, name: &str) {
        eprintln!("DEBUG: Chat renamed to: {name}");
    }

    fn rename_branch(&self, name: &str) {
        eprintln!("DEBUG: Branch renamed to: {name}");
    }
}

// noop responder
impl Responder for () {
    fn send(&self, _response: CommandResponse) {}

    fn agent_message(&self, _message: chat_completion::ChatMessage) {}

    fn system_message(&self, _message: &str) {}

    fn update(&self, _state: &str) {}

    fn rename_chat(&self, _name: &str) {}

    fn rename_branch(&self, _name: &str) {}
}
