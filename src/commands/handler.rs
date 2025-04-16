use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{bail, Result};
use tokio::{
    sync::{mpsc, Mutex},
    task::{self},
};
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use crate::indexing::Index;
use crate::{
    agent::{self, session::RunningSession},
    frontend::App,
    git,
    repository::Repository,
    util::accept_non_zero_exit,
};

use super::{
    command::{Command, CommandEvent},
    responder::{CommandResponse, Responder},
};

/// Commands always flow via the `CommandHandler`
///
/// It is the principle entry point for the backend, and handles all commands
pub struct CommandHandler<S: Index> {
    /// Receives commands
    rx: Option<mpsc::UnboundedReceiver<CommandEvent>>,
    /// Sends commands
    tx: mpsc::UnboundedSender<CommandEvent>,

    agent_sessions: Mutex<HashMap<Uuid, RunningSession>>,

    index: S,
}

impl<'command, I: Index + Clone + 'static> CommandHandler<I> {
    pub fn from_index(index: I) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        CommandHandler {
            rx: Some(rx),
            tx,
            agent_sessions: Mutex::new(HashMap::new()),
            index,
        }
    }

    /// Returns the sender for commands
    #[must_use]
    pub fn command_tx(&self) -> &mpsc::UnboundedSender<CommandEvent> {
        &self.tx
    }

    pub fn register_ui(&mut self, app: &'command mut App) {
        app.command_tx = Some(self.tx.clone());
    }

    /// Starts the command handler
    ///
    /// # Panics
    ///
    /// - Missing ui sender
    /// - Missing receiver for commands
    pub fn start(mut self) -> AbortOnDropHandle<()> {
        let index = self.index.clone();
        let mut rx = self.rx.take().expect("Expected a receiver");
        // Arguably we're spawning a single task and moving it once, the arc mutex should not be
        // needed.
        let this_handler = Arc::new(self);

        AbortOnDropHandle::new(task::spawn(async move {
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

                let storage = index.clone();
                let this_handler = Arc::clone(&this_handler);

                joinset.spawn(async move {
                    let event = event.clone();
                    let result = Box::pin(this_handler.handle_command_event(&storage, &event)).await;
                    event.responder().send(CommandResponse::Completed).await;

                    if let Err(error) = result {
                        tracing::error!(?error, cmd = %event.command(), "Failed to handle command {cmd} with error {error:#}", cmd= event.command());
                        event.responder().system_message(&format!(
                                "Failed to handle command: {error:#}"
                            )).await;

                    }
                });
            }

            tracing::warn!("CommandHandler shutting down");
        }))
    }

    #[tracing::instrument(skip_all, fields(otel.name = %event.command().to_string(), uuid = %event.uuid()), err)]
    async fn handle_command_event(&self, index: &I, event: &CommandEvent) -> Result<()> {
        let now = std::time::Instant::now();

        let repository = event.repository();
        let cmd = event.command();

        tracing::info!("Handling command {cmd}");

        #[allow(clippy::match_wildcard_for_single_variants)]
        match cmd {
            Command::StopAgent => {
                self.stop_agent(event.uuid(), event.clone_responder())
                    .await?;
            }
            Command::IndexRepository => {
                let Some(repository) = repository else {
                    bail!("`IndexRepository` expects a repository")
                };
                index
                    .index_repository(repository, Some(event.clone_responder()))
                    .await?;
            }
            Command::ShowConfig => {
                let Some(repository) = repository else {
                    bail!("`ShowConfig` expects a repository")
                };
                event
                    .responder()
                    .system_message(&toml::to_string_pretty(repository.config())?)
                    .await;
            }
            Command::Chat { message } => {
                let Some(repository) = repository else {
                    bail!("`Chat` expects a repository")
                };
                let message = message.clone();
                let session = self
                    .find_or_start_agent_by_uuid(
                        event.uuid(),
                        &message,
                        &repository,
                        event.clone_responder(),
                    )
                    .await?;
                let token = session.cancel_token().clone();

                tokio::select! {
                    () = token.cancelled() => Ok(()),
                    result = session.query_agent(&message) => result,

                }?;
            }
            // TODO: Can be replaced by using `Exec` on the other side to keep this clean
            Command::Diff => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()).await else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?")
                        .await;
                    return Ok(());
                };

                let base_sha = &session.agent_environment().start_ref;
                let diff = git::util::diff(session.executor(), &base_sha, true).await?;

                event.responder().system_message(&diff).await;
            }
            Command::Exec { cmd } => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()).await else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?")
                        .await;
                    return Ok(());
                };

                let output = accept_non_zero_exit(session.executor().exec_cmd(cmd).await)?.output;

                event.responder().system_message(&output).await;
            }
            Command::RetryChat => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()).await else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?")
                        .await;
                    return Ok(());
                };
                let mut token = session.cancel_token().clone();
                if token.is_cancelled() {
                    // if let Some(session) = self.agent_sessions.write().await.get_mut(&event.uuid())
                    if let Some(session) = self.agent_sessions.lock().await.get_mut(&event.uuid()) {
                        session.reset_cancel_token();
                        token = session.cancel_token().clone();
                    }
                }

                session.active_agent().agent_context.redrive().await;
                tokio::select! {
                    () = token.cancelled() => Ok(()),
                    result = session.run_agent() => result,

                }?;
            }
            Command::Quit => unreachable!("Quit should be handled earlier"),
        }
        // Sleep for a tiny bit to avoid racing with agent responses
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut elapsed = now.elapsed();

        // We cannot pause time in tokio because the larger tests
        // require multi thread and snapshot testing is still nice
        if cfg!(debug_assertions) {
            elapsed = Duration::from_secs(0);
        }

        event
            .responder()
            .system_message(&format!(
                "Command {cmd} successful in {} seconds",
                elapsed.as_secs_f64().round()
            ))
            .await;

        Ok(())
    }

    async fn find_or_start_agent_by_uuid(
        &self,
        uuid: Uuid,
        query: &str,
        repository: &Repository,
        responder: Arc<dyn Responder>,
    ) -> Result<RunningSession> {
        if let Some(session) = self.agent_sessions.lock().await.get_mut(&uuid) {
            session.reset_cancel_token();

            return Ok(session.clone());
        }

        let running_agent =
            agent::start_session(uuid, repository, &self.index, query, responder).await?;
        let cloned = running_agent.clone();

        self.agent_sessions.lock().await.insert(uuid, running_agent);

        Ok(cloned)
    }

    async fn find_agent_by_uuid(&self, uuid: Uuid) -> Option<RunningSession> {
        if let Some(session) = self.agent_sessions.lock().await.get(&uuid) {
            return Some(session.clone());
        }
        None
    }

    async fn stop_agent(&self, uuid: Uuid, responder: Arc<dyn Responder>) -> Result<()> {
        let lock = self.agent_sessions.lock().await;
        let Some(session) = lock.get(&uuid) else {
            responder
                .system_message("No agent found (yet), is it starting up?")
                .await;
            return Ok(());
        };

        if session.cancel_token().is_cancelled() {
            responder.system_message("Agent already stopped").await;
            return Ok(());
        }

        // TODO: If this fails inbetween tool calls and responses, the agent will be stuck
        // Perhaps something to re-align it?
        session.stop().await;

        responder.system_message("Agent stopped").await;
        Ok(())
    }
}
