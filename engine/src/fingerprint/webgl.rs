//! WebGL fingerprint spoofing.
//! Spoofs vendor/renderer strings and WebGL parameters to match a
//! consistent device profile. JavaScript can read these via
//! getParameter(UNMASKED_VENDOR_WEBGL) and getParameter(RENDERER).

use super::SimpleRng;
use serde::{Deserialize, Serialize};

/// WebGL fingerprint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebGLFingerprint {
    pub vendor: String,
    pub renderer: String,
    pub unmasked_vendor: String,
    pub unmasked_renderer: String,
    pub version: String,
    pub shading_language_version: String,
    pub max_texture_size: u32,
    pub max_viewport_dims: [u32; 2],
    pub extensions: Vec<String>,
    pub antialias: bool,
    pub depth: bool,
}

impl WebGLFingerprint {
    pub fn generate(_device: &crate::fingerprint::DeviceProfile, rng: &mut SimpleRng) -> Self {
        // In production, this uses the device profile from DEVICE_PROFILES.
        // For now, generate consistent values.
        let vendor = "NVIDIA Corporation".to_string();
        let renderer = "NVIDIA GeForce RTX 3060".to_string();

        Self {
            vendor: vendor.clone(),
            renderer: renderer.clone(),
            unmasked_vendor: vendor,
            unmasked_renderer: renderer,
            version: "WebGL 1.0 (OpenGL ES 2.0 Chromium)".to_string(),
            shading_language_version: "OpenGL ES GLSL ES 1.00 (Chromium)".to_string(),
            max_texture_size: 16384,
            max_viewport_dims: [16384, 16384],
            extensions: vec![
                "WEBGL_debug_renderer_info".to_string(),
                "WEBGL_lose_context".to_string(),
                "WEBGL_depth_texture".to_string(),
                "OES_element_index_uint".to_string(),
                "OES_texture_float".to_string(),
                "EXT_texture_filter_anisotropic".to_string(),
            ],
            antialias: true,
            depth: true,
        }
    }
}

impl Default for WebGLFingerprint {
    fn default() -> Self {
        let mut rng = SimpleRng { state: 0 };
        Self::generate(
            &crate::fingerprint::DEVICE_PROFILES[0],
            &mut rng,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    def webgl_fingerprint_has_expected_strings() {
        let mut rng = SimpleRng { state: 0 };
        let fp = WebGLFingerprint::generate(&crate::fingerprint::DEVICE_PROFILES[0], &mut rng);
        assert!(!fp.renderer.is_empty());
        assert!(!fp.vendor.is_empty());
    }
}
