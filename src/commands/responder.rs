use std::sync::Arc;

use dyn_clone::DynClone;
#[cfg(test)]
use mockall::mock;
use swiftide::chat_completion;

#[derive(Debug, Clone)]
pub enum CommandResponse {
    /// Messages coming from an agent
    Chat(chat_completion::ChatMessage),
    /// Short activity updates
    Activity(String),
    /// A chat has been renamed
    RenameChat(String),
    /// A chat branch has been renamed
    RenameBranch(String),
    /// Backend system messages (kwaak currently just renders these as system chat like messages)
    BackendMessage(String),
    /// A command has been completed
    Completed,
}

/// A responder reacts to updates from commands
///
/// Backend defines the interface, frontend can define ways to handle the responses
///
/// Backend expects the responder to know where it should go (i.e. the chat id)
///
/// TODO: Consider, perhaps with the new structure, less concrete methods are needed
/// and the frontend just uses a oneoff handler for each command
pub trait Responder: std::fmt::Debug + Send + Sync + DynClone {
    /// Generic handler for command responses
    fn send(&self, response: CommandResponse);

    /// Messages from an agent
    fn agent_message(&self, message: chat_completion::ChatMessage) {
        self.send(CommandResponse::Chat(message));
    }

    /// System messages from the backend
    fn system_message(&self, message: &str) {
        self.send(CommandResponse::BackendMessage(message.to_string()));
    }

    /// State updates with a message from the backend
    fn update(&self, state: &str) {
        self.send(CommandResponse::Activity(state.to_string()));
    }

    /// A chat has been renamed
    fn rename_chat(&self, name: &str) {
        self.send(CommandResponse::RenameChat(name.to_string()));
    }

    /// A git branch has been renamed
    fn rename_branch(&self, branch_name: &str) {
        self.send(CommandResponse::RenameBranch(branch_name.to_string()));
    }
}

dyn_clone::clone_trait_object!(Responder);

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub Responder {}

    impl Responder for Responder {
        fn send(&self, response: CommandResponse);
        fn agent_message(&self, message: chat_completion::ChatMessage);
        fn system_message(&self, message: &str);
        fn update(&self, state: &str);
        fn rename_chat(&self, name: &str);
        fn rename_branch(&self, name: &str);
    }

    impl Clone for Responder {
        fn clone(&self) -> Self;

    }
}

impl Responder for tokio::sync::mpsc::UnboundedSender<CommandResponse> {
    fn send(&self, response: CommandResponse) {
        let _ = self.send(response);
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
#[derive(Debug, Clone)]
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
