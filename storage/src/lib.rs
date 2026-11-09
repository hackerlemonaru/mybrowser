//! Per-profile storage for the browser.
//! Each profile gets its own isolated storage:
//!   - localStorage: key-value per origin
//!   - IndexedDB: object stores per origin
//!   - History: visited URLs with timestamps
//!   - Bookmarks: saved URLs
//!
//! All data is stored as JSON files under each profile's data directory,
//! giving Chrome-level isolation with zero cross-profile leakage.

pub mod local_storage;
pub mod indexed_db;
pub mod history;
pub mod bookmarks;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Type of storage data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageKind {
    LocalStorage,
    IndexedDB,
    History,
    Bookmarks,
}

/// Ensure a profile's storage directory exists for a given kind.
pub fn ensure_dir(data_dir: &Path, kind: StorageKind) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = data_dir.join(kind.dir_name());
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Read JSON from a file, returning default if missing/corrupt.
pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, default: T) -> T {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or(default),
        Err(_) => default,
    }
}

/// Write JSON to a file atomically (temp then rename).
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(value)?;
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Generate a unique key for an origin (e.g. "https://example.com").
pub fn origin_key(origin: &str) -> String {
    // Sanitize origin for use as a filename.
    origin
        .replace("://", "_")
        .replace('/', "_")
        .replace(':', "_")
        .replace('?', "_")
        .replace('&', "_")
        .replace('=', "_")
}

impl StorageKind {
    pub fn dir_name(self) -> &'static str {
        match self {
            StorageKind::LocalStorage => "localstorage",
            StorageKind::IndexedDB => "indexeddb",
            StorageKind::History => "history",
            StorageKind::Bookmarks => "bookmarks",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_key_is_safe_filename() {
        let key = origin_key("https://example.com/path?a=1&b=2");
        assert!(!key.contains('/'));
        assert!(!key.contains(':'));
        assert!(!key.contains('?'));
        assert!(!key.contains('&'));
    }
}
