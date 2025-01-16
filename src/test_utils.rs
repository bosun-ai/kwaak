#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
use swiftide::chat_completion::ChatCompletionResponse;
use swiftide_core::{ChatCompletion, EmbeddingModel, SimplePrompt};
use uuid::Uuid;

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

            [git]
            owner = "bosun-ai"
            repository = "kwaak"
            
            [llm.indexing]
            provider = "Testing"

            [llm.query]
            provider = "Testing"

            [llm.embedding]
            provider = "Testing"
            "#;
    let config: Config = toml.parse().unwrap();

    let mut repository = Repository::from_config(config);

    let tempdir = tempfile::tempdir().unwrap();
    *repository.path_mut() = tempdir.path().join("app");

    let config = repository.config_mut();
    config.project_name = Uuid::new_v4().to_string();
    config.cache_dir = tempdir.path().to_path_buf();
    config.log_dir = tempdir.path().join("logs");
    config.docker.context = tempdir.path().join("app");

    // Copy this dockerfile to the context
    std::fs::create_dir_all(&config.docker.context).unwrap();
    std::fs::copy("Dockerfile", config.docker.context.join("Dockerfile")).unwrap();

    std::fs::create_dir_all(&repository.config().cache_dir).unwrap();
    std::fs::create_dir_all(&repository.config().log_dir).unwrap();

    tracing::info!("Created repository at {:?}", repository.path());

    // Initialize git
    std::process::Command::new("git")
        .arg("init")
        .current_dir(repository.path())
        .output()
        .unwrap();

    // Add a hello world file and commit
    std::fs::write(repository.path().join("hello.txt"), "Hello, world!").unwrap();
    std::process::Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(repository.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("Initial commit")
        .current_dir(repository.path())
        .output()
        .unwrap();

    // debug files in app dir, list all including hidden
    tracing::debug!(
        "Files in app dir: {:?}",
        std::fs::read_dir(repository.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>()
    );

    (repository, TestGuard { tempdir })
}

#[derive(Debug, Clone)]
pub struct NoopLLM;

#[async_trait::async_trait]
impl SimplePrompt for NoopLLM {
    async fn prompt(&self, _prompt: swiftide::prompt::Prompt) -> anyhow::Result<String> {
        Ok("Kwek".to_string())
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for NoopLLM {
    async fn embed(&self, input: Vec<String>) -> anyhow::Result<swiftide::Embeddings> {
        Ok(vec![vec![0.0; input.len()]])
    }
}

#[async_trait::async_trait]
impl ChatCompletion for NoopLLM {
    async fn complete(
        &self,
        _request: &swiftide::chat_completion::ChatCompletionRequest,
    ) -> Result<
        swiftide::chat_completion::ChatCompletionResponse,
        swiftide::chat_completion::errors::ChatCompletionError,
    > {
        ChatCompletionResponse::builder()
            .message("Kwek kwek")
            .build()
            .map_err(std::convert::Into::into)
    }
}
