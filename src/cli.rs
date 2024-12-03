use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, about, version)]
pub struct Args {
    /// Optional path to overwrite the config
    #[arg(short, long, default_value = "kwaak.toml")]
    pub config_path: PathBuf,

    /// Run kwaak as a tui (default) or run an agent directly
    #[arg(short, long, default_value_t = ModeArgs::Tui)]
    pub mode: ModeArgs,
    /// When running the agent directly, the initial message to send to the agent
    #[arg(short, long, required_if_eq("mode", "run-agent"))]
    pub initial_message: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug, Default, strum::Display, strum::EnumString)]
pub enum ModeArgs {
    RunAgent,
    #[default]
    Tui,
}
