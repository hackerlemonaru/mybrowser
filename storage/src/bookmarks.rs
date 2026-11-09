//! Per-profile bookmarks.
//! Saved URLs organized in folders, isolated per profile.

use crate::{ensure_dir, read_json, write_json, StorageKind};
use chrono::{Utc, DateTime};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single bookmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub folder: String,
    pub created_at: DateTime<Utc>,
}

/// The full bookmarks file for a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bookmarks {
    pub items: Vec<Bookmark>,
}

/// Add a new bookmark.
pub fn add(
    profile_dir: &Path,
    url: &str,
    title: &str,
    folder: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::Bookmarks)?;
    let path = dir.join("bookmarks.json");
    let mut bookmarks: Bookmarks = read_json(&path, Bookmarks::default());

    // Don't duplicate existing URL.
    if bookmarks.items.iter().any(|b| b.url == url) {
        return Ok(());
    }

    bookmarks.items.push(Bookmark {
        id: uuid::Uuid::new_v4().to_string(),
        url: url.to_string(),
        title: title.to_string(),
        folder: folder.to_string(),
        created_at: Utc::now(),
    });

    write_json(&path, &bookmarks)
}

/// List all bookmarks.
pub fn list(profile_dir: &Path) -> Vec<Bookmark> {
    let dir = match ensure_dir(profile_dir, StorageKind::Bookmarks) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let path = dir.join("bookmarks.json");
    let bookmarks: Bookmarks = read_json(&path, Bookmarks::default());
    bookmarks.items
}

/// Remove a bookmark by ID.
pub fn remove(profile_dir: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::Bookmarks)?;
    let path = dir.join("bookmarks.json");
    let mut bookmarks: Bookmarks = read_json(&path, Bookmarks::default());
    bookmarks.items.retain(|b| b.id != id);
    write_json(&path, &bookmarks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_list() {
        let dir = tempfile::tempdir().unwrap();
        add(dir.path(), "https://example.com", "Example", "Work").unwrap();
        add(dir.path(), "https://rust-lang.org", "Rust", "Work").unwrap();

        let items = list(dir.path());
        assert_eq!(items.len(), 2);
    }
}
