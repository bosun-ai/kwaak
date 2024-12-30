//! This module identifies files changed since the last index date and removes them from the index.
//!
//!
//! NOTE: If more general settings are added to Redb, better extract this to a more general place.

use chrono::{DateTime, Local};

use crate::{repository::Repository, storage};

pub struct GarbageCollector {
    /// The repository to garbage collect
    repository: Repository,
    /// The last index date
    last_index_date: DateTime<Local>,
}
