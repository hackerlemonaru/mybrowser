//! Per-profile browsing history.
//! Each visited URL is recorded with timestamp, title, and visit count.
//! Isolated per profile — Work history never mixes with Personal history.

use crate::{ensure_dir, read_json, write_json, StorageKind};
use chrono::{Utc, DateTime};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visited: DateTime<Utc>,
    pub first_visited: DateTime<Utc>,
}

/// The full history file for a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

/// Add a visit to history (increment count if URL already exists).
pub fn add_visit(profile_dir: &Path, url: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::History)?;
    let path = dir.join("history.json");
    let mut history: History = read_json(&path, History::default());

    let now = Utc::now();
    if let Some(entry) = history.entries.iter_mut().find(|e| e.url == url) {
        entry.visit_count += 1;
        entry.last_visited = now;
        if !title.is_empty() {
            entry.title = title.to_string();
        }
    } else {
        history.entries.push(HistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            visit_count: 1,
            last_visited: now,
            first_visited: now,
        });
    }

    // Keep only the most recent 50,000 entries.
    history.entries.sort_by(|a, b| b.last_visited.cmp(&a.last_visited));
    history.entries.truncate(50_000);

    write_json(&path, &history)
}

/// Get all history entries, most recent first.
pub fn list(profile_dir: &Path) -> Vec<HistoryEntry> {
    let dir = match ensure_dir(profile_dir, StorageKind::History) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let path = dir.join("history.json");
    let history: History = read_json(&path, History::default());
    let mut entries = history.entries;
    entries.sort_by(|a, b| b.last_visited.cmp(&a.last_visited));
    entries
}

/// Clear all history for a profile.
pub fn clear(profile_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::History)?;
    let path = dir.join("history.json");
    write_json(&path, &History::default())
}

/// Search history by URL or title substring.
pub fn search(profile_dir: &Path, query: &str) -> Vec<HistoryEntry> {
    let q = query.to_lowercase();
    list(profile_dir)
        .into_iter()
        .filter(|e| e.url.to_lowercase().contains(&q) || e.title.to_lowercase().contains(&q))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_list() {
        let dir = tempfile::tempdir().unwrap();
        add_visit(dir.path(), "https://example.com", "Example").unwrap();
        add_visit(dir.path(), "https://example.com", "Example").unwrap();
        add_visit(dir.path(), "https://rust-lang.org", "Rust").unwrap();

        let entries = list(dir.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://example.com");
        assert_eq!(entries[0].visit_count, 2);
    }
}
