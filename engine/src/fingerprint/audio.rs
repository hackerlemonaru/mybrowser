//! AudioContext fingerprint spoofing.
//! The AudioContext API processes audio signals and output varies by
//! hardware. We spoof by returning consistent, manufactured values.

use super::SimpleRng;
use serde::{Deserialize, Serialize};

/// Audio fingerprint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFingerprint {
    /// Consistent hash produced by AudioContext processing.
    pub hash: String,
    /// Sample rate.
    pub sample_rate: u32,
    /// Whether audio is enabled (some privacy browsers disable it).
    pub enabled: bool,
    /// Channel count.
    pub channels: u32,
}

impl AudioFingerprint {
    pub fn generate(rng: &mut SimpleRng) -> Self {
        let sample_rate = 44100;
        let channels = 2;

        // Deterministic hash from RNG state.
        let hash_input = format!("audio-{}-{}", rng.next_u32(), sample_rate);
        let hash = super::simple_hash(&hash_input);

        Self {
            hash,
            sample_rate,
            enabled: true,
            channels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_fingerprint_is_deterministic() {
        let mut rng1 = SimpleRng { state: 99 };
        let mut rng2 = SimpleRng { state: 99 };
        let fp1 = AudioFingerprint::generate(&mut rng1);
        let fp2 = AudioFingerprint::generate(&mut rng2);
        assert_eq!(fp1.hash, fp2.hash);
    }
}
