//! SPS CLI library — command handlers usable by other binaries/tests.

#![allow(clippy::module_name_repetitions)]

pub mod commands;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use sps_core::event_store::EventStore;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::storage::port::StoragePort;
use sps_storage_sqlite::SqliteStorage;

/// Default SPS data directory.
pub fn default_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".sps")
}

/// Default database path.
pub fn default_db_path() -> PathBuf {
    default_data_dir().join("sps.db")
}

/// Open a SQLite storage at the given path, creating parent dirs.
pub fn open_storage(path: &PathBuf) -> Result<Arc<dyn StoragePort>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let storage = SqliteStorage::open(path).context("failed to open SQLite storage")?;
    Ok(Arc::new(storage))
}

/// Boot the kernel against the given storage.
pub fn boot_kernel(storage: Arc<dyn StoragePort>) -> Result<SpsKernel> {
    SpsKernel::boot(storage, KernelConfig::default()).context("kernel boot failed")
}

/// Open the event store against the given storage.
pub fn open_store(storage: Arc<dyn StoragePort>) -> Result<Arc<EventStore>> {
    let store = Arc::new(EventStore::new(storage)?);
    Ok(store)
}
