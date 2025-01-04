use crate::{config::Config, repository::Repository};

pub struct TestGuard {
    pub tempdir: tempfile::TempDir,
}
pub fn test_repository() -> (Repository, TestGuard) {
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

    let mut repository = Repository::from_config(config);

    let tempdir = tempfile::tempdir().unwrap();
    *repository.path_mut() = tempdir.path().to_path_buf();
    repository.config_mut().cache_dir = tempdir.path().to_path_buf();
    repository.config_mut().log_dir = tempdir.path().join("logs");
    std::fs::create_dir_all(&repository.config().cache_dir).unwrap();
    std::fs::create_dir_all(&repository.config().log_dir).unwrap();

    (repository, TestGuard { tempdir })
}
