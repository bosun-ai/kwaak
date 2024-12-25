use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use swiftide::integrations::treesitter::SupportedLanguages;

use super::api_key::ApiKey;
use super::defaults::{
    default_cache_dir, default_docker_context, default_dockerfile, default_log_dir,
    default_main_branch, default_project_name,
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
    cache_dir: PathBuf,
    #[serde(default = "default_log_dir")]
    log_dir: PathBuf,

    #[serde(default)]
    pub docker: DockerConfiguration,

    pub github: GithubConfiguration,

    /// Optional: Use tavily as a search tool
    #[serde(default)]
    pub tavily_api_key: Option<ApiKey>,

    #[serde(default)]
    pub tool_executor: SupportedToolExecutors,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SupportedToolExecutors {
    #[default]
    Docker,
    Local,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfiguration {
    // TODO: Repo and owner can probably be derived from the origin url
    // Personally would prefer an onboarding that prefils instead of inferring at runtime
    pub repository: String,
    pub owner: String,
    #[serde(default = "default_main_branch")]
    pub main_branch: String,

    pub token: Option<ApiKey>,
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("Failed to parse configuration")
    }
}

impl Config {
    /// Loads the configuration file from the current path
    pub(crate) async fn load(path: impl AsRef<Path>) -> Result<Config> {
        let file = tokio::fs::read(path)
            .await
            .context("Could not find `kwaak.toml` in current directory")?;

        toml::from_str(std::str::from_utf8(&file)?).context("Failed to parse configuration")
    }

    pub fn indexing_provider(&self) -> &LLMConfiguration {
        match &*self.llm {
            LLMConfigurations::Single(config) => config,
            LLMConfigurations::Multiple { indexing, .. } => indexing,
        }
    }

    pub fn embedding_provider(&self) -> &LLMConfiguration {
        match &*self.llm {
            LLMConfigurations::Single(config) => config,
            LLMConfigurations::Multiple { embedding, .. } => embedding,
        }
    }

    pub fn query_provider(&self) -> &LLMConfiguration {
        match &*self.llm {
            LLMConfigurations::Single(config) => config,
            LLMConfigurations::Multiple { query, .. } => query,
        }
    }

    pub fn cache_dir(&self) -> &Path {
        self.cache_dir.as_path()
    }

    pub fn log_dir(&self) -> &Path {
        self.log_dir.as_path()
    }
}

#[cfg(test)]
mod tests {
    #![allow(irrefutable_let_patterns)]
    use crate::config::{OpenAIEmbeddingModel, OpenAIPromptModel};

    use super::*;
    use swiftide::integrations::treesitter::SupportedLanguages;

    #[test]
    fn test_deserialize_toml_single() {
        let toml = r#"
            language = "rust"

            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"

            [github]
            owner = "bosun-ai"
            repository = "kwaak"
            token = "text:some-token"

            [llm]
            provider = "OpenAI"
            api_key = "text:test-key"
            prompt_model = "gpt-4o-mini"

            "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.language, SupportedLanguages::Rust);

        if let LLMConfigurations::Single(LLMConfiguration::OpenAI {
            api_key,
            prompt_model,
            ..
        }) = &*config.llm
        {
            assert_eq!(api_key.expose_secret(), "test-key");
            assert_eq!(prompt_model, &OpenAIPromptModel::GPT4OMini);
        } else {
            panic!("Expected single OpenAI configuration");
        }
    }

    #[test]
    fn test_deserialize_toml_multiple() {
        let toml = r#"
            language = "rust"

            [commands]
            test = "cargo test"
            coverage = "cargo tarpaulin"

            [github]
            owner = "bosun-ai"
            repository = "kwaak"
            token = "text:some-token"

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

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.language, SupportedLanguages::Rust);

        if let LLMConfigurations::Multiple {
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
                assert_eq!(api_key.expose_secret(), "test-key");
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
                assert_eq!(api_key.expose_secret(), "other-test-key");
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
                assert_eq!(api_key.expose_secret(), "other-test-key");
                assert_eq!(embedding_model, &OpenAIEmbeddingModel::TextEmbedding3Small);
            }
        } else {
            panic!("Expected multiple LLM configurations");
        }
    }
}
