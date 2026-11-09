//! Canvas fingerprint spoofing.
//! Injects consistent noise into canvas rendering at the paint level.
//! JavaScript overrides (e.g., overriding toDataURL) are detectable —
//! real spoofing happens at the rendering layer.

use super::SimpleRng;
use serde::{Deserialize, Serialize};

/// Canvas fingerprint configuration for a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasFingerprint {
    /// Noise level (0.0 = none, 1.0 = heavy). Consistent per profile.
    pub noise_level: f32,
    /// Whether to inject noise (true) or use clean rendering (false).
    pub inject_noise: bool,
    /// The canvas hash that will be produced — deterministic per profile.
    pub expected_hash: String,
    /// Color shift applied to all canvas rendering.
    pub color_shift: [f32; 3],
}

impl CanvasFingerprint {
    pub fn generate(rng: &mut SimpleRng) -> Self {
        // Each profile gets a consistent noise level and color shift.
        let noise_level = rng.next_f32() * 0.1 + 0.02; // 2% to 12% noise
        let color_shift = [
            rng.next_f32() * 0.1 - 0.05,
            rng.next_f32() * 0.1 - 0.05,
            rng.next_f32() * 0.1 - 0.05,
        ];

        // Compute a deterministic hash from the noise settings.
        let hash_input = format!("{:?}-{:?}", noise_level, color_shift);
        let expected_hash = super::simple_hash(&hash_input);

        Self {
            noise_level,
            inject_noise: true,
            expected_hash,
            color_shift,
        }
    }

    /// Apply canvas noise to rendered pixels.
    /// Called by the paint engine when rendering canvas elements.
    pub fn apply_noise(&self, pixels: &mut [u8], width: u32, height: u32) {
        if !self.inject_noise {
            return;
        }

        // Apply subtle, consistent noise to each pixel.
        // The noise is deterministic — same pixels always get same noise.
        let mut seed = 0u64;
        for i in (0..pixels.len()).step_by(4) {
            // Deterministic noise based on pixel position.
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((seed >> 33) as f32 / u64::MAX as f32) * self.noise_level * 255.0;

            // Apply color shift.
            pixels[i] = (pixels[i] as f32 + self.color_shift[0] * noise).clamp(0.0, 255.0) as u8;
            pixels[i + 1] = (pixels[i + 1] as f32 + self.color_shift[1] * noise).clamp(0.0, 255.0) as u8;
            pixels[i + 2] = (pixels[i + 2] as f32 + self.color_shift[2] * noise).clamp(0.0, 255.0) as u8;
            // Alpha unchanged.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_fingerprint_is_deterministic() {
        let mut rng1 = SimpleRng { state: 42 };
        let mut rng2 = SimpleRng { state: 42 };
        let fp1 = CanvasFingerprint::generate(&mut rng1);
        let fp2 = CanvasFingerprint::generate(&mut rng2);
        assert_eq!(fp1.expected_hash, fp2.expected_hash);
    }
}
