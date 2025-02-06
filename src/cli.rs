use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(author, about, version)]
pub struct Args {
    /// Optional path to overwrite the config
    #[arg(short, long, default_value = "kwaak.toml")]
    pub config_path: PathBuf,

    /// Skip initial indexing and splash screen
    #[arg(short, long, default_value_t = false)]
    pub skip_indexing: bool,

    /// Allow running with a dirty git directory
    #[arg(long, default_value_t = false)]
    pub allow_dirty: bool,

    /// Subcommands corresponding to each mode
    #[clap(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone, Default)]
pub enum Commands {
    /// Initializes a new kwaak project in the current directory
    Init {
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Output to a specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },
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
        tool_args: Option<String>,
    },
    /// Print the configuration and exit
    PrintConfig,
    /// Clear the index and cache for this project and exit
    ClearCache,
}
