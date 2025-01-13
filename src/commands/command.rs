use std::sync::Arc;

use derive_builder::Builder;
use uuid::Uuid;

use super::Responder;

/// Commands are the main way to interact with the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug,
    // PartialEq,
    // Eq,
    // strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIs,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    /// Cleanly stop the backend
    Quit,

    /// Print the config the backend is using
    ShowConfig,

    /// Re-index a repository
    IndexRepository,

    /// Stop an agent
    StopAgent,

    /// Chat with an agent
    Chat { message: String },

    /// Execute a tool executor compatible command in a running tool executor
    Exec { command: swiftide::traits::Command },
}

#[derive(Debug, Clone, Builder)]
pub struct CommandEvent {
    command: Command,
    uuid: Uuid,
    responder: Arc<dyn Responder>,
}

impl CommandEvent {
    #[must_use]
    pub fn builder() -> CommandEventBuilder {
        CommandEventBuilder::default()
    }

    #[must_use]
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[must_use]
    pub fn command(&self) -> &Command {
        &self.command
    }

    #[must_use]
    pub fn responder(&self) -> &dyn Responder {
        &self.responder
    }

    #[must_use]
    pub fn clone_responder(&self) -> Arc<dyn Responder> {
        Arc::clone(&self.responder)
    }

    #[must_use]
    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = uuid;
        self
    }
}
