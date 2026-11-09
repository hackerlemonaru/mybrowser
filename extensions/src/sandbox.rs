//! Extension sandbox security.
//! Implements FIST BUMP security model:
//!   - Content scripts run in extension process, NOT renderer process
//!   - DOM Proxy: content scripts access page DOM through proxy
//!   - Message validation: sender.id must match extension.id
//!   - No eval/dynamic code execution
//!   - No nativeMessaging
//!   - Privileged actions are allowlisted

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Security context for an extension component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub extension_id: String,
    pub component: ComponentType,
    pub permissions: HashSet<String>,
    pub host_permissions: HashSet<String>,
}

/// Component types in the extension architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType {
    /// Background service worker - full API access, no DOM.
    Background,
    /// Content script - DOM access via proxy, limited API.
    ContentScript,
    /// Popup / options UI - full API access, own window.
    Popup,
    /// Options page.
    Options,
}

/// Validated message from a content script or extension page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedMessage {
    pub extension_id: String,
    pub sender_component: ComponentType,
    pub action: String,
    pub payload: serde_json::Value,
    /// The tab that sent this message (for content scripts).
    pub sender_tab_id: Option<u32>,
    /// The origin of the sender (for content scripts).
    pub sender_origin: Option<String>,
}

/// Sandbox policy - defines what each component can do.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Actions that content scripts are allowed to request from background.
    pub content_script_allowed_actions: HashSet<String>,
    /// Actions that background can perform (allowlist).
    pub background_allowed_actions: HashSet<String>,
    /// Maximum storage size per extension (in bytes).
    pub max_storage_bytes: u64,
    /// Whether eval is allowed (should always be false).
    pub allow_eval: bool,
    /// Whether nativeMessaging is allowed (should always be false).
    pub allow_native_messaging: bool,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        let mut content_actions = HashSet::new();
        content_actions.insert("storage.get".to_string());
        content_actions.insert("storage.set".to_string());
        content_actions.insert("runtime.getManifest".to_string());
        content_actions.insert("tabs.sendMessage".to_string());

        let mut bg_actions = HashSet::new();
        bg_actions.insert("storage.get".to_string());
        bg_actions.insert("storage.set".to_string());
        bg_actions.insert("storage.remove".to_string());
        bg_actions.insert("tabs.create".to_string());
        bg_actions.insert("tabs.remove".to_string());
        bg_actions.insert("tabs.update".to_string());
        bg_actions.insert("notifications.create".to_string());
        bg_actions.insert("runtime.getManifest".to_string());
        bg_actions.insert("declarativeNetRequest".to_string());

        Self {
            content_script_allowed_actions: content_actions,
            background_allowed_actions: bg_actions,
            max_storage_bytes: 10 * 1024 * 1024, // 10 MB per extension
            allow_eval: false,
            allow_native_messaging: false,
        }
    }
}

/// Validate an incoming message from a content script or extension page.
/// Returns an error if the message is not from this extension or the action is not allowed.
pub fn validate_message(
    policy: &SandboxPolicy,
    extension_id: &str,
    message: &ValidatedMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Verify sender belongs to this extension.
    if message.extension_id != extension_id {
        return Err(format!("Message from foreign extension '{}' rejected", message.extension_id).into());
    }

    // 2. No eval allowed.
    if policy.allow_eval && message.action == "eval" {
        return Err("eval action is banned".into());
    }

    // 3. No nativeMessaging.
    if policy.allow_native_messaging && message.action == "nativeMessaging" {
        return Err("nativeMessaging is banned".into());
    }

    // 4. Check if action is allowed for this component.
    match message.sender_component {
        ComponentType::ContentScript => {
            if !policy.content_script_allowed_actions.contains(&message.action) {
                return Err(format!("Content script action '{}' not allowed", message.action).into());
            }
        }
        ComponentType::Background => {
            if !policy.background_allowed_actions.contains(&message.action) {
                return Err(format!("Background action '{}' not allowed", message.action).into());
            }
        }
        ComponentType::Popup | ComponentType::Options => {
            // Popups and options have full access to background actions.
            if !policy.background_allowed_actions.contains(&message.action) {
                return Err(format!("Popup action '{}' not allowed", message.action).into());
            }
        }
    }

    // 5. Scope effects to sender's tab only for content scripts.
    if message.sender_component == ComponentType::ContentScript {
        if message.sender_tab_id.is_none() {
            return Err("Content script message must include sender tab ID".into());
        }
    }

    Ok(())
}

/// DOM Proxy - content scripts access page DOM through this proxy.
/// Content scripts never hold direct references to DOM nodes.
#[derive(Debug, Clone)]
pub struct DomProxy {
    pub extension_id: String,
    /// Simulated DOM node references (opaque IDs, not real pointers).
    pub node_refs: HashSet<u64>,
}

impl DomProxy {
    pub fn new(extension_id: &str) -> Self {
        Self {
            extension_id: extension_id.to_string(),
            node_refs: HashSet::new(),
        }
    }

    /// Register a DOM node reference (opaque ID).
    pub fn register_node(&mut self, node_id: u64) {
        self.node_refs.insert(node_id);
    }

    /// Check if a node reference is valid for this extension.
    pub fn is_valid_node(&self, node_id: u64) -> bool {
        self.node_refs.contains(&node_id)
    }

    /// Release a node reference.
    pub fn release_node(&mut self, node_id: u64) {
        self.node_refs.remove(&node_id);
    }
}
