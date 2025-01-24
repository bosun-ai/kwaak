use config::{Config as ConfigRs, Environment, File, FileSourceFile};
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
    pub cache_dir: PathBuf,
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,

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

    #[serde(default)]
    pub tool_executor: SupportedToolExecutors,

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
}

fn default_otel_enabled() -> bool {
    false
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
pub struct GitConfiguration {
    // TODO: Repo and owner can probably be derived from the origin url
    // Personally would prefer an onboarding that prefils instead of inferring at runtime
    pub repository: Option<String>,
    pub owner: Option<String>,
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        toml::from_str(s)
            .context("Failed to parse configuration")
            .and_then(Config::fill_llm_api_keys)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let builder = ConfigRs::builder()
            .add_source(File::from(path))
            .add_source(File::with_name("kwaak.local").required(false))
            .add_source(
                Environment::with_prefix("KWAAK")
                    .separator("_")
                    .convert_case(config::Case::Lower),
            );

        let config = builder.build()?;

        config.try_deserialize().map_err(Into::into) // Here using serde to deserialize into Self
    }

    // Seeds the api keys into the LLM configurations
    pub fn fill_llm_api_keys(mut self) -> Result<Self> {
        let LLMConfigurations {
            indexing,
            embedding,
            query,
        } = &mut *self.llm;
        {
            fill_llm(indexing, self.openai_api_key.as_ref())?;
            fill_llm(embedding, self.openai_api_key.as_ref())?;
            fill_llm(query, self.openai_api_key.as_ref())?;
        }
        Ok(self)
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
            LLMConfiguration::Ollama { .. } => num_cpus::get(),
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
            LLMConfiguration::Ollama { .. } => 256,
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
                    anyhow::bail!("OpenAI config requires an `api_key`, and none was provided or available in the root");
                }
            }
        }
        LLMConfiguration::Ollama { .. } => {
            // Nothing to do for Ollama
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
}
