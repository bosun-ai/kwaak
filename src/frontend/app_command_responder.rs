use std::sync::Arc;

use async_trait::async_trait;
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
/// When dispatching commands, a responder is created that wraps each response with the chat id
#[derive(Debug)]
pub struct AppCommandResponder {
    // ui_tx: mpsc::UnboundedSender<UIEvent>,
    tx: mpsc::UnboundedSender<ResponseWithChatId>,
    _handle: AbortOnDropHandle<()>,
}

#[derive(Debug)]
struct ResponseWithChatId(Uuid, CommandResponse);

#[derive(Debug, Clone)]
pub struct AppCommandResponderForChatId {
    inner: mpsc::UnboundedSender<ResponseWithChatId>,
    uuid: uuid::Uuid,
}

impl AppCommandResponder {
    pub fn spawn_for(ui_tx: mpsc::UnboundedSender<UIEvent>) -> AppCommandResponder {
        tracing::info!("Initializing app command responder");
        let (tx, mut rx) = mpsc::unbounded_channel::<ResponseWithChatId>();
        let handle = tokio::spawn(async move {
            while let Some(response) = rx.recv().await {
                tracing::debug!("[RESPONDER] Received response: {:?}", response);
                let chat_id = response.0;
                let ui_event = match response.1 {
                    CommandResponse::Chat(msg) => UIEvent::ChatMessage(chat_id, msg.into()),
                    CommandResponse::Activity(state) => UIEvent::ActivityUpdate(chat_id, state),
                    CommandResponse::RenameChat(name) => UIEvent::RenameChat(chat_id, name),
                    CommandResponse::RenameBranch(name) => UIEvent::RenameBranch(chat_id, name),
                    CommandResponse::Completed => UIEvent::CommandDone(chat_id),
                    CommandResponse::BackendMessage(msg) => {
                        UIEvent::ChatMessage(chat_id, ChatMessage::new_system(&msg))
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
            _handle: AbortOnDropHandle::new(handle),
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
    async fn send(&self, response: CommandResponse) {
        tracing::debug!("[RESPONDER SENDER] Sending response: {:?}", response);
        let response = ResponseWithChatId(self.uuid, response);
        if let Err(err) = self.inner.send(response) {
            tracing::error!("Failed to send response for command: {:?}", err);
        }
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

        responder.system_message("Test message").await;

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

        responder.send(CommandResponse::Completed).await;

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
