//! Per-profile download storage.
//! Download metadata is stored as JSON files under each profile's data directory.
//! Isolated per profile — Work downloads never mix with Personal downloads.

use crate::{ensure_dir, read_json, write_json, StorageKind};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Download state (local copy to avoid engine dependency in storage crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    InProgress,
    Completed,
    Cancelled,
    Interrupted,
    Complete,
}

/// The full download history file for a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadHistory {
    pub items: Vec<DownloadRecord>,
}

/// A download record (persisted form of DownloadItem).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub state: DownloadState,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub mime_type: Option<String>,
    pub suggested_filename: String,
    pub error: Option<String>,
    pub profile_id: String,
    pub can_resume: bool,
}

/// Add a download record to history.
pub fn add_record(
    profile_dir: &Path,
    record: &DownloadRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::History)?;
    let path = dir.join("downloads.json");
    let mut history: DownloadHistory = read_json(&path, DownloadHistory::default());

    // Don't duplicate by ID.
    if history.items.iter().any(|r| r.id == record.id) {
        return Ok(());
    }

    history.items.push(record.clone());

    // Keep only the most recent 10,000 entries.
    history.items.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    history.items.truncate(10_000);

    write_json(&path, &history)
}

/// Update a download record's state and progress.
pub fn update_record(
    profile_dir: &Path,
    id: &str,
    state: DownloadState,
    downloaded_bytes: u64,
    error: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = ensure_dir(profile_dir, StorageKind::History)?;
    let path = dir.join("downloads.json");
    let mut history: DownloadHistory = read_json(&path, DownloadHistory::default());

    if let Some(record) = history.items.iter_mut().find(|r| r.id == id) {
        record.state = state;
        record.downloaded_bytes = downloaded_bytes;
        record.end_time = Some(Utc::now().timestamp() as u64);
        if let Some(e) = error {
            record.error = Some(e.to_string());
        }
    }

    write_json(&path, &history)
}

/// List all download records for a profile, most recent first.
pub fn list(profile_dir: &Path) -> Vec<DownloadRecord> {
    let dir = match ensure_dir(profile_dir, StorageKind::History) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let path = dir.join("downloads.json");
    let history: DownloadHistory = read_json(&path, DownloadHistory::default());
    history.items
}

/// Download statistics for a profile (local type to avoid engine dependency).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadStats {
    pub total_downloads: u64,
    pub completed_downloads: u64,
    pub cancelled_downloads: u64,
    pub interrupted_downloads: u64,
    pub total_bytes_downloaded: u64,
}

/// Get download statistics for a profile.
pub fn stats(profile_dir: &Path) -> DownloadStats {
    let records = list(profile_dir);
    let mut stats = DownloadStats::default();
    for r in &records {
        stats.total_downloads += 1;
        match r.state {
            DownloadState::Completed => stats.completed_downloads += 1,
            DownloadState::Cancelled => stats.cancelled_downloads += 1,
            DownloadState::Interrupted => stats.interrupted_downloads += 1,
            _ => {}
        }
        if r.state == DownloadState::Completed {
            stats.total_bytes_downloaded += r.downloaded_bytes;
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_record_serialization() {
        let record = DownloadRecord {
            id: "test-123".to_string(),
            url: "https://example.com/file.pdf".to_string(),
            path: "/downloads/file.pdf".to_string(),
            total_bytes: Some(1000),
            downloaded_bytes: 500,
            state: DownloadState::InProgress,
            start_time: 1234567890,
            end_time: None,
            mime_type: Some("application/pdf".to_string()),
            suggested_filename: "file.pdf".to_string(),
            error: None,
            profile_id: "profile1".to_string(),
            can_resume: true,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: DownloadRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, record.id);
        assert_eq!(deserialized.url, record.url);
    }
}
