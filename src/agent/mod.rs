pub mod agents;
mod commit_and_push;
pub mod conversation_summarizer;
pub mod env_setup;
pub mod running_agent;
pub mod session;
mod tool_summarizer;
pub mod tools;
mod util;
use session::{RunningSession, Session};
use std::sync::Arc;

use anyhow::Result;

/// Starts a new chat session based on the repository, its configuration, and the initial user query
#[tracing::instrument(skip(repository, command_responder))]
pub async fn start_session(
    uuid: Uuid,
    repository: &Repository,
    initial_query: &str,
    command_responder: Arc<dyn Responder>,
) -> Result<RunningSession> {
    command_responder
        .update("starting up agent for the first time, this might take a while")
        .await;

    Session::builder()
        .session_id(uuid)
        .repository(repository.clone())
        .default_responder(command_responder)
        .initial_query(initial_query.to_string())
        .start()
        .await
}

use uuid::Uuid;

use crate::{commands::Responder, repository::Repository};
