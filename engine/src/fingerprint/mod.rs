//! Browser Fingerprint Spoofing System.
//!
//! Each profile gets a consistent, internally-consistent fingerprint that
//! stays the same across sessions. Values are randomized once per profile
//! and then locked — randomization per page load is a detection signal.
//!
//! Spoofed vectors:
//!   - User-Agent + Client Hints (HTTP headers)
//!   - Canvas (noise injection at paint level)
//!   - WebGL (vendor/renderer string + parameters)
//!   - AudioContext (processing parameter modification)
//!   - Fonts (font list filtering)
//!   - Screen resolution + color depth
//!   - Hardware concurrency + device memory
//!   - Timezone + locale
//!   - Platform / OS
//!   - WebRTC (IP leak prevention)
//!
//! Security principle: All values must be internally consistent.
//! A Windows 11 profile with an NVIDIA GPU reports matching values across
//! every signal. Mismatched values are a detection signal.

pub mod canvas;
pub mod webgl;
pub mod audio;
pub mod navigator;
pub mod screen;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// A complete fingerprint profile for one browser profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintProfile {
    pub profile_id: String,
    pub user_agent: navigator::UserAgent,
    pub client_hints: navigator::ClientHints,
    pub webgl: webgl::WebGLFingerprint,
    pub canvas: canvas::CanvasFingerprint,
    pub audio: audio::AudioFingerprint,
    pub fonts: navigator::FontList,
    pub screen: screen::ScreenFingerprint,
    pub timezone: String,
    pub locale: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Global fingerprint store — keyed by profile ID.
static FINGERPRINT_STORE: Mutex<HashMap<String, FingerprintProfile>> = Mutex::new(HashMap::new());

/// Get or create a fingerprint profile for a given browser profile.
pub fn get_or_create(profile_id: &str) -> FingerprintProfile {
    let mut store = FINGERPRINT_STORE.lock().unwrap();
    if let Some(fp) = store.get(profile_id) {
        return fp.clone();
    }

    // Generate a new consistent fingerprint.
    let fp = FingerprintProfile::generate(profile_id);
    store.insert(profile_id.to_string(), fp.clone());
    fp
}

/// Get the fingerprint for a profile (returns None if not created yet).
pub fn get(profile_id: &str) -> Option<FingerprintProfile> {
    let store = FINGERPRINT_STORE.lock().unwrap();
    store.get(profile_id).cloned()
}

/// Remove a fingerprint profile (when profile is deleted).
pub fn remove(profile_id: &str) {
    let mut store = FINGERPRINT_STORE.lock().unwrap();
    store.remove(profile_id);
}

/// Apply fingerprint spoofing to HTTP headers for a request.
pub fn apply_to_headers(profile_id: &str, headers: &mut HashMap<String, String>) {
    let fp = get(profile_id);
    if let Some(fp) = fp {
        headers.insert("User-Agent".to_string(), fp.user_agent.to_string());
        headers.insert("Accept-Language".to_string(), fp.locale.clone());
        // Client Hints.
        headers.insert("Sec-CH-UA".to_string(), fp.client_hints.sec_ch_ua.clone());
        headers.insert("Sec-CH-UA-Mobile".to_string(), fp.client_hints.sec_ch_ua_mobile.clone());
        headers.insert("Sec-CH-UA-Platform".to_string(), fp.client_hints.sec_ch_ua_platform.clone());
        headers.insert("Sec-CH-UA-Platform-Version".to_string(), fp.client_hints.sec_ch_ua_platform_version.clone());
    }
}

impl FingerprintProfile {
    /// Generate a new, internally-consistent fingerprint for a profile.
    fn generate(profile_id: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Use profile_id as seed for reproducibility — same profile always gets same fingerprint.
        let mut hasher = DefaultHasher::new();
        profile_id.hash(&mut hasher);
        let seed = hasher.finish();

        // Select a real device configuration based on seed.
        let device = DEVICE_PROFILES[seed as usize % DEVICE_PROFILES.len()];

        // Deterministic "random" values based on seed.
        let mut rng = SimpleRng { state: seed };

        Self {
            profile_id: profile_id.to_string(),
            user_agent: navigator::UserAgent::generate(&device, &mut rng),
            client_hints: navigator::ClientHints::generate(&device, &mut rng),
            webgl: webgl::WebGLFingerprint::generate(&device, &mut rng),
            canvas: canvas::CanvasFingerprint::generate(&mut rng),
            audio: audio::AudioFingerprint::generate(&mut rng),
            fonts: navigator::FontList::generate(&device, &mut rng),
            screen: screen::ScreenFingerprint::generate(&device, &mut rng),
            timezone: device.timezone.clone(),
            locale: device.locale.clone(),
            created_at: chrono::Utc::now(),
        }
    }
}

/// Real device configurations for internally-consistent fingerprints.
struct DeviceProfile {
    name: &'static str,
    user_agent_template: &'static str,
    platform: &'static str,
    sec_ch_ua: &'static str,
    sec_ch_ua_platform: &'static str,
    sec_ch_ua_platform_version: &'static str,
    webgl_vendor: &'static str,
    webgl_renderer: &'static str,
    screen_width: u32,
    screen_height: u32,
    color_depth: u32,
    hardware_concurrency: u32,
    device_memory: f32,
    timezone: String,
    locale: String,
    fonts: Vec<&'static str>,
}

const DEVICE_PROFILES: [DeviceProfile; 6] = [
    DeviceProfile {
        name: "Windows 11 - NVIDIA RTX 3060",
        user_agent_template: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        platform: "Win32",
        sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"",
        sec_ch_ua_platform: "\"Windows\"",
        sec_ch_ua_platform_version: "\"10.0.0\"",
        webgl_vendor: "NVIDIA Corporation",
        webgl_renderer: "NVIDIA GeForce RTX 3060",
        screen_width: 1920,
        screen_height: 1080,
        color_depth: 24,
        hardware_concurrency: 6,
        device_memory: 8.0,
        timezone: "America/New_York".to_string(),
        locale: "en-US".to_string(),
        fonts: vec!["Arial", "Times New Roman", "Calibri", "Segoe UI", "Consolas"],
    },
    DeviceProfile {
        name: "macOS Sonoma - Apple M2",
        user_agent_template: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        platform: "MacIntel",
        sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"",
        sec_ch_ua_platform: "\"macOS\"",
        sec_ch_ua_platform_version: "\"14.0.0\"",
        webgl_vendor: "Apple",
        webgl_renderer: "Apple M2",
        screen_width: 2560,
        screen_height: 1600,
        color_depth: 30,
        hardware_concurrency: 8,
        device_memory: 8.0,
        timezone: "America/Los_Angeles".to_string(),
        locale: "en-US".to_string(),
        fonts: vec!["San Francisco", "Helvetica", "Times", "Courier", "Menlo"],
    },
    DeviceProfile {
        name: "Linux - Intel UHD",
        user_agent_template: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        platform: "Linux x86_64",
        sec_ch_ua: "\"Chromium\";v=\"120\", \"Not_A Brand\";v=\"8\"",
        sec_ch_ua_platform: "\"Linux\"",
        sec_ch_ua_platform_version: "\"6.5.0\"",
        webgl_vendor: "Intel",
        webgl_renderer: "Intel UHD Graphics 620",
        screen_width: 1366,
        screen_height: 768,
        color_depth: 24,
        hardware_concurrency: 4,
        device_memory: 4.0,
        timezone: "Europe/London".to_string(),
        locale: "en-GB".to_string(),
        fonts: vec!["DejaVu Sans", "Liberation Sans", "Ubuntu", "FreeSans"],
    },
    DeviceProfile {
        name: "Windows 10 - AMD Radeon",
        user_agent_template: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        platform: "Win32",
        sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"",
        sec_ch_ua_platform: "\"Windows\"",
        sec_ch_ua_platform_version: "\"10.0.0\"",
        webgl_vendor: "AMD",
        webgl_renderer: "AMD Radeon RX 580",
        screen_width: 1920,
        screen_height: 1080,
        color_depth: 24,
        hardware_concurrency: 8,
        device_memory: 16.0,
        timezone: "America/Chicago".to_string(),
        locale: "en-US".to_string(),
        fonts: vec!["Arial", "Times New Roman", "Calibri", "Segoe UI", "Consolas"],
    },
    DeviceProfile {
        name: "Android - Pixel 7",
        user_agent_template: "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
        platform: "Linux x86_64",
        sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"",
        sec_ch_ua_platform: "\"Android\"",
        sec_ch_ua_platform_version: "\"13.0.0\"",
        webgl_vendor: "Qualcomm",
        webgl_renderer: "Adreno GPU",
        screen_width: 1080,
        screen_height: 2400,
        color_depth: 24,
        hardware_concurrency: 8,
        device_memory: 8.0,
        timezone: "America/Denver".to_string(),
        locale: "en-US".to_string(),
        fonts: vec!["Roboto", "Droid Sans", "System"],
    },
    DeviceProfile {
        name: "iPhone - iOS 17",
        user_agent_template: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
        platform: "iPhone",
        sec_ch_ua: "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\"",
        sec_ch_ua_platform: "\"iOS\"",
        sec_ch_ua_platform_version: "\"17.0.0\"",
        webgl_vendor: "Apple",
        webgl_renderer: "Apple A16 GPU",
        screen_width: 390,
        screen_height: 844,
        color_depth: 30,
        hardware_concurrency: 6,
        device_memory: 6.0,
        timezone: "America/New_York".to_string(),
        locale: "en-US".to_string(),
        fonts: vec!["San Francisco", "Helvetica", "Times"],
    },
];

/// Simple deterministic RNG seeded from profile_id.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 33) as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn choose<T>(&mut self, items: &[T]) -> &T {
        let idx = (self.next_u32() as usize) % items.len();
        &items[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_consistent_per_profile() {
        let fp1 = get_or_create("profile-a");
        let fp2 = get_or_create("profile-a");
        assert_eq!(fp1.user_agent.to_string(), fp2.user_agent.to_string());
        assert_eq!(fp1.webgl.renderer, fp2.webgl.renderer);
    }

    #[test]
    fn fingerprints_differ_between_profiles() {
        let fp1 = get_or_create("profile-a");
        let fp2 = get_or_create("profile-b");
        // Very unlikely to be identical given different seeds.
        assert_ne!(fp1.user_agent.to_string(), fp2.user_agent.to_string());
    }
}
