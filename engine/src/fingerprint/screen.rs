//! Screen and hardware fingerprint spoofing.
//! Screen resolution, color depth, devicePixelRatio, hardware concurrency,
//! device memory.

use super::SimpleRng;
use serde::{Deserialize, Serialize};

/// Screen fingerprint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFingerprint {
    pub width: u32,
    pub height: u32,
    pub color_depth: u32,
    pub pixel_depth: u32,
    pub device_pixel_ratio: f32,
    pub hardware_concurrency: u32,
    pub device_memory: f32,
    pub touch_support: bool,
}

impl ScreenFingerprint {
    pub fn generate(device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // Use device profile values for internal consistency.
        let is_mobile = device.platform.contains("Android") || device.platform.contains("iPhone");

        Self {
            width: device.screen_width,
            height: device.screen_height,
            color_depth: device.color_depth,
            pixel_depth: device.color_depth,
            device_pixel_ratio: if is_mobile { 3.0 } else { 1.0 },
            hardware_concurrency: device.hardware_concurrency,
            device_memory: device.device_memory,
            touch_support: is_mobile,
        }
    }
}

impl Default for ScreenFingerprint {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_fingerprint_is_deterministic() {
        let mut rng1 = SimpleRng { state: 7 };
        let mut rng2 = SimpleRng { state: 7 };
        let fp1 = ScreenFingerprint::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng1);
        let fp2 = ScreenFingerprint::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng2);
        assert_eq!(fp1.width, fp2.width);
        assert_eq!(fp1.hardware_concurrency, fp2.hardware_concurrency);
    }
}
