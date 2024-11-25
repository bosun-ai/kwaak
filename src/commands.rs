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
                        ui_tx.send(UIEvent::AgentReady(cmd.uuid())).unwrap();
                    };
                });
            }
        })
    }

    /// TODO: Most commands should probably be handled in a tokio task
    /// Maybe generalize tasks to make ui updates easier?
    async fn handle_command(
        &self,
        repository: &Repository,
        ui_tx: &mpsc::UnboundedSender<UIEvent>,
        cmd: &Command,
    ) -> Result<()> {
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

                ui_tx.send(UIEvent::AgentReady(*uuid)).unwrap();
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
        // TODO: Can we do this nicer? Double arc is ugly
        if let Some(agent) = self.agents.read().await.get(&uuid) {
            return Ok(agent.clone());
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<CommandResponse>();
        let ui_tx_clone = self.ui_tx.clone().expect("expected ui tx");

        let handle = task::spawn(async move {
            while let Some(response) = rx.recv().await {
                match response {
                    CommandResponse::Chat(msg) => {
                        let _ = ui_tx_clone.send(msg.with_uuid(uuid).into());
                    }
                }
            }
        });
        let agent = agent::build_agent(&self.repository, query, tx).await?;

        let running_agent = RunningAgent {
            agent: Arc::new(Mutex::new(agent)),
            handle: Arc::new(handle),
        };

        let cloned = running_agent.clone();
        self.agents.write().await.insert(uuid, running_agent);

        Ok(cloned)
    }
}
