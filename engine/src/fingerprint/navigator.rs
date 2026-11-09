//! navigator object fingerprint spoofing.
//! User-Agent, Client Hints, platform, hardware concurrency, device memory,
//! language, fonts, webdriver flag.

use super::SimpleRng;
use serde::{Deserialize, Serialize};

/// Spoofed User-Agent string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAgent {
    pub full: String,
    pub platform: String,
    pub ua_full_version: String,
}

impl UserAgent {
    pub fn generate(device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // Use device profile's UA template for internal consistency.
        let full = device.user_agent_template.to_string();

        // Extract version from template (e.g., Chrome/120.0.0.0).
        let ua_full_version = "120.0.0.0".to_string();

        Self {
            full,
            platform: device.platform.to_string(),
            ua_full_version,
        }
    }

    pub fn to_string(&self) -> String {
        self.full.clone()
    }
}

/// Client Hints (Sec-CH-UA-* headers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHints {
    pub sec_ch_ua: String,
    pub sec_ch_ua_mobile: String,
    pub sec_ch_ua_platform: String,
    pub sec_ch_ua_platform_version: String,
    pub sec_ch_ua_full_version: String,
    pub sec_ch_ua_arch: String,
    pub sec_ch_ua_model: String,
}

impl ClientHints {
    pub fn generate(device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // Determine mobile based on platform.
        let is_mobile = device.platform.contains("Android") || device.platform.contains("iPhone");

        Self {
            sec_ch_ua: device.sec_ch_ua.to_string(),
            sec_ch_ua_mobile: if is_mobile { "?1".to_string() } else { "?0".to_string() },
            sec_ch_ua_platform: device.sec_ch_ua_platform.to_string(),
            sec_ch_ua_platform_version: device.sec_ch_ua_platform_version.to_string(),
            sec_ch_ua_full_version: "\"120.0.0.0\"".to_string(),
            sec_ch_ua_arch: if device.platform.contains("ARM") || device.platform.contains("Android") || device.platform.contains("iPhone") {
                "\"arm\"".to_string()
            } else {
                "\"x86\"".to_string()
            },
            sec_ch_ua_model: if is_mobile {
                format!("\"{}\"", device.name)
            } else {
                "\"\"".to_string()
            },
        }
    }
}

/// Font list spoofing — controls what fonts websites can detect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontList {
    pub fonts: Vec<String>,
    /// Whether to report a limited font set (anti-fingerprinting).
    pub limited: bool,
}

impl FontList {
    pub fn generate(device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // Use device profile fonts — consistent per device type.
        let fonts = device.fonts.iter().map(|f| f.to_string()).collect();

        Self {
            fonts,
            limited: false,
        }
    }
}

impl Default for UserAgent {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng)
    }
}

impl Default for ClientHints {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng)
    }
}

impl Default for FontList {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng)
    }
}
