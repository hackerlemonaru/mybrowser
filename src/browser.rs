use std::path::PathBuf;

/// Browser configuration tied to a single profile. When you switch
/// profiles, you construct a new BrowserConfig pointing at the new
/// data directory, giving each workload its own cookie jar, cache,
/// localStorage, and extension set.
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub profile_id: String,
    pub profile_name: String,
    pub user_data_dir: PathBuf,
    pub user_agent: String,
    pub window_title: String,
    pub homepage: Option<String>,
}

impl BrowserConfig {
    pub fn for_profile(profile_id: &str, profile_name: &str, data_dir: PathBuf) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            profile_name: profile_name.to_string(),
            user_data_dir: data_dir,
            user_agent: "Mozilla/5.0 MyBrowser/0.1 (Rust)".to_string(),
            window_title: "MyBrowser".to_string(),
            homepage: None,
        }
    }

    /// Path to this profile's cookie store (implementation detail for
    /// whatever storage backend you wire up).
    pub fn cookies_path(&self) -> PathBuf {
        self.user_data_dir.join("cookies").join("cookies.json")
    }

    pub fn history_path(&self) -> PathBuf {
        self.user_data_dir.join("history").join("history.json")
    }

    pub fn bookmarks_path(&self) -> PathBuf {
        self.user_data_dir.join("bookmarks").join("bookmarks.json")
    }
}
