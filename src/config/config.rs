use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use swiftide::integrations::treesitter::SupportedLanguages;

extern crate num_cpus;

use num_cpus;
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
