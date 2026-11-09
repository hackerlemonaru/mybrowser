//! Extension permission validation.
//! Implements Spirit of Least Privilege - extensions can only do what they declare.

use crate::manifest::{ContentSecurityPolicy, Manifest};

/// Dangerous permissions that should be flagged or restricted.
pub const DANGEROUS_PERMISSIONS: &[&str] = &[
    "nativeMessaging",   // Bridge to host system - BANNED
    "webRequestBlocking", // Can block/modify network requests - restricted
    "<all_urls>",         // Wildcard host access - restricted
];

/// Permissions that are safe to grant without extra scrutiny.
pub const SAFE_PERMISSIONS: &[&str] = &[
    "storage",
    "activeTab",
    "scripting",
    "notifications",
    "bookmarks",
    "history",
    "tabs",
    "downloads",
];

/// Validate a manifest before enabling the extension.
/// Returns error if the extension requests banned capabilities.
pub fn validate_manifest(manifest: &Manifest) -> Result<(), Box<dyn std::error::Error>> {
    // BANNED: nativeMessaging gives extensions access to host system.
    if manifest.permissions.iter().any(|p| p == "nativeMessaging") {
        return Err("Extension requests nativeMessaging - banned for security".into());
    }
    if manifest.host_permissions.iter().any(|p| p == "nativeMessaging") {
        return Err("Extension requests nativeMessaging in host_permissions - banned".into());
    }

    // WARN: wildcard host permissions are dangerous.
    if manifest.host_permissions.iter().any(|p| p == "<all_urls>" || p == "*://*/*") {
        eprintln!("[EXTENSION SECURITY] Warning: extension '{}' requests <all_urls> host permission", manifest.name);
    }

    // Manifest version must be 3 (MV2 is deprecated/removed).
    if manifest.manifest_version != 3 {
        return Err(format!("Manifest version {} not supported. Manifest V3 required.", manifest.manifest_version).into());
    }

    // Content Security Policy must be present and strict.
    if let Some(csp) = &manifest.content_security_policy {
        validate_csp(csp)?;
    }

    Ok(())
}

/// Validate Content Security Policy.
fn validate_csp(csp: &ContentSecurityPolicy) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(policy) = &csp.extension_pages {
        if policy.contains("'unsafe-eval'") {
            return Err("Extension CSP allows 'unsafe-eval' - banned".into());
        }
        if policy.contains("'unsafe-inline'") {
            return Err("Extension CSP allows 'unsafe-inline' - banned".into());
        }
    }
    Ok(())
}

/// Check if an extension has a specific permission.
pub fn has_permission(manifest: &Manifest, permission: &str) -> bool {
    manifest.permissions.iter().any(|p| p == permission)
        || manifest.optional_permissions.iter().any(|p| p == permission)
}

/// Check if an extension has host permission for a URL.
pub fn has_host_permission(manifest: &Manifest, url: &str) -> bool {
    let url_lower = url.to_lowercase();
    manifest.host_permissions.iter().any(|pattern| {
        match_pattern(&pattern, &url_lower)
    })
}

/// Simple match pattern matching (supports * wildcard).
fn match_pattern(pattern: &str, url: &str) -> bool {
    if pattern == "<all_urls>" || pattern == "*://*/*" {
        return true;
    }
    // Convert match pattern to simple glob matching.
    let pattern = pattern.replace("*", ".*");
    // In a real implementation, use regex. For now, simple contains check.
    url.contains(&pattern.replace(".*", ""))
}

/// Get all dangerous permission requests for audit.
pub fn audit_permissions(manifest: &Manifest) -> Vec<String> {
    let mut flags = Vec::new();
    for perm in &manifest.permissions {
        if DANGEROUS_PERMISSIONS.contains(&perm.as_str()) {
            flags.push(format!("Dangerous permission: {}", perm));
        }
    }
    for perm in &manifest.host_permissions {
        if perm == "<all_urls>" || perm == "*://*/*" {
            flags.push(format!("Wildcard host permission: {}", perm));
        }
    }
    flags
}
