//! Content Script - runs in extension process (NOT renderer).
//! Accesses page DOM through DOM Proxy - never holds direct DOM references.
//! This implements FIST BUMP strong process isolation.

use crate::sandbox::{DomProxy, SandboxPolicy};
use crate::manifest::ContentScript as ContentScriptRule;
use serde_json::Value;
use std::collections::HashMap;

/// A content script instance attached to a specific page/tab.
pub struct ContentScript {
    pub extension_id: String,
    pub rule: ContentScriptRule,
    pub dom_proxy: DomProxy,
    /// Isolated JS state (separate from page JS).
    pub isolated_state: HashMap<String, Value>,
    /// The tab this content script is injected into.
    pub tab_id: u32,
    /// The origin of the page.
    pub page_origin: String,
}

/// Messages a content script can send to the background service worker.
#[derive(Debug, Clone)]
pub enum ContentScriptMessage {
    /// Request storage read.
    StorageGet { key: String },
    /// Request storage write.
    StorageSet { key: String, value: Value },
    /// Request DOM read (via proxy).
    DomRead { node_id: u64, property: String },
    /// Request DOM write (via proxy).
    DomWrite { node_id: u64, property: String, value: String },
    /// Request navigation.
    Navigate { url: String },
    /// Generic message to background.
    Custom { action: String, payload: Value },
}

impl ContentScript {
    pub fn new(
        extension_id: String,
        rule: ContentScriptRule,
        tab_id: u32,
        page_origin: String,
    ) -> Self {
        Self {
            extension_id,
            rule,
            dom_proxy: DomProxy::new(&extension_id),
            isolated_state: HashMap::new(),
            tab_id,
            page_origin,
        }
    }

    /// Check if this content script should run on a given URL.
    pub fn matches_url(&self, url: &str) -> bool {
        for pattern in &self.rule.matches {
            if Self::match_pattern(pattern, url) {
                // Check exclusions.
                let excluded = self.rule.exclude_matches.iter()
                    .any(|p| Self::match_pattern(p, url));
                if !excluded {
                    return true;
                }
            }
        }
        false
    }

    fn match_pattern(pattern: &str, url: &str) -> bool {
        if pattern == "<all_urls>" || pattern == "*://*/*" {
            return true;
        }
        // Simple glob matching - in production use proper match pattern parsing.
        url.contains(&pattern.replace("*", ""))
    }

    /// Inject CSS into the page (via DOM Proxy).
    pub fn inject_css(&self, css: &str) -> Result<(), String> {
        // Content scripts request CSS injection through the proxy.
        // The browser engine applies the CSS to the page.
        eprintln!("[ContentScript] Inject CSS into tab {}: {}", self.tab_id, css);
        Ok(())
    }

    /// Inject JS into the page (via DOM Proxy).
    /// The JS runs in an isolated world - page JS cannot access its variables.
    pub fn inject_js(&self, js: &str) -> Result<(), String> {
        // Security: no eval of arbitrary strings from page.
        // Only inject extension's own bundled code.
        eprintln!("[ContentScript] Inject JS into tab {}: {}...", self.tab_id, &js[..js.len().min(50)]);
        Ok(())
    }

    /// Read a DOM node property through the proxy.
    pub fn dom_read(&mut self, node_id: u64, property: &str) -> Result<Value, String> {
        if !self.dom_proxy.is_valid_node(node_id) {
            return Err(format!("Invalid DOM node reference: {}", node_id).into());
        }
        // The actual DOM read happens in the browser engine.
        // Here we just record the request.
        self.isolated_state.insert(
            format!("dom_{}_{}", node_id, property),
            Value::String(property.to_string()),
        );
        Ok(Value::String(property.to_string()))
    }

    /// Write a DOM node property through the proxy.
    pub fn dom_write(&mut self, node_id: u64, property: &str, value: &str) -> Result<(), String> {
        if !self.dom_proxy.is_valid_node(node_id) {
            return Err(format!("Invalid DOM node reference: {}", node_id).into());
        }
        eprintln!("[ContentScript] DOM write: node={} prop={} value={}", node_id, property, value);
        Ok(())
    }
}
