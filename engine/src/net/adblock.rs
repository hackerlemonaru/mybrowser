//! Brave's adblock-rust integration for MyBrowser.
//!
//! Provides per-profile network request filtering using Brave's open-source
//! Rust adblock engine (uBlock Origin-compatible syntax).
//!
//! Note: adblock::Engine contains Rc-based types that are not Send/Sync,
//! so we use thread_local! storage instead of a global static.

use adblock::{
    blocker::BlockerResult,
    engine::Engine,
    lists::{FilterSet, ParseOptions},
    request::Request,
};
use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Result of a network request check.
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkDecision {
    /// Request is allowed.
    Allow,
    /// Request is blocked.
    Block,
}

/// Per-profile adblock engine manager (thread-local).
/// Each thread gets its own map of engines to avoid Send/Sync issues.
pub struct AdBlockManager {
    engines: RefCell<HashMap<String, Rc<Engine>>>,
    default_filters: String, // Joined filter list text
    enabled: bool,
}

impl AdBlockManager {
    /// Create a new AdBlockManager with default filter lists.
    pub fn new() -> Result<Self> {
        let default_filters = Self::load_default_filters()?;
        Ok(Self {
            engines: RefCell::new(HashMap::new()),
            default_filters,
            enabled: true,
        })
    }

    /// Load default filter lists (EasyList + EasyPrivacy essentials).
    fn load_default_filters() -> Result<String> {
        let filters = vec![
            // EasyList essentials
            "||doubleclick.net^",
            "||googlesyndication.com^",
            "||googleadservices.com^",
            "||amazon-adsystem.com^",
            "||facebook.com/tr^",
            "||google-analytics.com^",
            "||googletagmanager.com^",
            "||adsystem.com^",
            "||adnxs.com^",
            "||rubiconproject.com^",
            // EasyPrivacy essentials
            "||google-analytics.com/analytics.js^",
            "||googletagmanager.com/gtm.js^",
            "||facebook.com/tr^",
            "||connect.facebook.net^",
            "||analytics.twitter.com^",
            "||ads.linkedin.com^",
            // Common tracking domains
            "||scorecardresearch.com^",
            "||addthis.com^",
            "||sharethis.com^",
        ];
        Ok(filters.join("\n"))
    }

    /// Enable or disable adblocking globally.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if adblocking is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get or create an adblock engine for a profile (thread-local).
    pub fn engine_for_profile(&self, profile_id: &str) -> Result<Rc<Engine>> {
        let engines = self.engines.borrow();
        if let Some(engine) = engines.get(profile_id) {
            return Ok(engine.clone());
        }
        drop(engines);

        // Build a new engine for this profile.
        let mut filter_set = FilterSet::new(true);
        filter_set.add_filter_list(self.default_filters.clone(), ParseOptions::default());
        let engine = Engine::new_with_filter_set(filter_set);

        let engine_rc = Rc::new(engine);

        let mut engines = self.engines.borrow_mut();
        engines.insert(profile_id.to_string(), engine_rc.clone());
        Ok(engine_rc)
    }

    /// Remove a profile's engine (when profile is deleted).
    pub fn remove_profile(&self, profile_id: &str) {
        let mut engines = self.engines.borrow_mut();
        engines.remove(profile_id);
    }
}

/// Thread-local adblock manager.
thread_local! {
    static ADBLOCK_MANAGER: RefCell<AdBlockManager> = RefCell::new(
        AdBlockManager::new().expect("Failed to init adblock manager")
    );
}

/// Check if adblocking is enabled globally.
pub fn is_enabled() -> bool {
    ADBLOCK_MANAGER.with(|m| m.borrow().is_enabled())
}

/// Check if a network request should be blocked for a given profile.
///
/// Returns `Ok(NetworkDecision::Allow)` if the request should proceed,
/// `Ok(NetworkDecision::Block)` if it should be blocked.
pub fn check_request(
    url: &str,
    source_url: &str,
    resource_type: &str,
    method: &str,
    profile_id: Option<&str>,
) -> Result<NetworkDecision> {
    if !is_enabled() {
        return Ok(NetworkDecision::Allow);
    }

    let profile_id = profile_id.unwrap_or("default");

    ADBLOCK_MANAGER.with(|mgr| {
        let manager = mgr.borrow();
        let engine = manager.engine_for_profile(profile_id)?;

        let request = Request::new(url, source_url, resource_type, method)
            .map_err(|e| anyhow::anyhow!("Failed to build adblock request: {}", e))?;

        let result: BlockerResult = engine.check_network_request(&request);

        Ok(if result.should_block() {
            NetworkDecision::Block
        } else {
            NetworkDecision::Allow
        })
    })
}
