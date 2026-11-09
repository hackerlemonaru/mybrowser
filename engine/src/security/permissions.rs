//! Permission system — controls what sites and extensions can access.
//! Camera, microphone, geolocation, notifications, etc.
//! Permissions are per-profile and per-origin.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// A permission type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    Camera,
    Microphone,
    Geolocation,
    Notifications,
    ClipboardRead,
    ClipboardWrite,
    PointerLock,
    Fullscreen,
    StorageAccess,
    MidiSysex,
    Bluetooth,
    USB,
    Serial,
}

/// Permission state for an origin within a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionState {
    Allow,
    Deny,
    Ask, // Prompt the user
}

/// A permission grant record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub profile_id: String,
    pub origin: String,
    pub permission: Permission,
    pub state: PermissionState,
    pub last_asked: Option<chrono::DateTime<chrono::Utc>>,
}

/// Manages permission grants.
pub struct PermissionManager {
    grants: Mutex<HashMap<(String, String, String), PermissionGrant>>,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
        }
    }

    /// Set a permission state for an origin.
    pub fn set(
        &self,
        profile_id: &str,
        origin: &str,
        permission: Permission,
        state: PermissionState,
    ) {
        let key = (profile_id.to_string(), origin.to_string(), format!("{:?}", permission));
        let grant = PermissionGrant {
            profile_id: profile_id.to_string(),
            origin: origin.to_string(),
            permission,
            state,
            last_asked: if state == PermissionState::Ask {
                Some(chrono::Utc::now())
            } else {
                None
            },
        };
        self.grants.lock().unwrap().insert(key, grant);
    }

    /// Get the permission state for an origin.
    pub fn get(&self, profile_id: &str, origin: &str, permission: Permission) -> PermissionState {
        let key = (profile_id.to_string(), origin.to_string(), format!("{:?}", permission));
        self.grants.lock().unwrap()
            .get(&key)
            .map(|g| g.state)
            .unwrap_or(PermissionState::Ask)
    }

    /// Check if a permission is allowed.
    pub fn is_allowed(&self, profile_id: &str, origin: &str, permission: Permission) -> bool {
        self.get(profile_id, origin, permission) == PermissionState::Allow
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_defaults_to_ask() {
        let mgr = PermissionManager::new();
        assert_eq!(
            mgr.get("profile-1", "https://example.com", Permission::Camera),
            PermissionState::Ask
        );
    }

    #[test]
    fn set_and_get() {
        let mgr = PermissionManager::new();
        mgr.set("profile-1", "https://example.com", Permission::Camera, PermissionState::Allow);
        assert!(mgr.is_allowed("profile-1", "https://example.com", Permission::Camera));
        assert!(!mgr.is_allowed("profile-1", "https://example.com", Permission::Microphone));
    }
}
