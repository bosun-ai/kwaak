use std::io;

use anyhow::Result;
use app::{run_app, App};
use config::Config;
use ratatui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

mod app;
mod chat_message;
mod commands;
mod config;
mod indexing;
mod repository;
mod storage;
mod tracing;
mod ui;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load().await?;
    let repository = repository::Repository::from_config(config);

    std::fs::create_dir_all(repository.config().cache_dir())?;
    std::fs::create_dir_all(repository.config().log_dir())?;

    crate::tracing::init(&repository)?;

    ::tracing::info!("Loaded configuration: {:?}", repository.config());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start the application
    let mut app = App::default();
    let _guard = commands::CommandHandler::start_with_ui_app(&mut app, repository);

    let res = run_app(&mut app, &mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}