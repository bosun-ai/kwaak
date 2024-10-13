use std::sync::Arc;

use anyhow::Result;
use tokio::{sync::mpsc, task};
use uuid::Uuid;

use crate::{
    chat_message::ChatMessage,
    frontend::{App, UIEvent},
    indexing, query,
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
    rx: mpsc::UnboundedReceiver<Command>,
    #[allow(dead_code)]
    /// Sends commands
    tx: mpsc::UnboundedSender<Command>,
    /// Sends `UIEvents` to the connected frontend
    ui_tx: Option<mpsc::UnboundedSender<UIEvent>>,
    /// Repository to interact with
    repository: Arc<Repository>,
}

impl CommandHandler {
    pub fn from_repository(repository: impl Into<Repository>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        CommandHandler {
            rx,
            tx,
            ui_tx: None,
            repository: Arc::new(repository.into()),
        }
    }

    pub fn register_ui(&mut self, app: &mut App) {
        self.ui_tx = Some(app.ui_tx.clone());
        app.command_tx = Some(self.tx.clone());
    }

    pub fn start(mut self) -> tokio::task::JoinHandle<()> {
        task::spawn(async move {
            while let Some(cmd) = self.rx.recv().await {
                let tx = self.ui_tx.clone().expect("Expected a registered ui");
                let repository = Arc::clone(&self.repository);

                tokio::spawn(async move {
                    if let Err(error) =
                        Self::handle_command(&Arc::clone(&repository), &tx, &cmd).await
                    {
                        tracing::error!(?error, %cmd, "Failed to handle command {cmd} with error {error:#}");
                        tx.send(
                            ChatMessage::new_system(format!("Failed to handle command: {error:#}"))
                                .uuid(cmd.uuid())
                                .to_owned()
                                .into(),
                        )
                        .unwrap();
                    }
                });
            }
        })
    }

    /// TODO: Most commands should probably be handled in a tokio task
    /// Maybe generalize tasks to make ui updates easier?
    #[tracing::instrument(skip(repository, tx))]
    async fn handle_command(
        repository: &Repository,
        tx: &mpsc::UnboundedSender<UIEvent>,
        cmd: &Command,
    ) -> Result<()> {
        let now = std::time::Instant::now();
        tracing::warn!("Handling command {cmd}");

        #[allow(clippy::match_wildcard_for_single_variants)]
        match cmd {
            Command::IndexRepository { .. } => indexing::index_repository(repository).await?,
            Command::ShowConfig { uuid } => {
                tx.send(
                    ChatMessage::new_system(toml::to_string_pretty(repository.config())?)
                        .uuid(*uuid)
                        .to_owned()
                        .into(),
                )
                .unwrap();
            }
            Command::Chat { uuid, ref message } => {
                let response = query::query(repository, message).await?;
                tracing::info!(%response, "Chat message received, sending to frontend");
                let response = ChatMessage::new_system(response).uuid(*uuid).to_owned();

                tx.send(response.into()).unwrap();
            }
            // Anything else we forward to the UI
            _ => tx.send(cmd.clone().into()).unwrap(),
        }
        let elapsed = now.elapsed();
        tx.send(
            ChatMessage::new_system(format!(
                "Command {cmd} successful in {} seconds",
                elapsed.as_secs_f64()
            ))
            .uuid(cmd.uuid())
            .into(),
        )
        .unwrap();

        Ok(())
    }
}
