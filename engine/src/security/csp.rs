//! Content Security Policy enforcement.
//! Restricts what scripts, styles, images, and other resources a page can load.
//! Prevents XSS and injection attacks.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A parsed CSP directive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentSecurityPolicy {
    /// Sources allowed for scripts.
    pub script_src: Vec<String>,
    /// Sources allowed for styles.
    pub style_src: Vec<String>,
    /// Sources allowed for images.
    pub img_src: Vec<String>,
    /// Sources allowed for connections (fetch, XHR, WebSocket).
    pub connect_src: Vec<String>,
    /// Sources allowed for frames.
    pub frame_src: Vec<String>,
    /// Sources allowed for fonts.
    pub font_src: Vec<String>,
    /// Whether 'unsafe-inline' is allowed (should be false for extensions).
    pub unsafe_inline: bool,
    /// Whether 'unsafe-eval' is allowed (should be false for extensions).
    pub unsafe_eval: bool,
    /// Default source fallback.
    pub default_src: Vec<String>,
}

/// Policy alias used by the JS runner and the HTML sanitizer.
pub type CspPolicy = ContentSecurityPolicy;

impl ContentSecurityPolicy {
    /// Parse a CSP header string.
    pub fn parse(header: &str) -> Self {
        parse_csp(header)
    }

    fn script_sources(&self) -> &Vec<String> {
        if self.script_src.is_empty() {
            &self.default_src
        } else {
            &self.script_src
        }
    }

    /// Whether an inline `<script>` may run, given an optional nonce or hash.
    pub fn allows_inline_script(&self, nonce: Option<&str>, hash: Option<&str>) -> bool {
        let sources = self.script_sources();
        if sources.is_empty() {
            return true;
        }
        if sources.iter().any(|s| s == "'unsafe-inline'") {
            return true;
        }
        if let Some(nonce) = nonce {
            let needle = format!("'nonce-{}'", nonce);
            if sources.iter().any(|s| *s == needle) {
                return true;
            }
        }
        if let Some(hash) = hash {
            if sources.iter().any(|s| s.contains(hash)) {
                return true;
            }
        }
        false
    }

    /// Whether a script URL is allowed by `script-src` (falling back to
    /// `default-src`).
    pub fn allows_script_src(&self, url: &str) -> bool {
        if self.script_sources().is_empty() {
            return true;
        }
        is_allowed(self, "script-src", url)
    }
}

/// Whether an attribute is safe to keep on a parsed element.
///
/// Event handler attributes, `srcdoc` and script URLs are rejected; the rest
/// are ordinary content attributes.
pub fn is_safe_attribute(_tag: &str, name: &str, value: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    if name.starts_with("on") {
        return false;
    }
    if name == "srcdoc" {
        return false;
    }
    let value_lower = value.trim().to_ascii_lowercase();
    let compact: String = value_lower.chars().filter(|c| !c.is_whitespace()).collect();
    !(compact.starts_with("javascript:") || compact.starts_with("vbscript:"))
}

/// Parse a CSP header string into a structured policy.
pub fn parse_csp(header: &str) -> ContentSecurityPolicy {
    let mut policy = ContentSecurityPolicy::default();

    for directive in header.split(';') {
        let directive = directive.trim();
        if directive.is_empty() {
            continue;
        }

        let parts: Vec<&str> = directive.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let name = parts[0];
        let sources: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

        match name {
            "script-src" => {
                policy.script_src = sources.clone();
                policy.unsafe_inline = sources.iter().any(|s| s == "'unsafe-inline'");
                policy.unsafe_eval = sources.iter().any(|s| s == "'unsafe-eval'");
            }
            "style-src" => policy.style_src = sources,
            "img-src" => policy.img_src = sources,
            "connect-src" => policy.connect_src = sources,
            "frame-src" => policy.frame_src = sources,
            "font-src" => policy.font_src = sources,
            "default-src" => policy.default_src = sources,
            _ => {}
        }
    }

    policy
}

/// Check if a resource URL is allowed by CSP for a given directive.
pub fn is_allowed(policy: &ContentSecurityPolicy, directive: &str, url: &str) -> bool {
    let sources = match directive {
        "script-src" => &policy.script_src,
        "style-src" => &policy.style_src,
        "img-src" => &policy.img_src,
        "connect-src" => &policy.connect_src,
        "frame-src" => &policy.frame_src,
        "font-src" => &policy.font_src,
        _ => &policy.default_src,
    };

    if sources.is_empty() {
        return true; // No restriction
    }

    // Simple source matching (supports 'self', 'none', https://, etc.)
    for source in sources {
        if source == "'none'" {
            return false;
        }
        if source == "'self'" {
            // In production, compare against the page's origin.
            return true;
        }
        if source.starts_with("https://") || source.starts_with("http://") {
            if url.starts_with(source.trim_end_matches('*')) {
                return true;
            }
        }
        if source == "*" {
            return true;
        }
    }

    false
}

/// Strict CSP for extension pages (Manifest V3 requirement).
pub fn strict_extension_csp() -> ContentSecurityPolicy {
    ContentSecurityPolicy {
        script_src: vec!["'self'".to_string()],
        style_src: vec!["'self'".to_string()],
        img_src: vec!["'self'".to_string()],
        connect_src: vec!["'self'".to_string()],
        frame_src: vec![],
        font_src: vec!["'self'".to_string()],
        unsafe_inline: false,
        unsafe_eval: false,
        default_src: vec!["'self'".to_string()],
    }
}

/// Validate that a CSP is strict enough for extensions.
pub fn validate_extension_csp(policy: &ContentSecurityPolicy) -> Result<(), String> {
    if policy.unsafe_inline {
        return Err("Extension CSP must not allow 'unsafe-inline'".to_string());
    }
    if policy.unsafe_eval {
        return Err("Extension CSP must not allow 'unsafe-eval'".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csp_basic() {
        let policy = parse_csp("script-src 'self' https://cdn.example.com; style-src 'self';");
        assert!(policy.script_src.contains(&"'self'".to_string()));
        assert!(policy.script_src.contains(&"https://cdn.example.com".to_string()));
        assert!(!policy.unsafe_inline);
        assert!(!policy.unsafe_eval);
    }

    #[test]
    fn strict_extension_csp_rejects_unsafe() {
        let policy = strict_extension_csp();
        assert!(!policy.unsafe_inline);
        assert!(!policy.unsafe_eval);
        assert!(validate_extension_csp(&policy).is_ok());
    }

    #[test]
    fn is_allowed_none_blocks_all() {
        let policy = ContentSecurityPolicy {
            script_src: vec!["'none'".to_string()],
            ..Default::default()
        };
        assert!(!is_allowed(&policy, "script-src", "https://example.com"));
    }
}
