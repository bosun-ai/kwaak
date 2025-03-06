use anyhow::Result;
use std::collections::HashMap;

use serde_json::json;
use strum::VariantNames as _;

use crate::{
    config::{
        AnthropicModel, FastembedModel, LLMConfiguration, OpenAIEmbeddingModel, OpenAIPromptModel,
    },
    onboarding::util::prompt_text,
};

use super::util::{prompt_api_key, prompt_select};

pub async fn llm_questions(context: &mut tera::Context) -> Result<()> {
    println!(
        "\nKwaak supports multiple LLM providers and uses multiple models for various tasks. What providers would you like to use?"
    );
    let valid_llms = LLMConfiguration::VARIANTS
        .iter()
        .map(AsRef::as_ref) // Kinda weird that we need to do this
        .filter(|v| *v != "FastEmbed" && *v != "AzureOpenAI")
        .collect::<Vec<&str>>();

    let valid_llm: LLMConfiguration = prompt_select(
        "What LLM would you like to use?",
        valid_llms,
        Some("OpenAI"),
    )?
    .parse()?;

    match valid_llm {
        LLMConfiguration::OpenAI { .. } => openai_questions(context)?,
        LLMConfiguration::Ollama { .. } => ollama_questions(context)?,
        LLMConfiguration::OpenRouter { .. } => open_router_questions(context).await?,
        LLMConfiguration::AzureOpenAI { .. } => {
            println!("{valid_llm} is not selectable yet, skipping configuration");
        }
        LLMConfiguration::Anthropic { .. } => anthropic_questions(context)?,
        LLMConfiguration::FastEmbed { .. } => {
            println!("{valid_llm} is not selectable yet, skipping configuration");
        }
        #[cfg(debug_assertions)]
        LLMConfiguration::Testing => {
            println!("{valid_llm} is not meant for production use, skipping configuration");
        }
    }

    Ok(())
}

fn openai_questions(context: &mut tera::Context) -> Result<()> {
    let api_key = prompt_api_key(
        "Where can we find your OpenAI api key? (https://platform.openai.com/api-keys)",
        Some("env:OPENAI_API_KEY"),
    )
    .prompt()?;
    let indexing_model = prompt_select(
        "Model used for fast operations (like indexing)",
        OpenAIPromptModel::VARIANTS.to_vec(),
        Some("gpt-4o-mini"),
    )?;
    let query_model = prompt_select(
        "Model used for querying and code generation",
        OpenAIPromptModel::VARIANTS.to_vec(),
        Some("gpt-4o"),
    )?;

    let embedding_model = prompt_select(
        "Model used for embeddings",
        OpenAIEmbeddingModel::VARIANTS.to_vec(),
        Some("text-embedding-3-large"),
    )?;

    context.insert("openai_api_key", &api_key);
    context.insert(
        "llm",
        &json!({
            "provider": "OpenAI",
            "indexing_model": indexing_model,
            "query_model": query_model,
            // "embedding_model": embedding_model,
            "base_url": None::<String>,
        }),
    );
    context.insert(
        "embed_llm",
        &json!({
            "provider": "OpenAI",
            "embedding_model": embedding_model,
            "base_url": None::<String>,
        }),
    );

    Ok(())
}

fn anthropic_questions(context: &mut tera::Context) -> Result<()> {
    let api_key = prompt_api_key(
        "Where can we find your anthropic api key? (https://console.anthropic.com/account/keys)",
        Some("env:ANTHROPIC_API_KEY"),
    )
    .prompt()?;

    let indexing_model = prompt_select(
        "Model used for fast operations (like indexing)",
        AnthropicModel::VARIANTS.to_vec(),
        Some(&AnthropicModel::Claude35Haiku.to_string()),
    )?;
    let query_model = prompt_select(
        "Model used for querying and code generation",
        AnthropicModel::VARIANTS.to_vec(),
        Some(&AnthropicModel::default().to_string()),
    )?;

    context.insert("anthropic_api_key", &api_key);
    context.insert(
        "llm",
        &json!({
            "provider": "Anthropic",
            "indexing_model": indexing_model,
            "query_model": query_model,
            "base_url": None::<String>,
        }),
    );

    println!(
        "\nAnthropic does not provide embeddings. Currently we suggest to use FastEmbed. If you want to use a different provider you can change it in your config later."
    );
    fastembed_questions(context)
}

async fn get_open_router_models() -> Option<Vec<HashMap<String, serde_json::Value>>> {
    let client = reqwest::Client::new();
    let response = match client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
    {
        Ok(response) => Some(response),
        Err(e) => {
            tracing::error!("Failed to fetch OpenRouter models: {e}");
            None
        }
    }?;

    let models: HashMap<String, Vec<HashMap<String, serde_json::Value>>> =
        response.json().await.ok()?;
    models.get("data").map(Vec::to_owned)
}
async fn open_router_questions(context: &mut tera::Context) -> Result<()> {
    println!(
        "\nOpenRouter allows you to use a variety of managed models via a single api. You can find models at https://openrouter.ai/models."
    );

    let api_key = prompt_api_key(
        "Where can we find your OpenRouter api key? (https://openrouter.ai/settings/keys)",
        Some("env:OPEN_ROUTER_API_KEY"),
    )
    .prompt()?;

    let openrouter_models = get_open_router_models().await;

    let autocompletion = OpenRouterCompletion {
        models: openrouter_models.clone(),
    };

    let validator = move |input: &str| {
        openrouter_models
            .as_ref()
            .map_or(Ok(inquire::validator::Validation::Valid), |models| {
                if models.iter().any(|m| {
                    m.get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_lowercase)
                        == Some(input.to_lowercase())
                }) {
                    Ok(inquire::validator::Validation::Valid)
                } else {
                    Ok(inquire::validator::Validation::Invalid(
                        "Model not found".into(),
                    ))
                }
            })
    };

    let indexing_model = prompt_text(
        "Model used for fast operations (like indexing)",
        Some("openai/gpt-4o-mini"),
    )
    .with_autocomplete(autocompletion.clone())
    .with_validator(validator.clone())
    .prompt()?;
    let query_model = prompt_text(
        "Model used for querying and code generation",
        Some("anthropic/claude-3.5-sonnet"),
    )
    .with_autocomplete(autocompletion.clone())
    .with_validator(validator.clone())
    .prompt()?;

    context.insert("open_router_api_key", &api_key);

    context.insert(
        "llm",
        &json!({
            "provider": "OpenRouter",
            "indexing_model": indexing_model,
            "query_model": query_model,
            "base_url": None::<String>,
        }),
    );

    println!(
        "\nOpenRouter does not support embeddings yet. Currently we suggest to use FastEmbed. If you want to use a different provider you can change it in your config later."
    );
    fastembed_questions(context)
}

fn ollama_questions(context: &mut tera::Context) -> Result<()> {
    println!("Note that you need to have a running Ollama instance.");

    let indexing_model = prompt_text(
        "Model used for fast operations (like indexing). This model does not need to support tool calls.",
        None

    ).prompt()?;

    let query_model = prompt_text(
        "Model used for querying and code generation. This model needs to support tool calls.",
        None,
    )
    .prompt()?;

    let embedding_model =
        prompt_text("Model used for embeddings, bge-m3 is a solid choice", None).prompt()?;

    let vector_size = inquire::Text::new("Vector size for the embedding model")
        .with_validator(|input: &str| match input.parse::<usize>() {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Invalid number".into(),
            )),
        })
        .prompt()?;

    let base_url = inquire::Text::new("Custom base url? (optional, <esc> to skip)")
        .with_validator(|input: &str| match url::Url::parse(input) {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Invalid URL".into(),
            )),
        })
        .prompt_skippable()?;

    context.insert(
        "llm",
        &json!({
            "provider": "Ollama",
            "indexing_model": indexing_model,
            "query_model": query_model,
            "base_url": base_url,
        }),
    );
    context.insert(
        "embed_llm",
        &json!({
            "provider": "Ollama",
            "base_url": None::<String>,
            "embedding_model": format!("{{name = \"{embedding_model}\", vector_size = {vector_size}}}")
        }),
    );

    Ok(())
}

pub fn fastembed_questions(context: &mut tera::Context) -> Result<()> {
    println!(
        "\nFastEmbed provides embeddings that are generated quickly locally. Unless you have a specific need for a different model, the default is a good choice."
    );

    let embedding_model: FastembedModel = prompt_select(
        "Embedding model",
        FastembedModel::list_supported_models(),
        Some(FastembedModel::default().to_string()),
    )?
    .parse()?;

    context.insert(
        "embed_llm",
        &json!({
            "provider": "FastEmbed",
            "embedding_model": embedding_model.to_string(),
            "base_url": None::<String>,
        }),
    );

    Ok(())
}

#[derive(Clone)]
struct OpenRouterCompletion {
    models: Option<Vec<HashMap<String, serde_json::Value>>>,
}

impl inquire::Autocomplete for OpenRouterCompletion {
    fn get_suggestions(
        &mut self,
        input: &str,
    ) -> std::result::Result<Vec<String>, inquire::CustomUserError> {
        if let Some(models) = &self.models {
            Ok(models
                .iter()
                .filter_map(|m| m.get("id"))
                .filter_map(|n| n.as_str())
                .filter(|n| n.to_lowercase().contains(&input.to_lowercase()))
                .map(ToString::to_string)
                .collect())
        } else {
            Ok(vec![])
        }
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> std::result::Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        if highlighted_suggestion.is_some() {
            return Ok(highlighted_suggestion);
        }

        // Searches for the shortest common prefix of all the suggestions
        if let Some(models) = &self.models {
            let ids = models
                .iter()
                .filter_map(|m| m.get("id"))
                .filter_map(|n| n.as_str())
                .filter(|n| n.to_lowercase().starts_with(&input.to_lowercase()))
                .collect::<Vec<_>>();

            if ids.is_empty() {
                return Ok(None);
            }

            let matched = ids.iter().skip(1).fold(ids[0], |prefix, &s| {
                let overlap_len = prefix
                    .chars()
                    .zip(s.chars())
                    .take_while(|(a, b)| a == b)
                    .count();

                &prefix[..prefix
                    .char_indices()
                    .nth(overlap_len)
                    .map_or(prefix.len(), |(i, _)| i)]
            });

            Ok(Some(matched.to_string()))
        } else {
            Ok(None)
        }
    }
}
