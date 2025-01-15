use std::sync::Arc;

use async_trait::async_trait;
use swiftide::chat_completion;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use crate::{
    chat_message::ChatMessage,
    commands::{CommandResponse, Responder},
};

use super::ui_event::UIEvent;

/// Handles responses from commands application wide
///
/// Basically converts command responses into UI events
/// The responder is send with commands so that the backend has a way to communicate with the
/// frontend, without knowing about the frontend
///
/// Only one is expected to be running at a time
///
/// TODO: If only used in app, singleton is not needed
#[derive(Debug)]
pub struct AppCommandResponder {
    // ui_tx: mpsc::UnboundedSender<UIEvent>,
    tx: mpsc::UnboundedSender<CommandResponse>,
    handle: AbortOnDropHandle<()>,
}

#[derive(Debug, Clone)]
pub struct AppCommandResponderForChatId {
    inner: mpsc::UnboundedSender<CommandResponse>,
    uuid: uuid::Uuid,
}

impl AppCommandResponder {
    pub fn spawn_for(ui_tx: mpsc::UnboundedSender<UIEvent>) -> AppCommandResponder {
        tracing::info!("Initializing app command responder");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(response) = rx.recv().await {
                tracing::debug!("[RESPONDER] Received response: {:?}", response);
                let ui_event = match response {
                    CommandResponse::Chat(uuid, msg) => UIEvent::ChatMessage(uuid, msg.into()),
                    CommandResponse::Activity(uuid, state) => UIEvent::ActivityUpdate(uuid, state),
                    CommandResponse::RenameChat(uuid, name) => UIEvent::RenameChat(uuid, name),
                    CommandResponse::Completed(uuid) => UIEvent::CommandDone(uuid),
                    CommandResponse::BackendMessage(uuid, msg) => {
                        UIEvent::ChatMessage(uuid, ChatMessage::new_system(&msg))
                    }
                };

                if let Err(err) = ui_tx.send(ui_event) {
                    tracing::error!("Failed to send response to ui: {:#}", err);
                }
            }
            tracing::info!("App command responder shutting down");
        });

        // Create the app responder, spawn a task to handle responses, the once cell returns a
        // clone of the responder without the rx
        AppCommandResponder {
            tx,
            handle: AbortOnDropHandle::new(handle),
        }
    }

    #[must_use]
    pub fn for_chat_id(&self, uuid: Uuid) -> Arc<dyn Responder> {
        Arc::new(AppCommandResponderForChatId {
            inner: self.tx.clone(),
            uuid,
        }) as Arc<dyn Responder>
    }
}

#[async_trait]
impl Responder for AppCommandResponderForChatId {
    fn send(&self, response: CommandResponse) {
        tracing::debug!("[RESPONDER SENDER] Sending response: {:?}", response);
        let response = response.with_uuid(self.uuid);
        if let Err(err) = self.inner.send(response) {
            tracing::error!("Failed to send response for command: {:?}", err);
        }
    }

    fn system_message(&self, message: &str) {
        self.send(CommandResponse::Chat(
            self.uuid,
            chat_completion::ChatMessage::new_system(message),
        ));
    }

    fn update(&self, state: &str) {
        self.send(CommandResponse::Activity(self.uuid, state.into()));
    }

    fn rename(&self, name: &str) {
        self.send(CommandResponse::RenameChat(self.uuid, name.into()));
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        self.send(CommandResponse::Chat(self.uuid, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    const TEST_UUID: Uuid = Uuid::from_u128(0x1234_5678_90ab_cdef_1234_5678_90ab_cdef);

    #[tokio::test]
    async fn test_app_responder() {
        let (ui_tx, mut ui_rx) = mpsc::unbounded_channel();
        let app = AppCommandResponder::spawn_for(ui_tx);

        let responder = app.for_chat_id(TEST_UUID);

        responder.system_message("Test message");

        let Some(ui_event) = ui_rx.recv().await else {
            panic!("No UI event received");
        };

        match ui_event {
            UIEvent::ChatMessage(received_uuid, received_message) => {
                assert_eq!(received_uuid, TEST_UUID);
                assert_eq!(received_message.content(), "Test message");
                assert!(received_message.role().is_system());
            }
            _ => panic!("Unexpected UI event received"),
        }

        responder.send(CommandResponse::Completed(Uuid::new_v4()));

        // Verify the UI event is received
        if let Some(ui_event) = ui_rx.recv().await {
            match ui_event {
                UIEvent::CommandDone(received_uuid) => assert_eq!(received_uuid, TEST_UUID),
                _ => panic!("Unexpected UI event received"),
            }
        } else {
            panic!("No UI event received");
        }
    }
}
