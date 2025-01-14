use std::sync::{Arc, OnceLock};

use anyhow::Result;
use swiftide::chat_completion;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::commands::{CommandResponse, Responder};

use super::ui_event::UIEvent;

static INSTANCE: OnceLock<Arc<AppCommandResponder>> = OnceLock::new();

/// Handles responses from commands application wide
///
/// Basically converts command responses into UI events
/// The responder is send with commands so that the backend has a way to communicate with the
/// frontend, without knowing about the frontend
///
/// Only one is expected to be running at a time
#[derive(Debug, Clone)]
pub struct AppCommandResponder {
    // ui_tx: mpsc::UnboundedSender<UIEvent>,
    tx: mpsc::UnboundedSender<CommandResponse>,
    _handle: Arc<tokio::task::JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct AppCommandResponderForChatId {
    inner: Arc<AppCommandResponder>,
    uuid: uuid::Uuid,
}

impl AppCommandResponder {
    pub fn init(ui_tx: mpsc::UnboundedSender<UIEvent>) -> Result<()> {
        if INSTANCE.get().is_some() {
            anyhow::bail!("App command responder already initialized");
        }

        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(response) = rx.recv().await {
                let ui_event = match response {
                    CommandResponse::Chat(uuid, msg) => UIEvent::ChatMessage(uuid, msg.into()),
                    CommandResponse::ActivityUpdate(uuid, state) => {
                        UIEvent::ActivityUpdate(uuid, state)
                    }
                    CommandResponse::RenameChat(uuid, name) => UIEvent::RenameChat(uuid, name),
                    CommandResponse::Completed(uuid) => UIEvent::CommandDone(uuid),
                };

                if let Err(err) = ui_tx.send(ui_event) {
                    tracing::error!("Failed to send response to ui: {:#}", err);
                }
            }
        });

        // Create the app responder, spawn a task to handle responses, the once cell returns a
        // clone of the responder without the rx
        INSTANCE
            .set(Arc::new(AppCommandResponder {
                tx,
                _handle: handle.into(),
            }))
            .map_err(|_| anyhow::anyhow!("Failed to set global frontend command responder"))?;

        Ok(())
    }

    pub fn for_chat_id(uuid: Uuid) -> Arc<dyn Responder> {
        let inner = INSTANCE
            .get()
            .expect("App command responder not initialized")
            .clone();

        Arc::new(AppCommandResponderForChatId { inner, uuid }) as Arc<dyn Responder>
    }
}

impl Responder for AppCommandResponderForChatId {
    fn handle(&self, response: CommandResponse) {
        let response = response.with_uuid(self.uuid);
        if let Err(err) = self.inner.tx.send(response) {
            tracing::error!("Failed to send response for command: {:?}", err);
        }
    }

    fn system_message(&self, message: &str) {
        self.handle(CommandResponse::Chat(
            self.uuid,
            chat_completion::ChatMessage::new_system(message),
        ));
    }

    fn update(&self, state: &str) {
        self.handle(CommandResponse::ActivityUpdate(self.uuid, state.into()));
    }

    fn rename(&self, name: &str) {
        self.handle(CommandResponse::RenameChat(self.uuid, name.into()));
    }

    fn agent_message(&self, message: chat_completion::ChatMessage) {
        self.handle(CommandResponse::Chat(self.uuid, message));
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
        let _ = AppCommandResponder::init(ui_tx);

        let responder = AppCommandResponder::for_chat_id(TEST_UUID);

        responder.system_message("Test message");

        let Some(ui_event) = ui_rx.recv().await else {
            panic!("No UI event received");
        };

        match ui_event {
            UIEvent::ChatMessage(received_uuid, received_message) => {
                assert_eq!(received_uuid, TEST_UUID);
                assert_eq!(received_message.formatted_content(), "Test message");
                assert!(received_message.role().is_system());
            }
            _ => panic!("Unexpected UI event received"),
        }

        responder.handle(CommandResponse::Completed(Uuid::new_v4()));

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
