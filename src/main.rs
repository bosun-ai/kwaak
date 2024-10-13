use std::{
    io::{self, stdout},
    panic::{set_hook, take_hook},
};

use anyhow::Result;
use config::Config;
use frontend::App;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

mod chat;
mod chat_message;
mod commands;
mod config;
mod frontend;
mod indexing;
mod query;
mod repository;
mod storage;
mod tracing;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    init_panic_hook();
    // Load configuration
    let config = Config::load().await?;
    let repository = repository::Repository::from_config(config);

    std::fs::create_dir_all(repository.config().cache_dir())?;
    std::fs::create_dir_all(repository.config().log_dir())?;

    crate::tracing::init(&repository)?;

    ::tracing::info!("Loaded configuration: {:?}", repository.config());

    // Setup terminal
    let mut terminal = init_tui()?;

    // Start the application
    let mut app = App::default();

    if cfg!(feature = "test-markdown") {
        app.ui_tx
            .send(chat_message::ChatMessage::new_system(MARKDOWN_TEST).into())?;
    }

    let app_result = {
        let mut handler = commands::CommandHandler::from_repository(&repository);
        handler.register_ui(&mut app);

        let _guard = handler.start();

        app.run(&mut terminal).await
    };

    // TODO: Add panic unwind hook to alwqays restore terminal
    // Restore terminal
    restore_tui()?;
    terminal.show_cursor()?;

    if let Err(error) = app_result {
        ::tracing::error!(?error, "Application error");
    }

    Ok(())
}
pub fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = restore_tui();
        original_hook(panic_info);
    }));
}

pub fn init_tui() -> io::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

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
