use serde::{Deserialize, Serialize};
// TODO:
//
// Support whitelists/blacklists
// Error handling?
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum McpTool {
    /// Spawns the mcp server in a sub process
    Process { cmd: String },
    // Sse support, need to consider auth, etc
    // Url(url)
}
