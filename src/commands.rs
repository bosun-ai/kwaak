use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use swiftide::agents::Agent;
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task,
};
use uuid::Uuid;

use crate::{
    agent,
    chat_message::ChatMessage,
    frontend::{App, UIEvent},
    indexing,
    repository::Repository,
};

/// Commands represent concrete actions from a user or in the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug,
    PartialEq,
    Eq,
    strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIs,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    Quit {
        uuid: Uuid,
    },
    ShowConfig {
        uuid: Uuid,
    },
    IndexRepository {
        uuid: Uuid,
    },
    /// Default when no command is provided
    Chat {
        uuid: Uuid,
        message: String,
    },
}

pub enum CommandResponse {
    Chat(ChatMessage),
    ActivityUpdate(Uuid, String),
}

#[derive(Clone)]
pub struct CommandResponder {
    tx: mpsc::UnboundedSender<CommandResponse>,
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
}

impl From<ChatMessage> for CommandResponse {
    fn from(msg: ChatMessage) -> Self {
        CommandResponse::Chat(msg)
    }
}

impl Command {
    pub fn uuid(&self) -> Uuid {
        match self {
            Command::Quit { uuid }
            | Command::ShowConfig { uuid }
            | Command::IndexRepository { uuid }
            | Command::Chat { uuid, .. } => *uuid,
        }
    }

    pub fn with_uuid(self, uuid: Uuid) -> Self {
        match self {
            Command::Quit { .. } => Command::Quit { uuid },
            Command::ShowConfig { .. } => Command::ShowConfig { uuid },
            Command::IndexRepository { .. } => Command::IndexRepository { uuid },
            Command::Chat { message, .. } => Command::Chat { uuid, message },
        }
    }
}

/// Commands always flow via the `CommandHandler`
pub struct CommandHandler {
    /// Receives commands
    rx: Option<mpsc::UnboundedReceiver<Command>>,
    /// Sends commands
    tx: mpsc::UnboundedSender<Command>,

    /// Sends `UIEvents` to the connected frontend
    ui_tx: Option<mpsc::UnboundedSender<UIEvent>>,
    /// Repository to interact with
    repository: Arc<Repository>,

    /// TODO: Fix this, too tired to think straight
    agents: Arc<RwLock<HashMap<Uuid, RunningAgent>>>,
}

#[derive(Clone)]
struct RunningAgent {
    agent: Arc<Mutex<Agent>>,

    #[allow(dead_code)]
    handle: Arc<tokio::task::JoinHandle<()>>,
}

impl RunningAgent {
    pub async fn query(&self, query: &str) -> Result<()> {
        self.agent.lock().await.query(query).await
    }
}

impl CommandHandler {
    pub fn from_repository(repository: impl Into<Repository>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        CommandHandler {
            rx: Some(rx),
            tx,
            ui_tx: None,
            repository: Arc::new(repository.into()),
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_ui(&mut self, app: &mut App) {
        self.ui_tx = Some(app.ui_tx.clone());
        app.command_tx = Some(self.tx.clone());
    }

    pub fn start(mut self) -> tokio::task::JoinHandle<()> {
        let repository = Arc::clone(&self.repository);
        let ui_tx = self.ui_tx.clone().expect("Expected a registered ui");
        let mut rx = self.rx.take().expect("Expected a receiver");
        let this_handler = Arc::new(self);

        task::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                let repository = Arc::clone(&repository);
                let ui_tx = ui_tx.clone();
                let this_handler = Arc::clone(&this_handler);

                tokio::spawn(async move {
                    let result = this_handler.handle_command(&repository, &ui_tx, &cmd).await;
                    ui_tx.send(UIEvent::CommandDone(cmd.uuid())).unwrap();

                    if let Err(error) = result {
                        tracing::error!(?error, %cmd, "Failed to handle command {cmd} with error {error:#}");
                        ui_tx
                            .send(
                                ChatMessage::new_system(format!(
                                    "Failed to handle command: {error:#}"
                                ))
                                .uuid(cmd.uuid())
                                .to_owned()
                                .into(),
                            )
                            .unwrap();
                    };
                });
            }
        })
    }

    /// TODO: Most commands should probably be handled in a tokio task
    /// Maybe generalize tasks to make ui updates easier?
    #[tracing::instrument(parent = None, skip(self, repository, ui_tx), fields(cmd = %cmd.to_string(), uuid = cmd.uuid().to_string()))]
    async fn handle_command(
        &self,
        repository: &Repository,
        ui_tx: &mpsc::UnboundedSender<UIEvent>,
        cmd: &Command,
    ) -> Result<()> {
        tracing::Span::current().record("otel.name", format!("command.{cmd}"));

        let now = std::time::Instant::now();
        tracing::warn!("Handling command {cmd}");

        #[allow(clippy::match_wildcard_for_single_variants)]
        match cmd {
            Command::IndexRepository { .. } => indexing::index_repository(repository).await?,
            Command::ShowConfig { uuid } => {
                ui_tx
                    .send(
                        ChatMessage::new_system(toml::to_string_pretty(repository.config())?)
                            .uuid(*uuid)
                            .to_owned()
                            .into(),
                    )
                    .unwrap();
            }
            Command::Chat { uuid, ref message } => {
                let agent = self.find_or_start_agent_by_uuid(*uuid, message).await?;

                agent.query(message).await?;
            }
            // Anything else we forward to the UI
            _ => ui_tx.send(cmd.clone().into()).unwrap(),
        }
        // Sleep for a tiny bit to avoid racing with agent responses
        tokio::time::sleep(Duration::from_millis(50)).await;
        let elapsed = now.elapsed();
        ui_tx
            .send(
                ChatMessage::new_system(format!(
                    "Command {cmd} successful in {} seconds",
                    elapsed.as_secs_f64().round()
                ))
                .uuid(cmd.uuid())
                .into(),
            )
            .unwrap();

        Ok(())
    }

    async fn find_or_start_agent_by_uuid(&self, uuid: Uuid, query: &str) -> Result<RunningAgent> {
        if let Some(agent) = self.agents.read().await.get(&uuid) {
            return Ok(agent.clone());
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<CommandResponse>();
        let command_responder = CommandResponder { tx, uuid };

        let ui_tx_clone = self.ui_tx.clone().expect("expected ui tx");

        // TODO: Perhaps nicer to have a single loop for all agents
        // Then the majority of this can be moved to i.e. agents/running_agent
        // Design wise: Agents should not know about UI, command handler and UI should not know
        // about agent internals
        let handle = task::spawn(async move {
            while let Some(response) = rx.recv().await {
                match response {
                    CommandResponse::Chat(msg) => {
                        let _ = ui_tx_clone.send(msg.into());
                    }
                    CommandResponse::ActivityUpdate(uuid, state) => {
                        let _ = ui_tx_clone.send(UIEvent::AgentActivity(uuid, state));
                    }
                }
            }
        });
        let agent = agent::build_agent(&self.repository, query, command_responder).await?;

        let running_agent = RunningAgent {
            agent: Arc::new(Mutex::new(agent)),
            handle: Arc::new(handle),
        };

        let cloned = running_agent.clone();
        self.agents.write().await.insert(uuid, running_agent);

        Ok(cloned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_command_handler() -> Result<()> {
        // Create a mock repository
        let config = repository::Config::default();
        let repository = repository::Repository::from_config(config);

        // Setup CommandHandler with mocks
        let mut command_handler = CommandHandler::from_repository(repository);

        // Setup channels for sending and receiving UI events
        let (ui_tx, mut ui_rx) = mpsc::unbounded_channel();
        command_handler.ui_tx = Some(ui_tx.clone());

        // Start the command handler
        let _handler = command_handler.start();

        // Simulate sending a command to show the current configuration
        let command_uuid = Uuid::new_v4();
        command_handler
            .tx
            .send(Command::ShowConfig { uuid: command_uuid })
            .expect("Failed to send command");

        // Verify that a UI event with configuration details is received
        if let Some(UIEvent::ChatMessage(ChatMessage::System { content, .. })) = ui_rx.recv().await
        {
            assert!(content.contains("[package]")); // Example check
        } else {
            panic!("Expecting a system message about configuration");
        }

        // Simulate sending a quit command
        let quit_uuid = Uuid::new_v4();
        command_handler
            .tx
            .send(Command::Quit { uuid: quit_uuid })
            .expect("Failed to send quit command");

        // Verify that the command handler processes the quit command
        assert!(ui_rx.recv().await.is_some());

        Ok(())
    }
}

