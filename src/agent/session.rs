use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result};
use derive_builder::Builder;
use rmcp::{
    model::{ClientInfo, Implementation},
    transport::TokioChildProcess,
    ServiceExt as _,
};
use swiftide::{
    agents::tools::mcp::McpToolbox,
    chat_completion::{ParamSpec, Tool, ToolSpec},
    traits::{SimplePrompt, ToolBox, ToolExecutor},
};
use tavily::Tavily;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use uuid::Uuid;

use crate::{
    agent::{tools::DelegateAgent, util},
    commands::Responder,
    config::{self, mcp::McpServer, AgentEditMode},
    indexing::Index,
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
/// NOTE: Seriously though, this file is a mess on purpose so we can figure out the best way to
/// to architect this.
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
            session
                .repository
                .start_tool_executor(Some(session.session_id)),
            // TODO: Below should probably be agent specific
            util::create_branch_name(
                &session.initial_query,
                &session.session_id,
                &fast_query_provider,
                &session.default_responder
            ),
            generate_initial_context(&session.repository, &session.initial_query, index)
        )?;

        let env_setup = EnvSetup::new(&session.repository, &*executor);
        let agent_environment = env_setup.exec_setup_commands(branch_name).await?;

        let builtin_tools =
            available_builtin_tools(&session.repository, Some(&agent_environment), index)?;

        let mcp_toolboxes = start_mcp_toolboxes(&session.repository).await?;

        let active_agent = match session.repository.config().agent {
            config::SupportedAgentConfigurations::Coding => {
                agents::coding::start(
                    &session,
                    &executor,
                    &builtin_tools,
                    &mcp_toolboxes,
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
                    &builtin_tools,
                    &mcp_toolboxes,
                    &agent_environment,
                    &initial_context,
                )
                .await
            }
        }?;

        let mut running_session = RunningSession {
            active_agent: Arc::new(Mutex::new(active_agent)),
            session,
            executor,
            agent_environment,
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
    tool_boxes: &[Box<dyn ToolBox>],
    agent_environment: &AgentEnvironment,
    initial_context: &str,
) -> Result<RunningAgent> {
    let coding_agent = agents::coding::start(
        &session,
        &executor,
        &available_tools,
        &tool_boxes,
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
        &tool_boxes,
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

    executor: Arc<dyn ToolExecutor>,
    agent_environment: AgentEnvironment,

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
async fn generate_initial_context(
    repository: &Repository,
    query: &str,
    index: &impl Index,
) -> Result<String> {
    let retrieved_context = index.query_repository(repository, query).await?;
    let formatted_context = format!("Additional information:\n\n{retrieved_context}");
    Ok(formatted_context)
}

pub fn available_builtin_tools(
    repository: &Arc<Repository>,
    agent_env: Option<&env_setup::AgentEnvironment>,
    index: &impl Index,
) -> Result<Vec<Box<dyn Tool>>> {
    let index = index.clone();
    let mut tools = vec![
        tools::write_file(),
        tools::search_file(),
        tools::git(),
        tools::shell_command(),
        tools::search_code(),
        tools::fetch_url(),
        Box::new(tools::ExplainCode::new(index, Arc::clone(&repository))),
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
    if let Some(github_session) = repository.github_session() {
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

pub async fn start_mcp_toolboxes(repository: &Repository) -> Result<Vec<Box<dyn ToolBox>>> {
    let mut services = Vec::new();
    if let Some(mcp_services) = &repository.config().mcp {
        for service in mcp_services {
            match service {
                McpServer::SubProcess {
                    name,
                    command,
                    args,
                    filter,
                    env,
                } => {
                    if command.is_empty() {
                        anyhow::bail!("Empty command for mcp tool");
                    }
                    let client_info = ClientInfo {
                        client_info: Implementation {
                            name: "kwaak".into(),
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                        ..Default::default()
                    };

                    let mut cmd = tokio::process::Command::new(command);

                    cmd.args(args);

                    if let Some(env) = env {
                        for (key, value) in env {
                            cmd.env(key, value.expose_secret());
                        }
                    }

                    let service = client_info.serve(TokioChildProcess::new(&mut cmd)?).await?;

                    let mut toolbox = McpToolbox::from_running_service(service)
                        .with_name(name)
                        .to_owned();

                    if let Some(filter) = filter {
                        toolbox.with_filter(filter.clone());
                    }

                    services.push(toolbox.boxed());
                }
            }
        }
    }
    Ok(services)
}
