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
    #[must_use]
    pub fn new(secret: SecretString) -> Self {
        ApiKey(secret)
    }

    #[must_use]
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
            Err(serde::de::Error::custom(
                "expected an api key prefixed with `env:`, `text:` or `file:`",
            ))
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

#[allow(clippy::from_over_into)]
impl Into<SecretString> for ApiKey {
    fn into(self) -> SecretString {
        self.0
    }
}

#[allow(clippy::from_over_into)]
impl Into<SecretString> for &ApiKey {
    fn into(self) -> SecretString {
        self.clone().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::env;

    #[derive(Debug, Deserialize)]
    struct Config {
        api_key: ApiKey,
    }

    #[test]
    fn test_deserialize_api_key_from_text() {
        let toml = r#"
            api_key = "text:my-secret-key"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.api_key.expose_secret(), "my-secret-key");
    }

    #[test]
    fn test_deserialize_api_key_from_env() {
        env::set_var("MY_SECRET_KEY", "env-secret-key");

        let toml = r#"
            api_key = "env:MY_SECRET_KEY"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.api_key.expose_secret(), "env-secret-key");

        env::remove_var("MY_SECRET_KEY");
    }

    #[test]
    fn test_deserialize_api_key_from_file() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "file-secret-key").unwrap();

        let toml = format!(
            r#"
            api_key = "file:{}"
        "#,
            file_path.to_str().unwrap()
        );

        let config: Config = toml::from_str(&toml).unwrap();
        assert_eq!(config.api_key.expose_secret(), "file-secret-key");
    }

    #[test]
    fn test_deserialize_api_key_without_prefix() {
        let toml = r#"
            api_key = "plain-secret-key"
        "#;

        let expected = "expected an api key prefixed with `env:`, `text:` or `file:`";
        let config: Result<Config, _> = toml::from_str(toml);
        assert!(config.is_err());
        assert!(config.unwrap_err().to_string().contains(expected));
    }

    #[test]
    fn test_correct_error_on_missing_env() {
        let toml = r#"
            api_key = "env:MY_SECRET_KEY_MISSING"
        "#;

        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(err.to_string().contains("environment variable not found"));

        env::remove_var("MY_SECRET_KEY_MISSING");
    }
}
