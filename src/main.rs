use std::{
    io::{self, stdout},
    panic::{self, set_hook, take_hook},
    sync::Arc,
};

use crate::config::Config;
use agent::session::available_builtin_tools;
use anyhow::{Context as _, Result};
use clap::Parser;
use commands::CommandResponse;
use frontend::App;
use git::github::GithubSession;
#[cfg(feature = "evaluations")]
use kwaak::evaluations;
use kwaak::{
    agent::{self, session::start_mcp_toolboxes},
    cli, commands, config, frontend, git,
    indexing::{
        self,
        duckdb_index::{get_duckdb, DuckdbIndex},
        index_repository,
    },
    onboarding, repository,
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use ::tracing::instrument;
use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use swiftide::{agents::DefaultContext, chat_completion::Tool, traits::AgentContext};
use swiftide_docker_executor::DockerExecutor;
use tokio::{fs, sync::mpsc};
use uuid::Uuid;

#[cfg(test)]
mod test_utils;

#[tokio::main]
#[allow(clippy::too_many_lines)] // I know, I know, it's not.
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Handle the `init` command immediately after parsing args
    if let Some(cli::Commands::Init { dry_run, file }) = args.command {
        if let Err(error) = onboarding::run(file, dry_run).await {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return Ok(());
    }

    init_panic_hook();

    // Load configuration
    let config = match Config::load(args.config_path.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Failed to load configuration: {error:#}");

            std::process::exit(1);
        }
    };
    let repository = repository::Repository::from_config(config);

    fs::create_dir_all(repository.config().cache_dir()).await?;
    fs::create_dir_all(repository.config().log_dir()).await?;

    let app_result = {
        // Only enable the tui logger if we're running the tui
        let command = args.command.as_ref().unwrap_or(&cli::Commands::Tui);

        let tui_logger_enabled = matches!(command, cli::Commands::Tui);

        let _guard = kwaak::kwaak_tracing::init(&repository, tui_logger_enabled)?;

        let _root_span = tracing::info_span!("main", "otel.name" = "main").entered();

        if git::util::is_dirty(repository.path()).await && !args.allow_dirty {
            eprintln!(
                "Error: The repository has uncommitted changes. Use --allow-dirty to override."
            );
            std::process::exit(1);
        }

        match command {
            cli::Commands::RunAgent { initial_message } => {
                start_agent(repository, &initial_message, &args).await
            }
            cli::Commands::Tui => start_tui(repository, &args).await,
            cli::Commands::ListTools => {
                let github_session = Arc::new(GithubSession::from_repository(&repository)?);
                let repository = Arc::new(repository);
                let index = DuckdbIndex::default();
                let tools =
                    available_builtin_tools(&repository, Some(&github_session), None, &index)?;

                println!("**Enabled built-in tools:**");
                for tool in tools {
                    println!(" - {}", tool.name());
                }

                let mcp_toolboxes = start_mcp_toolboxes(&repository).await?;

                println!("\n**MCP tools:**");
                for toolbox in mcp_toolboxes {
                    println!("::{}", toolbox.name());

                    for tool in toolbox.available_tools().await? {
                        println!(" - {}", tool.name());
                        println!("{:?}", tool.tool_spec());
                    }
                }

                Ok(())
            }
            cli::Commands::Index => {
                index_repository(&repository, &get_duckdb(&repository), None).await
            }
            cli::Commands::TestTool {
                tool_name,
                tool_args,
            } => test_tool(repository.into(), &tool_name, tool_args.as_deref()).await,
            cli::Commands::Query { query: query_param } => {
                let result =
                    indexing::query(&repository, &get_duckdb(&repository), query_param).await;

                if let Ok(result) = result.as_deref() {
                    println!("{result}");
                }

                result.map(|_| ())
            }
            cli::Commands::ClearCache => {
                let result = repository.clear_cache().await;
                println!("Cache cleared");

                result
            }
            cli::Commands::PrintConfig => {
                println!("{}", toml::to_string_pretty(repository.config())?);
                Ok(())
            }
            #[cfg(feature = "evaluations")]
            cli::Commands::Eval { eval_type } => match eval_type {
                cli::EvalCommands::Patch { iterations } => {
                    evaluations::run_patch_evaluation(*iterations).await
                }
                cli::EvalCommands::Ragas {
                    input,
                    output,
                    questions,
                    record_ground_truth,
                } => {
                    evaluations::evaluate_query_pipeline(
                        &repository,
                        input.as_deref(),
                        &output,
                        questions.as_deref(),
                        *record_ground_truth,
                    )
                    .await?;
                    Ok(())
                }
            },
            cli::Commands::Init { .. } => unreachable!(),
        }
    };

    if let Err(error) = app_result {
        ::tracing::error!(?error, "Kwaak encountered an error\n {error:#}");
        eprintln!("Kwaak encountered an error\n {error}");
        std::process::exit(1);
    }

    Ok(())
}

async fn test_tool(
    repository: Arc<repository::Repository>,
    tool_name: &str,
    tool_args: Option<&str>,
) -> Result<()> {
    let github_session = Arc::new(GithubSession::from_repository(&repository)?);
    let index = DuckdbIndex::default();
    let tool = available_builtin_tools(&repository, Some(&github_session), None, &index)?
        .into_iter()
        .find(|tool| tool.name() == tool_name)
        .context("Tool not found")?;

    let mut executor = DockerExecutor::default();
    let dockerfile = &repository.config().docker.dockerfile;

    println!(
        "Starting executor with dockerfile: {}",
        dockerfile.display()
    );
    let running_executor = executor
        .with_context_path(&repository.config().docker.context)
        .with_image_name(repository.config().project_name.to_lowercase())
        .with_dockerfile(dockerfile)
        .to_owned()
        .start()
        .await?;

    let agent_context = DefaultContext::from_executor(running_executor);

    println!("Invoking tool: {tool_name}");
    let output = tool
        .invoke(&agent_context as &dyn AgentContext, tool_args)
        .await?;

    println!("{output}");

    Ok(())
}

#[instrument(skip_all)]
async fn start_agent(
    mut repository: repository::Repository,
    initial_message: &str,
    args: &cli::Args,
) -> Result<()> {
    repository.config_mut().endless_mode = true;

    if !args.skip_indexing {
        indexing::index_repository(&repository, &get_duckdb(&repository), None).await?;
    }

    let (tx, mut rx) = mpsc::unbounded_channel();

    let handle = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            match response {
                CommandResponse::Chat(message) => {
                    println!("{message}");
                }
                CommandResponse::Activity(message) => {
                    println!(">> {message}");
                }
                CommandResponse::BackendMessage(message) => {
                    println!("Backend: {message}");
                }
                CommandResponse::RenameChat(..)
                | CommandResponse::RenameBranch(..)
                | CommandResponse::Completed => {}
            }
        }
    });

    let query = initial_message.to_string();
    let index = DuckdbIndex::default();
    let agent =
        agent::start_session(Uuid::new_v4(), &repository, &index, &query, Arc::new(tx)).await?;

    agent.active_agent().query(&query).await?;
    handle.abort();
    Ok(())
}

#[instrument(skip_all)]
#[allow(clippy::field_reassign_with_default)]
async fn start_tui(repository: repository::Repository, args: &cli::Args) -> Result<()> {
    ::tracing::info!("Loaded configuration: {:?}", repository.config());

    // Before starting the TUI, check if there is already a kwaak running on the project
    if panic::catch_unwind(|| {
        get_duckdb(&repository);
    })
    .is_err()
    {
        eprintln!("Failed to load database; are you running more than one kwaak on a project?");
        std::process::exit(1);
    }

    // Setup terminal
    let mut terminal = init_tui()?;

    // Start the application
    let repository = Arc::new(repository);
    let mut app = App::default_from_repository(repository.clone());
    app.ui_config = repository.config().ui.clone();

    debug_assert!(
        app.chats.len() == 1,
        "App should only have one chat at startup"
    );

    if args.skip_indexing {
        app.skip_indexing = true;
    }

    let app_result = {
        let kwaak_index = DuckdbIndex::default();
        let mut handler = commands::CommandHandler::from_index(kwaak_index);
        handler.register_ui(&mut app);

        let _guard = handler.start();

        app.run(&mut terminal).await
    };

    restore_tui()?;
    terminal.show_cursor()?;

    if let Err(error) = app_result {
        ::tracing::error!(?error, "Application error");
        eprintln!("Kwaak encountered an error:\n {error}");
        std::process::exit(1);
    }

    // Force exit the process, as any dangling threads can now safely be dropped
    std::process::exit(0);
}

pub fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        ::tracing::error!("Panic: {:?}", panic_info);
        let _ = restore_tui();

        original_hook(panic_info);
    }));
}

/// Initializes the terminal backend in raw mode
///
/// # Errors
///
/// Errors if the terminal backend cannot be initialized
pub fn init_tui() -> io::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(
        stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::all())
    )?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

/// Restores the terminal to its original state
///
/// # Errors
///
/// Errors if the terminal cannot be restored
pub fn restore_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    execute!(stdout(), PopKeyboardEnhancementFlags)?;
    Ok(())
}
