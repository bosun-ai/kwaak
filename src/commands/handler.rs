use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::{self},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{agent, frontend::App, indexing, repository::Repository};

use super::{
    command::{Command, CommandEvent},
    responder::{CommandResponse, Responder},
    running_agent::RunningAgent,
};

/// Commands always flow via the `CommandHandler`
///
/// It is the principle entry point for the backend, and handles all commands
pub struct CommandHandler {
    /// Receives commands
    rx: Option<mpsc::UnboundedReceiver<CommandEvent>>,
    /// Sends commands
    tx: mpsc::UnboundedSender<CommandEvent>,

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
            repository: Arc::new(repository.into()),
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_ui(&mut self, app: &mut App) {
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
        let mut rx = self.rx.take().expect("Expected a receiver");
        let this_handler = Arc::new(self);

        task::spawn(async move {
            // Handle spawned commands gracefully on quit
            // JoinSet invokes abort on drop
            let mut joinset = tokio::task::JoinSet::new();

            while let Some(event) = rx.recv().await {
                // On `Quit`, abort all running tasks, wait for them to finish then break.
                if event.command().is_quit() {
                    tracing::warn!("Backend received quit command, shutting down");
                    joinset.shutdown().await;
                    tracing::warn!("Backend shutdown complete");

                    break;
                }

                let repository = Arc::clone(&repository);
                let this_handler = Arc::clone(&this_handler);

                joinset.spawn(async move {
                    let result = this_handler.handle_command_event(&repository, &event, &event.command()).await;
                    // ui_tx.send(UIEvent::CommandDone(cmd.uuid())).unwrap();
                    event.responder().handle(CommandResponse::Completed(event.uuid()));

                    if let Err(error) = result {
                        tracing::error!(?error, cmd = %event.command(), "Failed to handle command {cmd} with error {error:#}", cmd= event.command());
                            event.responder().system_message(&format!(
                                    "Failed to handle command: {error:#}"
                                ));
                    };
                });
            }

            tracing::warn!("CommandHandler shutting down");
        })
    }

    /// TODO: Most commands should probably be handled in a tokio task
    /// Maybe generalize tasks to make ui updates easier?
    #[tracing::instrument(skip_all, fields(otel.name = %cmd.to_string(), uuid = %event.uuid()), err)]
    async fn handle_command_event(
        &self,
        repository: &Repository,
        event: &CommandEvent,
        cmd: &Command,
    ) -> Result<()> {
        let now = std::time::Instant::now();
        tracing::warn!("Handling command {cmd}");

        #[allow(clippy::match_wildcard_for_single_variants)]
        match cmd {
            Command::StopAgent => {
                self.stop_agent(event.uuid(), event.clone_responder())
                    .await?;
            }
            Command::IndexRepository { .. } => {
                indexing::index_repository(repository, Some(event.clone_responder())).await?;
            }
            Command::ShowConfig => event
                .responder()
                .system_message(&toml::to_string_pretty(repository.config())?),
            Command::Chat { ref message } => {
                let message = message.clone();
                let agent = self
                    .find_or_start_agent_by_uuid(event.uuid(), &message, event.clone_responder())
                    .await?;
                let token = agent.cancel_token.clone();

                tokio::select! {
                    () = token.cancelled() => Ok(()),
                    result = agent.query(&message) => result,

                }?;
            }
            Command::Exec { command } => {
                let Some(agent) = self.find_agent_by_uuid(event.uuid()).await else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?");
                    return Ok(());
                };

                let result = agent.executor.exec_cmd(&command).await;
                todo!();

                // And now it needs to go back to the frontend again
            }
            Command::Quit { .. } => unreachable!("Quit should be handled earlier"),
        }
        // Sleep for a tiny bit to avoid racing with agent responses
        tokio::time::sleep(Duration::from_millis(50)).await;
        let elapsed = now.elapsed();
        event.responder().system_message(&format!(
            "Command {cmd} successful in {} seconds",
            elapsed.as_secs_f64().round()
        ));

        Ok(())
    }

    async fn find_or_start_agent_by_uuid(
        &self,
        uuid: Uuid,
        query: &str,
        responder: Arc<dyn Responder>,
    ) -> Result<RunningAgent> {
        if let Some(agent) = self.agents.write().await.get_mut(&uuid) {
            // Ensure we always send a fresh cancellation token
            agent.cancel_token = CancellationToken::new();
            return Ok(agent.clone());
        }

        let (agent, executor) =
            agent::build_agent(uuid, &self.repository, query, responder).await?;

        let running_agent = RunningAgent {
            agent: Arc::new(Mutex::new(agent)),
            cancel_token: CancellationToken::new(),
            executor,
        };

        let cloned = running_agent.clone();
        self.agents.write().await.insert(uuid, running_agent);

        Ok(cloned)
    }

    async fn find_agent_by_uuid(&self, uuid: Uuid) -> Option<RunningAgent> {
        let agents = self.agents.read().await;
        agents.get(&uuid).cloned()
    }

    async fn stop_agent(&self, uuid: Uuid, responder: Arc<dyn Responder>) -> Result<()> {
        let mut locked_agents = self.agents.write().await;
        let Some(agent) = locked_agents.get_mut(&uuid) else {
            responder.system_message("No agent found (yet), is it starting up?");
            return Ok(());
        };

        if agent.cancel_token.is_cancelled() {
            responder.system_message("Agent already stopped");
            return Ok(());
        }

        // TODO: If this fails inbetween tool calls and responses, the agent will be stuck
        // Perhaps something to re-align it?
        agent.stop().await;

        responder.system_message("Agent stopped");
        Ok(())
    }
}
