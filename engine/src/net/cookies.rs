//! Cookie jar — parses Set-Cookie headers and sends Cookie headers.
//! Profile-aware: each profile has its own cookie jar persisted to disk
//! under <profile_data_dir>/cookies/cookies.json.
//!
//! Supports:
//! - Set-Cookie: name=value; Path=/; Domain=example.com; Secure; HttpOnly
//! - Cookie: name1=value1; name2=value2
//! - Domain matching (simplified)
//! - Path matching
//! - Expiration (via max-age)
//! - Secure flag (only sent over HTTPS)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// A stored cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub max_age: Option<u64>,
}

/// Profile-aware cookie storage. Keyed by profile ID.
struct CookieStore {
    jars: HashMap<String, Vec<Cookie>>,
}

static COOKIE_STORE: once_cell::sync::Lazy<Mutex<CookieStore>> =
    once_cell::sync::Lazy::new(|| Mutex::new(CookieStore {
        jars: HashMap::new(),
    }));

/// Set the active profile ID for the current thread's cookie operations.
/// In a multi-threaded browser, each profile's window/tab should pass its
/// profile_id explicitly to the cookie functions.
static ACTIVE_PROFILE: Mutex<Option<String>> = Mutex::new(None);

pub fn set_active_profile(id: Option<&str>) {
    *ACTIVE_PROFILE.lock().unwrap() = id.map(|s| s.to_string());
}

pub fn active_profile() -> Option<String> {
    ACTIVE_PROFILE.lock().unwrap().clone()
}

/// Get the Cookie header value for a URL, using the active profile's jar.
pub fn get_cookie_header(url: &str, profile_id: Option<&str>) -> Option<String> {
    let active = active_profile();
    let pid = profile_id.or(active.as_deref())?;
    let (domain, path, is_secure) = parse_url_parts(url);
    let jar = get_jar(pid);

    let matching: Vec<&Cookie> = jar
        .iter()
        .filter(|c| {
            domain_matches(&domain, &c.domain)
                && path_matches(&path, &c.path)
                && (!c.secure || is_secure)
        })
        .collect();

    if matching.is_empty() {
        return None;
    }

    Some(
        matching
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Process Set-Cookie headers from a response, storing in the active profile's jar.
pub fn process_response_cookies(url: &str, resp: &ureq::Response, profile_id: Option<&str>) {
    let active = active_profile();
    let Some(pid) = profile_id.or(active.as_deref()) else {
        return;
    };
    let (domain, _, _) = parse_url_parts(url);

    for header_name in resp.headers_names() {
        if header_name.eq_ignore_ascii_case("set-cookie") {
            if let Some(value) = resp.header(&header_name) {
                parse_set_cookie(value, &domain, pid);
            }
        }
    }
}

/// Persist the active profile's cookies to disk.
pub fn persist_cookies(profile_id: &str, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let jar = get_jar(profile_id);
    let cookies_path = data_dir.join("cookies").join("cookies.json");
    if let Some(parent) = cookies_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&*jar)?;
    fs::write(cookies_path, content)?;
    Ok(())
}

/// Load a profile's cookies from disk.
pub fn load_cookies(profile_id: &str, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cookies_path = data_dir.join("cookies").join("cookies.json");
    if !cookies_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(cookies_path)?;
    let cookies: Vec<Cookie> = serde_json::from_str(&content)?;
    let mut store = COOKIE_STORE.lock().unwrap();
    store.jars.insert(profile_id.to_string(), cookies);
    Ok(())
}

/// Clear a profile's cookies (memory and disk).
pub fn clear_cookies(profile_id: &str, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = COOKIE_STORE.lock().unwrap();
    store.jars.remove(profile_id);
    drop(store);

    let cookies_path = data_dir.join("cookies").join("cookies.json");
    let _ = fs::remove_file(cookies_path);
    Ok(())
}

fn get_jar(profile_id: &str) -> Vec<Cookie> {
    let store = COOKIE_STORE.lock().unwrap();
    store
        .jars
        .get(profile_id)
        .cloned()
        .unwrap_or_default()
}

fn parse_set_cookie(header: &str, default_domain: &str, profile_id: &str) {
    let parts: Vec<&str> = header.split(';').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return;
    }

    let nv = parts[0];
    let eq = match nv.find('=') {
        Some(i) => i,
        None => return,
    };
    let name = nv[..eq].trim().to_string();
    let value = nv[eq + 1..].trim().to_string();

    let mut domain = default_domain.to_string();
    let mut path = "/".to_string();
    let mut secure = false;
    let mut http_only = false;
    let mut max_age = None;

    for part in &parts[1..] {
        let lower = part.to_lowercase();
        if lower.starts_with("domain=") {
            domain = part[7..].trim().to_string();
        } else if lower.starts_with("path=") {
            path = part[5..].trim().to_string();
        } else if lower == "secure" {
            secure = true;
        } else if lower == "httponly" {
            http_only = true;
        } else if lower.starts_with("max-age=") {
            max_age = part[8..].trim().parse().ok();
        }
    }

    let cookie = Cookie {
        name,
        value,
        domain,
        path,
        secure,
        http_only,
        max_age,
    };

    let mut store = COOKIE_STORE.lock().unwrap();
    let jar = store.jars.entry(profile_id.to_string()).or_default();
    jar.retain(|c| {
        !(c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path)
    });
    jar.push(cookie);
}

fn parse_url_parts(url: &str) -> (String, String, bool) {
    let is_secure = url.starts_with("https://") || url.starts_with("wss://");
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("ws://"))
        .or_else(|| url.strip_prefix("wss://"))
        .unwrap_or(url);

    let (domain_part, path_part) = match stripped.find('/') {
        Some(i) => (&stripped[..i], &stripped[i..]),
        None => (stripped, "/"),
    };

    let domain = domain_part
        .split(':')
        .next()
        .unwrap_or(domain_part)
        .to_string();
    let path = if path_part.is_empty() {
        "/".to_string()
    } else {
        path_part.to_string()
    };

    (domain, path, is_secure)
}

fn domain_matches(request_domain: &str, cookie_domain: &str) -> bool {
    if cookie_domain.is_empty() {
        return true;
    }
    let cd = cookie_domain.trim_start_matches('.');
    request_domain == cd || request_domain.ends_with(&format!(".{}", cd))
}

fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    if cookie_path == "/" {
        return true;
    }
    request_path.starts_with(cookie_path)
}

/// Get all cookies as a HashMap (for JS document.cookie — future).
pub fn all_cookies(profile_id: &str) -> HashMap<String, String> {
    let jar = get_jar(profile_id);
    jar.iter()
        .map(|c| (c.name.clone(), c.value.clone()))
        .collect()
}

use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_parts() {
        let (d, p, s) = parse_url_parts("https://example.com/path/page");
        assert_eq!(d, "example.com");
        assert_eq!(p, "/path/page");
        assert!(s);

        let (d, p, s) = parse_url_parts("http://sub.example.com:8080/");
        assert_eq!(d, "sub.example.com");
        assert_eq!(p, "/");
        assert!(!s);
    }

    #[test]
    fn domain_matching() {
        assert!(domain_matches("example.com", "example.com"));
        assert!(domain_matches("www.example.com", "example.com"));
        assert!(domain_matches("www.example.com", ".example.com"));
        assert!(!domain_matches("other.com", "example.com"));
    }

    #[test]
    fn path_matching() {
        assert!(path_matches("/page", "/"));
        assert!(path_matches("/api/users", "/api"));
        assert!(!path_matches("/other", "/api"));
    }
}
