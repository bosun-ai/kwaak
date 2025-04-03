use serde::{Deserialize, Serialize};
use swiftide::agents::tools::mcp::ToolFilter;

use super::ApiKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServer {
    /// Spawns the mcp server in a sub process
    SubProcess {
        /// The name of the mcp server
        name: String,
        /// The main command to run
        command: String,
        /// Any arguments to pass to the the command
        #[serde(default)]
        args: Vec<String>,
        /// Any filters to apply to the tools
        #[serde(default, with = "opt_ext_tool_filter")]
        filter: Option<ToolFilter>,
        #[serde(default)]
        /// Any environment variables to set
        env: Option<Vec<(String, ApiKey)>>,
    },
}

// Wraps the swiftide tool filter so we can control how it is serialized
//
// In toml it looks like this:
//
// ```toml
// filter = { type = "whitelist", tool_names = ["tool1", "tool2"] }
//
// # or
//
// [mcp.filter]
// type = "whitelist"
// tool_names = ["tool1", "tool2"]
// ```
mod opt_ext_tool_filter {
    use super::ToolFilter;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(
        remote = "ToolFilter",
        tag = "type",
        content = "tool_names",
        rename_all = "snake_case"
    )]
    pub enum ToolFilterDef {
        Whitelist(Vec<String>),
        Blacklist(Vec<String>),
    }

    #[allow(clippy::ref_option, reason = "not with serde")]
    pub fn serialize<S>(value: &Option<ToolFilter>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Helper<'a>(#[serde(with = "ToolFilterDef")] &'a ToolFilter);

        value.as_ref().map(Helper).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<ToolFilter>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(with = "ToolFilterDef")] ToolFilter);

        let helper = Option::deserialize(deserializer)?;
        Ok(helper.map(|Helper(external)| external))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml; // Make sure to include `toml` in your `Cargo.toml` dependencies

    #[test]
    fn test_deserialize_subprocess_basic() {
        let toml_data = r#"
            name = "test_server"
            command = "run"
        "#;

        let server: McpServer = toml::from_str(toml_data).expect("Deserialization failed");

        match server {
            McpServer::SubProcess {
                name,
                command,
                args,
                filter,
                env,
            } => {
                assert_eq!(name, "test_server");
                assert_eq!(command, "run");
                assert!(args.is_empty());
                assert!(filter.is_none());
                assert!(env.is_none());
            }
        }
    }

    #[test]
    fn test_deserialize_subprocess_with_args() {
        let toml_data = r#"
            name = "test_server"
            command = "run"
            args = ["--verbose", "--debug"]
        "#;

        let server: McpServer = toml::from_str(toml_data).expect("Deserialization failed");

        match server {
            McpServer::SubProcess {
                name,
                command,
                args,
                ..
            } => {
                assert_eq!(name, "test_server");
                assert_eq!(command, "run");
                assert_eq!(args, vec!["--verbose".to_string(), "--debug".to_string()]);
            }
        }
    }

    #[test]
    fn test_deserialize_subprocess_with_filter() {
        let toml_data = r#"
            name = "test_server"
            command = "run"
            
            [filter]
            type = "whitelist"
            tool_names = ["tool1", "tool2"]
        "#;

        let server: McpServer = toml::from_str(toml_data).expect("Deserialization failed");

        if let McpServer::SubProcess {
            filter: Some(ToolFilter::Whitelist(opts)),
            ..
        } = server
        {
            assert_eq!(opts, vec!["tool1".to_string(), "tool2".to_string()]);
        } else {
            panic!("Expected McpServer::SubProcess with filter");
        }
    }
}
