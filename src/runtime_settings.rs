//! Runtime settings are persisted settings that are used to configure the behavior of the application at runtime.
//!
//! Unlike the configuration, these can during runtime, and are only intended to be used for
//! internal operation of kwaak.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use swiftide::integrations::duckdb::Duckdb;
use tokio::sync::RwLock;

use crate::{indexing::duckdb_index::get_duckdb, repository::Repository};

pub struct RuntimeSettings {
    db: Duckdb,
    schema_created: RwLock<bool>,
}

impl RuntimeSettings {
    #[must_use]
    pub fn from_repository(repository: &Repository) -> Self {
        let db = get_duckdb(repository.config());

        Self {
            db,
            schema_created: false.into(),
        }
    }

    #[must_use]
    pub fn from_db(db: Duckdb) -> Self {
        Self {
            db,
            schema_created: false.into(),
        }
    }

    #[must_use]
    pub async fn get<VALUE: for<'a> Deserialize<'a>>(&self, key: &str) -> Option<VALUE> {
        self.lazy_create_schema().await.ok()?;

        let conn = self.db.connection().lock().unwrap();
        let sql = "SELECT value FROM runtime_settings WHERE key = ?";

        serde_json::from_str(
            &conn
                .query_row(sql, [key], |row| row.get::<_, String>(0))
                .ok()?,
        )
        .ok()
    }

    pub async fn set<VALUE: Serialize>(&self, key: &str, value: VALUE) -> Result<()> {
        self.lazy_create_schema().await?;
        let conn = self.db.connection().lock().unwrap();
        let sql = "INSERT OR REPLACE INTO runtime_settings (key, value) VALUES (?, ?)";

        conn.execute(sql, [key, &serde_json::to_string(&value)?])
            .context("Failed to set runtime setting")?;

        Ok(())
    }

    async fn lazy_create_schema(&self) -> Result<()> {
        if *self.schema_created.read().await {
            return Ok(());
        }
        let mut lock = self.schema_created.write().await;

        let sql = "CREATE TABLE IF NOT EXISTS runtime_settings (key TEXT PRIMARY KEY, value TEXT)";
        let conn = self.db.connection().lock().unwrap();
        conn.execute(sql, [])
            .context("Failed to create runtime settings table")?;

        *lock = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_utils;

    #[test_log::test(tokio::test)]
    async fn test_set_and_get() {
        let (repository, _guard) = test_utils::test_repository();
        let runtime_settings = RuntimeSettings::from_repository(&repository);

        let key = "test_key";
        let value = "test_value";

        // Set the value
        runtime_settings.set(key, value).await.unwrap();

        // Get the value
        let retrieved_value = runtime_settings.get::<String>(key).await.unwrap();

        assert_eq!(retrieved_value, value);
    }

    #[test_log::test(tokio::test)]
    async fn test_with_non_string() {
        let (repository, _guard) = test_utils::test_repository();
        let runtime_settings = RuntimeSettings::from_repository(&repository);

        let key = "test_key2";
        let value = 10;

        // Set the value
        {}
        runtime_settings.set(key, value).await.unwrap();

        // Get the value
        let retrieved_value = runtime_settings.get::<i32>(key).await.unwrap();

        assert_eq!(retrieved_value, value);
    }
}
