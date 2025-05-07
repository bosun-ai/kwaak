pub mod agents;
pub mod commit_and_push;
pub mod conversation_summarizer;
pub mod git_agent_environment;
pub mod running_agent;
pub mod session;
pub mod tool_summarizer;
pub mod tools;
pub mod util;
use crate::{commands::Responder, indexing::Index, repository::Repository};
use session::{RunningSession, Session};
use std::sync::Arc;
use uuid::Uuid;

use anyhow::Result;

/// Starts a new chat session based on the repository, its configuration, and the initial user query
#[tracing::instrument(skip(repository, command_responder))]
pub async fn start_session(
    uuid: Uuid,
    repository: &Repository,
    index: &impl Index,
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
        .start(index)
        .await
}
