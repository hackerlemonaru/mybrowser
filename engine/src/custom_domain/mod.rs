//! Custom Domain System — 118 non-registrable TLDs only.
//!
//! Users register `name.tld` → `serving_url` mappings. When the browser
//! navigates to `https://mysite.mybrowser`, the resolver looks up the
//! serving URL and fetches from it (hidden from the user). The URL bar
//! shows the custom domain.
//!
//! Hugging Face is only contacted for registered custom domains — never
//! for regular .com/.net/.org traffic.
//!
//! Per-profile storage: each profile's custom domains live in its isolated
//! data directory (`<profile_data_dir>/custom_domains.json`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub mod hf_client;

pub use hf_client::{HfClient, HfError, ResolutionLog, ResolutionLogStore, ResolutionSource};

/// The 118 allowed custom TLDs (non-registrable, ICANN-not-recognized).
pub const ALLOWED_TLDS: &[&str] = &[
    // Tier 1: Browser & System (20)
    "mybrowser", "browse", "surf", "nav", "tab", "page", "site", "view",
    "window", "frame", "viewport", "render", "engine", "falco", "profile",
    "session", "cookie", "cache", "bookmark", "history",
    // Tier 2: Web3 & Decentralized (20)
    "pwb", "bit", "eth", "web3", "crypto", "dao", "defi", "nft", "meta",
    "verse", "chain", "coin", "token", "wallet", "exchange", "swap", "bridge",
    "oracle", "node", "validator",
    // Tier 3: Network & Proxy (20)
    "vpn", "proxy", "tor", "i2p", "onion", "tunnel", "relay", "mesh", "peer",
    "p2p", "socks", "http", "dns", "resolve", "route", "hop", "exit", "entry",
    "gateway",
    // Tier 4: Private & Internal (20)
    "local", "home", "internal", "private", "secure", "safe", "hidden", "dark",
    "deep", "shadow", "cloak", "mask", "veil", "ghost", "stealth", "anon",
    "incog", "burn", "temp", "guest",
    // Tier 5: Single Letter (26)
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o",
    "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
    // Tier 6: Single Digit (10)
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    // Tier 7: Short & Trendy (2)
    "xyz", "ooo",
];

/// A registered custom domain entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEntry {
    /// e.g. "mysite.mybrowser"
    pub domain: String,
    /// The hidden serving URL (must be HTTPS, public IP only)
    pub serving_url: String,
    /// When the registration was created (Unix seconds)
    pub created_at: u64,
    /// Last time this domain was resolved
    pub last_resolved: Option<u64>,
    /// Number of times resolved
    pub resolve_count: u64,
    /// Profile ID this domain belongs to
    pub profile_id: String,
}

/// Result of a domain resolution.
#[derive(Debug, Clone)]
pub enum ResolutionResult {
    /// Found a serving URL for the custom domain
    Resolved { serving_url: String, domain: String },
    /// Not a custom domain (regular domain, pass through)
    NotCustomDomain,
    /// Custom domain but not registered
    NotRegistered,
}

/// Rate limiter: token bucket per profile.
#[derive(Debug, Clone)]
struct RateLimiter {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    refill_rate_per_sec: f64,
}

impl RateLimiter {
    fn new(max_tokens: f64, refill_rate_per_sec: f64) -> Self {
        Self {
            tokens: max_tokens,
            last_refill: Instant::now(),
            max_tokens,
            refill_rate_per_sec,
        }
    }

    /// Try to consume one token. Returns true if allowed, false if rate-limited.
    fn allow(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        self.tokens = (self.tokens + elapsed * self.refill_rate_per_sec).min(self.max_tokens);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Custom domain registry — per-profile, thread-safe.
pub struct CustomDomainRegistry {
    /// profile_id → domain → entry
    domains: Mutex<HashMap<String, HashMap<String, DomainEntry>>>,
    /// profile_id → rate limiter
    rate_limiters: Mutex<HashMap<String, RateLimiter>>,
    /// Storage directory base (profiles dir)
    profiles_dir: PathBuf,
    /// HF backend configuration
    hf_config: HfConfig,
}

/// Hugging Face backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfConfig {
    /// HF API token (from env, never logged)
    pub token: Option<String>,
    /// HF Space API endpoint for domain resolution
    pub resolve_endpoint: Option<String>,
    /// Whether HF resolution is enabled
    pub enabled: bool,
    /// Cloudflare Workers URL (optional alternative to HF)
    pub cloudflare_worker_url: Option<String>,
}

impl Default for HfConfig {
    fn default() -> Self {
        Self {
            token: std::env::var("HF_TOKEN").ok(),
            resolve_endpoint: std::env::var("HF_RESOLVE_ENDPOINT").ok(),
            enabled: std::env::var("HF_CUSTOM_DOMAIN_ENABLED")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false),
            cloudflare_worker_url: std::env::var("CLOUDFLARE_WORKER_URL").ok(),
        }
    }
}

impl CustomDomainRegistry {
    /// Create a new registry. Profiles dir is where profile data directories live.
    pub fn new(profiles_dir: PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            domains: Mutex::new(HashMap::new()),
            rate_limiters: Mutex::new(HashMap::new()),
            profiles_dir,
            hf_config: HfConfig::default(),
        })
    }

    /// Check if a TLD is one of the 118 allowed custom TLDs.
    pub fn is_allowed_tld(tld: &str) -> bool {
        ALLOWED_TLDS.contains(&tld)
    }

    /// Extract the TLD from a domain like "mysite.mybrowser" → "mybrowser".
    pub fn extract_tld(domain: &str) -> Option<&str> {
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() >= 2 {
            Some(parts.last()?)
        } else {
            None
        }
    }

    /// Check if a domain uses a custom TLD.
    pub fn is_custom_domain(domain: &str) -> bool {
        match Self::extract_tld(domain) {
            Some(tld) => Self::is_allowed_tld(tld),
            None => false,
        }
    }

    /// Register a custom domain → serving URL mapping for a profile.
    /// Returns an error if the TLD is not allowed, the URL is unsafe, or
    /// the domain is already registered by another profile.
    pub fn register_domain(
        &self,
        profile_id: &str,
        domain: &str,
        serving_url: &str,
    ) -> anyhow::Result<()> {
        // Validate TLD
        if !Self::is_custom_domain(domain) {
            anyhow::bail!(
                "Domain '{}' does not use an allowed custom TLD. Only the 118 custom TLDs are permitted.",
                domain
            );
        }

        // Validate serving URL safety
        validate_serving_url(serving_url)?;

        // Normalize domain (lowercase)
        let domain = domain.to_lowercase();

        // Check if already registered by a different profile
        {
            let domains = self.domains.lock().unwrap();
            for (pid, entries) in domains.iter() {
                if pid != profile_id && entries.contains_key(&domain) {
                    anyhow::bail!(
                        "Domain '{}' is already registered by profile '{}'",
                        domain, pid
                    );
                }
            }
        }

        let entry = DomainEntry {
            domain: domain.clone(),
            serving_url: serving_url.to_string(),
            created_at: now_seconds(),
            last_resolved: None,
            resolve_count: 0,
            profile_id: profile_id.to_string(),
        };

        // Save to profile data dir
        self.save_entry(profile_id, &entry)?;

        // Update in-memory registry
        {
            let mut domains = self.domains.lock().unwrap();
            domains
                .entry(profile_id.to_string())
                .or_default()
                .insert(domain, entry);
        }

        Ok(())
    }

    /// Remove a custom domain registration for a profile.
    pub fn unregister_domain(&self, profile_id: &str, domain: &str) -> anyhow::Result<()> {
        let domain = domain.to_lowercase();

        {
            let mut domains = self.domains.lock().unwrap();
            if let Some(entries) = domains.get_mut(profile_id) {
                entries.remove(&domain);
            }
        }

        // Remove from disk
        self.remove_entry_file(profile_id, &domain)?;

        Ok(())
    }

    /// Resolve a custom domain to its serving URL.
    /// Returns ResolutionResult::Resolved if found, NotCustomDomain if not a
    /// custom TLD, NotRegistered if custom TLD but not registered.
    pub fn resolve(&self, profile_id: &str, domain: &str) -> ResolutionResult {
        if !Self::is_custom_domain(domain) {
            return ResolutionResult::NotCustomDomain;
        }

        let domain = domain.to_lowercase();

        // Check in-memory registry first
        {
            let domains = self.domains.lock().unwrap();
            if let Some(entries) = domains.get(profile_id) {
                if let Some(entry) = entries.get(&domain) {
                    return ResolutionResult::Resolved {
                        serving_url: entry.serving_url.clone(),
                        domain: entry.domain.clone(),
                    };
                }
            }
        }

        // Try loading from disk (may not be in memory yet)
        if let Ok(entry) = self.load_entry(profile_id, &domain) {
            return ResolutionResult::Resolved {
                serving_url: entry.serving_url.clone(),
                domain: entry.domain.clone(),
            };
        }

        ResolutionResult::NotRegistered
    }

    /// List all custom domains for a profile.
    /// Loads from disk if not already in memory.
    pub fn list_domains(&self, profile_id: &str) -> Vec<DomainEntry> {
        // Load from disk if not in memory
        {
            let domains = self.domains.lock().unwrap();
            if domains.contains_key(profile_id) {
                return domains
                    .get(profile_id)
                    .map(|entries| entries.values().cloned().collect())
                    .unwrap_or_default();
            }
        }

        // Not in memory — load from disk
        match self.load_profile_domains(profile_id) {
            Ok(entries) => entries,
            Err(_) => vec![],
        }
    }

    /// Record a resolution hit (for HF tracking and stats).
    pub fn record_resolution(&self, profile_id: &str, domain: &str) {
        let domain = domain.to_lowercase();
        let mut domains = self.domains.lock().unwrap();
        if let Some(entries) = domains.get_mut(profile_id) {
            if let Some(entry) = entries.get_mut(&domain) {
                entry.last_resolved = Some(now_seconds());
                entry.resolve_count += 1;
            }
        }
    }

    /// Check if the profile is rate-limited for HF requests.
    pub fn hf_request_allowed(&self, profile_id: &str) -> bool {
        let mut limiters = self.rate_limiters.lock().unwrap();
        let limiter = limiters
            .entry(profile_id.to_string())
            .or_insert_with(|| RateLimiter::new(60.0, 1.0)); // 60 tokens, 1/sec refill
        limiter.allow()
    }

    /// Get HF config.
    pub fn hf_config(&self) -> &HfConfig {
        &self.hf_config
    }

    /// Check if HF resolution is configured and enabled.
    pub fn hf_enabled(&self) -> bool {
        self.hf_config.enabled
            && (self.hf_config.token.is_some()
                || self.hf_config.resolve_endpoint.is_some()
                || self.hf_config.cloudflare_worker_url.is_some())
    }

    // ── Persistence ──

    fn entry_file_path(&self, profile_id: &str, domain: &str) -> PathBuf {
        self.profiles_dir
            .join(profile_id)
            .join("custom_domains")
            .join(format!("{}.json", domain))
    }

    fn save_entry(&self, profile_id: &str, entry: &DomainEntry) -> anyhow::Result<()> {
        let dir = self.profiles_dir.join(profile_id).join("custom_domains");
        std::fs::create_dir_all(&dir)?;
        let path = self.entry_file_path(profile_id, &entry.domain);
        let content = serde_json::to_string_pretty(entry)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn load_entry(&self, profile_id: &str, domain: &str) -> anyhow::Result<DomainEntry> {
        let path = self.entry_file_path(profile_id, domain);
        let content = std::fs::read_to_string(&path)?;
        let entry: DomainEntry = serde_json::from_str(&content)?;
        Ok(entry)
    }

    fn remove_entry_file(&self, profile_id: &str, domain: &str) -> anyhow::Result<()> {
        let path = self.entry_file_path(profile_id, domain);
        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    /// Load all entries for a profile from disk into memory.
    pub fn load_profile_domains(&self, profile_id: &str) -> anyhow::Result<Vec<DomainEntry>> {
        let dir = self.profiles_dir.join(profile_id).join("custom_domains");
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = std::fs::read_to_string(&path)?;
                let entry: DomainEntry = serde_json::from_str(&content)?;
                entries.push(entry);
            }
        }

        // Populate in-memory registry
        {
            let mut domains = self.domains.lock().unwrap();
            let profile_entries = domains.entry(profile_id.to_string()).or_default();
            for e in &entries {
                profile_entries.insert(e.domain.clone(), e.clone());
            }
        }

        Ok(entries)
    }
}

impl Default for CustomDomainRegistry {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("mybrowser_custom_domains")).unwrap()
    }
}

/// Current Unix timestamp in seconds.
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Validate a serving URL for safety.
///
/// Rules:
/// - Must be HTTPS (no HTTP)
/// - Must not point to private IP ranges (SSRF protection)
/// - Must not be localhost/127.0.0.1/::1
/// - Must not contain credentials (@ in host)
/// - Path must not contain .. traversal
fn validate_serving_url(url: &str) -> anyhow::Result<()> {
    let parsed: url::Url = url
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid URL '{}': {}", url, e))?;

    // Must be HTTPS
    if parsed.scheme() != "https" {
        anyhow::bail!("Serving URL must use HTTPS (got: {})", parsed.scheme());
    }

    // No credentials in URL
    if parsed.username() != "" || parsed.password().is_some() {
        anyhow::bail!("Serving URL must not contain credentials");
    }

    // Check host
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Serving URL must have a host"))?;

    // Block localhost
    if host == "localhost" || host == "localhost.localdomain" {
        anyhow::bail!("Serving URL must not point to localhost");
    }

    // Block IP addresses (private ranges + loopback)
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(ipv4) => {
                if ipv4.is_loopback()
                    || ipv4.is_private()
                    || ipv4.is_link_local()
                    || ipv4.is_multicast()
                    || ipv4.is_broadcast()
                {
                    anyhow::bail!(
                        "Serving URL must not point to private/local IP: {}",
                        ipv4
                    );
                }
                // Block 100.64.0.0/10 (CGN)
                if ipv4.octets()[0] == 100 && ipv4.octets()[1] >= 64 && ipv4.octets()[1] <= 127 {
                    anyhow::bail!("Serving URL must not point to carrier-grade NAT IP");
                }
            }
            std::net::IpAddr::V6(ipv6) => {
                if ipv6.is_loopback()
                    || ipv6.is_unique_local()
                    || ipv6.is_unicast_link_local()
                    || ipv6.is_multicast()
                {
                    anyhow::bail!(
                        "Serving URL must not point to private/local IPv6: {}",
                        ipv6
                    );
                }
            }
        }
    }

    // Block path traversal
    let path = parsed.path();
    if path.contains("..") {
        anyhow::bail!("Serving URL path must not contain '..'");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_tld() {
        assert!(CustomDomainRegistry::is_allowed_tld("mybrowser"));
        assert!(CustomDomainRegistry::is_allowed_tld("a"));
        assert!(CustomDomainRegistry::is_allowed_tld("0"));
        assert!(CustomDomainRegistry::is_allowed_tld("xyz"));
        assert!(!CustomDomainRegistry::is_allowed_tld("com"));
        assert!(!CustomDomainRegistry::is_allowed_tld("net"));
        assert!(!CustomDomainRegistry::is_allowed_tld("org"));
    }

    #[test]
    fn test_is_custom_domain() {
        assert!(CustomDomainRegistry::is_custom_domain("mysite.mybrowser"));
        assert!(CustomDomainRegistry::is_custom_domain("x.a"));
        assert!(!CustomDomainRegistry::is_custom_domain("google.com"));
        assert!(!CustomDomainRegistry::is_custom_domain("example.org"));
    }

    #[test]
    fn test_validate_serving_url() {
        assert!(validate_serving_url("https://example.com/page").is_ok());
        assert!(validate_serving_url("https://myapp.mybrowser.app:8443/path").is_ok());
        assert!(validate_serving_url("http://example.com").is_err()); // not HTTPS
        assert!(validate_serving_url("https://localhost:8080").is_err()); // localhost
        assert!(validate_serving_url("https://192.168.1.1/page").is_err()); // private IP
        assert!(validate_serving_url("https://10.0.0.1/page").is_err()); // private IP
        assert!(validate_serving_url("https://user:pass@example.com").is_err()); // creds
        assert!(validate_serving_url("https://example.com/../secret").is_err()); // traversal
    }
}
