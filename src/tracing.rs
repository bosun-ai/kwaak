use crate::repository::Repository;
use anyhow::Result;
use log::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

// For now just log to stdout and file
pub fn init(repository: &Repository) -> Result<()> {
    let log_dir = repository.config().log_dir();

    let file_appender = tracing_appender::rolling::never(
        log_dir,
        format!("{}.log", repository.config().project_name),
    );

    let fmt_layer = fmt::layer().with_writer(file_appender);

    // Logs the file layer will capture
    let env_filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("kwaak=info,swiftide=info,error"))?;

    let tui_layer = tui_logger::tracing_subscriber_layer();

    // The log level tui logger will capture
    let default_level = if cfg!(debug_assertions) {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };
    tui_logger::init_logger(default_level)?;
    tui_logger::set_default_level(default_level);

    tracing_subscriber::registry()
        .with(tui_layer)
        .with(env_filter_layer)
        .with(fmt_layer)
        .try_init()?;

    Ok(())
}
