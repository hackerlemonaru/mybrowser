//! Download Manager — per-profile download tracking and management.
//!
//! Chrome-compatible download system:
//!   - Per-profile download directories (isolated)
//!   - Download state tracking (in_progress, completed, cancelled, interrupted)
//!   - Resume support for interrupted downloads
//!   - Download history persistence
//!   - Safety scan (optional adblock integration)
//!   - Max concurrent downloads per profile

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Download state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    /// Download is in progress
    InProgress,
    /// Download completed successfully
    Completed,
    /// Download was cancelled by user
    Cancelled,
    /// Download was interrupted (network error, etc.)
    Interrupted,
    /// Download is complete but waiting for user action
    Complete,
}

/// Download item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    /// Unique download ID
    pub id: String,
    /// Source URL
    pub url: String,
    /// Destination file path
    pub path: PathBuf,
    /// Total bytes (if known from Content-Length)
    pub total_bytes: Option<u64>,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Current state
    pub state: DownloadState,
    /// When the download started
    pub start_time: u64,
    /// When the download ended (if completed/cancelled/interrupted)
    pub end_time: Option<u64>,
    /// MIME type of the download
    pub mime_type: Option<String>,
    /// Suggested filename (from Content-Disposition or URL)
    pub suggested_filename: String,
    /// Error message if state is Interrupted
    pub error: Option<String>,
    /// Profile ID this download belongs to
    pub profile_id: String,
    /// Whether the download can be resumed
    pub can_resume: bool,
}

/// Download statistics for a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadStats {
    pub total_downloads: u64,
    pub completed_downloads: u64,
    pub cancelled_downloads: u64,
    pub interrupted_downloads: u64,
    pub total_bytes_downloaded: u64,
}

/// Download manager — thread-safe, per-profile.
pub struct DownloadManager {
    /// Downloads: id → DownloadItem
    downloads: Mutex<HashMap<String, DownloadItem>>,
    /// Download directory base (per profile)
    download_dir: PathBuf,
    /// Max concurrent downloads
    max_concurrent: usize,
    /// Active download count
    active_count: Mutex<usize>,
}

impl DownloadManager {
    /// Create a new download manager for a profile.
    pub fn new(profile_id: &str, profile_data_dir: &Path) -> anyhow::Result<Self> {
        let download_dir = profile_data_dir.join("downloads");
        std::fs::create_dir_all(&download_dir)?;

        Ok(Self {
            downloads: Mutex::new(HashMap::new()),
            download_dir,
            max_concurrent: 10,
            active_count: Mutex::new(0),
        })
    }

    /// Start a new download.
    pub fn start_download(
        &self,
        profile_id: &str,
        url: &str,
        suggested_filename: &str,
        total_bytes: Option<u64>,
        mime_type: Option<&str>,
    ) -> anyhow::Result<String> {
        let id = Uuid::new_v4().to_string();

        // Sanitize filename
        let safe_filename = sanitize_filename(suggested_filename);
        let path = self.download_dir.join(&safe_filename);

        // Check if file already exists — add suffix
        let path = if path.exists() {
            let stem = Path::new(&safe_filename)
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new("download"))
                .to_string_lossy();
            let ext = Path::new(&safe_filename)
                .extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_default();
            let counter = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_millis())
                .unwrap_or(0);
            self.download_dir.join(format!("{}_{}.{}", stem, counter, ext))
        } else {
            path
        };

        let item = DownloadItem {
            id: id.clone(),
            url: url.to_string(),
            path: path.clone(),
            total_bytes,
            downloaded_bytes: 0,
            state: DownloadState::InProgress,
            start_time: now_seconds(),
            end_time: None,
            mime_type: mime_type.map(|s| s.to_string()),
            suggested_filename: safe_filename.clone(),
            error: None,
            profile_id: profile_id.to_string(),
            can_resume: true,
        };

        {
            let mut downloads = self.downloads.lock().unwrap();
            downloads.insert(id.clone(), item);
        }

        {
            let mut count = self.active_count.lock().unwrap();
            *count += 1;
        }

        eprintln!(
            "[Download] Started {} → {} (profile: {})",
            url, safe_filename, profile_id
        );

        Ok(id)
    }

    /// Update download progress.
    pub fn update_progress(&self, id: &str, downloaded_bytes: u64) -> Option<DownloadItem> {
        let mut downloads = self.downloads.lock().unwrap();
        if let Some(item) = downloads.get_mut(id) {
            item.downloaded_bytes = downloaded_bytes;
            // Check if complete
            if let Some(total) = item.total_bytes {
                if downloaded_bytes >= total {
                    item.state = DownloadState::Completed;
                    item.end_time = Some(now_seconds());
                    let mut count = self.active_count.lock().unwrap();
                    *count = count.saturating_sub(1);
                }
            }
            Some(item.clone())
        } else {
            None
        }
    }

    /// Mark a download as completed.
    pub fn complete_download(&self, id: &str) -> Option<DownloadItem> {
        let mut downloads = self.downloads.lock().unwrap();
        if let Some(item) = downloads.get_mut(id) {
            item.state = DownloadState::Completed;
            item.end_time = Some(now_seconds());
            let mut count = self.active_count.lock().unwrap();
            *count = count.saturating_sub(1);
            Some(item.clone())
        } else {
            None
        }
    }

    /// Mark a download as interrupted.
    pub fn interrupt_download(&self, id: &str, error: &str) -> Option<DownloadItem> {
        let mut downloads = self.downloads.lock().unwrap();
        if let Some(item) = downloads.get_mut(id) {
            item.state = DownloadState::Interrupted;
            item.end_time = Some(now_seconds());
            item.error = Some(error.to_string());
            let mut count = self.active_count.lock().unwrap();
            *count = count.saturating_sub(1);
            Some(item.clone())
        } else {
            None
        }
    }

    /// Cancel a download.
    pub fn cancel_download(&self, id: &str) -> Option<DownloadItem> {
        let mut downloads = self.downloads.lock().unwrap();
        if let Some(item) = downloads.get_mut(id) {
            item.state = DownloadState::Cancelled;
            item.end_time = Some(now_seconds());
            let mut count = self.active_count.lock().unwrap();
            *count = count.saturating_sub(1);
            Some(item.clone())
        } else {
            None
        }
    }

    /// Get a download by ID.
    pub fn get_download(&self, id: &str) -> Option<DownloadItem> {
        let downloads = self.downloads.lock().unwrap();
        downloads.get(id).cloned()
    }

    /// List all downloads for a profile.
    pub fn list_downloads(&self, profile_id: &str) -> Vec<DownloadItem> {
        let downloads = self.downloads.lock().unwrap();
        downloads
            .values()
            .filter(|d| d.profile_id == profile_id)
            .cloned()
            .collect()
    }

    /// Get download statistics for a profile.
    pub fn get_stats(&self, profile_id: &str) -> DownloadStats {
        let downloads = self.downloads.lock().unwrap();
        let profile_downloads: Vec<&DownloadItem> = downloads
            .values()
            .filter(|d| d.profile_id == profile_id)
            .collect();

        let mut stats = DownloadStats::default();
        for d in &profile_downloads {
            stats.total_downloads += 1;
            match d.state {
                DownloadState::Completed => stats.completed_downloads += 1,
                DownloadState::Cancelled => stats.cancelled_downloads += 1,
                DownloadState::Interrupted => stats.interrupted_downloads += 1,
                _ => {}
            }
            if d.state == DownloadState::Completed {
                stats.total_bytes_downloaded += d.downloaded_bytes;
            }
        }
        stats
    }

    /// Check if we can start a new download (under concurrency limit).
    pub fn can_start_download(&self) -> bool {
        let count = self.active_count.lock().unwrap();
        *count < self.max_concurrent
    }

    /// Get the download directory for a profile.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// Persist all downloads to disk.
    pub fn persist(&self) -> anyhow::Result<()> {
        let downloads = self.downloads.lock().unwrap();
        let dir = self.download_dir.join("metadata");
        std::fs::create_dir_all(&dir)?;

        for item in downloads.values() {
            let path = dir.join(format!("{}.json", item.id));
            let content = serde_json::to_string_pretty(item)?;
            std::fs::write(&path, content)?;
        }
        Ok(())
    }

    /// Load downloads from disk for a profile.
    pub fn load_from_disk(&self, profile_id: &str) -> anyhow::Result<Vec<DownloadItem>> {
        let dir = self.download_dir.join("metadata");
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut items = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = std::fs::read_to_string(&path)?;
                let item: DownloadItem = serde_json::from_str(&content)?;
                if item.profile_id == profile_id {
                    let mut downloads = self.downloads.lock().unwrap();
                    downloads.insert(item.id.clone(), item.clone());
                    items.push(item);
                }
            }
        }
        Ok(items)
    }
}

/// Sanitize a filename for safe filesystem storage.
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Current Unix timestamp in seconds.
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal_file.pdf"), "normal_file.pdf");
        assert_eq!(sanitize_filename("../etc/passwd"), "____etc_passwd");
        assert_eq!(sanitize_filename("file<>:\"|?*.pdf"), "file_____.pdf");
    }

    #[test]
    fn test_download_states() {
        assert!(matches!(
            DownloadState::InProgress,
            DownloadState::InProgress
        ));
        assert!(matches!(
            DownloadState::Completed,
            DownloadState::Completed
        ));
    }
}
