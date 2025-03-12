use std::sync::Arc;

use derive_builder::Builder;
use uuid::Uuid;

use super::Responder;

/// Commands are the main way to interact with the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug,
    strum_macros::Display,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIs,
    Clone,
    serde::Serialize,
    serde::Deserialize,
)]
// #[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
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

    /// Get the current changes made by the agent
    Diff,

    /// Execute a command in the context of an agent
    /// and get the output
    Exec { cmd: swiftide::traits::Command },

    /// Retry the last chat with the agent
    /// Will reset history to the point of the last chat, then re-run the chat
    RetryChat,
}

#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Builder, serde::Serialize, serde::Deserialize)]
pub struct CommandEvent {
    command: Command,
    uuid: Uuid,
    responder: Arc<dyn Responder>,
}

impl CommandEvent {
    #[must_use]
    pub fn quit() -> Self {
        CommandEvent {
            command: Command::Quit,
            responder: Arc::new(()),
            uuid: Uuid::new_v4(),
        }
    }
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

#[cfg(test)]
mod tests {
    use crate::commands::MockResponder;

    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn test_command_event_builder() {
        let command = Command::Quit;
        let uuid = Uuid::new_v4();
        let responder = Arc::new(MockResponder::new());

        let event = CommandEvent::builder()
            .command(command.clone())
            .uuid(uuid)
            .responder(responder.clone())
            .build()
            .unwrap();

        let dyn_responder = responder as Arc<dyn Responder>;
        assert!(event.command().is_quit());
        assert_eq!(event.uuid(), uuid);
        assert!(Arc::ptr_eq(&event.clone_responder(), &dyn_responder));
    }

    #[test]
    fn test_with_uuid() {
        let command = Command::ShowConfig;
        let uuid = Uuid::new_v4();
        let new_uuid = Uuid::new_v4();
        let responder = Arc::new(MockResponder::new());

        let event = CommandEvent::builder()
            .command(command.clone())
            .uuid(uuid)
            .responder(responder.clone())
            .build()
            .unwrap()
            .with_uuid(new_uuid);

        assert_eq!(event.uuid(), new_uuid);
    }
}
