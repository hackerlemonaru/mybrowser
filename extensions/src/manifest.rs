//! Chrome Extension Manifest V3 parser.
//! Defines the security contract between extension and browser.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Chrome Extension Manifest V3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icons: Option<HashMap<String, String>>,
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default)]
    pub background: Option<Background>,
    #[serde(default)]
    pub content_scripts: Vec<ContentScript>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub host_permissions: Vec<String>,
    #[serde(default)]
    pub optional_permissions: Vec<String>,
    #[serde(default)]
    pub options_ui: Option<OptionsUI>,
    #[serde(default)]
    pub web_accessible_resources: Vec<WebAccessibleResource>,
    #[serde(default)]
    pub externally_connectable: Option<ExternallyConnectable>,
    #[serde(default)]
    pub content_security_policy: Option<ContentSecurityPolicy>,
    #[serde(default)]
    pub declarative_net_request: Option<DeclarativeNetRequest>,
    /// Chrome extension ID (32 chars) - set by browser at install time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Browser action / toolbar button.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub default_icon: Option<HashMap<String, String>>,
    #[serde(default)]
    pub default_title: Option<String>,
    #[serde(default)]
    pub default_popup: Option<String>,
}

/// Background service worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Background {
    #[serde(default)]
    pub service_worker: Option<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub persistent: Option<bool>,
}

/// Content script injection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentScript {
    pub matches: Vec<String>,
    #[serde(default)]
    pub exclude_matches: Vec<String>,
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,
    #[serde(default)]
    pub js: Vec<String>,
    #[serde(default)]
    pub css: Vec<String>,
    #[serde(default = "default_run_at")]
    pub run_at: String,
    #[serde(default)]
    pub all_frames: Option<bool>,
    #[serde(default)]
    pub match_origin_as_fallback: Option<bool>,
}

fn default_run_at() -> String {
    "document_idle".to_string()
}

/// Options page UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsUI {
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub open_in_tab: Option<bool>,
}

/// Web accessible resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAccessibleResource {
    pub resources: Vec<String>,
    #[serde(default)]
    pub matches: Vec<String>,
    #[serde(default)]
    pub ids: Vec<String>,
}

/// Externally connectable (web pages that can message this extension).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternallyConnectable {
    pub ids: Option<Vec<String>>,
    pub matches: Option<Vec<String>>,
    #[serde(default)]
    pub accepts_tls_channel_id: Option<bool>,
}

/// Content Security Policy for extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSecurityPolicy {
    #[serde(default)]
    pub extension_pages: Option<String>,
    #[serde(default)]
    pub sandboxed: Option<String>,
}

/// Declarative Net Request rules (MV3 replacement for webRequest blocking).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarativeNetRequest {
    #[serde(default)]
    pub rule_resources: Vec<RuleResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResource {
    #[serde(default)]
    pub id: Option<String>,
    pub enabled: Option<bool>,
    pub path: String,
}
