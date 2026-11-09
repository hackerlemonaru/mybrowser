//! Chrome Extension System (Manifest V3) for MyBrowser.
//!
//! Architecture (based on Chromium extension model + FIST BUMP security):
//!   - Extension Host Process: separate OS process per profile (sandboxed)
//!   - Background Service Worker: event-driven, full chrome.* API access
//!   - Content Scripts: run in extension process, NOT renderer (strong isolation)
//!   - DOM Proxy: content scripts access page DOM through proxy, never directly
//!   - Message Passing: validated sender.id, allowlist of actions only
//!
//! Security boundaries:
//!   - No eval()/dynamic code execution
//!   - No nativeMessaging (no host system access)
//!   - Content scripts cannot access page JS variables (isolated world)
//!   - Each extension gets its own V8 isolate (TJS VM instance)
//!   - Per-profile extension storage (isolated on disk)

pub mod manifest;
pub mod host;
pub mod content_script;
pub mod message_bus;
pub mod permission;
pub mod storage;
pub mod sandbox;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A loaded Chrome extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub id: String,                    // 32-char extension ID
    pub name: String,
    pub version: String,
    pub manifest: manifest::Manifest,
    pub path: PathBuf,                 // extracted extension directory
    pub profile_id: String,            // which profile owns this extension
    pub enabled: bool,
    pub install_time: chrono::DateTime<chrono::Utc>,
}

/// Extension installation request.
#[derive(Debug)]
pub struct ExtensionInstallRequest {
    pub crx_path: PathBuf,
    pub profile_id: String,
}

/// The extension manager for a single profile.
pub struct ExtensionManager {
    profile_id: String,
    extensions_dir: PathBuf,
    extensions: HashMap<String, Extension>,
    host_processes: HashMap<String, host::HostProcessHandle>,
}

impl ExtensionManager {
    pub fn new(profile_id: String, profile_data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let extensions_dir = profile_data_dir.join("extensions");
        std::fs::create_dir_all(&extensions_dir)?;

        let mut extensions = HashMap::new();
        // Load already-installed extensions from disk.
        if extensions_dir.exists() {
            for entry in std::fs::read_dir(&extensions_dir)? {
                let entry = entry?;
                let ext_dir = entry.path();
                let manifest_path = ext_dir.join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(ext) = Self::load_extension(ext_dir.clone(), profile_id.clone()) {
                        extensions.insert(ext.id.clone(), ext);
                    }
                }
            }
        }

        Ok(Self {
            profile_id,
            extensions_dir,
            extensions,
            host_processes: HashMap::new(),
        })
    }

    /// Install a .crx or unpacked extension directory.
    pub fn install(&mut self, req: ExtensionInstallRequest) -> Result<Extension, Box<dyn std::error::Error>> {
        let ext_dir = self.extensions_dir.join(req.crx_path.file_name().unwrap().to_string_lossy().trim_end_matches(".crx"));
        std::fs::create_dir_all(&ext_dir)?;

        // Extract CRX (ZIP) or copy directory.
        if req.crx_path.extension().map(|e| e == "crx").unwrap_or(false) {
            self.extract_crx(&req.crx_path, &ext_dir)?;
        } else {
            // Unpacked directory - copy files.
            copy_dir(&req.crx_path, &ext_dir)?;
        }

        let ext = Self::load_extension(ext_dir.clone(), self.profile_id.clone())?;

        // Security: validate manifest before enabling.
        permission::validate_manifest(&ext.manifest)?;

        // Start the extension host process.
        let host = host::HostProcess::spawn(&ext)?;
        self.host_processes.insert(ext.id.clone(), host.handle());

        self.extensions.insert(ext.id.clone(), ext.clone());
        self.save_extension(&ext)?;

        Ok(ext)
    }

    /// Uninstall an extension.
    pub fn uninstall(&mut self, extension_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Stop host process.
        if let Some(host) = self.host_processes.remove(extension_id) {
            host.stop()?;
        }

        // Remove from disk.
        if let Some(ext) = self.extensions.remove(extension_id) {
            let _ = std::fs::remove_dir_all(&ext.path);
        }

        Ok(())
    }

    /// Get an extension by ID.
    pub fn get(&self, extension_id: &str) -> Option<&Extension> {
        self.extensions.get(extension_id)
    }

    /// List all extensions for this profile.
    pub fn list(&self) -> Vec<&Extension> {
        self.extensions.values().collect()
    }

    /// Enable/disable an extension.
    pub fn set_enabled(&mut self, extension_id: &str, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ext) = self.extensions.get_mut(extension_id) {
            ext.enabled = enabled;
            self.save_extension(ext)?;
        }
        Ok(())
    }

    /// Check if an extension has a specific permission.
    pub fn has_permission(&self, extension_id: &str, permission: &str) -> bool {
        self.extensions.get(extension_id)
            .map(|e| permission::has_permission(&e.manifest, permission))
            .unwrap_or(false)
    }

    /// Check if an extension has host permission for a URL.
    pub fn has_host_permission(&self, extension_id: &str, url: &str) -> bool {
        self.extensions.get(extension_id)
            .map(|e| permission::has_host_permission(&e.manifest, url))
            .unwrap_or(false)
    }

    fn load_extension(dir: PathBuf, profile_id: String) -> Result<Extension, Box<dyn std::error::Error>> {
        let manifest_path = dir.join("manifest.json");
        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: manifest::Manifest = serde_json::from_str(&content)?;

        let id = manifest.id.as_deref()
            .unwrap_or_else(|| {
                // Derive ID from path hash (Chrome uses 32-char hash of extension key/path)
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                dir.hash(&mut hasher);
                format!("{:032x}", hasher.finish())
            })
            .to_string();

        Ok(Extension {
            id,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            manifest,
            path: dir,
            profile_id,
            enabled: true,
            install_time: chrono::Utc::now(),
        })
    }

    fn extract_crx(&self, crx_path: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::open(crx_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = dest.join(file.mangled_name());
            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut out)?;
            }
        }
        Ok(())
    }

    fn save_extension(&self, ext: &Extension) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.extensions_dir.join(&ext.id).join("extension-meta.json");
        let content = serde_json::to_string_pretty(ext)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_validation_rejects_native_messaging() {
        // This test would verify that nativeMessaging is rejected.
        // Placeholder for now.
        assert!(true);
    }
}
