use tempfile::tempdir;

use crate::{config::Config, repository::Repository};

/// Creates a repository for testing purposes with a temporary directory
pub fn test_repository() -> Repository {
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
    let config: Config = toml.parse().unwrap();

    Repository::from_config(config)
}
