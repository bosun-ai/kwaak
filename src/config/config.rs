use config::{Config as ConfigRs, Environment, File};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use swiftide::integrations::treesitter::SupportedLanguages;

use super::api_key::ApiKey;
use super::defaults::{
    default_auto_push_remote, default_cache_dir, default_docker_context, default_dockerfile,
    default_log_dir, default_main_branch, default_project_name,
};
use super::{CommandConfiguration, LLMConfiguration, LLMConfigurations};

// TODO: Improving parsing by enforcing invariants
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_project_name")]
    pub project_name: String,
    pub language: SupportedLanguages,
    pub llm: Box<LLMConfigurations>,
    pub commands: CommandConfiguration,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,

    /// The agent model to use by default in chats
    #[serde(default)]
    pub agent: SupportedAgentConfigurations,

    #[serde(default)]
    /// Concurrency for indexing
    /// By default for IO-bound LLMs, we assume 4x the number of CPUs
    /// For Ollama, it's the number of CPUs
    indexing_concurrency: Option<usize>,
    #[serde(default)]
    /// Batch size for indexing
    /// By default for IO-bound LLMs, we use a smaller batch size, as we can run it in parallel
    /// For local embeddings it's 256
    indexing_batch_size: Option<usize>,

    #[serde(default)]
    pub docker: DockerConfiguration,

    /// Optional: Backoff configuration for api calls
    /// this is currently only used for open-ai and open-router
    #[serde(default)]
    pub backoff: BackoffConfiguration,

    pub git: GitConfiguration,

    /// Optional: Use tavily as a search tool
    #[serde(default)]
    pub tavily_api_key: Option<ApiKey>,

    /// Optional: Use github for code search, creating pull requests, and automatic pushing to
    /// remotes
    #[serde(default)]
    pub github_api_key: Option<ApiKey>,

    /// Required if using `OpenAI`
    #[serde(default)]
    pub openai_api_key: Option<ApiKey>,

    /// Required if using 'Anthropic'
    #[serde(default)]
    pub anthropic_api_key: Option<ApiKey>,

    /// Required if using `Open Router`
    #[serde(default)]
    pub open_router_api_key: Option<ApiKey>,

    /// Required if using `Azure OpenAI`
    #[serde(default)]
    pub azure_openai_api_key: Option<ApiKey>,

    #[serde(default)]
    pub tool_executor: SupportedToolExecutors,

    #[serde(default)]
    pub disabled_tools: DisabledTools,

    /// By default the agent stops if the last message was its own and there are no new
    /// completions.
    ///
    /// When endless mode is enabled, the agent will keep running until it either cannot complete,
    /// did complete or was manually stopped.
    ///
    /// In addition, the agent is instructed that it cannot ask for feedback, but should try to
    /// complete its task instead.
    ///
    /// When running without a TUI, the agent will always run in endless mode.
    ///
    /// WARN: There currently is _no_ limit for endless mode
    #[serde(default)]
    pub endless_mode: bool,

    /// OpenTelemetry tracing feature toggle
    #[serde(default = "default_otel_enabled")]
    pub otel_enabled: bool,

    /// How the agent will edit files, defaults to whole
    #[serde(default)]
    pub agent_edit_mode: AgentEditMode,

    /// Additional constraints / instructions for the agent
    ///
    /// These are passes to the agent in the system prompt and are rendered in a list. If you
    /// intend to use more complicated instructions, consider adding a file to read in the
    /// repository instead.
    #[serde(default)]
    pub agent_custom_constraints: Option<Vec<String>>,

    #[serde(default)]
    pub ui: UIConfig,

    /// Number of completions before the agent summarizes the conversation.
    /// This is used to steer the agent to focus on the current task. If this value is too small
    /// the agent will have clear loss of context when performing tasks. If this value is too large
    /// the agent will not have focus and not understand what is relevant and important.
    ///
    /// Additionally, summarizing the conversation will reduce the context window which can be
    /// beneficial for APIs with stringent limits on context tokens.
    ///
    /// Defaults to 10.
    #[serde(default = "default_num_completions_for_summary")]
    pub num_completions_for_summary: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UIConfig {
    pub hide_header: bool,
}

fn default_otel_enabled() -> bool {
    false
}

fn default_num_completions_for_summary() -> usize {
    10
}

/// Opt out of certain tools an agent can use
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisabledTools {
    /// List of tool names to disable
    #[serde(default)]
    pub disabled_tools: Vec<String>,
}

impl DisabledTools {
    /// Checks if a tool is disabled
    #[must_use]
    pub fn is_tool_disabled(&self, tool_name: &str) -> bool {
        self.disabled_tools.iter().any(|name| name == tool_name)
    }
}

/// Agent session configurations supported by Kwaak
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub enum SupportedAgentConfigurations {
    /// Single looping agent that has all tools available
    #[default]
    #[serde(alias = "V1")]
    Coding,
    /// A two stage agent, starting with a planning agent that delegates to the coding
    /// agent
    PlanAct,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SupportedToolExecutors {
    #[default]
    Docker,
    Local,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default, strum_macros::EnumIs)]
#[serde(rename_all = "kebab-case")]
pub enum AgentEditMode {
    #[default]
    Whole,
    Line,
    // i.e. udiff, llm reviewed, etc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfiguration {
    #[serde(default = "default_dockerfile")]
    pub dockerfile: PathBuf,
    #[serde(default = "default_docker_context")]
    pub context: PathBuf,
}

impl Default for DockerConfiguration {
    fn default() -> Self {
        Self {
            dockerfile: "Dockerfile".into(),
            context: ".".into(),
        }
    }
}

/// Backoff configuration for api calls.
/// Each time an api call fails backoff will wait an increasing period of time for each subsequent
/// retry attempt. see <https://docs.rs/backoff/latest/backoff/> for more details.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BackoffConfiguration {
    /// Initial interval in seconds between retries
    pub initial_interval_sec: u64,
    /// The factor by which the interval is multiplied on each retry attempt
    pub multiplier: f64,
    /// Introduces randomness to avoid retry storms
    pub randomization_factor: f64,
    /// Total time all attempts are allowed in seconds. Once a retry must wait longer than this,
    /// the request is considered to have failed.
    pub max_elapsed_time_sec: u64,
}

impl Default for BackoffConfiguration {
    fn default() -> Self {
        Self {
            initial_interval_sec: 15,
            multiplier: 2.0,
            randomization_factor: 0.05,
            max_elapsed_time_sec: 120,
        }
    }
}

impl From<BackoffConfiguration> for backoff::ExponentialBackoff {
    fn from(from_backoff: BackoffConfiguration) -> Self {
        backoff::ExponentialBackoffBuilder::default()
            .with_initial_interval(Duration::from_secs(from_backoff.initial_interval_sec))
            .with_multiplier(from_backoff.multiplier)
            .with_randomization_factor(from_backoff.randomization_factor)
            .with_max_elapsed_time(Some(Duration::from_secs(from_backoff.max_elapsed_time_sec)))
            .build()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfiguration {
    // TODO: Repo and owner can probably be derived from the origin url
    // Personally would prefer an onboarding that prefils instead of inferring at runtime
    pub repository: Option<String>,
    pub owner: Option<String>,
    #[serde(default = "default_main_branch")]
    pub main_branch: String,

    /// Automatically push to the remote after every completion (if changes were made)
    #[serde(default = "default_auto_push_remote")]
    pub auto_push_remote: bool,

    /// Opt out of automatically committing changes after each completion
    #[serde(default)]
    pub auto_commit_disabled: bool,

    /// The git user name to use for the agent when committing changes
    #[serde(default = "default_agent_user_name")]
    pub agent_user_name: String,

    /// The git email to use for the agent when committing changes
    #[serde(default = "default_agent_user_email")]
    pub agent_user_email: String,

    /// Generate commit messages with an LLM; if disabled, the agent will use a default message
    #[serde(default = "default_generate_commit_message")]
    pub generate_commit_message: bool,
}

fn default_agent_user_name() -> String {
    "kwaak".to_string()
}

fn default_agent_user_email() -> String {
    "kwaak@bosun.ai".to_string()
}

fn default_generate_commit_message() -> bool {
    true
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        toml::from_str(s)
            .context("Failed to parse configuration")
            .and_then(Config::fill_llm_api_keys)
            .map(Config::add_project_name_to_paths)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let builder = ConfigRs::builder()
            .add_source(File::from(path))
            .add_source(File::with_name("kwaak.local").required(false))
            .add_source(Environment::with_prefix("KWAAK").separator("__"));

        let config = builder.build()?;

        config
            .try_deserialize()
            .map_err(Into::into)
            .and_then(Config::fill_llm_api_keys)
            .map(Config::add_project_name_to_paths)
    }

    // Seeds the api keys into the LLM configurations
    fn fill_llm_api_keys(mut self) -> Result<Self> {
        let previous = self.clone();

        let LLMConfigurations {
            indexing,
            embedding,
            query,
        } = &mut *self.llm;

        for config in &mut [indexing, embedding, query] {
            let maybe_root_key = previous.root_provider_api_key_for(config);
            fill_llm(config, maybe_root_key)?;
        }
        Ok(self)
    }

    fn add_project_name_to_paths(mut self) -> Self {
        if self.cache_dir.ends_with("kwaak") {
            self.cache_dir.push(&self.project_name);
        }
        if self.log_dir.ends_with("kwaak/logs") {
            self.log_dir.push(&self.project_name);
        }

        self
    }

    #[must_use]
    fn root_provider_api_key_for(&self, provider: &LLMConfiguration) -> Option<&ApiKey> {
        match provider {
            LLMConfiguration::OpenAI { .. } => self.openai_api_key.as_ref(),
            LLMConfiguration::OpenRouter { .. } => self.open_router_api_key.as_ref(),
            LLMConfiguration::Anthropic { .. } => self.anthropic_api_key.as_ref(),
            LLMConfiguration::AzureOpenAI { .. } => self.azure_openai_api_key.as_ref(),
            _ => None,
        }
    }

    #[must_use]
    pub fn indexing_provider(&self) -> &LLMConfiguration {
        let LLMConfigurations { indexing, .. } = &*self.llm;
        indexing
    }

    #[must_use]
    pub fn embedding_provider(&self) -> &LLMConfiguration {
        let LLMConfigurations { embedding, .. } = &*self.llm;
        embedding
    }

    #[must_use]
    pub fn query_provider(&self) -> &LLMConfiguration {
        let LLMConfigurations { query, .. } = &*self.llm;
        query
    }

    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        self.cache_dir.as_path()
    }

    #[must_use]
    pub fn log_dir(&self) -> &Path {
        self.log_dir.as_path()
    }

    #[must_use]
    pub fn indexing_concurrency(&self) -> usize {
        if let Some(concurrency) = self.indexing_concurrency {
            return concurrency;
        };

        match self.indexing_provider() {
            LLMConfiguration::OpenAI { .. } => num_cpus::get() * 4,
            LLMConfiguration::AzureOpenAI { .. } => num_cpus::get() * 4,
            LLMConfiguration::OpenRouter { .. } => num_cpus::get() * 4,
            LLMConfiguration::Ollama { .. } => num_cpus::get(),
            LLMConfiguration::FastEmbed { .. } => num_cpus::get(),
            LLMConfiguration::Anthropic { .. } => num_cpus::get() * 4,
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => num_cpus::get(),
        }
    }

    #[must_use]
    pub fn indexing_batch_size(&self) -> usize {
        if let Some(batch_size) = self.indexing_batch_size {
            return batch_size;
        };

        match self.indexing_provider() {
            LLMConfiguration::OpenAI { .. } => 12,
            LLMConfiguration::AzureOpenAI { .. } => 12,
            LLMConfiguration::Ollama { .. } => 256,
            LLMConfiguration::OpenRouter { .. } => 12,
            LLMConfiguration::FastEmbed { .. } => 256,
            LLMConfiguration::Anthropic { .. } => 12,
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => 1,
        }
    }

    #[must_use]
    pub fn is_github_enabled(&self) -> bool {
        self.github_api_key.is_some() && self.git.owner.is_some() && self.git.repository.is_some()
    }
}

fn fill_llm(llm: &mut LLMConfiguration, root_key: Option<&ApiKey>) -> Result<()> {
    match llm {
        LLMConfiguration::OpenAI { api_key, .. } => {
            // If the user omitted api_key in the config,
            // fill from the root-level openai_api_key if present.
            if api_key.is_none() {
                if let Some(root) = root_key {
                    *api_key = Some(root.clone());
                } else {
                    anyhow::bail!(
                        "OpenAI config requires an `api_key`, and none was provided or available in the root"
                    );
                }
            }
        }
        LLMConfiguration::AzureOpenAI { api_key, .. } => {
            if api_key.is_none() {
                if let Some(root) = root_key {
                    *api_key = Some(root.clone());
                } else {
                    anyhow::bail!(
                        "AzureOpenAI config requires an `api_key`, and none was provided or available in the root"
                    );
                }
            }
        }
        LLMConfiguration::Anthropic { api_key, .. } => {
            if api_key.is_none() {
                if let Some(root) = root_key {
                    *api_key = Some(root.clone());
                } else {
                    anyhow::bail!(
                        "Anthropic config requires an `api_key`, and none was provided or available in the root"
                    );
                }
            }
        }
        LLMConfiguration::OpenRouter { api_key, .. } => {
            if api_key.is_none() {
                if let Some(root) = root_key {
                    *api_key = Some(root.clone());
                } else {
                    anyhow::bail!(
                        "OpenRouter config requires an `api_key`, and none was provided or available in the root"
                    );
                }
            }
            // Nothing to do for OpenRouter
        }
        LLMConfiguration::Ollama { .. } | LLMConfiguration::FastEmbed { .. } => {
            // Nothing to do for Ollama / FastEmbed
        }
        #[cfg(debug_assertions)]
        LLMConfiguration::Testing => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(irrefutable_let_patterns)]
    use crate::config::{OpenAIEmbeddingModel, OpenAIPromptModel};

    use super::*;
    use swiftide::integrations::treesitter::SupportedLanguages;

    #[test]
    fn test_deserialize_toml_multiple() {
        let toml = r#"
            language = "rust"

            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"

            [git]
            owner = "bosun-ai"
            repository = "kwaak"

            [llm.indexing]
            provider = "OpenAI"
            api_key = "text:test-key"
            prompt_model = "gpt-4o-mini"

            [llm.query]
            provider = "OpenAI"
            api_key = "text:other-test-key"
            prompt_model = "gpt-4o-mini"

            [llm.embedding]
            provider = "OpenAI"
            api_key = "text:other-test-key"
            embedding_model = "text-embedding-3-small"
            "#;

        let config: Config = Config::from_str(toml).unwrap();
        assert_eq!(config.language, SupportedLanguages::Rust);

        if let LLMConfigurations {
            indexing,
            embedding,
            query,
        } = &*config.llm
        {
            if let LLMConfiguration::OpenAI {
                api_key,
                prompt_model,
                ..
            } = indexing
            {
                assert_eq!(api_key.as_ref().unwrap().expose_secret(), "test-key");
                assert_eq!(prompt_model, &OpenAIPromptModel::GPT4OMini);
            } else {
                panic!("Expected OpenAI configuration for indexing");
            }

            if let LLMConfiguration::OpenAI {
                api_key,
                prompt_model,
                ..
            } = query
            {
                assert_eq!(api_key.as_ref().unwrap().expose_secret(), "other-test-key");
                assert_eq!(prompt_model, &OpenAIPromptModel::GPT4OMini);
            } else {
                panic!("Expected OpenAI configuration for query");
            }

            if let LLMConfiguration::OpenAI {
                api_key,
                embedding_model,
                ..
            } = embedding
            {
                assert_eq!(api_key.as_ref().unwrap().expose_secret(), "other-test-key");
                assert_eq!(embedding_model, &OpenAIEmbeddingModel::TextEmbedding3Small);
            }
        } else {
            panic!("Expected multiple LLM configurations");
        }

        // Verify default otel_enabled
        assert!(!config.otel_enabled);
    }

    #[test]
    fn test_seed_openai_api_key_from_root_multiple_with_overwrite() {
        let toml = r#"
            language = "rust"

            openai_api_key = "text:root-api-key"

            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"

            [git]
            owner = "bosun-ai"
            repository = "kwaak"

            [llm.indexing]
            provider = "OpenAI"
            prompt_model = "gpt-4o-mini"

            [llm.query]
            provider = "OpenAI"
            api_key = "text:child-api-key"
            prompt_model = "gpt-4o-mini"

            [llm.embedding]
            provider = "OpenAI"
            embedding_model = "text-embedding-3-small"
        "#;

        let config: Config = Config::from_str(toml).unwrap();

        let LLMConfiguration::OpenAI { api_key, .. } = config.indexing_provider() else {
            panic!("Expected OpenAI configuration for indexing")
        };

        assert_eq!(
            api_key.as_ref().unwrap().expose_secret(),
            config.openai_api_key.as_ref().unwrap().expose_secret()
        );

        let LLMConfiguration::OpenAI { api_key, .. } = config.query_provider() else {
            panic!("Expected OpenAI configuration for indexing")
        };

        assert_eq!(api_key.as_ref().unwrap().expose_secret(), "child-api-key");
    }

    #[test]
    fn test_add_project_name_to_paths() {
        let toml = r#"
            language = "rust"
            project_name = "test"

            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"

            [git]
            owner = "bosun-ai"
            repository = "kwaak"

            [llm.indexing]
            provider = "OpenAI"
            api_key = "text:test-key"
            prompt_model = "gpt-4o-mini"

            [llm.query]
            provider = "OpenAI"
            api_key = "text:other-test-key"
            prompt_model = "gpt-4o-mini"

            [llm.embedding]
            provider = "OpenAI"
            api_key = "text:other-test-key"
            embedding_model = "text-embedding-3-small"
            "#;

        let config: Config = Config::from_str(toml).unwrap();

        assert!(config.cache_dir.ends_with("kwaak/test"));
        assert!(config.log_dir().ends_with("kwaak/logs/test"));
    }
    #[test]
    fn test_disabled_tools() {
        // Test with disabled_tools list
        let mut disabled_tools = DisabledTools::default();
        disabled_tools.disabled_tools = vec!["git".to_string(), "shell_command".to_string()];
        
        // Tools explicitly in the list should be disabled
        assert!(disabled_tools.is_tool_disabled("git"));
        assert!(disabled_tools.is_tool_disabled("shell_command"));
        
        // Tools not in the list should not be disabled
        assert!(!disabled_tools.is_tool_disabled("write_file"));
        assert!(!disabled_tools.is_tool_disabled("read_file"));
    }

    #[test]
    fn test_deserialize_disabled_tools_list() {
        let toml = r#"
            language = "rust"
            
            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"
            
            
            [disabled_tools]
            # For tuple struct, the vector is the unnamed field
            disabled_tools = ["git", "shell_command", "write_file"]
            
            [llm.indexing]
            provider = "OpenAI"
            api_key = "text:test-key"
            prompt_model = "gpt-4o-mini"
            
            [llm.query]
            provider = "OpenAI"
            api_key = "text:test-key"
            prompt_model = "gpt-4o-mini"
            
            [llm.embedding]
            provider = "OpenAI"
            api_key = "text:test-key"
            embedding_model = "text-embedding-3-small"
            
            [git]
            repository = "kwaak"
            owner = "bosun-ai"
        "#;
        
        let config: Config = Config::from_str(toml).unwrap();
        
        // Verify the disabled_tools list is correctly parsed
        assert_eq!(config.disabled_tools.0.len(), 3);
        assert!(config.disabled_tools.0.contains(&"git".to_string()));
        assert!(config.disabled_tools.0.contains(&"shell_command".to_string()));
        assert!(config.disabled_tools.0.contains(&"write_file".to_string()));
        
        // Verify the is_tool_disabled method works as expected with the parsed config
        assert!(config.disabled_tools.is_tool_disabled("git"));
        assert!(config.disabled_tools.is_tool_disabled("shell_command"));
        assert!(config.disabled_tools.is_tool_disabled("write_file"));
        assert!(!config.disabled_tools.is_tool_disabled("read_file"));
    }
    #[test]
    fn test_serialize_disabled_tools() {
        // Create a DisabledTools with some values
        let disabled_tools = DisabledTools(vec!["git".to_string(), "shell_command".to_string()]);
        
        // Serialize it to a string
        let serialized = toml::to_string(&disabled_tools).unwrap();
        
        println!("Serialized DisabledTools: {}", serialized);
        
        // Deserialize it back
        let deserialized: DisabledTools = toml::from_str(&serialized).unwrap();
        
        // Check that the values match
        assert_eq!(deserialized.0.len(), 2);
        assert!(deserialized.0.contains(&"git".to_string()));
        assert!(deserialized.0.contains(&"shell_command".to_string()));
    }
}
