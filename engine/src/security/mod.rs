//! Security module — Site Isolation, CSP, Sandbox, and extension security.

pub mod site_isolation;
pub mod csp;
pub mod origin;
pub mod sandbox;
pub mod permissions;

// Re-export key types.
pub use site_isolation::{SiteIsolation, IsolationContext};
pub use csp::{ContentSecurityPolicy, parse_csp, is_allowed, strict_extension_csp, validate_extension_csp};
pub use sandbox::{SandboxManager, SandboxConfig, SandboxLevel};

/// Global security manager shared across the browser.
static SECURITY: once_cell::sync::Lazy<SecurityManager> = once_cell::sync::Lazy::new(SecurityManager::new);

pub struct SecurityManager {
    pub site_isolation: SiteIsolation,
    pub sandbox: SandboxManager,
}

impl SecurityManager {
    pub fn new() -> Self {
        Self {
            site_isolation: SiteIsolation::new(),
            sandbox: SandboxManager::new(),
        }
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn global() -> &'static SecurityManager {
    &SECURITY
}
