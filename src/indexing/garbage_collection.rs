//! This module identifies files changed since the last index date and removes them from the index.
//!
//!
//! NOTE: If more general settings are added to Redb, better extract this to a more general place.

use anyhow::Result;
use std::{borrow::Cow, path::PathBuf, time::SystemTime};
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
    lancedb: LanceDB,
    redb: Redb,
    /// Extensions to consider for GC
    file_extensions: Vec<&'repository str>,
}

impl<'repository> GarbageCollector<'repository> {
    pub fn from_repository(repository: &'repository Repository) -> Self {
        let mut file_extensions = repository.config().language.file_extensions().to_vec();
        file_extensions.push("md");

        Self {
            repository: Cow::Borrowed(repository),
            lancedb: storage::get_lancedb(repository),
            redb: storage::get_redb(repository),
            file_extensions,
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

    fn update_last_cleaned_up_at(&self, date: SystemTime) {
        if let Err(e) = self.runtime_settings().set(LAST_CLEANED_UP_AT, date) {
            tracing::error!("Failed to update last cleaned up at: {:#}", e);
        }
    }

    fn files_deleted_since_last_index(&self) -> Vec<PathBuf> {
        let Some(timestamp) = self.get_last_cleaned_up_at() else {
            return vec![];
        };
        // if current dir is not a git repository, we can't determine deleted files
        // so just return an empty list
        if !self.repository.path().join(".git").exists() {
            return vec![];
        }

        let before = format!(
            "--before={}",
            timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        // Adjust git command to ensure accurate detection
        let last_indexed_commit_command = std::process::Command::new("git")
            .args(["rev-list", "-1", &before, "HEAD"])
            .current_dir(self.repository.path())
            .output()
            .expect("Failed to execute git rev-list command");

        let last_indexed_commit = String::from_utf8_lossy(&last_indexed_commit_command.stdout)
            .trim()
            .to_string();

        tracing::debug!("Determined last indexed commit: {}", last_indexed_commit);

        // Ensure deleted files are correctly tracked from last indexed state
        let output = std::process::Command::new("git")
            .args([
                "diff",
                "--name-only",
                "--diff-filter=D",
                &format!("{last_indexed_commit}^..HEAD"),
            ])
            .current_dir(self.repository.path())
            .output()
            .expect("Failed to execute git diff command");

        let deleted_files = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("Deleted files detected: {deleted_files}");
        deleted_files
            .lines()
            .filter_map(|p| {
                // Only consider files with the given extensions
                self.file_extensions
                    .iter()
                    .find(|ext| p.ends_with(*ext))
                    .map(|_| PathBuf::from(p))
            })
            .collect::<Vec<_>>()
    }

    fn files_changed_since_last_index(&self) -> Vec<PathBuf> {
        tracing::info!("Checking for files changed since last index.");

        let prefix = self.repository.path();
        let last_cleaned_up_at = self.get_last_cleaned_up_at();
        let modified_files = ignore::Walk::new(self.repository.path())
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter(|entry| {
                // If the file does not have any of the extensions, skip it
                if self
                    .file_extensions
                    .iter()
                    .all(|ext| !entry.path().to_string_lossy().ends_with(ext))
                {
                    tracing::debug!(
                        "Skipping file with extension not in list: {}",
                        entry.path().display()
                    );
                    return false;
                }

                // If no clean up is known, all files are considered changed
                let Some(last_cleaned_up_at) = last_cleaned_up_at else {
                    tracing::warn!("No last clean up date found; assuming all files changed");
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
            .map(|path| path.strip_prefix(prefix).unwrap().to_path_buf())
            .collect::<Vec<_>>();

        modified_files
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

        let prefix = self.repository.path();
        // Read all files and build a fake node
        let node_ids = files
            .iter()
            .filter_map(|path| {
                let Ok(content) = std::fs::read_to_string(prefix.join(path)) else {
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

        tracing::debug!("Node IDs to delete: {:?}", node_ids);
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
        // Introduce logging for step-by-step tracing
        tracing::info!("Starting cleanup process.");

        let files = [
            self.files_changed_since_last_index(),
            self.files_deleted_since_last_index(),
        ]
        .concat();

        if files.is_empty() {
            tracing::info!("No files changed since last index; skipping garbage collection");
            self.update_last_cleaned_up_at(SystemTime::now());
            return Ok(());
        }

        if self.never_been_indexed().await {
            tracing::warn!("No index date found; skipping garbage collection");
            self.update_last_cleaned_up_at(SystemTime::now());
            return Ok(());
        }

        tracing::warn!(
            "Found {} changed/deleted files since last index; garbage collecting ...",
            files.len()
        );

        tracing::debug!(?files, "Files changed since last index");

        {
            if let Err(e) = self.delete_files_from_cache(&files) {
                self.update_last_cleaned_up_at(SystemTime::now());
                return Err(e);
            };

            if let Err(e) = self.delete_files_from_index(files).await {
                self.update_last_cleaned_up_at(SystemTime::now());
                return Err(e);
            }
        }

        self.update_last_cleaned_up_at(SystemTime::now());

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
        indexing::{Node, transformers::metadata_qa_code},
        traits::{NodeCache, Persist},
    };

    use crate::test_utils::{self, TestGuard};

    use super::*;

    struct TestContext {
        redb: Redb,
        lancedb: LanceDB,
        node: Node,
        subject: GarbageCollector<'static>,
        _guard: TestGuard,
        repository: Repository,
    }

    async fn setup() -> TestContext {
        let (repository, guard) = test_utils::test_repository();

        let dir = repository.path();
        let tempfile = dir.join("test_file.md");
        std::fs::write(&tempfile, "Test node").unwrap();

        let relative_path = tempfile
            .strip_prefix(repository.path())
            .unwrap()
            .display()
            .to_string();
        let mut node = Node::builder()
            .chunk("Test node")
            .path(relative_path.as_str())
            .build()
            .unwrap();

        node.metadata.insert("path", relative_path.as_str());
        node.metadata
            .insert(metadata_qa_code::NAME, "test".to_string());

        let redb = storage::build_redb(&repository).unwrap();

        {
            redb.set(&node).await;
        }
        assert!(redb.get(&node).await);

        let lancedb = storage::build_lancedb(&repository).unwrap();

        if let Err(error) = lancedb.setup().await {
            tracing::warn!(%error, "Error setting up LanceDB");
        }
        let node = lancedb.store(node).await.unwrap();

        let subject = GarbageCollector {
            repository: Cow::Owned(repository.clone()),
            lancedb: lancedb.clone(),
            redb: redb.clone(),
            file_extensions: vec!["md"],
        };
        TestContext {
            redb,
            lancedb,
            node,
            subject,
            _guard: guard,
            repository,
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
            .update_last_cleaned_up_at(SystemTime::now() - Duration::from_secs(60));

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

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
            .update_last_cleaned_up_at(SystemTime::now() + Duration::from_secs(600));

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        tracing::info!("Executing clean up for nothing changed scenario.");
        context.subject.clean_up().await.unwrap();

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);
        assert!(context.redb.get(&context.node).await);
    }

    #[test_log::test(tokio::test)]
    async fn test_detect_deleted_file() {
        let context = setup().await;
        context
            .subject
            .update_last_cleaned_up_at(SystemTime::now() + Duration::from_secs(600));

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 1);

        std::process::Command::new("git")
            .arg("add")
            .arg(&context.node.path)
            .current_dir(context.repository.path())
            .output()
            .expect("failed to stage file for git");
        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add file before removal")
            .current_dir(context.repository.path())
            .output()
            .expect("failed to commit file");

        std::fs::remove_file(context.repository.path().join(&context.node.path)).unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("-u")
            .current_dir(context.repository.path())
            .output()
            .expect("failed to stage file for deletion");

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Remove file")
            .current_dir(context.repository.path())
            .output()
            .expect("failed to commit file deletion");

        // debug the git log
        let output = std::process::Command::new("git")
            .arg("log")
            .current_dir(context.repository.path())
            .output()
            .expect("failed to execute git log command");

        tracing::debug!("Git log:\n{}", String::from_utf8_lossy(&output.stdout));

        tracing::info!("Starting clean up after detecting file deletion.");
        context.subject.clean_up().await.unwrap();

        let cache_result = context.redb.get(&context.node).await;
        tracing::debug!("Cache result after detection clean up: {:?}", cache_result);

        assert_rows_with_path_in_lancedb!(&context, context.node.path, 0);

        // TODO: Figure out a nice way to deal with clearing the cache on removed files
        // Since we hash on the content, we cannot get the cache key properly
        // If the file gets added again with the exact same content, it will not be indexed
        // assert!(!cache_result);
    }

    #[test_log::test(tokio::test)]
    async fn test_file_extension_filtering() {
        let context = setup().await;

        // Add a file with an extension that should be filtered out
        let filtered_file = context.repository.path().join("filtered_file.txt");
        std::fs::write(&filtered_file, "This should be filtered out").unwrap();

        // Add a file with an extension that should be included
        let included_file = context.repository.path().join("included_file.md");
        std::fs::write(&included_file, "This should be included").unwrap();

        // Update the last cleaned up time to ensure both files are considered
        context
            .subject
            .update_last_cleaned_up_at(SystemTime::now() - Duration::from_secs(60));

        // Perform cleanup
        context.subject.clean_up().await.unwrap();

        // Check that the file with the filtered extension is not in the index
        assert_rows_with_path_in_lancedb!(
            &context,
            filtered_file
                .strip_prefix(context.repository.path())
                .unwrap(),
            0
        );

        // Check that the file with the included extension is in the index
        assert_rows_with_path_in_lancedb!(
            &context,
            included_file
                .strip_prefix(context.repository.path())
                .unwrap(),
            0
        );
    }
}
