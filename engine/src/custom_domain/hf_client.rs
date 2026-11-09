//! Hugging Face backend client for custom domain resolution.
//!
//! HF is only contacted for registered custom domains — never for regular
//! .com/.net/.org traffic. All requests are rate-limited, validated, and
//! logged for presentation/storage.

use crate::custom_domain::HfConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ureq;
use urlencoding;

/// Request to HF to resolve a custom domain.
#[derive(Debug, Clone, Serialize)]
pub struct HfResolveRequest {
    pub domain: String,
    pub profile_id: String,
}

/// Response from HF resolution endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct HfResolveResponse {
    pub serving_url: Option<String>,
    pub registered: bool,
}

/// Error from HF client.
#[derive(Debug)]
pub enum HfError {
    NotConfigured,
    HttpError(String),
    JsonError(String),
    RateLimited,
    NotRegistered,
}

impl std::fmt::Display for HfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HfError::NotConfigured => write!(f, "HF not configured"),
            HfError::HttpError(s) => write!(f, "HTTP error: {}", s),
            HfError::JsonError(s) => write!(f, "JSON error: {}", s),
            HfError::RateLimited => write!(f, "Rate limited by HF"),
            HfError::NotRegistered => write!(f, "Domain not registered on HF backend"),
        }
    }
}

impl std::error::Error for HfError {}

/// Hugging Face client for custom domain resolution.
pub struct HfClient {
    config: HfConfig,
    client: ureq::Agent,
}

impl HfClient {
    /// Create a new HF client from config.
    pub fn new(config: &HfConfig) -> Self {
        let client = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .build();
        Self {
            config: config.clone(),
            client,
        }
    }

    /// Check if HF is available and configured.
    pub fn is_available(&self) -> bool {
        self.config.enabled
            && (self.config.token.is_some()
                || self.config.resolve_endpoint.is_some()
                || self.config.cloudflare_worker_url.is_some())
    }

    /// Resolve a custom domain via HF backend.
    /// Returns the serving URL if registered, or HfError::NotRegistered.
    pub fn resolve_domain(&self, domain: &str, profile_id: &str) -> Result<String, HfError> {
        if !self.is_available() {
            return Err(HfError::NotConfigured);
        }

        // Try Cloudflare Workers first (if configured) — fastest, no HF rate limits
        if let Some(ref worker_url) = self.config.cloudflare_worker_url {
            if let Ok(url) = self.resolve_via_worker(worker_url, domain, profile_id) {
                return Ok(url);
            }
        }

        // Fall back to HF Space endpoint
        if let Some(ref endpoint) = self.config.resolve_endpoint {
            return self.resolve_via_hf(endpoint, domain, profile_id);
        }

        Err(HfError::NotConfigured)
    }

    /// Resolve via Cloudflare Workers (custom worker deployed by user).
    fn resolve_via_worker(
        &self,
        worker_url: &str,
        domain: &str,
        profile_id: &str,
    ) -> Result<String, HfError> {
        let url = format!(
            "{}?domain={}&profile_id={}",
            worker_url,
            urlencoding::encode(domain),
            urlencoding::encode(profile_id)
        );

        let resp = self
            .client
            .get(&url)
            .set("Accept", "application/json")
            .call()
            .map_err(|e| HfError::HttpError(e.to_string()))?;

        if resp.status() == 429 {
            return Err(HfError::RateLimited);
        }
        if resp.status() == 404 {
            return Err(HfError::NotRegistered);
        }
        if !(200..=299).contains(&resp.status()) {
            return Err(HfError::HttpError(format!("status {}", resp.status())));
        }

        let body_str = resp
            .into_string()
            .map_err(|e| HfError::HttpError(e.to_string()))?;
        let body: HfResolveResponse = serde_json::from_str(&body_str)
            .map_err(|e| HfError::JsonError(e.to_string()))?;

        body.serving_url
            .ok_or(HfError::NotRegistered)
    }

    /// Resolve via Hugging Face Space API endpoint.
    fn resolve_via_hf(
        &self,
        endpoint: &str,
        domain: &str,
        profile_id: &str,
    ) -> Result<String, HfError> {
        let token = self
            .config
            .token
            .as_deref()
            .ok_or(HfError::NotConfigured)?;

        let url = format!(
            "{}?domain={}&profile_id={}",
            endpoint,
            urlencoding::encode(domain),
            urlencoding::encode(profile_id)
        );

        let resp = self
            .client
            .get(&url)
            .set("Accept", "application/json")
            .set("Authorization", &format!("Bearer {}", token))
            .set("User-Agent", "MyBrowser/1.0 (custom-domain-resolver)")
            .call()
            .map_err(|e| HfError::HttpError(e.to_string()))?;

        if resp.status() == 429 {
            return Err(HfError::RateLimited);
        }
        if resp.status() == 404 {
            return Err(HfError::NotRegistered);
        }
        if !(200..=299).contains(&resp.status()) {
            return Err(HfError::HttpError(format!("status {}", resp.status())));
        }

        let body_str = resp
            .into_string()
            .map_err(|e| HfError::HttpError(e.to_string()))?;
        let body: HfResolveResponse = serde_json::from_str(&body_str)
            .map_err(|e| HfError::JsonError(e.to_string()))?;

        body.serving_url
            .ok_or(HfError::NotRegistered)
    }

    /// Register a domain on the HF backend (admin operation).
    pub fn register_on_hf(
        &self,
        domain: &str,
        serving_url: &str,
        profile_id: &str,
    ) -> Result<(), HfError> {
        if !self.is_available() {
            return Err(HfError::NotConfigured);
        }

        let token = self
            .config
            .token
            .as_deref()
            .ok_or(HfError::NotConfigured)?;

        let endpoint = self
            .config
            .resolve_endpoint
            .as_deref()
            .ok_or(HfError::NotConfigured)?;

        let req = serde_json::json!({
            "domain": domain,
            "serving_url": serving_url,
            "profile_id": profile_id,
        });

        let req_str = serde_json::to_string(&req).map_err(|e| HfError::JsonError(e.to_string()))?;

        let resp = self
            .client
            .post(endpoint)
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .set("Authorization", &format!("Bearer {}", token))
            .send_string(&req_str)
            .map_err(|e| HfError::HttpError(e.to_string()))?;

        if resp.status() == 429 {
            return Err(HfError::RateLimited);
        }
        if !(200..=299).contains(&resp.status()) {
            return Err(HfError::HttpError(format!("status {}", resp.status())));
        }

        Ok(())
    }
}

/// Resolution log entry — for presentation/storage (everything under record).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionLog {
    pub timestamp: u64,
    pub profile_id: String,
    pub domain: String,
    pub serving_url: String,
    pub source: ResolutionSource,
    pub success: bool,
    pub error: Option<String>,
}

/// Where the resolution came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionSource {
    LocalRegistry,
    HfBackend,
    CloudflareWorker,
}

/// In-memory resolution log (thread-safe). Persisted to disk per profile.
pub struct ResolutionLogStore {
    logs: std::sync::Mutex<Vec<ResolutionLog>>,
    max_entries: usize,
}

impl ResolutionLogStore {
    pub fn new(max_entries: usize) -> Self {
        Self {
            logs: std::sync::Mutex::new(Vec::new()),
            max_entries,
        }
    }

    pub fn record(&self, entry: ResolutionLog) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(entry);
        if logs.len() > self.max_entries {
            let len = logs.len();
            let drain_count = len - self.max_entries;
            logs.drain(0..drain_count);
        }
    }

    pub fn get_all(&self) -> Vec<ResolutionLog> {
        self.logs.lock().unwrap().clone()
    }

    pub fn get_for_profile(&self, profile_id: &str) -> Vec<ResolutionLog> {
        self.logs
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.profile_id == profile_id)
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.logs.lock().unwrap().clear();
    }
}

impl Default for ResolutionLogStore {
    fn default() -> Self {
        Self::new(10000)
    }
}
