//! Runtime settings are persisted settings that are used to configure the behavior of the application at runtime.
//!
//! Unlike the configuration, these can during runtime, and are only intended to be used for
//! internal operation of kwaak.

use std::sync::Arc;

use anyhow::{Context, Result};
use redb::TableDefinition;
use serde::{Deserialize, Serialize};
use swiftide::integrations::redb::Redb;

use crate::{repository::Repository, storage};

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("runtime_settings");

pub struct RuntimeSettings {
    db: Arc<Redb>,
}

impl RuntimeSettings {
    pub fn from_repository(repository: &Repository) -> Self {
        let db = storage::get_redb(repository);

        Self { db }
    }

    #[cfg(debug_assertions)]
    pub fn from_db(db: Arc<Redb>) -> Self {
        Self { db }
    }

    pub fn get<VALUE: for<'a> Deserialize<'a>>(&self, key: &str) -> Option<VALUE> {
        let read = self
            .db
            .database()
            .begin_read()
            .ok()?
            .open_table(TABLE)
            .ok()?;

        let value = serde_json::from_str(read.get(key).ok().flatten()?.value()).ok()?;

        Some(value)
    }

    pub fn set<VALUE: Serialize>(&self, key: &str, value: VALUE) -> Result<()> {
        let write_tx = self.db.database().begin_write()?;

        {
            let value = serde_json::to_value(&value)
                .context("Could not serialize value")?
                .to_string();
            write_tx.open_table(TABLE)?.insert(key, value.as_str())?;
        }

        write_tx.commit()?;

        Ok(())
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_utils;

    #[test_log::test]
    fn test_set_and_get() {
        let (repository, _guard) = test_utils::test_repository();
        let runtime_settings = RuntimeSettings::from_repository(&repository);

        let key = "test_key";
        let value = "test_value";

        // Set the value
        runtime_settings.set(key, value).unwrap();

        // Get the value
        let retrieved_value = runtime_settings.get::<String>(key).unwrap();

        assert_eq!(retrieved_value, value);
    }

    #[test_log::test]
    fn test_with_non_string() {
        let (repository, _guard) = test_utils::test_repository();
        let runtime_settings = RuntimeSettings::from_repository(&repository);

        let key = "test_key";
        let value = 10;

        // Set the value
        runtime_settings.set(key, value).unwrap();

        // Get the value
        let retrieved_value = runtime_settings.get::<i32>(key).unwrap();

        assert_eq!(retrieved_value, value);
    }
}
