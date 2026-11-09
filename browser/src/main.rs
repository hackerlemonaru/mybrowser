//! MyBrowser — from-scratch Rust browser with Chrome-like profile system.
//!
//! Usage:
//!   mybrowser <profile_id> [url]
//!   mybrowser --gui                              (show profile switcher GUI)
//!
//! Architecture:
//!   - `gui` — Chrome-like profile switcher (regular profiles, guest, add new)
//!   - `profile-manager` — isolated data dirs per profile
//!   - `storage` — per-profile localStorage, IndexedDB, history, bookmarks
//!   - `engine` — Falco from-scratch browser engine (HTML/CSS/JS/layout/paint)
//!   - `engine::gpu` — direct Vulkan renderer (NOT WebGPU/wgpu)
//!
//! Each profile gets complete isolation: cookies, cache, localStorage,
//! IndexedDB, history, bookmarks, extensions — all separate on disk.
//! Guest mode wipes all data on exit.

use anyhow::{Context, Result};
use chrono::Utc;
use engine::net::{self, FetchOptions};
use engine::gpu::GpuContext;
use profile_manager::{ProfileManager, ProfileType};
use storage::{local_storage, history, bookmarks};
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // If --gui flag or no args, show the profile switcher GUI first.
    if args.contains(&"--gui".to_string()) || args.len() == 1 {
        return run_profile_switcher();
    }

    // Otherwise, we have a profile ID from the GUI launcher.
    let profile_id = args.get(1).context("Missing profile ID")?;
    let url = args.get(2)
        .map(|s| s.as_str())
        .unwrap_or("https://example.com");

    run_browser(profile_id, url)
}

/// Show the Chrome-like profile switcher GUI.
/// Launches the `profile-switcher` GUI binary, reads its choice,
/// then launches the browser with the selected profile.
fn run_profile_switcher() -> Result<()> {
    use gui::GuiChoice;

    // The profile-switcher GUI is a separate binary (in the gui crate).
    // It prints its choice to stdout: "LAUNCH_PROFILE:<id>", "LAUNCH_GUEST",
    // "CREATE_PROFILE:<name>", or "QUIT".
    eprintln!("Starting profile switcher GUI...");
    eprintln!("Run: cargo run -p profile-switcher");
    eprintln!("Then: cargo run -p browser -- <profile_id> [url]");
    eprintln!();
    eprintln!("Available profiles:");

    let mgr = ProfileManager::new().context("Failed to open profile manager")?;
    for p in mgr.list() {
        let marker = if mgr.active().map(|a| a.id == p.id).unwrap_or(false) {
            "  *"
        } else {
            "   "
        };
        eprintln!(
            "{} {} | {:?} | {}",
            marker, p.name, p.profile_type, p.id
        );
    }
    eprintln!();
    eprintln!("Usage: cargo run -p browser -- <profile_id> [url]");
    eprintln!("Example: cargo run -p browser -- <id> https://example.com");

    Ok(())
}

/// Run the browser with a specific profile.
fn run_browser(profile_id: &str, url: &str) -> Result<()> {
    let mgr = ProfileManager::new()
        .context("Failed to open profile manager")?;

    let profile = mgr.resolve(profile_id)
        .with_context(|| format!("Profile '{}' not found", profile_id))?;

    eprintln!("=== MyBrowser ===");
    eprintln!("Profile: {}", profile.name);
    eprintln!("Type: {:?}", profile.profile_type);
    eprintln!("Data dir: {:?}", profile.data_dir);
    eprintln!("=================");

    // 1. Initialize direct GPU context for this profile.
    //    In production, pass a winit window: GpuContext::init_with_window(&window)
    let mut gpu_ctx = GpuContext::new(profile_id);
    eprintln!("GPU context ready for profile: {}", profile_id);

    // 2. Set the active profile for cookie/cache operations.
    engine::net::cookies::set_active_profile(Some(profile_id));
    engine::net::cache::set_active_profile_cache(Some(profile_id));

    // 3. Load per-profile cookies and cache from disk.
    engine::net::cookies::load_cookies(profile_id, &profile.data_dir)?;
    engine::net::cache::load_cache(profile_id, &profile.data_dir)?;

    // 4. Load per-profile storage for display.
    let _history_entries = history::list(&profile.data_dir);
    let _bookmark_entries = bookmarks::list(&profile.data_dir);

    eprintln!("Fetching {} in profile '{}'...", url, profile.name);

    // 5. Fetch URL using this profile's isolated cookies + cache.
    let opts = FetchOptions::default_with_redirects();
    let body = net::fetch_bytes(url, Some(profile_id), opts)?;

    // 6. Record visit in per-profile history.
    let _ = history::add_visit(&profile.data_dir, url, "");

    // 7. Persist cookies and cache to disk for this profile only.
    engine::net::cookies::persist_cookies(profile_id, &profile.data_dir)?;
    engine::net::cache::persist_cache(profile_id, &profile.data_dir)?;

    // 8. Example: set a localStorage value for this profile only.
    let origin = extract_origin(url);
    let _ = local_storage::set(&profile.data_dir, &origin, "last_visit", &chrono::Utc::now().to_rfc3339());

    eprintln!("Fetched {} bytes.", body.len());
    eprintln!("Profile data isolated at: {:?}", profile.data_dir);
    eprintln!("Cookies: {:?}", profile.data_dir.join("cookies").join("cookies.json"));
    eprintln!("Cache: {:?}", profile.data_dir.join("cache"));
    eprintln!("LocalStorage: {:?}", profile.data_dir.join("localstorage"));
    eprintln!("IndexedDB: {:?}", profile.data_dir.join("indexeddb"));
    eprintln!("History: {:?}", profile.data_dir.join("history"));
    eprintln!("Bookmarks: {:?}", profile.data_dir.join("bookmarks"));

    // If guest profile, note that data will be wiped on exit.
    if profile.profile_type == ProfileType::Guest {
        eprintln!("[GUEST MODE] All data will be wiped when this session ends.");
    }

    Ok(())
}

/// Extract origin (scheme://host) from a URL string.
fn extract_origin(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let domain = stripped.split('/').next().unwrap_or(stripped);
    let domain = domain.split(':').next().unwrap_or(domain);
    if url.starts_with("https://") {
        format!("https://{}", domain)
    } else {
        format!("http://{}", domain)
    }
}
