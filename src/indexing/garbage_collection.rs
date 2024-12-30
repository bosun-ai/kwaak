//! This module identifies files changed since the last index date and removes them from the index.
//!
//!
//! NOTE: If more general settings are added to Redb, better extract this to a more general place.

use anyhow::Result;
use std::{collections::HashSet, path::PathBuf, time::SystemTime};
use swiftide::traits::Persist;

use crate::{repository::Repository, storage};

const LAST_INDEX_DATE: &str = "last_index_date";

pub struct GarbageCollector<'repository> {
    /// The last index date
    last_cleaned_up_at: Option<SystemTime>,
    repository: &'repository Repository,
}

impl<'repository> GarbageCollector<'repository> {
    pub fn from_repository(repository: &'repository Repository) -> Self {
        let last_cleaned_up_at: Option<SystemTime> =
            repository.runtime_settings().get(LAST_INDEX_DATE);

        Self {
            last_cleaned_up_at,
            repository,
        }
    }

    fn files_changed_since_last_index(&self) -> Vec<PathBuf> {
        ignore::Walk::new(self.repository.path())
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter(|entry| {
                // If no clean up is known, all files are considered changed
                let Some(last_cleaned_up_at) = self.last_cleaned_up_at else {
                    return true;
                };

                // If we can't get the modified time, we can't know if it's changed
                let Some(modified_at) = entry.metadata().ok().and_then(|m| m.modified().ok())
                else {
                    return false;
                };

                modified_at > last_cleaned_up_at
            })
            .map(ignore::DirEntry::into_path)
            .collect()
    }

    async fn delete_files_from_index(&self, files: Vec<PathBuf>) -> Result<()> {
        let lancedb = storage::get_lancedb(self.repository);

        // Ensure the table is set up
        // lancedb.setup().await?;

        let table = lancedb.open_table().await?;

        for file in files {
            let predicate = format!("path = \"{}\"", file.display());
            table.delete(&predicate).await?;
        }
        Ok(())
    }

    // This works under the assumption that relatively litle files change at a time
    //
    // There are much better ways to do this, but for now this is the simplest
    fn delete_files_from_cache(&self, files: &[PathBuf]) -> Result<()> {
        let redb = storage::get_redb(self.repository);

        // Read all files and build a fake node
        let node_ids = files
            .iter()
            .filter_map(|path| {
                let Ok(content) = std::fs::read_to_string(path) else {
                    return None;
                };

                let node = swiftide::indexing::Node::builder()
                    .path(path)
                    .chunk(content)
                    .build()
                    .expect("infallible");

                Some(redb.node_key(&node))
            })
            .collect::<Vec<_>>();

        let write_tx = redb.database().begin_write()?;
        {
            let mut table = write_tx.open_table(redb.table_definition())?;
            for id in &node_ids {
                let debug = table.remove(id).ok();
                dbg!(&debug.flatten().map(|r| r.value()));
            }
        }

        write_tx.commit()?;

        Ok(())
    }

    // Returns true if no rows were indexed, or otherwise errors were encountered
    async fn never_indexed(&self) -> bool {
        let lance_db = storage::get_lancedb(self.repository);

        if let Ok(table) = lance_db.open_table().await {
            table.count_rows(None).await.map(|n| n == 0).unwrap_or(true)
        } else {
            true
        }
    }

    pub async fn clean_up(&self) -> Result<()> {
        let files = self.files_changed_since_last_index();

        if files.is_empty() {
            tracing::info!("No files changed since last index; skipping garbage collection");
            return Ok(());
        }

        if self.never_indexed().await {
            tracing::warn!("No index date found; skipping garbage collection");
            return Ok(());
        }

        tracing::warn!(
            "Found {} changed files since last index; garbage collecting ...",
            files.len()
        );

        // should delete files from cache and index
        // should early return if no files are found, or index is empty
        // if index is empty and cache not => clear cache

        {
            self.delete_files_from_cache(&files)?;
            self.delete_files_from_index(files).await?;
        }

        self.repository
            .runtime_settings()
            .set(LAST_INDEX_DATE, SystemTime::now())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use swiftide::{
        indexing::{transformers::metadata_qa_code, Node},
        query::{Retrieve, TryStreamExt as _},
        traits::{NodeCache, Persist},
    };
    use tempfile;

    use crate::test_utils;

    use super::*;
    use lancedb::query::{ExecutableQuery, QueryBase};

    // Would be nice if this (part of) was part of the test repository helper
    //
    // Creates a repository, temporary folders, adds a node to both the cache and the index as if
    // it was indexed
    async fn setup() -> (Repository, Node, tempfile::TempDir) {
        let mut repository = test_utils::test_repository();
        let tempdir = tempfile::tempdir().unwrap();
        repository.path = tempdir.path().to_path_buf();
        repository.config.cache_dir = tempdir.path().join("cache");
        repository.config.log_dir = tempdir.path().join("log");
        std::fs::create_dir_all(&repository.config.cache_dir).unwrap();
        std::fs::create_dir_all(&repository.config.log_dir).unwrap();

        let tempfile = tempdir.path().join("test_file");
        std::fs::write(&tempfile, "Test node").unwrap();

        let mut node = Node::builder()
            .chunk("Test node")
            .path(&tempfile)
            .build()
            .unwrap();

        node.metadata.insert("path", tempfile.display().to_string());
        node.metadata
            .insert(metadata_qa_code::NAME, "test".to_string());

        let redb = storage::get_redb(&repository);
        redb.set(&node).await;
        assert!(redb.get(&node).await);

        let lancedb = storage::get_lancedb(&repository);
        // Ignore any errors here
        let _ = lancedb.setup().await;
        let node = lancedb.store(node).await.unwrap();

        (repository, node, tempdir)
    }

    macro_rules! assert_rows_with_path_in_lancedb {
        ($repository:expr, $path:expr, $count:expr) => {
            let lancedb = storage::get_lancedb($repository);
            let predicate = format!("path = \"{}\"", $path.display());

            let count = {
                let table = lancedb.open_table().await.unwrap();
                table.count_rows(Some(predicate)).await.unwrap()
            };

            assert_eq!(count, $count);
        };
    }

    #[test_log::test(tokio::test)]
    async fn test_clean_up_never_done_before() {
        let (repository, node, _guard) = setup().await;

        // Store nodes in cache and lancedb
        let redb = storage::get_redb(&repository);

        assert_rows_with_path_in_lancedb!(&repository, node.path, 1);

        // Now run the garbage collector
        let garbage_collector = GarbageCollector::from_repository(&repository);
        garbage_collector.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&repository, node.path, 0);
        assert!(!redb.get(&node).await);
    }

    #[test_log::test(tokio::test)]
    async fn test_clean_up_changed_file() {
        let (repository, node, _guard) = setup().await;

        // Store nodes in cache and lancedb
        let redb = storage::get_redb(&repository);

        repository
            .runtime_settings()
            .set(LAST_INDEX_DATE, SystemTime::now() - Duration::from_secs(60))
            .unwrap();

        assert_rows_with_path_in_lancedb!(&repository, node.path, 1);

        // Now run the garbage collector
        let garbage_collector = GarbageCollector::from_repository(&repository);
        garbage_collector.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&repository, node.path, 0);
        assert!(!redb.get(&node).await);
    }

    #[test_log::test(tokio::test)]
    async fn test_nothing_changed() {
        let (repository, node, _guard) = setup().await;

        // Store nodes in cache and lancedb
        let redb = storage::get_redb(&repository);

        repository
            .runtime_settings()
            .set(LAST_INDEX_DATE, SystemTime::now() + Duration::from_secs(60))
            .unwrap();

        assert_rows_with_path_in_lancedb!(&repository, node.path, 1);

        // Now run the garbage collector
        let garbage_collector = GarbageCollector::from_repository(&repository);
        garbage_collector.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&repository, node.path, 1);
        assert!(redb.get(&node).await);
    }
}
