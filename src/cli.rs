use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, about, version)]
pub struct Args {
    /// Optional path to overwrite the config
    #[arg(short, long, default_value = "kwaak.toml")]
    pub config_path: PathBuf,

    /// Run kwaak as a tui (default) or run an agent directly
    #[arg(short, long, default_value = "tui")]
    pub mode: ModeArgs,
    /// When running the agent directly, the initial message to send to the agent
    #[arg(short, long, required_if_eq("mode", "run-agent"))]
    pub initial_message: Option<String>,

    /// Print the configuration and exit
    #[arg(long)]
    pub print_config: bool,

    /// Clear the the index and cache for this project and exit
    #[arg(long, name = "clear-cache", default_value_t = false)]
    pub clear_cache: bool,

    /// Initializes a new kwaak project in the current directory
    #[arg(long, default_value_t = false)]
    pub init: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum ModeArgs {
    RunAgent,
    #[default]
    Tui,
}
