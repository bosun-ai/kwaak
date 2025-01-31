#[cfg(debug_assertions)]
use crate::test_utils::NoopLLM;

use super::ApiKey;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use swiftide::{
    chat_completion::ChatCompletion,
    integrations::{
        self,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingModelWithSize {
    pub name: String,
    pub vector_size: i32,
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

            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => 1,
        }
    }
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

fn build_openai(
    api_key: Option<&ApiKey>,
    embedding_model: &OpenAIEmbeddingModel,
    prompt_model: &OpenAIPromptModel,
    base_url: Option<&Url>,
) -> Result<integrations::openai::OpenAI> {
    let api_key = api_key.context("Expected an api key")?;
    let mut config =
        async_openai::config::OpenAIConfig::default().with_api_key(api_key.expose_secret());

    if let Some(base_url) = base_url {
        config = config.with_api_base(base_url.to_string());
    };

    integrations::openai::OpenAI::builder()
        .client(async_openai::Client::with_config(config))
        .default_prompt_model(prompt_model.to_string())
        .default_embed_model(embedding_model.to_string())
        .build()
        .context("Failed to build OpenAI client")
}

fn build_ollama(llm_config: &LLMConfiguration) -> Result<Ollama> {
    let LLMConfiguration::Ollama {
        prompt_model,
        embedding_model,
        base_url,
        ..
    } = llm_config
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

fn build_open_router(llm_config: &LLMConfiguration) -> Result<OpenRouter> {
    let LLMConfiguration::OpenRouter {
        prompt_model,
        api_key,
    } = llm_config
    else {
        anyhow::bail!("Expected OpenRouter configuration")
    };

    let api_key = api_key.as_ref().context("Expected an api key")?;
    let config = OpenRouterConfig::builder()
        .api_key(api_key)
        .site_url("https://github.com/bosun-ai/kwaak")
        .site_name("Kwaak")
        .build()?;

    OpenRouter::builder()
        .client(async_openai::Client::with_config(config))
        .default_prompt_model(prompt_model)
        .to_owned()
        .build()
        .context("Failed to build OpenRouter client")
}

impl TryInto<Box<dyn EmbeddingModel>> for &LLMConfiguration {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<Box<dyn EmbeddingModel>, Self::Error> {
        let boxed = match self {
            LLMConfiguration::OpenAI {
                api_key,
                embedding_model,
                prompt_model,
                base_url,
            } => Box::new(build_openai(
                api_key.as_ref(),
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?) as Box<dyn EmbeddingModel>,
            LLMConfiguration::Ollama { .. } => {
                Box::new(build_ollama(self)?) as Box<dyn EmbeddingModel>
            }
            LLMConfiguration::OpenRouter { .. } => {
                panic!("OpenRouter does not have an embedding model")
            }
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn EmbeddingModel>,
        };

        Ok(boxed)
    }
}

impl TryInto<Box<dyn SimplePrompt>> for &LLMConfiguration {
    type Error = anyhow::Error;
    fn try_into(self) -> std::result::Result<Box<dyn SimplePrompt>, Self::Error> {
        let boxed = match self {
            LLMConfiguration::OpenAI {
                api_key,
                embedding_model,
                prompt_model,
                base_url,
            } => Box::new(build_openai(
                api_key.as_ref(),
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?) as Box<dyn SimplePrompt>,
            LLMConfiguration::Ollama { .. } => {
                Box::new(build_ollama(self)?) as Box<dyn SimplePrompt>
            }
            LLMConfiguration::OpenRouter { .. } => {
                Box::new(build_open_router(self)?) as Box<dyn SimplePrompt>
            }
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn SimplePrompt>,
        };

        Ok(boxed)
    }
}

impl TryInto<Box<dyn ChatCompletion>> for &LLMConfiguration {
    type Error = anyhow::Error;
    fn try_into(self) -> std::result::Result<Box<dyn ChatCompletion>, Self::Error> {
        let boxed = match self {
            LLMConfiguration::OpenAI {
                api_key,
                embedding_model,
                prompt_model,
                base_url,
            } => Box::new(build_openai(
                api_key.as_ref(),
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?) as Box<dyn ChatCompletion>,
            LLMConfiguration::Ollama { .. } => {
                Box::new(build_ollama(self)?) as Box<dyn ChatCompletion>
            }
            LLMConfiguration::OpenRouter { .. } => {
                Box::new(build_open_router(self)?) as Box<dyn ChatCompletion>
            }
            #[cfg(debug_assertions)]
            LLMConfiguration::Testing => Box::new(NoopLLM) as Box<dyn ChatCompletion>,
        };

        Ok(boxed)
    }
}
