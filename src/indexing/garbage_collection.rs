//! This module identifies files changed since the last index date and removes them from the index.
//!
//!
//! NOTE: If more general settings are added to Redb, better extract this to a more general place.

use anyhow::Result;
use std::{borrow::Cow, path::PathBuf, sync::Arc, time::SystemTime};
use swiftide::{
    integrations::{lancedb::LanceDB, redb::Redb},
    traits::Persist,
};

use crate::{repository::Repository, runtime_settings::RuntimeSettings, storage};

const LAST_CLEANED_UP_AT: &str = "last_cleaned_up_at";

#[derive(Debug)]
pub struct GarbageCollector<'repository> {
    /// The last index date
    repository: Cow<'repository, Repository>,
    lancedb: Arc<LanceDB>,
    redb: Arc<Redb>,
}

impl<'repository> GarbageCollector<'repository> {
    pub fn from_repository(repository: &'repository Repository) -> Self {
        Self {
            repository: Cow::Borrowed(repository),
            lancedb: storage::get_lancedb(repository),
            redb: storage::get_redb(repository),
        }
    }

    fn runtime_settings(&self) -> RuntimeSettings {
        if cfg!(test) {
            RuntimeSettings::from_db(self.redb.clone())
        } else {
            self.repository.runtime_settings()
        }
    }
    fn get_last_cleaned_up_at(&self) -> Option<SystemTime> {
        self.runtime_settings().get(LAST_CLEANED_UP_AT)
    }

    fn update_last_cleaned_up_at(&self, date: SystemTime) -> Result<()> {
        self.runtime_settings().set(LAST_CLEANED_UP_AT, date)
    }

    fn files_changed_since_last_index(&self) -> Vec<PathBuf> {
        tracing::info!("Checking for files changed since last index.");

        // Get the last commit hash before the last indexed date
        let last_indexed_commit = std::process::Command::new("git")
            .args(["log", "--format=%H", "-1", "--skip=1"])
            .output()
            .expect("failed to execute process")
            .stdout;

        let last_indexed_commit = String::from_utf8_lossy(&last_indexed_commit)
            .trim()
            .to_string();

        // Identify deleted files since that commit
        let output = std::process::Command::new("git")
            .args([
                "diff",
                "--name-only",
                "--diff-filter=D",
                &last_indexed_commit,
                "HEAD",
            ])
            .output()
            .expect("failed to execute git diff command");

        let deleted_files = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("Deleted files detected: {deleted_files}");
        let deleted_paths = deleted_files.lines().map(PathBuf::from).collect::<Vec<_>>();

        let last_cleaned_up_at = self.get_last_cleaned_up_at();
        let modified_files = ignore::Walk::new(self.repository.path())
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter(|entry| {
                // If no clean up is known, all files are considered changed
                let Some(last_cleaned_up_at) = last_cleaned_up_at else {
                    return true;
                };

                // If we can't get the modified time, we can't know if it's changed
                let Some(modified_at) = entry.metadata().ok().and_then(|m| m.modified().ok())
                else {
                    return false;
                };

                tracing::debug!(
                    ?modified_at,
                    ?last_cleaned_up_at,
                    "Comparing file modified times for {}",
                    entry.path().display()
                );
                modified_at > last_cleaned_up_at
            })
            .map(ignore::DirEntry::into_path)
            .collect::<Vec<_>>();

        tracing::info!(
            "Found {} modified and {} deleted paths.",
            modified_files.len(),
            deleted_paths.len()
        );

        [modified_files, deleted_paths].concat()
    }

    async fn delete_files_from_index(&self, files: Vec<PathBuf>) -> Result<()> {
        // Ensure the table is set up
        tracing::info!(
            "Setting up LanceDB table for deletion of files: {:?}",
            files
        );
        self.lancedb.setup().await?;

        let table = self.lancedb.open_table().await?;

        for file in files {
            let predicate = format!("path = \"{}\"", file.display());
            tracing::debug!(
                "Deleting file from LanceDB index with predicate: {}",
                predicate
            );
            table.delete(&predicate).await?;
        }
        Ok(())
    }

    fn delete_files_from_cache(&self, files: &[PathBuf]) -> Result<()> {
        tracing::info!("Deleting files from cache: {:?}", files);
        // Read all files and build a fake node
        let node_ids = files
            .iter()
            .filter_map(|path| {
                let Ok(content) = std::fs::read_to_string(path) else {
                    tracing::warn!(
                        "Could not read content for file but deleting: {}",
                        path.display()
                    );
                    return None;
                };

                let node = swiftide::indexing::Node::builder()
                    .path(path)
                    .chunk(content)
                    .build()
                    .expect("infallible");

                Some(self.redb.node_key(&node))
            })
            .collect::<Vec<_>>();

        let write_tx = self.redb.database().begin_write()?;
        {
            let mut table = write_tx.open_table(self.redb.table_definition())?;
            for id in &node_ids {
                tracing::debug!("Removing ID from cache: {}", id);
                table.remove(id).ok();
            }
        }

        write_tx.commit()?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn clean_up(&self) -> Result<()> {
        let files = self.files_changed_since_last_index();

        tracing::debug!("Files detected as changed: {:?}", files);

        if files.is_empty() {
            tracing::info!("No files changed since last index; skipping garbage collection");
            return Ok(());
        }

        if self.never_been_indexed().await {
            tracing::warn!("No index date found; skipping garbage collection");
            return Ok(());
        }

        tracing::warn!(
            "Found {} changed files since last index; garbage collecting ...",
            files.len()
        );

        tracing::debug!(?files, "Files changed since last index");

        {
            self.delete_files_from_cache(&files)?;
            self.delete_files_from_index(files).await?;
        }

        self.update_last_cleaned_up_at(SystemTime::now())?;

        tracing::info!("Garbage collection completed and cleaned up at updated.");

        Ok(())
    }

    // Returns true if no rows were indexed, or otherwise errors were encountered
    #[tracing::instrument(skip(self))]
    async fn never_been_indexed(&self) -> bool {
        if let Ok(table) = self.lancedb.open_table().await {
            table.count_rows(None).await.map(|n| n == 0).unwrap_or(true)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use swiftide::{
        indexing::{transformers::metadata_qa_code, Node},
        traits::{NodeCache, Persist},
    };

    use crate::test_utils::{self, TestGuard};

    use super::*;

    struct TestContext {
        redb: Arc<Redb>,
        lancedb: Arc<LanceDB>,
        node: Node,
        subject: GarbageCollector<'static>,
        _guard: TestGuard,
    }

    async fn setup() -> TestContext {
        let (repository, guard) = test_utils::test_repository();

        let tempfile = guard.tempdir.path().join("test_file");
        std::fs::write(&tempfile, "Test node").unwrap();

        let mut node = Node::builder()
            .chunk("Test node")
            .path(&tempfile)
            .build()
            .unwrap();

        node.metadata.insert("path", tempfile.display().to_string());
        node.metadata
            .insert(metadata_qa_code::NAME, "test".to_string());

        let redb = Arc::new(storage::build_redb(&repository).unwrap().build().unwrap());

        {
            redb.set(&node).await;
        }
        assert!(redb.get(&node).await);

        let lancedb = Arc::new(
            storage::build_lancedb(&repository)
                .unwrap()
                .build()
                .unwrap(),
        );
        if let Err(error) = lancedb.setup().await {
            tracing::warn!(%error, "Error setting up LanceDB");
        }
        let node = lancedb.store(node).await.unwrap();

        let subject = GarbageCollector {
            repository: Cow::Owned(repository.clone()),
            lancedb: lancedb.clone(),
            redb: redb.clone(),
        };
        TestContext {
            redb,
            lancedb,
            node,
            subject,
            _guard: guard,
        }
    }

    macro_rules! assert_rows_with_path_in_lancedb {
        ($context:expr, $path:expr, $count:expr) => {
            let predicate = format!("path = \"{}\"", $path.display());

            let count = {
                let table = $context.lancedb.open_table().await.unwrap();
                table.count_rows(Some(predicate)).await.unwrap()
            };

            assert_eq!(count, $count);
        };
    }

    #[test_log::test(tokio::test)]
    async fn test_clean_up_never_done_before() {
        let context = setup().await;

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        tracing::info!("Executing clean up for never done before test.");
        context.subject.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 0);
        assert!(!context.redb.get(&context.node).await);
    }

    #[test_log::test(tokio::test)]
    async fn test_clean_up_changed_file() {
        let context = setup().await;

        context
            .subject
            .update_last_cleaned_up_at(SystemTime::now() - Duration::from_secs(60))
            .unwrap();

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        std::process::Command::new("git")
            .arg("add")
            .arg(&context.node.path)
            .output()
            .expect("failed to stage file for git");
        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add file before removal")
            .output()
            .expect("failed to commit file");

        std::fs::remove_file(&context.node.path).unwrap();
        tracing::info!("File removed, staging deletion.");

        std::process::Command::new("git")
            .arg("add")
            .arg("-u")
            .output()
            .expect("failed to stage file for deletion");

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .output()
            .expect("failed to commit file deletion");

        tracing::info!("Clean up after file changes.");
        context.subject.clean_up().await.unwrap();

        let cache_result = context.redb.get(&context.node).await;
        tracing::debug!("Cache result after clean up: {:?}", cache_result);

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 0);
        assert!(!cache_result);
    }

    #[test_log::test(tokio::test)]
    async fn test_nothing_changed() {
        let context = setup().await;

        context
            .subject
            .update_last_cleaned_up_at(SystemTime::now() + Duration::from_secs(600))
            .unwrap();

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        tracing::info!("Executing clean up for nothing changed scenario.");
        context.subject.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);
        assert!(context.redb.get(&context.node).await);
    }

    #[test_log::test(tokio::test)]
    async fn test_detect_deleted_file() {
        let context = setup().await;

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        std::process::Command::new("git")
            .arg("add")
            .arg(&context.node.path)
            .output()
            .expect("failed to stage file for git");
        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add file before removal")
            .output()
            .expect("failed to commit file");

        std::fs::remove_file(&context.node.path).unwrap();
        tracing::info!("File removal staged for deletion test.");

        std::process::Command::new("git")
            .arg("add")
            .arg("-u")
            .output()
            .expect("failed to stage file for deletion");

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Remove file")
            .output()
            .expect("failed to commit file deletion");

        tracing::info!("Starting clean up after detecting file deletion.");
        context.subject.clean_up().await.unwrap();

        let cache_result = context.redb.get(&context.node).await;
        tracing::debug!("Cache result after detection clean up: {:?}", cache_result);

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 0);
        assert!(!cache_result);
    }
}
