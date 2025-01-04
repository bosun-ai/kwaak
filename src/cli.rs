use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(author, about, version)]
pub struct Args {
    /// Optional path to overwrite the config
    #[arg(short, long, default_value = "kwaak.toml")]
    pub config_path: PathBuf,

    /// Print the configuration and exit
    #[arg(long)]
    pub print_config: bool,

    /// Clear the index and cache for this project and exit
    #[arg(long, name = "clear-cache", default_value_t = false)]
    pub clear_cache: bool,

    /// Initializes a new kwaak project in the current directory
    #[arg(long, default_value_t = false)]
    pub init: bool,

    /// Skip initial indexing and splash screen
    #[arg(short, long, default_value_t = false)]
    pub skip_indexing: bool,

    /// Subcommands corresponding to each mode
    #[clap(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone, Default)]
pub enum Commands {
    /// Start the TUI (default)
    #[default]
    Tui,
    /// Query the indexed project
    Query {
        #[arg(short, long)]
        query: String,
    },
    /// Run an agent directly
    RunAgent {
        #[arg(short, long)]
        initial_message: String,
    },
    /// Index the current project
    Index,
    /// Tests a tool
    TestTool {
        tool_name: String,
        #[arg()]
        tool_args: Option<String>,
    },
}
