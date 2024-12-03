//! In a configuration file, the API key is stored as a secret string.
//!
//! This module provides an interface such that the api key can be configured in different ways,
//! i.e.:
//!
//! # From an environment variable
//! key: "`env:ENVIRONMENT_VARIABLE_NAME`"
//!
//! # Directly in the configuration file
//! key: "text:my-secret-key"
//!
//! # From a file
//! key: "<file:/path>
//!
//! It can also be used in Serde.
//!
//! # Example
//!
//! Given a configuration like:
//! ```toml
//! api_key = "text:my-secret-key"
//! ```
//!
//! With a config like:
//!
//! ```ignore
//! #[derive(Deserialize)]
//! struct Config {
//!    api_key: ApiKey
//! }
//!
//! ```
//!
//! The `api_key` field will be deserialized into an `ApiKey` struct.

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize};
#[derive(Clone)]
pub struct ApiKey(SecretString);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(****)")
    }
}

impl ApiKey {
    pub fn new(secret: SecretString) -> Self {
        ApiKey(secret)
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl<T: AsRef<str>> From<T> for ApiKey {
    fn from(secret: T) -> Self {
        ApiKey(SecretString::from(secret.as_ref()))
    }
}

impl<'de> Deserialize<'de> for ApiKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        if let Some(var_name) = s.strip_prefix("env:") {
            let secret = std::env::var(var_name).map_err(serde::de::Error::custom)?;
            Ok(ApiKey(SecretString::from(secret)))
        } else if let Some(secret) = s.strip_prefix("text:") {
            Ok(ApiKey(SecretString::from(secret)))
        } else if let Some(path) = s.strip_prefix("file:") {
            let secret = std::fs::read_to_string(path).map_err(serde::de::Error::custom)?;
            Ok(ApiKey(SecretString::from(secret.trim().to_string())))
        } else {
            Err(serde::de::Error::custom("Invalid API key format"))
        }
    }
}

impl Serialize for ApiKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        "ApiKey(****)".serialize(serializer)
    }
}
