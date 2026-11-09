//! Per-profile, per-extension storage.
//! Isolated on disk - extension A cannot read extension B's data.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Storage area types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageArea {
    Local,   // Persists across sessions
    Session, // Cleared when browser closes
    Sync,    // Synced across devices (not implemented - local only)
}

/// A stored value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredValue {
    pub value: serde_json::Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Per-extension storage.
pub struct ExtensionStorage {
    profile_id: String,
    extension_id: String,
    storage_dir: PathBuf,
    cache: std::collections::HashMap<StorageArea, BTreeMap<String, StoredValue>>,
}

impl ExtensionStorage {
    pub fn new(profile_id: &str, extension_id: &str, profile_data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let storage_dir = profile_data_dir.join("extensions").join(extension_id).join("storage");
        fs::create_dir_all(&storage_dir)?;

        let mut cache = std::collections::HashMap::new();
        for area in [StorageArea::Local, StorageArea::Session, StorageArea::Sync] {
            cache.insert(area, Self::load_area(&storage_dir, area)?);
        }

        Ok(Self {
            profile_id: profile_id.to_string(),
            extension_id: extension_id.to_string(),
            storage_dir,
            cache,
        })
    }

    /// Get a value from storage.
    pub fn get(&self, area: StorageArea, key: &str) -> Option<serde_json::Value> {
        self.cache.get(&area)
            .and_then(|m| m.get(key))
            .map(|v| v.value.clone())
    }

    /// Set a value in storage.
    pub fn set(
        &mut self,
        area: StorageArea,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entry = StoredValue {
            value,
            updated_at: chrono::Utc::now(),
        };

        // Update memory cache.
        self.cache.entry(area).or_default().insert(key.to_string(), entry);

        // Persist to disk.
        self.persist_area(area)?;

        Ok(())
    }

    /// Remove a key from storage.
    pub fn remove(&mut self, area: StorageArea, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.cache.entry(area).or_default().remove(key);
        self.persist_area(area)?;
        Ok(())
    }

    /// Clear all storage for an area.
    pub fn clear(&mut self, area: StorageArea) -> Result<(), Box<dyn std::error::Error>> {
        self.cache.entry(area).or_default().clear();
        self.persist_area(area)?;
        Ok(())
    }

    /// Get all keys in an area.
    pub fn keys(&self, area: StorageArea) -> Vec<String> {
        self.cache.get(&area)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn load_area(
        storage_dir: &Path,
        area: StorageArea,
    ) -> Result<BTreeMap<String, StoredValue>, Box<dyn std::error::Error>> {
        let area_dir = storage_dir.join(format!("{:?}", area).to_lowercase());
        let mut map = BTreeMap::new();

        if area_dir.exists() {
            for entry in fs::read_dir(area_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(value) = serde_json::from_str::<StoredValue>(&content) {
                            let key = path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            map.insert(key, value);
                        }
                    }
                }
            }
        }

        Ok(map)
    }

    fn persist_area(&self, area: StorageArea) -> Result<(), Box<dyn std::error::Error>> {
        let area_dir = self.storage_dir.join(format!("{:?}", area).to_lowercase());
        fs::create_dir_all(&area_dir)?;

        if let Some(values) = self.cache.get(&area) {
            for (key, value) in values.iter() {
                let path = area_dir.join(format!("{}.json", key));
                let content = serde_json::to_string_pretty(value)?;
                fs::write(&path, content)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn set_and_get() {
        let dir = tempdir().unwrap();
        let mut storage = ExtensionStorage::new("profile-1", "ext-1", dir.path()).unwrap();
        storage.set(StorageArea::Local, "theme", serde_json::json!("dark")).unwrap();
        let val = storage.get(StorageArea::Local, "theme");
        assert_eq!(val, Some(serde_json::json!("dark")));
    }
}
