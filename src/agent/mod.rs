mod conversation_summarizer;
mod env_setup;
mod running_agent;
mod tool_summarizer;
pub mod tools;
mod util;
mod v1;
use std::sync::Arc;

use anyhow::Result;
use swiftide_core::Tool;

/// NOTE: On architecture, when more agents are added, it would be nice to have the concept of an
/// (Agent/Chat) session that wraps all this complexity => Responders then update on the session.
/// Makes everything a lot simpler. The session can then also references the running agent,
/// executor, etc

#[tracing::instrument(skip(repository, command_responder))]
pub async fn start_agent(
    uuid: Uuid,
    repository: &Repository,
    initial_query: &str,
    command_responder: Arc<dyn Responder>,
) -> Result<RunningAgent> {
    command_responder.update("starting up agent for the first time, this might take a while");

    match repository.config().agent {
        crate::config::SupportedAgents::V1 => {
            v1::start(initial_query, uuid, repository, command_responder).await
        }
    }
}

pub fn available_tools(
    repository: &Repository,
    github_session: Option<&Arc<GithubSession>>,
    agent_env: Option<&env_setup::AgentEnvironment>,
) -> Result<Vec<Box<dyn Tool>>> {
    match repository.config().agent {
        crate::config::SupportedAgents::V1 => {
            v1::available_tools(repository, github_session, agent_env)
        }
    }
}

pub use running_agent::RunningAgent;
use uuid::Uuid;

use crate::{commands::Responder, git::github::GithubSession, repository::Repository};
