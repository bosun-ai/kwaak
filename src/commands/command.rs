use std::sync::Arc;

use derive_builder::Builder;
use uuid::Uuid;

use crate::repository::Repository;

use super::Responder;

/// Commands are the main way to interact with the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug, strum_macros::Display, strum_macros::IntoStaticStr, strum_macros::EnumIs, Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    /// Cleanly stop the backend
    Quit,

    /// Print the config the config for a repository
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

#[derive(Debug, Clone, Builder)]
pub struct CommandEvent {
    command: Command,
    repository: Option<Arc<Repository>>,
    uuid: Uuid,
    responder: Arc<dyn Responder>,
}

impl CommandEvent {
    #[must_use]
    pub fn builder() -> CommandEventBuilder {
        CommandEventBuilder::default()
    }

    pub fn repository(&self) -> Option<&Repository> {
        self.repository.as_deref()
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

    pub fn with_uuid(&mut self, uuid: Uuid) -> &mut Self {
        self.uuid = uuid;
        self
    }

    pub fn with_repository(&mut self, repository: Arc<Repository>) -> &mut Self {
        self.repository = Some(repository);
        self
    }

    pub fn with_maybe_repository(&mut self, repository: Option<Arc<Repository>>) -> &mut Self {
        self.repository = repository;
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
            .with_uuid(new_uuid)
            .to_owned();

        assert_eq!(event.uuid(), new_uuid);
    }
}
