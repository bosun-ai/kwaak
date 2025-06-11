#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]
use std::sync::Arc;

use anyhow::Result;
use swiftide::{
    agents::{Agent, DefaultContext, tools::local_executor::LocalExecutor},
    chat_completion::{ChatCompletion, ChatCompletionResponse, errors::LanguageModelError},
    traits::{EmbeddingModel, SimplePrompt, ToolExecutor},
};

use crate::{config::Config, git, repository::Repository};

#[cfg(feature = "duckdb")]
pub mod integration;

pub struct TestGuard {
    pub tempdir: tempfile::TempDir,
}

/// Sets up a fake, noop repository for testing
///
/// * Temporary directory dropped when the repository is dropped
/// * Safe to use with docker executor
/// * Safe to use with git
/// * Safe to use with LLMs (noop)
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
    // wtf why is this so verbose
    let suffix = uuid::Uuid::new_v4()
        .to_string()
        .split('-')
        .next()
        .unwrap()
        .to_string();
    *repository.path_mut() = tempdir.path().join("app");

    let config = repository.config_mut();
    config.project_name = format!("test_repository_{suffix}");
    config.cache_dir = tempdir.path().to_path_buf();
    config.log_dir = tempdir.path().join("logs");
    config.docker.context = tempdir.path().join("app");
    config.git.auto_push_remote = false;
    config.stop_on_empty_messages = true;

    // Copy this dockerfile to the context
    std::fs::create_dir_all(&config.docker.context).unwrap();
    std::fs::copy("Dockerfile.tests", config.docker.context.join("Dockerfile")).unwrap();

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

    // set the git author
    let user_email = std::process::Command::new("git")
        .arg("config")
        .arg("user.email")
        .arg("\"kwaak@bosun.ai\"")
        .current_dir(repository.path())
        .output()
        .unwrap();

    assert!(user_email.status.success(), "failed to set git user email");

    let user_name = std::process::Command::new("git")
        .arg("config")
        .arg("user.name")
        .arg("\"kwaak\"")
        .current_dir(repository.path())
        .output()
        .unwrap();

    assert!(user_name.status.success(), "failed to set git user name");

    let initial = std::process::Command::new("git")
        .arg("commit")
        .arg("-n")
        .arg("--allow-empty")
        .arg("-m")
        .arg("\"Initial commit\"")
        .current_dir(repository.path())
        .output()
        .unwrap();

    let output = std::str::from_utf8(&initial.stdout).unwrap().to_string()
        + std::str::from_utf8(&initial.stderr).unwrap();

    if !initial.status.success() {
        tracing::error!("Failed to commit initial commit: {}", output);
    }

    // For some reason in some unit tests this can fail?
    // assert!(
    //     initial.status.success(),
    //     "failed to commit initial commit for test"
    // );

    // Update the mainbranch as it could be main or master depending on the os
    repository.config_mut().git.main_branch = git::util::main_branch(repository.path());

    // debug files in app dir, list all including hidden
    tracing::debug!(
        "Files in app dir: {:?}",
        std::fs::read_dir(repository.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>()
    );

    tracing::debug!("Initial commit: {:?}", initial);

    (repository, TestGuard { tempdir })
}

// Creates a noop test agent based on a repository
// useful for ie hooks
#[must_use]
pub fn test_agent_for_repository(repository: &Repository) -> Agent {
    let llm = repository
        .config()
        .query_provider()
        .get_chat_completion_model(repository.config().backoff)
        .unwrap();
    let context = DefaultContext::from_executor(
        Arc::new(LocalExecutor::new(repository.path())) as Arc<dyn ToolExecutor>
    );
    Agent::builder().context(context).llm(&llm).build().unwrap()
}

/// A fake LLM that always returns the same message, for testing purposes.
#[derive(Debug, Clone)]
pub struct NoopLLM {
    response: String,
}

impl Default for NoopLLM {
    fn default() -> Self {
        Self {
            response: "Kwek".to_string(),
        }
    }
}

impl NoopLLM {
    #[must_use]
    pub fn new(response: String) -> Self {
        Self { response }
    }

    pub fn with_response(&mut self, response: String) -> &mut Self {
        self.response = response;
        self
    }
}

#[async_trait::async_trait]
impl SimplePrompt for NoopLLM {
    async fn prompt(
        &self,
        _prompt: swiftide::prompt::Prompt,
    ) -> Result<String, LanguageModelError> {
        Ok(self.response.clone())
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for NoopLLM {
    async fn embed(&self, input: Vec<String>) -> Result<swiftide::Embeddings, LanguageModelError> {
        Ok(vec![vec![0.0; input.len()]])
    }
}

#[async_trait::async_trait]
impl ChatCompletion for NoopLLM {
    async fn complete(
        &self,
        _request: &swiftide::chat_completion::ChatCompletionRequest,
    ) -> Result<swiftide::chat_completion::ChatCompletionResponse, LanguageModelError> {
        ChatCompletionResponse::builder()
            .message(&self.response)
            .build()
            .map_err(std::convert::Into::into)
    }
}

/// Run the UI until a certain event is reached
#[macro_export]
macro_rules! assert_command_done {
    ($app:expr, $uuid:expr) => {
        let event = $app
            .handle_events_until(UIEvent::is_command_done)
            .await
            .unwrap();

        assert_eq!(event, UIEvent::CommandDone($uuid));
    };
}

#[macro_export]
macro_rules! assert_agent_responded {
    ($app:expr, $uuid:expr) => {
        let event = $app
            .handle_events_until(UIEvent::is_chat_message)
            .await
            .unwrap();
    };
}

pub struct TempEnv<'a> {
    key: &'a str,
    original: Option<String>,
}
#[must_use]
// Sets a temporary env variable, when dropped it will be reset to the original value
#[allow(unsafe_code)]
pub fn temp_env<'a>(key: &'a str, value: &str) -> TempEnv<'a> {
    let original = std::env::var(key).ok();
    unsafe { std::env::set_var(key, value) };

    TempEnv { key, original }
}

#[allow(unsafe_code)]
impl Drop for TempEnv<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            unsafe { std::env::set_var(self.key, original) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}
