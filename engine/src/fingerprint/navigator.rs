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
    pub fn generate(_device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // In production, use the device profile's template.
        // For now, generate a consistent UA.
        let full = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string();
        Self {
            full,
            platform: "Win32".to_string(),
            ua_full_version: "120.0.0.0".to_string(),
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
    pub fn generate(_device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        Self {
            sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"".to_string(),
            sec_ch_ua_mobile: "?0".to_string(),
            sec_ch_ua_platform: "\"Windows\"".to_string(),
            sec_ch_ua_platform_version: "\"10.0.0\"".to_string(),
            sec_ch_ua_full_version: "\"120.0.0.0\"".to_string(),
            sec_ch_ua_arch: "\"x86\"".to_string(),
            sec_ch_ua_model: "\"\"".to_string(),
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
    pub fn generate(_device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        let fonts = vec![
            "Arial".to_string(),
            "Times New Roman".to_string(),
            "Calibri".to_string(),
            "Segoe UI".to_string(),
            "Consolas".to_string(),
        ];
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
