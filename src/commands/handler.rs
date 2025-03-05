use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use tokio::{
    sync::mpsc,
    task::{self},
};
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use crate::{
    agent::{self, session::RunningSession},
    frontend::App,
    git, indexing,
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
pub struct CommandHandler {
    /// Receives commands
    rx: Option<mpsc::UnboundedReceiver<CommandEvent>>,
    /// Sends commands
    tx: mpsc::UnboundedSender<CommandEvent>,

    /// Repository to interact with
    repository: Arc<Repository>,

    // agent_sessions: Arc<RwLock<HashMap<Uuid, RunningSession>>>,
    agent_sessions: HashMap<Uuid, RunningSession>,
}

impl CommandHandler {
    pub fn from_repository(repository: impl Into<Repository>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        CommandHandler {
            rx: Some(rx),
            tx,
            repository: Arc::new(repository.into()),
            // agent_sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_sessions: HashMap::new(),
        }
    }

    /// Returns the sender for commands
    #[must_use]
    pub fn command_tx(&self) -> &mpsc::UnboundedSender<CommandEvent> {
        &self.tx
    }

    pub fn register_ui(&mut self, app: &mut App) {
        app.command_tx = Some(self.tx.clone());
    }

    /// Starts the command handler
    ///
    /// # Panics
    ///
    /// - Missing ui sender
    /// - Missing receiver for commands
    pub fn start(mut self) -> AbortOnDropHandle<()> {
        let repository = Arc::clone(&self.repository);
        let mut rx = self.rx.take().expect("Expected a receiver");
        // Arguably we're spawning a single task and moving it once, the arc mutex should not be
        // needed.
        let this_handler = Arc::new(tokio::sync::Mutex::new(self));

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

                let repository = Arc::clone(&repository);
                let this_handler = Arc::clone(&this_handler);

                joinset.spawn(async move {
                    let result = this_handler.lock().await.handle_command_event(&repository, &event, &event.command()).await;
                    event.responder().send(CommandResponse::Completed);

                    if let Err(error) = result {
                        tracing::error!(?error, cmd = %event.command(), "Failed to handle command {cmd} with error {error:#}", cmd= event.command());
                        event.responder().system_message(&format!(
                                "Failed to handle command: {error:#}"
                            ));

                    };
                });
            }

            tracing::warn!("CommandHandler shutting down");
        }))
    }

    #[tracing::instrument(skip_all, fields(otel.name = %cmd.to_string(), uuid = %event.uuid()), err)]
    async fn handle_command_event(
        &mut self,
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
            Command::Chat { message } => {
                let message = message.clone();
                let session = self
                    .find_or_start_agent_by_uuid(event.uuid(), &message, event.clone_responder())
                    .await?;
                let token = session.cancel_token().clone();

                tokio::select! {
                    () = token.cancelled() => Ok(()),
                    result = session.query_agent(&message) => result,

                }?;
            }
            Command::Diff => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()) else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?");
                    return Ok(());
                };

                let base_sha = &session.agent_environment().start_ref;
                let diff = git::util::diff(session.executor(), &base_sha, true).await?;

                event.responder().system_message(&diff);
            }
            Command::Exec { cmd } => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()) else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?");
                    return Ok(());
                };

                let output = accept_non_zero_exit(session.executor().exec_cmd(cmd).await)?.output;

                event.responder().system_message(&output);
            }
            Command::RetryChat => {
                let Some(session) = self.find_agent_by_uuid(event.uuid()) else {
                    event
                        .responder()
                        .system_message("No agent found (yet), is it starting up?");
                    return Ok(());
                };
                let mut token = session.cancel_token().clone();
                if token.is_cancelled() {
                    // if let Some(session) = self.agent_sessions.write().await.get_mut(&event.uuid())
                    if let Some(session) = self.agent_sessions.get_mut(&event.uuid()) {
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
            Command::Quit { .. } => unreachable!("Quit should be handled earlier"),
        }
        // Sleep for a tiny bit to avoid racing with agent responses
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut elapsed = now.elapsed();

        // We cannot pause time in tokio because the larger tests
        // require multi thread and snapshot testing is still nice
        if cfg!(debug_assertions) {
            elapsed = Duration::from_secs(0);
        }

        event.responder().system_message(&format!(
            "Command {cmd} successful in {} seconds",
            elapsed.as_secs_f64().round()
        ));

        Ok(())
    }

    async fn find_or_start_agent_by_uuid(
        &mut self,
        uuid: Uuid,
        query: &str,
        responder: Arc<dyn Responder>,
    ) -> Result<RunningSession> {
        if let Some(session) = self.agent_sessions.get_mut(&uuid) {
            session.reset_cancel_token();

            return Ok(session.clone());
        }

        let running_agent = agent::start_session(uuid, &self.repository, query, responder).await?;
        let cloned = running_agent.clone();

        self.agent_sessions.insert(uuid, running_agent);

        Ok(cloned)
    }

    fn find_agent_by_uuid(&self, uuid: Uuid) -> Option<RunningSession> {
        if let Some(session) = self.agent_sessions.get(&uuid) {
            return Some(session.clone());
        }
        None
    }

    async fn stop_agent(&self, uuid: Uuid, responder: Arc<dyn Responder>) -> Result<()> {
        let Some(session) = self.agent_sessions.get(&uuid) else {
            responder.system_message("No agent found (yet), is it starting up?");
            return Ok(());
        };

        if session.cancel_token().is_cancelled() {
            responder.system_message("Agent already stopped");
            return Ok(());
        }

        // TODO: If this fails inbetween tool calls and responses, the agent will be stuck
        // Perhaps something to re-align it?
        session.stop().await;

        responder.system_message("Agent stopped");
        Ok(())
    }
}
