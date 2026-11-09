//! Chrome-like profile manager for the browser.
//! Each profile owns an isolated data directory:
//!   cookies/, history/, bookmarks/, extensions/, cache/, localstorage/, sessions/
//! Switching profiles = switching the active data directory — zero session leakage.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub created_at: String,
    pub last_used: Option<String>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    pub active_profile_id: Option<String>,
}

pub struct ProfileManager {
    profiles_dir: PathBuf,
    list: ProfileList,
}

impl ProfileManager {
    /// Create / open the profile store under the platform data dir.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut profiles_dir = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        profiles_dir.push("mybrowser");
        profiles_dir.push("profiles");
        fs::create_dir_all(&profiles_dir)?;

        let list_path = profiles_dir.join("profiles.json");
        let list = if list_path.exists() {
            let content = fs::read_to_string(&list_path)?;
            serde_json::from_str(&content)?
        } else {
            ProfileList {
                profiles: vec![],
                active_profile_id: None,
            }
        };

        Ok(Self { profiles_dir, list })
    }

    pub fn list(&self) -> &[Profile] {
        &self.list.profiles
    }

    pub fn active(&self) -> Option<&Profile> {
        self.list
            .active_profile_id
            .as_ref()
            .and_then(|id| self.list.profiles.iter().find(|p| p.id == id))
    }

    /// Resolve a profile by exact id or by name (case-insensitive).
    pub fn resolve(&self, id_or_name: &str) -> Option<&Profile> {
        self.list
            .profiles
            .iter()
            .find(|p| p.id == id_or_name || p.name.eq_ignore_ascii_case(id_or_name))
    }

    pub fn active_data_dir(&self) -> Option<&Path> {
        self.active().map(|p| p.data_dir.as_path())
    }

    pub fn create(
        &mut self,
        name: String,
    ) -> Result<Profile, Box<dyn std::error::Error>> {
        let id = uuid::Uuid::new_v4().to_string();
        let data_dir = self.profiles_dir.join(&id);
        fs::create_dir_all(&data_dir)?;

        for sub in [
            "cookies",
            "history",
            "bookmarks",
            "extensions",
            "cache",
            "localstorage",
            "sessions",
        ] {
            fs::create_dir_all(data_dir.join(sub))?;
        }

        let profile = Profile {
            id,
            name,
            avatar: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_used: None,
            data_dir,
        };

        let first = self.list.profiles.is_empty();
        self.list.profiles.push(profile.clone());
        if first {
            self.list.active_profile_id = Some(profile.id.clone());
        }
        self.save()?;
        Ok(profile)
    }

    pub fn switch(&mut self, id_or_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let profile = self
            .resolve(id_or_name)
            .ok_or_else(|| format!("profile '{}' not found", id_or_name))?;

        self.list.active_profile_id = Some(profile.id.clone());
        if let Some(p) = self.list.profiles.iter_mut().find(|p| p.id == profile.id) {
            p.last_used = Some(chrono::Utc::now().to_rfc3339());
        }
        self.save()?;
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let idx = self
            .list
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("profile '{}' not found", id))?;
        let profile = self.list.profiles.remove(idx);

        let _ = fs::remove_dir_all(&profile.data_dir);

        if self.list.active_profile_id.as_deref() == Some(id) {
            self.list.active_profile_id = self.list.profiles.first().map(|p| p.id.clone());
        }
        self.save()?;
        Ok(())
    }

    pub fn set_avatar(&mut self, id: &str, avatar: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(p) = self.list.profiles.iter_mut().find(|p| p.id == id) {
            p.avatar = Some(avatar);
            self.save()?;
            Ok(())
        } else {
            Err(format!("profile '{}' not found", id).into())
        }
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let list_path = self.profiles_dir.join("profiles.json");
        let content = serde_json::to_string_pretty(&self.list)?;
        fs::write(list_path, content)?;
        Ok(())
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new().expect("failed to initialise profile manager")
    }
}
