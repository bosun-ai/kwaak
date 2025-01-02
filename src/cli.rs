use std::path::PathBuf;

use clap::Parser;

#[allow(clippy::struct_excessive_bools)]
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

    /// When querying the indexed project, the query to run
    #[arg(short, long, required_if_eq("mode", "query"))]
    pub query: Option<String>,

    /// Print the configuration and exit
    #[arg(long)]
    pub print_config: bool,

    /// Clear the the index and cache for this project and exit
    #[arg(long, name = "clear-cache", default_value_t = false)]
    pub clear_cache: bool,

    /// Initializes a new kwaak project in the current directory
    #[arg(long, default_value_t = false)]
    pub init: bool,

    /// Skip initial indexing and splash screen
    #[arg(short, long, default_value_t = false)]
    pub skip_indexing: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, Default, strum_macros::AsRefStr)]
pub enum ModeArgs {
    /// Index the current project
    Index,
    /// Query the indexed project
    Query,
    /// Run an agent directly
    RunAgent,
    /// Start the TUI
    #[default]
    Tui,
}
