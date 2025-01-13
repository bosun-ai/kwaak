use tokio::sync::mpsc;
use uuid::Uuid;

use crate::chat_message::ChatMessage;

#[derive(Debug, Clone)]
pub enum CommandResponse {
    Chat(ChatMessage),
    ActivityUpdate(Uuid, String),
    RenameChat(Uuid, String),
}

#[derive(Debug)]
pub struct CommandResponder {
    tx: mpsc::UnboundedSender<CommandResponse>,
    rx: Option<mpsc::UnboundedReceiver<CommandResponse>>,
    uuid: Uuid,
}

impl CommandResponder {
    #[allow(dead_code)]
    pub fn send_system_message(&self, message: impl Into<String>) {
        self.send_message(ChatMessage::new_system(message).build());
    }

    pub fn send_message(&self, msg: impl Into<ChatMessage>) {
        let _ = self
            .tx
            .send(CommandResponse::Chat(msg.into().with_uuid(self.uuid)));
    }

    pub fn send_update(&self, state: impl Into<String>) {
        let _ = self
            .tx
            .send(CommandResponse::ActivityUpdate(self.uuid, state.into()));
    }

    // TODO: this feels overly specific, but its a real thing
    pub fn send_rename(&self, name: impl Into<String>) {
        let _ = self
            .tx
            .send(CommandResponse::RenameChat(self.uuid, name.into()));
    }

    #[must_use]
    /// Start receiving command responses
    ///
    /// # Panics
    ///
    /// Panics if the recev is already taken
    pub async fn recv(&mut self) -> Option<CommandResponse> {
        let rx = self.rx.as_mut().expect("Expected a receiver");
        rx.recv().await
    }

    #[must_use]
    pub fn with_uuid(self, uuid: Uuid) -> Self {
        CommandResponder {
            tx: self.tx,
            rx: self.rx,
            uuid,
        }
    }
}

impl Default for CommandResponder {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        CommandResponder {
            tx,
            rx: Some(rx),
            uuid: Uuid::default(),
        }
    }
}

/// Cheap clone that uninitializes the receiver
impl Clone for CommandResponder {
    fn clone(&self) -> Self {
        CommandResponder {
            tx: self.tx.clone(),
            rx: None,
            uuid: self.uuid,
        }
    }
}

impl From<ChatMessage> for CommandResponse {
    fn from(msg: ChatMessage) -> Self {
        CommandResponse::Chat(msg)
    }
}
