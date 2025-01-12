#![recursion_limit = "256"] // Temporary fix so tracing plays nice with lancedb
use std::{
    io::{self, stdout},
    panic::{self, set_hook, take_hook},
    sync::Arc,
};

use agent::available_tools;
use anyhow::{Context as _, Result};
use clap::Parser;
use commands::{CommandResponder, CommandResponse};
use config::Config;
use frontend::App;
use git::github::GithubSession;
use kwaak::{
    agent, chat_message, cli, commands, config, frontend, git,
    indexing::{self, index_repository},
    onboarding, repository, storage,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use ::tracing::instrument;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use swiftide::{agents::DefaultContext, chat_completion::Tool, traits::AgentContext};
use tokio::fs;
use uuid::Uuid;

#[cfg(test)]
mod test_utils;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Handle the `init` command immediately after parsing args
    if let Some(cli::Commands::Init) = args.command {
        if let Err(error) = onboarding::run() {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    init_panic_hook();

    // Load configuration
    let config = Config::load(&args.config_path).await?;
    let repository = repository::Repository::from_config(config);

    fs::create_dir_all(repository.config().cache_dir()).await?;
    fs::create_dir_all(repository.config().log_dir()).await?;

    let app_result = {
        let _guard = kwaak::kwaak_tracing::init(&repository)?;

        let _root_span = tracing::info_span!("main", "otel.name" = "main").entered();

        let command = args.command.as_ref().unwrap_or(&cli::Commands::Tui);

        match command {
            cli::Commands::RunAgent { initial_message } => {
                start_agent(repository, initial_message).await
            }
            cli::Commands::Tui => start_tui(&repository, &args).await,
            cli::Commands::Index => index_repository(&repository, None).await,
            cli::Commands::TestTool {
                tool_name,
                tool_args,
            } => test_tool(&repository, tool_name, tool_args.as_deref()).await,
            cli::Commands::Query { query: query_param } => {
                let result = indexing::query(&repository, query_param.clone()).await;

                if let Ok(result) = result.as_deref() {
                    println!("{result}");
                };

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
            cli::Commands::Init => unreachable!(),
        }
    };

    if cfg!(feature = "otel") {
        opentelemetry::global::shutdown_tracer_provider();
    }

    if let Err(error) = app_result {
        ::tracing::error!("Kwaak encountered an error\n {error:#}");
        std::process::exit(1);
    }

    Ok(())
}

async fn test_tool(
    repository: &repository::Repository,
    tool_name: &str,
    tool_args: Option<&str>,
) -> Result<()> {
    let github_session = Arc::new(GithubSession::from_repository(&repository)?);
    let tool = available_tools(repository, Some(&github_session), None)?
        .into_iter()
        .find(|tool| tool.name() == tool_name)
        .context("Tool not found")?;

    let agent_context = DefaultContext::default();

    let output = tool
        .invoke(&agent_context as &dyn AgentContext, tool_args)
        .await?;
    println!("{output}");

    Ok(())
}

#[instrument]
async fn start_agent(mut repository: repository::Repository, initial_message: &str) -> Result<()> {
    repository.config_mut().endless_mode = true;

    indexing::index_repository(&repository, None).await?;

    let mut command_responder = CommandResponder::default();
    let responder_for_agent = command_responder.clone();

    let handle = tokio::spawn(async move {
        while let Some(response) = command_responder.recv().await {
            match response {
                CommandResponse::Chat(message) => {
                    if let Some(original) = message.original() {
                        println!("{original}");
                    }
                }
                CommandResponse::ActivityUpdate(.., message) => {
                    println!(">> {message}");
                }
                CommandResponse::RenameChat(..) => {}
            }
        }
    });

    let query = initial_message.to_string();
    let mut agent =
        agent::build_agent(Uuid::new_v4(), &repository, &query, responder_for_agent).await?;

    agent.query(&query).await?;
    handle.abort();
    Ok(())
}

#[instrument]
async fn start_tui(repository: &repository::Repository, args: &cli::Args) -> Result<()> {
    ::tracing::info!("Loaded configuration: {:?}", repository.config());

    // Before starting the TUI, check if there is already a kwaak running on the project
    // TODO: This is not very reliable. Potentially redb needs to be reconsidered
    if panic::catch_unwind(|| {
        storage::get_redb(&repository);
    })
    .is_err()
    {
        eprintln!("Failed to load database; are you running more than one kwaak on a project?");
        std::process::exit(1);
    }

    // Setup terminal
    let mut terminal = init_tui()?;

    // Start the application
    let mut app = App::default();

    if args.skip_indexing {
        app.skip_indexing = true;
    }

    if cfg!(feature = "test-layout") {
        app.ui_tx
            .send(chat_message::ChatMessage::new_user("Hello, show me some markdown!").into())?;
        app.ui_tx
            .send(chat_message::ChatMessage::new_system("showing markdown").into())?;
        app.ui_tx
            .send(chat_message::ChatMessage::new_assistant(MARKDOWN_TEST).into())?;
    }

    let app_result = {
        let mut handler = commands::CommandHandler::from_repository(repository);
        handler.register_ui(&mut app);

        let _guard = handler.start();

        app.run(&mut terminal).await
    };

    restore_tui()?;
    terminal.show_cursor()?;

    if let Err(error) = app_result {
        ::tracing::error!(?error, "Application error");
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

        if cfg!(feature = "otel") {
            opentelemetry::global::shutdown_tracer_provider();
        }
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
    Ok(())
}

static MARKDOWN_TEST: &str = r#"
# Main header
## Examples

Indexing a local code project, chunking into smaller pieces, enriching the nodes with metadata, and persisting into [Qdrant](https://qdrant.tech):

```rust
indexing::Pipeline::from_loader(FileLoader::new(".").with_extensions(&["rs"]))
        .with_default_llm_client(openai_client.clone())
        .filter_cached(Redis::try_from_url(
            redis_url,
            "swiftide-examples",
        )?)
        .then_chunk(ChunkCode::try_for_language_and_chunk_size(
            "rust",
            10..2048,
        )?)
        .then(MetadataQACode::default())
        .then(move |node| my_own_thing(node))
        .then_in_batch(Embed::new(openai_client.clone()))
        .then_store_with(
            Qdrant::builder()
                .batch_size(50)
                .vector_size(1536)
                .build()?,
        )
        .run()
        .await?;
```

Querying for an example on how to use the query pipeline:

```rust
query::Pipeline::default()
    .then_transform_query(GenerateSubquestions::from_client(
        openai_client.clone(),
    ))
    .then_transform_query(Embed::from_client(
        openai_client.clone(),
    ))
    .then_retrieve(qdrant.clone())
    .then_answer(Simple::from_client(openai_client.clone()))
    .query("How can I use the query pipeline in Swiftide?")
    .await?;
"#;
