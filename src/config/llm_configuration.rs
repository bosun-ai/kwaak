use super::ApiKey;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use swiftide::{
    chat_completion::ChatCompletion,
    integrations,
    traits::{EmbeddingModel, SimplePrompt},
};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)] // Parent is always on the heap in config
pub enum LLMConfigurations {
    Single(LLMConfiguration),
    Multiple {
        // TODO: Should probably be with reduced attrs on needed per item
        indexing: LLMConfiguration,
        embedding: LLMConfiguration,
        query: LLMConfiguration,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "provider")]
pub enum LLMConfiguration {
    OpenAI {
        api_key: ApiKey,
        #[serde(default)]
        prompt_model: OpenAIPromptModel,
        #[serde(default)]
        embedding_model: OpenAIEmbeddingModel,
        base_url: Option<Url>,
    },
    // Groq {
    //     api_key: SecretString,
    //     prompt_model: String,
    // },
    // Ollama {
    //     prompt_model: Option<String>,
    //     embedding_model: Option<String>,
    //     vector_size: Option<usize>,
    // },
    // AWSBedrock {
    //     prompt_model: String,
    // },
    // FastEmbed {
    //     embedding_model: String,
    //     vector_size: usize,
    // },
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
    api_key: &ApiKey,
    embedding_model: &OpenAIEmbeddingModel,
    prompt_model: &OpenAIPromptModel,
    base_url: Option<&Url>,
) -> Result<integrations::openai::OpenAI> {
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
                api_key,
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?),
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
                api_key,
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?),
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
                api_key,
                embedding_model,
                prompt_model,
                base_url.as_ref(),
            )?),
        };

        Ok(boxed)
    }
}
