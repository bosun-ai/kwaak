use std::{str::FromStr, time::Duration};

#[cfg(debug_assertions)]
use crate::test_utils::NoopLLM;

use super::ApiKey;
use crate::config::BackoffConfiguration;
use anyhow::{Context as _, Result};
use backoff::ExponentialBackoffBuilder;
use fastembed::{InitOptions, TextEmbedding};
use serde::{Deserialize, Serialize};
use swiftide::{
    chat_completion::ChatCompletion,
    integrations::{
        self,
        anthropic::Anthropic,
        fastembed::FastEmbed,
        ollama::{config::OllamaConfig, Ollama},
        open_router::{config::OpenRouterConfig, OpenRouter},
    },
    traits::{EmbeddingModel, SimplePrompt},
};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfigurations {
    pub indexing: LLMConfiguration,
    pub embedding: LLMConfiguration,
    pub query: LLMConfiguration,
}

// Custom deserialize for LLMConfigurations so it gives better errors (i.e. on partial match llm
// configuration or missing 'query' from multiple)

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    strum_macros::EnumString,
    strum_macros::VariantNames,
    strum_macros::Display,
)]
#[serde(tag = "provider")]
#[strum(ascii_case_insensitive)]
pub enum LLMConfiguration {
    OpenAI {
        api_key: Option<ApiKey>,
        #[serde(default)]
        prompt_model: OpenAIPromptModel,
        #[serde(default)]
        embedding_model: OpenAIEmbeddingModel,
        #[serde(default)]
        base_url: Option<Url>,
    },
    Ollama {
        #[serde(default)]
        prompt_model: Option<String>,
        #[serde(default)]
        embedding_model: Option<EmbeddingModelWithSize>,
        #[serde(default)]
        base_url: Option<Url>,
    },
    OpenRouter {
        #[serde(default)]
        api_key: Option<ApiKey>,
        #[serde(default)]
        prompt_model: String,
    },
    FastEmbed {
        // TODO: Currently we only support the default. There is a PR open that adds
        // serialize/deserialize to all models, making a proper setup a lot easier.
        embedding_model: FastembedModel,
    },
    Anthropic {
        api_key: Option<ApiKey>,
        prompt_model: AnthropicModel,
    },
    #[cfg(debug_assertions)]
    Testing, // Groq {
             //     api_key: SecretString,
             //     prompt_model: String,
             // },
             // AWSBedrock {
             //     prompt_model: String,
             // },
             // FastEmbed {
             //     embedding_model: String,
             //     vector_size: usize,
             // },
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    PartialEq,
    strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::VariantNames,
    Default,
)]
pub enum AnthropicModel {
    #[strum(serialize = "claude-3-5-sonnet-latest")]
    #[serde(rename = "claude-3-5-sonnet-latest")]
    #[default]
    Claude35Sonnet,
    #[strum(serialize = "claude-3-5-haiku-latest")]
    #[serde(rename = "claude-3-5-haiku-latest")]
    Claude35Haiku,
}

#[derive(Debug, Clone)]
pub struct FastembedModel(fastembed::EmbeddingModel);

impl FastembedModel {
    /// Retrieves the vector size of the fastembed model
    ///
    /// # Panics
    ///
    /// Panics if it cannot get the model info from fastembed.
    #[must_use]
    pub fn vector_size(&self) -> i32 {
        i32::try_from(
            TextEmbedding::get_model_info(&self.0)
                .expect("Could not get model info")
                .dim,
        )
        .expect(
            "Could not convert embedding dimensions to i32; this should never happen and is a bug",
        )
    }

    pub fn inner_text_embedding(&self) -> Result<TextEmbedding> {
        TextEmbedding::try_new(
            InitOptions::new(self.0.clone())
                .with_show_download_progress(false)
                .with_cache_dir(dirs::cache_dir().unwrap_or_default()),
        )
    }

    #[must_use]
    pub fn list_supported_models() -> Vec<String> {
        TextEmbedding::list_supported_models()
            .iter()
            .map(|model| model.model_code.to_string())
            .collect()
    }
}

impl Default for FastembedModel {
    fn default() -> Self {
        Self(fastembed::EmbeddingModel::BGESmallENV15)
    }
}

impl std::fmt::Display for FastembedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let model = TextEmbedding::get_model_info(&self.0)
            .map_err(|_| std::fmt::Error)?
            .model_code
            .clone();

        write!(f, "{model}")
    }
}

/// Currently just does default, as soon as Fastembed has serialize/deserialize support we can do a
/// proper lookup
impl<'de> Deserialize<'de> for FastembedModel {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string_name: String = Deserialize::deserialize(deserializer)?;
        let models = TextEmbedding::list_supported_models();

        let retrieved_model = models
            .iter()
            .find(|model| model.model_code.to_lowercase() == string_name.to_lowercase())
            .ok_or(serde::de::Error::custom("Could not find model"))?;

        Ok(FastembedModel(retrieved_model.model.clone()))
    }
}

impl FromStr for FastembedModel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let models = TextEmbedding::list_supported_models();

        let retrieved_model = models
            .iter()
            .find(|model| model.model_code.to_lowercase() == s.to_lowercase())
            .ok_or_else(|| anyhow::anyhow!("Could not find model"))?;

        Ok(FastembedModel(retrieved_model.model.clone()))
    }
}

/// Currently just serializes to the default
impl Serialize for FastembedModel {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let model = TextEmbedding::get_model_info(&self.0)
            .map_err(|_| serde::ser::Error::custom("Could not find model"))?;

        serializer.serialize_str(&model.model_code)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingModelWithSize {
    pub name: String,
    pub vector_size: i32,
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    PartialEq,
    strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::VariantNames,
    Default,
)]
pub enum OpenAIPromptModel {
    #[strum(serialize = "gpt-4o-mini")]
    #[serde(rename = "gpt-4o-mini")]
    #[default]
    GPT4OMini,
    #[strum(serialize = "gpt-4o")]
    #[serde(rename = "gpt-4o")]
    GPT4O,
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::VariantNames,
    PartialEq,
    Default,
)]
pub enum OpenAIEmbeddingModel {
    #[strum(serialize = "text-embedding-3-small")]
    #[serde(rename = "text-embedding-3-small")]
    TextEmbedding3Small,
    #[strum(serialize = "text-embedding-3-large")]
    #[serde(rename = "text-embedding-3-large")]
    #[default]
    TextEmbedding3Large,
}

impl LLMConfiguration {
    pub(crate) fn vector_size(&self) -> i32 {
        match self {
            LLMConfiguration::OpenAI {
                embedding_model, ..
            } => match embedding_model {
                OpenAIEmbeddingModel::TextEmbedding3Small => 1536,
                OpenAIEmbeddingModel::TextEmbedding3Large => 3072,
            },
            LLMConfiguration::Ollama {
                embedding_model, ..
            } => {
                embedding_model
                    .as_ref()
                    .expect("Expected an embedding model for ollama")
                    .vector_size
            }
            LLMConfiguration::OpenRouter { .. } => {
                panic!("OpenRouter does not have an embedding model")
            }
            LLMConfiguration::FastEmbed { embedding_model } => embedding_model.vector_size(),

            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => 1,
            LLMConfiguration::Anthropic { .. } => {
                panic!("Anthropic does not have an embedding model")
            }
        }
    }

    fn build_openai(
        &self,
        backoff_config: BackoffConfiguration,
    ) -> Result<integrations::openai::OpenAI> {
        let LLMConfiguration::OpenAI {
            api_key,
            embedding_model,
            prompt_model,
            base_url,
        } = self
        else {
            anyhow::bail!("Expected Ollama configuration")
        };

        let api_key = api_key.as_ref().context("Expected an api key")?;

        let mut config =
            async_openai::config::OpenAIConfig::default().with_api_key(api_key.expose_secret());

        if let Some(base_url) = base_url {
            config = config.with_api_base(base_url.to_string());
        };

        // Load backoff settings from configuration
        let backoff = ExponentialBackoffBuilder::default()
            .with_initial_interval(Duration::from_secs(backoff_config.initial_interval_sec))
            .with_multiplier(backoff_config.multiplier)
            .with_randomization_factor(backoff_config.randomization_factor)
            .with_max_elapsed_time(Some(Duration::from_secs(
                backoff_config.max_elapsed_time_sec,
            )))
            .build();

        let client = async_openai::Client::with_config(config).with_backoff(backoff);

        integrations::openai::OpenAI::builder()
            .client(client)
            .default_prompt_model(prompt_model.to_string())
            .default_embed_model(embedding_model.to_string())
            .build()
            .context("Failed to build OpenAI client")
    }

    fn build_ollama(&self) -> Result<Ollama> {
        let LLMConfiguration::Ollama {
            prompt_model,
            embedding_model,
            base_url,
            ..
        } = self
        else {
            anyhow::bail!("Expected Ollama configuration")
        };

        let mut config = OllamaConfig::default();

        if let Some(base_url) = base_url {
            config.with_api_base(base_url.as_str());
        };

        let mut builder = Ollama::builder()
            .client(async_openai::Client::with_config(config))
            .to_owned();

        if let Some(embedding_model) = embedding_model {
            builder.default_embed_model(embedding_model.name.clone());
        }

        if let Some(prompt_model) = prompt_model {
            builder.default_prompt_model(prompt_model);
        }

        builder.build().context("Failed to build Ollama client")
    }

    fn build_anthropic(&self) -> Result<Anthropic> {
        let LLMConfiguration::Anthropic {
            api_key,
            prompt_model,
        } = self
        else {
            anyhow::bail!("Expected Anthropic configuration")
        };

        let api_key = api_key.as_ref().context("Expected an api key")?;
        let client = async_anthropic::Client::from_api_key(api_key);

        Anthropic::builder()
            .client(client)
            .default_prompt_model(prompt_model.to_string())
            .build()
            .context("Failed to build Anthropic client")
    }

    fn build_open_router(&self, backoff_config: BackoffConfiguration) -> Result<OpenRouter> {
        let LLMConfiguration::OpenRouter {
            prompt_model,
            api_key,
        } = self
        else {
            anyhow::bail!("Expected OpenRouter configuration")
        };

        let api_key = api_key.as_ref().context("Expected an api key")?;
        let config = OpenRouterConfig::builder()
            .api_key(api_key)
            .site_url("https://github.com/bosun-ai/kwaak")
            .site_name("Kwaak")
            .build()?;

        // Load backoff settings from configuration
        let backoff = ExponentialBackoffBuilder::default()
            .with_initial_interval(Duration::from_secs(backoff_config.initial_interval_sec))
            .with_multiplier(backoff_config.multiplier)
            .with_randomization_factor(backoff_config.randomization_factor)
            .with_max_elapsed_time(Some(Duration::from_secs(
                backoff_config.max_elapsed_time_sec,
            )))
            .build();

        let client = async_openai::Client::with_config(config).with_backoff(backoff);

        OpenRouter::builder()
            .client(client)
            .default_prompt_model(prompt_model)
            .to_owned()
            .build()
            .context("Failed to build OpenRouter client")
    }

    pub fn get_embedding_model(
        &self,
        backoff_config: BackoffConfiguration,
    ) -> Result<Box<dyn EmbeddingModel>> {
        let boxed = match self {
            LLMConfiguration::OpenAI { .. } => {
                Box::new(self.build_openai(backoff_config)?) as Box<dyn EmbeddingModel>
            }
            LLMConfiguration::Ollama { .. } => {
                Box::new(self.build_ollama()?) as Box<dyn EmbeddingModel>
            }
            LLMConfiguration::OpenRouter { .. } => {
                anyhow::bail!("OpenRouter does not have an embedding model")
            }
            LLMConfiguration::FastEmbed { embedding_model } => Box::new(
                FastEmbed::builder()
                    .embedding_model(embedding_model.inner_text_embedding()?)
                    .build()?,
            )
                as Box<dyn EmbeddingModel>,

            LLMConfiguration::Anthropic { .. } => {
                anyhow::bail!("Anthropic does not have an embedding model")
            }

            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn EmbeddingModel>,
        };
        Ok(boxed)
    }

    pub fn get_simple_prompt_model(
        &self,
        backoff_config: BackoffConfiguration,
    ) -> Result<Box<dyn SimplePrompt>> {
        let boxed = match self {
            LLMConfiguration::OpenAI { .. } => {
                Box::new(self.build_openai(backoff_config)?) as Box<dyn SimplePrompt>
            }
            LLMConfiguration::Ollama { .. } => {
                Box::new(self.build_ollama()?) as Box<dyn SimplePrompt>
            }
            LLMConfiguration::OpenRouter { .. } => {
                Box::new(self.build_open_router(backoff_config)?) as Box<dyn SimplePrompt>
            }
            LLMConfiguration::FastEmbed { .. } => {
                anyhow::bail!("FastEmbed does not have a simpl prompt model")
            }
            LLMConfiguration::Anthropic { .. } => {
                Box::new(self.build_anthropic()?) as Box<dyn SimplePrompt>
            }
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn SimplePrompt>,
        };
        Ok(boxed)
    }

    pub fn get_chat_completion_model(
        &self,
        backoff_config: BackoffConfiguration,
    ) -> Result<Box<dyn ChatCompletion>> {
        let boxed = match self {
            LLMConfiguration::OpenAI { .. } => {
                Box::new(self.build_openai(backoff_config)?) as Box<dyn ChatCompletion>
            }
            LLMConfiguration::Ollama { .. } => {
                Box::new(self.build_ollama()?) as Box<dyn ChatCompletion>
            }
            LLMConfiguration::OpenRouter { .. } => {
                Box::new(self.build_open_router(backoff_config)?) as Box<dyn ChatCompletion>
            }
            LLMConfiguration::FastEmbed { .. } => {
                anyhow::bail!("FastEmbed does not have a chat completion model")
            }
            LLMConfiguration::Anthropic { .. } => {
                Box::new(self.build_anthropic()?) as Box<dyn ChatCompletion>
            }
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn ChatCompletion>,
        };
        Ok(boxed)
    }
}
