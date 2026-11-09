//! Per-profile localStorage.
//! Stores key-value pairs per origin, persisted as JSON.
//! Each profile has its own localStorage — no cross-profile access.

use crate::{ensure_dir, origin_key, read_json, write_json, StorageKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// A localStorage entry for one origin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalStorage {
    pub origin: String,
    pub items: BTreeMap<String, String>,
}

/// Load localStorage for an origin within a profile.
pub fn load(profile_dir: &Path, origin: &str) -> LocalStorage {
    let dir = ensure_dir(profile_dir, StorageKind::LocalStorage).unwrap();
    let path = dir.join(format!("{}.json", origin_key(origin)));
    read_json(&path, LocalStorage { origin: origin.to_string(), items: BTreeMap::new() })
}

/// Save localStorage for an origin within a profile.
pub fn save(profile_dir: &Path, storage: &LocalStorage) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::LocalStorage).unwrap();
    let path = dir.join(format!("{}.json", origin_key(&storage.origin)));
    write_json(&path, storage)
}

/// Get a single value from localStorage.
pub fn get(profile_dir: &Path, origin: &str, key: &str) -> Option<String> {
    let storage = load(profile_dir, origin);
    storage.items.get(key).cloned()
}

/// Set a single value in localStorage.
pub fn set(
    profile_dir: &Path,
    origin: &str,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut storage = load(profile_dir, origin);
    storage.items.insert(key.to_string(), value.to_string());
    save(profile_dir, &storage)
}

/// Remove a key from localStorage.
pub fn remove(profile_dir: &Path, origin: &str, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut storage = load(profile_dir, origin);
    storage.items.remove(key);
    save(profile_dir, &storage)
}

/// Clear all localStorage for an origin.
pub fn clear(profile_dir: &Path, origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let storage = LocalStorage { origin: origin.to_string(), items: BTreeMap::new() };
    save(profile_dir, &storage)
}

/// List all origins that have localStorage in this profile.
pub fn list_origins(profile_dir: &Path) -> Vec<String> {
    let dir = match ensure_dir(profile_dir, StorageKind::LocalStorage) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut origins = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Reverse the sanitization roughly for display.
                    origins.push(stem.replace('_', "://").replace("_", "/"));
                }
            }
        }
    }
    origins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let dir = tempfile::tempdir().unwrap();
        set(dir.path(), "https://example.com", "theme", "dark").unwrap();
        let val = get(dir.path(), "https://example.com", "theme");
        assert_eq!(val, Some("dark".to_string()));
        let missing = get(dir.path(), "https://example.com", "nonexistent");
        assert_eq!(missing, None);
    }
}
