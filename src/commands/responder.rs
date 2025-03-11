use std::sync::Arc;

use async_trait::async_trait;
use dyn_clone::DynClone;
#[cfg(test)]
use mockall::mock;
use serde::{Deserialize, Serialize};
use swiftide::chat_completion;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
#[async_trait]
pub trait Responder: std::fmt::Debug + Send + Sync + DynClone {
    /// Generic handler for command responses
    async fn send(&self, response: CommandResponse);

    /// Messages from an agent
    async fn agent_message(&self, message: chat_completion::ChatMessage) {
        self.send(CommandResponse::Chat(message)).await;
    }

    /// System messages from the backend
    async fn system_message(&self, message: &str) {
        self.send(CommandResponse::BackendMessage(message.to_string()))
            .await;
    }

    /// State updates with a message from the backend
    async fn update(&self, state: &str) {
        self.send(CommandResponse::Activity(state.to_string()))
            .await;
    }

    /// A chat has been renamed
    async fn rename_chat(&self, name: &str) {
        self.send(CommandResponse::RenameChat(name.to_string()))
            .await;
    }

    /// A git branch has been renamed
    async fn rename_branch(&self, branch_name: &str) {
        self.send(CommandResponse::RenameBranch(branch_name.to_string()))
            .await;
    }
}

dyn_clone::clone_trait_object!(Responder);

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub Responder {}

    #[async_trait]
    impl Responder for Responder {
        async fn send(&self, response: CommandResponse);
        async fn agent_message(&self, message: chat_completion::ChatMessage);
        async fn system_message(&self, message: &str);
        async fn update(&self, state: &str);
        async fn rename_chat(&self, name: &str);
        async fn rename_branch(&self, name: &str);
    }

    impl Clone for Responder {
        fn clone(&self) -> Self;

    }
}

#[async_trait]
impl Responder for tokio::sync::mpsc::UnboundedSender<CommandResponse> {
    async fn send(&self, response: CommandResponse) {
        let _ = self.send(response);
    }
}

#[async_trait]
impl Responder for Arc<dyn Responder> {
    async fn send(&self, response: CommandResponse) {
        self.as_ref().send(response).await;
    }
}

// Debug responder that logs all messages to stderr
#[derive(Debug, Clone)]
pub struct DebugResponder;

#[async_trait]
impl Responder for DebugResponder {
    async fn send(&self, response: CommandResponse) {
        eprintln!("DEBUG: Response: {response:?}");
    }

    async fn agent_message(&self, message: chat_completion::ChatMessage) {
        eprintln!("DEBUG: Agent message: {message:?}");
    }

    async fn system_message(&self, message: &str) {
        eprintln!("DEBUG: System message: {message}");
    }

    async fn update(&self, state: &str) {
        eprintln!("DEBUG: State update: {state}");
    }

    async fn rename_chat(&self, name: &str) {
        eprintln!("DEBUG: Chat renamed to: {name}");
    }

    async fn rename_branch(&self, name: &str) {
        eprintln!("DEBUG: Branch renamed to: {name}");
    }
}

// noop responder
#[async_trait]
impl Responder for () {
    async fn send(&self, _response: CommandResponse) {}
}
