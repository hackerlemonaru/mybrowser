//! WebRTC leak prevention.
//! WebRTC can leak real IP addresses even when behind a proxy or VPN.
//! This module prevents WebRTC from revealing the real local/network IP.
//! Options: disable WebRTC entirely, or route all WebRTC traffic through proxy.

use serde::{Deserialize, Serialize};

/// WebRTC configuration per profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRTCConfig {
    /// Whether WebRTC is enabled at all. If false, WebRTC APIs are stubbed.
    pub enabled: bool,
    /// Whether to prevent IP leaks by not exposing local addresses.
    pub prevent_ip_leak: bool,
    /// Force WebRTC to use relay (TURN) only, no direct connections.
    pub force_relay: bool,
    /// Fake IP address reported to WebRTC when leak prevention is on.
    pub fake_local_ip: String,
}

impl Default for WebRTCConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prevent_ip_leak: true,
            force_relay: false,
            fake_local_ip: "192.168.1.100".to_string(),
        }
    }
}

impl WebRTCConfig {
    /// Create a privacy-hardened WebRTC config (leaks prevented).
    pub fn hardened() -> Self {
        Self {
            enabled: true,
            prevent_ip_leak: true,
            force_relay: true,
            fake_local_ip: "10.0.0.1".to_string(),
        }
    }

    /// Create a config that disables WebRTC entirely.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            prevent_ip_leak: true,
            force_relay: false,
            fake_local_ip: "0.0.0.0".to_string(),
        }
    }
}

/// Check if WebRTC should expose local IPs for a given profile.
pub fn should_prevent_ip_leak(profile_id: &str) -> bool {
    use crate::fingerprint::get;
    let fp = get(profile_id);
    fp.map_or(true, |_| true) // Default: prevent leaks
}

/// Get the fake local IP to report for WebRTC.
pub fn fake_local_ip() -> String {
    "192.168.1.100".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardened_config_prevents_leaks() {
        let cfg = WebRTCConfig::hardened();
        assert!(cfg.prevent_ip_leak);
        assert!(cfg.force_relay);
    }

    #[test]
    fn disabled_config_has_no_enabled() {
        let cfg = WebRTCConfig::disabled();
        assert!(!cfg.enabled);
    }
}
