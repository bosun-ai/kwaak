pub mod agent;
pub mod chat;
pub mod chat_message;
pub mod cli;
pub mod commands;
pub mod config;
#[cfg(all(feature = "evaluations", feature = "duckdb"))]
pub mod evaluations;
pub mod frontend;
pub mod git;
pub mod indexing;
pub mod kwaak_tracing;
pub mod onboarding;
pub mod repository;
#[cfg(feature = "duckdb")]
pub mod runtime_settings;
pub mod templates;
pub mod util;

#[cfg(debug_assertions)]
pub mod test_utils;
