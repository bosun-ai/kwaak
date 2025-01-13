use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::{self, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    agent,
    chat_message::ChatMessage,
    frontend::{App, UIEvent},
    indexing,
    repository::Repository,
};

use super::{
    command::Command,
    responder::{CommandResponder, CommandResponse},
    running_agent::RunningAgent,
};

/// Commands always flow via the `CommandHandler`
pub struct CommandHandler {
    /// Receives commands
    rx: Option<mpsc::UnboundedReceiver<Command>>,
    /// Sends commands
    tx: mpsc::UnboundedSender<Command>,

    /// TODO: Remove this and use the command responder everywhere
    /// Then there can also be a single command responder, and removes coupling with frontend
    /// fully
    ///
    /// Sends `UIEvents` to the connected frontend
    ui_tx: Option<mpsc::UnboundedSender<UIEvent>>,
    /// Repository to interact with
    repository: Arc<Repository>,

    /// TODO: Fix this, too tired to think straight
    agents: Arc<RwLock<HashMap<Uuid, RunningAgent>>>,
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

    #[must_use]
    /// Starts the command handler
    ///
    /// # Panics
    ///
    /// - Missing ui sender
    /// - Missing receiver for commands
    pub fn start(mut self) -> tokio::task::JoinHandle<()> {
        let repository = Arc::clone(&self.repository);
        let ui_tx = self.ui_tx.clone().expect("Expected a registered ui");
        let mut rx = self.rx.take().expect("Expected a receiver");
        let this_handler = Arc::new(self);

        task::spawn(async move {
            // Handle spawned commands gracefully on quit
            // JoinSet invokes abort on drop
            let mut joinset = tokio::task::JoinSet::new();

            while let Some(cmd) = rx.recv().await {
                // On `Quit`, abort all running tasks, wait for them to finish then break.
                if cmd.is_quit() {
                    tracing::warn!("Backend received quit command, shutting down");
                    joinset.shutdown().await;
                    tracing::warn!("Backend shutdown complete");

                    break;
                }

                let repository = Arc::clone(&repository);
                let ui_tx = ui_tx.clone();
                let this_handler = Arc::clone(&this_handler);

                joinset.spawn(async move {
                    let result = this_handler.handle_command(&repository,  &cmd).await;
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

            tracing::warn!("CommandHandler shutting down");
        })
    }

    /// TODO: Most commands should probably be handled in a tokio task
    /// Maybe generalize tasks to make ui updates easier?
    #[tracing::instrument(skip_all, fields(otel.name = %cmd.to_string(), uuid = %cmd.uuid()), err)]
    async fn handle_command(&self, repository: &Repository, cmd: &Command) -> Result<()> {
        let now = std::time::Instant::now();
        tracing::warn!("Handling command {cmd}");

        #[allow(clippy::match_wildcard_for_single_variants)]
        match cmd {
            Command::StopAgent { uuid } => self.stop_agent(*uuid).await?,
            Command::IndexRepository { .. } => {
                let (command_responder, _guard) = self.spawn_command_responder(&cmd.uuid());
                indexing::index_repository(repository, Some(command_responder)).await?;
            }
            Command::ShowConfig { uuid } => {
                self.send_ui_event(
                    ChatMessage::new_system(toml::to_string_pretty(repository.config())?)
                        .uuid(*uuid)
                        .to_owned(),
                );
            }
            Command::Chat { uuid, ref message } => {
                let message = message.clone();
                let agent = self.find_or_start_agent_by_uuid(*uuid, &message).await?;
                let token = agent.cancel_token.clone();

                tokio::select! {
                    () = token.cancelled() => Ok(()),
                    result = agent.query(&message) => result,

                }?;
            }
            Command::Quit { .. } => unreachable!("Quit should be handled earlier"),
        }
        // Sleep for a tiny bit to avoid racing with agent responses
        tokio::time::sleep(Duration::from_millis(50)).await;
        let elapsed = now.elapsed();
        self.send_ui_event(
            ChatMessage::new_system(format!(
                "Command {cmd} successful in {} seconds",
                elapsed.as_secs_f64().round()
            ))
            .uuid(cmd.uuid()),
        );

        Ok(())
    }

    async fn find_or_start_agent_by_uuid(&self, uuid: Uuid, query: &str) -> Result<RunningAgent> {
        if let Some(agent) = self.agents.write().await.get_mut(&uuid) {
            // Ensure we always send a fresh cancellation token
            agent.cancel_token = CancellationToken::new();
            return Ok(agent.clone());
        }

        let (responder, handle) = self.spawn_command_responder(&uuid);

        let agent = agent::build_agent(uuid, &self.repository, query, responder).await?;

        let running_agent = RunningAgent {
            agent: Arc::new(Mutex::new(agent)),
            response_handle: Arc::new(handle),
            cancel_token: CancellationToken::new(),
        };

        let cloned = running_agent.clone();
        self.agents.write().await.insert(uuid, running_agent);

        Ok(cloned)
    }

    async fn stop_agent(&self, uuid: Uuid) -> Result<()> {
        let mut locked_agents = self.agents.write().await;
        let Some(agent) = locked_agents.get_mut(&uuid) else {
            self.send_ui_event(
                ChatMessage::new_system("No agent found (yet), is it starting up?").uuid(uuid),
            );
            return Ok(());
        };

        if agent.cancel_token.is_cancelled() {
            self.send_ui_event(ChatMessage::new_system("Agent already stopped").uuid(uuid));
            return Ok(());
        }

        // TODO: If this fails inbetween tool calls and responses, the agent will be stuck
        // Perhaps something to re-align it?
        agent.stop().await;

        self.send_ui_event(ChatMessage::new_system("Agent stopped").uuid(uuid));
        Ok(())
    }

    // Try to send a UI event, ignore if the UI is not connected
    fn send_ui_event(&self, event: impl Into<UIEvent>) {
        let Some(ui_tx) = &self.ui_tx else { return };
        let _ = ui_tx.send(event.into());
    }

    /// Forwards updates from the backend (i.e. agents) to the UI
    fn spawn_command_responder(&self, uuid: &Uuid) -> (CommandResponder, JoinHandle<()>) {
        let mut command_responder = CommandResponder::default().with_uuid(*uuid);

        let ui_tx_clone = self.ui_tx.clone().expect("expected ui tx");

        // TODO: Perhaps nicer to have a single loop for all agents
        // Then the majority of this can be moved to i.e. agents/running_agent
        // Design wise: Agents should not know about UI, command handler and UI should not know
        // about agent internals
        // As long as nobody is running thousands of agents, this is fine
        // TODO: Spawn should be impl on the command responder > putting ui tx behind an interface
        // such that mapping of response to ui, can then be in the frontend.
        let cloned_responder = command_responder.clone();
        let handle = task::spawn(async move {
            while let Some(response) = command_responder.recv().await {
                match response {
                    CommandResponse::Chat(msg) => {
                        let _ = ui_tx_clone.send(msg.into());
                    }
                    CommandResponse::ActivityUpdate(uuid, state) => {
                        let _ = ui_tx_clone.send(UIEvent::ActivityUpdate(uuid, state));
                    }
                    CommandResponse::RenameChat(uuid, name) => {
                        let _ = ui_tx_clone.send(UIEvent::RenameChat(uuid, name));
                    }
                }
            }
        });

        (cloned_responder, handle)
    }
}
