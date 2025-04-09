use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result};
use derive_builder::Builder;
use swiftide::{
    agents::tools::local_executor::LocalExecutor,
    chat_completion::{ParamSpec, Tool, ToolSpec},
    traits::{SimplePrompt, ToolExecutor},
};
use swiftide_docker_executor::DockerExecutor;
use tavily::Tavily;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use uuid::Uuid;

use crate::{
    agent::{tools::DelegateAgent, util},
    commands::Responder,
    config::{self, AgentEditMode, SupportedToolExecutors},
    git::github::GithubSession,
    indexing::{self, Index},
    repository::Repository,
};

use super::{
    agents,
    env_setup::{self, AgentEnvironment, EnvSetup},
    running_agent::RunningAgent,
    tools,
};

/// Session represents the abstract state of an ongoing agent interaction (i.e. in a chat)
///
/// Consider the implementation 'emergent architecture' (an excuse for an isolated mess)
///
/// Some future ideas:
///     - Session configuration from a file
///     - A registry pattern for agents, so you could in theory run multiple concurrent
#[derive(Clone, Builder)]
#[builder(build_fn(private), setter(into))]
pub struct Session {
    pub session_id: Uuid,
    pub repository: Arc<Repository>,
    pub default_responder: Arc<dyn Responder>,
    pub initial_query: String,

    /// Handle to send messages to the running session
    running_session_tx: UnboundedSender<SessionMessage>,
}

/// Messages that can be send from i.e. a tool to an active session
#[derive(Clone)]
pub enum SessionMessage {
    SwapAgent(RunningAgent),
}

impl std::fmt::Debug for SessionMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SwapAgent(_) => f.debug_tuple("SwapAgent").finish(),
        }
    }
}

impl Session {
    #[must_use]
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    /// Inform the running session that the agent has been swapped
    pub fn swap_agent(&self, agent: RunningAgent) -> Result<()> {
        self.running_session_tx
            .send(SessionMessage::SwapAgent(agent))
            .map_err(Into::into)
    }
}

impl SessionBuilder {
    /// Starts a session
    #[tracing::instrument(skip_all)]
    pub async fn start(&mut self, index: &impl Index) -> Result<RunningSession> {
        let (running_session_tx, running_session_rx) = tokio::sync::mpsc::unbounded_channel();

        let session = Arc::new(
            self.running_session_tx(running_session_tx)
                .build()
                .context("Failed to build session")?,
        );

        let github_session = match session.repository.config().github_api_key {
            Some(_) => Some(Arc::new(GithubSession::from_repository(
                &session.repository,
            )?)),
            None => None,
        };

        let backoff = session.repository.config().backoff;
        let fast_query_provider: Box<dyn SimplePrompt> = session
            .repository
            .config()
            .indexing_provider()
            .get_simple_prompt_model(backoff)?;

        let ((), executor, branch_name, initial_context) = tokio::try_join!(
            util::rename_chat(
                &session.initial_query,
                &fast_query_provider,
                &session.default_responder
            ),
            start_tool_executor(session.session_id, &session.repository),
            // TODO: Below should probably be agent specific
            util::create_branch_name(
                &session.initial_query,
                &session.session_id,
                &fast_query_provider,
                &session.default_responder
            ),
            generate_initial_context(&session.repository, &session.initial_query, index)
        )?;

        let env_setup = EnvSetup::new(&session.repository, github_session.as_deref(), &*executor);
        let agent_environment = env_setup.exec_setup_commands(branch_name).await?;

        let available_tools = available_tools(
            &session.repository,
            github_session.as_ref(),
            Some(&agent_environment),
            index,
        )?;

        let active_agent = match session.repository.config().agent {
            config::SupportedAgentConfigurations::Coding => {
                agents::coding::start(
                    &session,
                    &executor,
                    &available_tools,
                    &agent_environment,
                    initial_context,
                )
                .await
            }
            // TODO: Strip tools for delegate agent and add tool for delegate
            config::SupportedAgentConfigurations::PlanAct => {
                start_plan_and_act(
                    &session,
                    &executor,
                    &available_tools,
                    &agent_environment,
                    &initial_context,
                )
                .await
            }
        }?;

        let mut running_session = RunningSession {
            active_agent: Arc::new(Mutex::new(active_agent)),
            session,
            github_session,
            executor,
            agent_environment,
            available_tools: available_tools.into(),
            cancel_token: Arc::new(Mutex::new(CancellationToken::new())),
            message_task_handle: None,
        };

        // TODO: Consider how this might be dropped
        let handle = tokio::spawn(running_message_handler(
            running_session.clone(),
            running_session_rx,
        ));

        running_session.message_task_handle = Some(Arc::new(AbortOnDropHandle::new(handle)));

        Ok(running_session)
    }
}

/// Spawns a small task to handle messages sent to the active session
async fn running_message_handler(
    running_session: RunningSession,
    mut running_session_rx: tokio::sync::mpsc::UnboundedReceiver<SessionMessage>,
) {
    while let Some(message) = running_session_rx.recv().await {
        tracing::debug!(?message, "Session received message");
        match message {
            SessionMessage::SwapAgent(agent) => {
                running_session.swap_agent(agent);
            }
        }
    }
}

static BLACKLIST_DELEGATE_TOOLS: &[&str] = &[
    "write_file",
    "shell_command",
    "write_file",
    "replace_lines",
    "add_lines",
];

async fn start_plan_and_act(
    session: &Arc<Session>,
    executor: &Arc<dyn ToolExecutor>,
    available_tools: &[Box<dyn Tool>],
    agent_environment: &AgentEnvironment,
    initial_context: &str,
) -> Result<RunningAgent> {
    let coding_agent = agents::coding::start(
        &session,
        &executor,
        &available_tools,
        &agent_environment,
        String::new(),
    )
    .await?;

    let delegate_tool = DelegateAgent::builder()
        .session(Arc::clone(&session))
        .agent(coding_agent)
        .tool_spec(
            ToolSpec::builder()
                .name("delegate_coding_agent")
                .description("If you have a coding task, delegate to the coding agent. Provide a thorough description of the task and relevant details.")
                .parameters(vec![ParamSpec::builder()
                    .name("task")
                    .description("An in depth description of the task")
                    .build()?])
                .build()?,
        )
        .build()
        .context("Failed to build delegate tool")?;

    // Blacklist tools from the list then add the delegate tool
    let delegate_tools = available_tools
        .iter()
        .filter(|tool| !BLACKLIST_DELEGATE_TOOLS.contains(&tool.name().as_ref()))
        .cloned()
        .chain(std::iter::once(delegate_tool.boxed()))
        .collect::<Vec<_>>();

    agents::delegate::start(
        &session,
        &executor,
        &delegate_tools,
        &agent_environment,
        initial_context,
    )
    .await
}

/// References a running session
/// Meant to be cloned
// TODO: Merge with session?
#[derive(Clone)]
#[allow(dead_code)]
pub struct RunningSession {
    session: Arc<Session>,
    active_agent: Arc<Mutex<RunningAgent>>,
    message_task_handle: Option<Arc<AbortOnDropHandle<()>>>,

    github_session: Option<Arc<GithubSession>>,
    executor: Arc<dyn ToolExecutor>,
    agent_environment: AgentEnvironment,
    available_tools: Arc<Vec<Box<dyn Tool>>>,

    cancel_token: Arc<Mutex<CancellationToken>>,
}

impl RunningSession {
    /// Get a cheap copy of the active agent
    ///
    /// # Panics
    ///
    /// Panics if the agent mutex is poisoned
    #[must_use]
    pub fn active_agent(&self) -> RunningAgent {
        self.active_agent.lock().unwrap().clone()
    }

    /// Run an agent with a query
    pub async fn query_agent(&self, query: &str) -> Result<()> {
        self.active_agent().query(query).await
    }

    /// Run an agent without a query
    pub async fn run_agent(&self) -> Result<()> {
        self.active_agent().run().await
    }

    /// Swap the current active agent with a new one
    ///
    /// # Panics
    ///
    /// Panics if the agent mutex is poisoned
    pub fn swap_agent(&self, running_agent: RunningAgent) {
        let mut lock = self.active_agent.lock().unwrap();
        *lock = running_agent;
    }

    #[must_use]
    pub fn executor(&self) -> &dyn ToolExecutor {
        &self.executor
    }

    #[must_use]
    pub fn agent_environment(&self) -> &AgentEnvironment {
        &self.agent_environment
    }

    /// Retrieve a copy of the cancel token
    ///
    /// # Panics
    ///
    /// Panics if the cancel token mutex is poisoned
    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.lock().unwrap().clone()
    }

    /// Resets the cancel token
    ///
    /// # Panics
    ///
    /// Panics if the agent mutex is poisoned
    pub fn reset_cancel_token(&self) {
        let mut lock = self.cancel_token.lock().unwrap();
        *lock = CancellationToken::new();
    }

    /// Stops the active agent
    ///
    /// # Panics
    ///
    /// Panics if the agent mutex is poisoned
    pub async fn stop(&self) {
        // When sessions have multiple agents, they should be stopped here
        self.reset_cancel_token();
        let lock = self.active_agent.lock().unwrap().clone();
        lock.stop().await;
    }
}

#[tracing::instrument(skip_all)]
async fn start_tool_executor(uuid: Uuid, repository: &Repository) -> Result<Arc<dyn ToolExecutor>> {
    let boxed = match repository.config().tool_executor {
        SupportedToolExecutors::Docker => {
            let mut executor = DockerExecutor::default();
            let dockerfile = &repository.config().docker.dockerfile;

            if std::fs::metadata(dockerfile).is_err() {
                tracing::error!("Dockerfile not found at {}", dockerfile.display());
                return Err(anyhow::anyhow!("Dockerfile not found"));
            }
            let running_executor = executor
                .with_context_path(&repository.config().docker.context)
                .with_image_name(repository.config().project_name.to_lowercase())
                .with_dockerfile(dockerfile)
                .with_container_uuid(uuid)
                .to_owned()
                .start()
                .await?;

            Arc::new(running_executor) as Arc<dyn ToolExecutor>
        }
        SupportedToolExecutors::Local => Arc::new(LocalExecutor::new(".")) as Arc<dyn ToolExecutor>,
    };

    Ok(boxed)
}

#[tracing::instrument(skip_all)]
async fn generate_initial_context(
    repository: &Repository,
    query: &str,
    index: &impl Index,
) -> Result<String> {
    let retrieved_context = index.query_repository(repository, &query).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

pub fn available_tools(
    repository: &Repository,
    github_session: Option<&Arc<GithubSession>>,
    agent_env: Option<&env_setup::AgentEnvironment>,
    index: &impl Index,
) -> Result<Vec<Box<dyn Tool>>> {
    let query_pipeline = index.build_query_pipeline(repository)?;
    let mut tools = vec![
        tools::write_file(),
        tools::search_file(),
        tools::git(),
        tools::shell_command(),
        tools::search_code(),
        tools::fetch_url(),
        tools::ExplainCode::new(query_pipeline).boxed(),
    ];

    // agent edit mode specific tools
    match repository.config().agent_edit_mode {
        AgentEditMode::Whole => {
            tools.push(tools::write_file());
            tools.push(tools::read_file());
        }
        AgentEditMode::Line => {
            tools.push(tools::read_file_with_line_numbers());
            tools.push(tools::replace_lines());
            tools.push(tools::add_lines());
        }
        AgentEditMode::Patch => {
            tools.push(tools::read_file_with_line_numbers());
            tools.push(tools::patch_file());
        }
    }

    // gitHub-related tools
    if let Some(github_session) = github_session {
        tools.push(tools::CreateOrUpdatePullRequest::new(github_session).boxed());
        tools.push(tools::GithubSearchCode::new(github_session).boxed());
    }

    // web search tool
    if let Some(tavily_api_key) = &repository.config().tavily_api_key {
        let tavily = Tavily::builder(tavily_api_key.expose_secret()).build()?;
        tools.push(tools::SearchWeb::new(tavily, tavily_api_key.clone()).boxed());
    }

    // test-related tools
    if let Some(test_command) = &repository.config().commands.test {
        tools.push(tools::RunTests::new(test_command).boxed());
    }

    if let Some(coverage_command) = &repository.config().commands.coverage {
        tools.push(tools::RunCoverage::new(coverage_command).boxed());
    }

    // reset file tool
    if let Some(env) = agent_env {
        tools.push(tools::ResetFile::new(&env.start_ref).boxed());
    }

    tools.retain(|tool| {
        !repository
            .config()
            .disabled_tools()
            .iter()
            .any(|s| *s == tool.name().as_ref())
    });

    Ok(tools)
}
