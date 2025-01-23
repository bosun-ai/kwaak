//! Configuration for commands that tools can use to operate on the project.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandConfiguration {
    /// Optional: Enables the agent to run the tests
    pub test: Option<String>,
    /// Optional: Enables the agent to retrieve coverage information (it should print, preferably
    /// concise, output to stdout)
    pub coverage: Option<String>,
    /// Optional: Lint and fix the project
    /// This command is run if any files were written to the project.
    ///
    /// i.e. in Rust `cargo clippy --fix --allow-dirty --allow-staged && cargo fmt`
    #[serde(default)]
    pub lint_and_fix: Option<String>,
}
