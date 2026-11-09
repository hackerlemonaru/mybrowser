//! Per-profile proxy support.
//! Each profile can route its network traffic through its own proxy
//! (SOCKS5, HTTP, or direct). This gives per-profile IP isolation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// Proxy configuration for a single profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// If true, DNS resolution also goes through the proxy (SOCKS5h).
    pub proxy_dns: bool,
    /// Bypass proxy for these hosts (local addresses, etc.).
    pub bypass_list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProxyType {
    Direct,
    Http,
    Socks5,
    Socks5h, // SOCKS5 with remote DNS resolution
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: ProxyType::Direct,
            host: String::new(),
            port: 0,
            username: None,
            password: None,
            proxy_dns: true,
            bypass_list: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        }
    }
}

/// Per-profile proxy manager.
pub struct ProxyManager {
    /// Keyed by profile ID.
    configs: Mutex<HashMap<String, ProxyConfig>>,
}

static PROXY_MANAGER: once_cell::sync::Lazy<ProxyManager> = once_cell::sync::Lazy::new(|| {
    ProxyManager {
        configs: Mutex::new(HashMap::new()),
    }
});

impl ProxyManager {
    /// Set the proxy config for a profile.
    pub fn set_config(profile_id: &str, config: ProxyConfig) {
        let mut configs = Self::configs();
        configs.insert(profile_id.to_string(), config);
    }

    /// Get the proxy config for a profile.
    pub fn get_config(profile_id: &str) -> Option<ProxyConfig> {
        let configs = Self::configs();
        configs.get(profile_id).cloned()
    }

    /// Remove the proxy config for a profile (revert to direct).
    pub fn remove_config(profile_id: &str) {
        let mut configs = Self::configs();
        configs.remove(profile_id);
    }

    /// Check if a URL should bypass the proxy for a profile.
    pub fn should_bypass(profile_id: &str, url: &str) -> bool {
        let config = match Self::get_config(profile_id) {
            Some(c) => c,
            None => return true, // No proxy = always bypass
        };

        if !config.enabled {
            return true;
        }

        // Check bypass list.
        for bypass in &config.bypass_list {
            if url.contains(bypass) {
                return true;
            }
        }

        false
    }

    /// Build a proxy URL string for reqwest/ureq.
    pub fn proxy_url(profile_id: &str) -> Option<String> {
        let config = Self::get_config(profile_id)?;
        if !config.enabled {
            return None;
        }

        let auth = match (&config.username, &config.password) {
            (Some(u), Some(p)) => format!("{}:{}@", u, p),
            _ => String::new(),
        };

        match config.proxy_type {
            ProxyType::Http => Some(format!("http://{} {}:{}", auth, config.host, config.port).replace(" ", "")),
            ProxyType::Socks5 => Some(format!("socks5://{} {}:{}", auth, config.host, config.port).replace(" ", "")),
            ProxyType::Socks5h => Some(format!("socks5h://{} {}:{}", auth, config.host, config.port).replace(" ", "")),
            ProxyType::Direct => None,
        }
    }

    fn configs() -> std::sync::MutexGuard<'static, HashMap<String, ProxyConfig>> {
        PROXY_MANAGER.configs.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_config_roundtrip() {
        let config = ProxyConfig {
            enabled: true,
            proxy_type: ProxyType::Socks5,
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            proxy_dns: true,
            bypass_list: vec!["localhost".to_string()],
        };
        ProxyManager::set_config("test-profile", config);
        let retrieved = ProxyManager::get_config("test-profile");
        assert!(retrieved.is_some());
        let c = retrieved.unwrap();
        assert!(c.enabled);
        assert_eq!(c.port, 1080);
        assert_eq!(ProxyManager::proxy_url("test-profile").unwrap().as_str(), "socks5://user:pass@127.0.0.1:1080");
        assert!(ProxyManager::should_bypass("test-profile", "http://localhost/"));
        assert!(!ProxyManager::should_bypass("test-profile", "http://example.com/"));
        ProxyManager::remove_config("test-profile");
        assert!(ProxyManager::get_config("test-profile").is_none());
    }
}
