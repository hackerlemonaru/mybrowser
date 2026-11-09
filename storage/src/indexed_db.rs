//! Per-profile IndexedDB-lite.
//! Simple object-store-per-origin key-value storage.
//! Isolated per profile — no cross-profile IndexedDB access.

use crate::{ensure_dir, origin_key, read_json, write_json, StorageKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// One IndexedDB object store for one origin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectStore {
    pub origin: String,
    pub store_name: String,
    pub entries: BTreeMap<String, serde_json::Value>,
}

/// Load an object store for an origin within a profile.
pub fn load(profile_dir: &Path, origin: &str, store_name: &str) -> ObjectStore {
    let dir = ensure_dir(profile_dir, StorageKind::IndexedDB).unwrap();
    let key = format!("{}_{}", origin_key(origin), store_name);
    let path = dir.join(format!("{}.json", key));
    read_json(&path, ObjectStore { origin: origin.to_string(), store_name: store_name.to_string(), entries: BTreeMap::new() })
}

/// Save an object store.
pub fn save(profile_dir: &Path, store: &ObjectStore) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::IndexedDB).unwrap();
    let key = format!("{}_{}", origin_key(&store.origin), &store.store_name);
    let path = dir.join(format!("{}.json", key));
    write_json(&path, store)
}

/// Set a value in an object store.
pub fn set(
    profile_dir: &Path,
    origin: &str,
    store_name: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = load(profile_dir, origin, store_name);
    store.entries.insert(key.to_string(), value);
    save(profile_dir, &store)
}

/// Get a value from an object store.
pub fn get(profile_dir: &Path, origin: &str, store_name: &str, key: &str) -> Option<serde_json::Value> {
    let store = load(profile_dir, origin, store_name);
    store.entries.get(key).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let dir = tempfile::tempdir().unwrap();
        set(
            dir.path(),
            "https://example.com",
            "drafts",
            "post_1",
            serde_json::json!({"title": "Hello"}),
        )
        .unwrap();
        let val = get(dir.path(), "https://example.com", "drafts", "post_1");
        assert_eq!(val, Some(serde_json::json!({"title": "Hello"})));
    }
}
