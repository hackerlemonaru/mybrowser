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
    pub fn generate(_device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // In production, use device profile values.
        Self {
            width: 1920,
            height: 1080,
            color_depth: 24,
            pixel_depth: 24,
            device_pixel_ratio: 1.0,
            hardware_concurrency: 6,
            device_memory: 8.0,
            touch_support: false,
        }
    }
}

impl Default for ScreenFingerprint {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng)
    }
}

/// Simple string hash for fingerprint consistency.
pub(crate) fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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
