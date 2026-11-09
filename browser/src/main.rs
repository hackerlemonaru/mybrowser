//! MyBrowser — a Rust from-scratch browser with Chrome-like profile system.
//!
//! Built on:
//!   - `engine` — Falco, a from-scratch Rust browser engine (HTML/CSS/JS/layout/paint)
//!   - `profile-manager` — Chrome-style isolated profiles (cookies, cache, history, bookmarks, extensions)
//!
//! Each profile gets its own data directory. Switching profiles swaps the
//! active cookie jar, HTTP cache, and storage — zero session leakage between
//! workloads (Work, Personal, Banking, etc.).

use engine::net::{self, FetchOptions};
use profile_manager::ProfileManager;
use std::env;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut mgr = ProfileManager::new()?;

    match args.get(1).map(|s| s.as_str()) {
        Some("list") => list_profiles(&mgr),
        Some("create") => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("Work");
            match mgr.create(name.to_string()) {
                Ok(p) => {
                    println!("Created profile '{}' (id: {})", p.name, p.id);
                    println!("Data dir: {:?}", p.data_dir);
                }
                Err(e) => eprintln!("Failed to create profile: {}", e),
            }
        }
        Some("switch") => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match mgr.switch(target) {
                Ok(()) => {
                    let p = mgr.resolve(target).unwrap();
                    println!("Switched to profile '{}'", p.name);
                    println!("Active data dir: {:?}", p.data_dir);
                }
                Err(e) => eprintln!("{}", e),
            }
        }
        Some("delete") => {
            let id = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match mgr.delete(id) {
                Ok(()) => println!("Deleted profile '{}'", id),
                Err(e) => eprintln!("{}", e),
            }
        }
        Some("active") => match mgr.active() {
            Some(p) => {
                println!("Active profile: {} ({})", p.name, p.id);
                println!("Data dir: {:?}", p.data_dir);
            }
            None => println!("No profiles yet. Run: mybrowser create <name>"),
        },
        Some("browse") => {
            let url = args.get(2).map(|s| s.as_str()).unwrap_or("https://example.com");
            let target = args.get(3).map(|s| s.as_str()).unwrap_or("");
            browse(url, target, &mgr)?
        }
        Some("config") => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if let Some(p) = mgr.resolve(target) {
                show_config(p);
            } else {
                eprintln!("Profile '{}' not found", target);
            }
        }
        _ => {
            print_usage();
        }
    }

    Ok(())
}

/// Browse a URL within a specific profile's isolated context.
fn browse(url: &str, profile_id_or_name: &str, mgr: &ProfileManager) -> anyhow::Result<()> {
    let profile = mgr
        .resolve(profile_id_or_name)
        .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", profile_id_or_name))?;

    // Set the active profile for cookie/cache operations.
    engine::net::cookies::set_active_profile(Some(&profile.id));
    engine::net::cache::set_active_profile_cache(Some(&profile.id));

    // Load persisted cookies and cache for this profile.
    engine::net::cookies::load_cookies(&profile.id, &profile.data_dir)?;
    engine::net::cache::load_cache(&profile.id, &profile.data_dir)?;

    println!("Browsing in profile '{}' ({})", profile.name, profile.id);
    println!("URL: {}", url);
    println!("Data dir: {:?}", profile.data_dir);

    let opts = FetchOptions::default_with_redirects();
    let body = net::fetch_bytes(url, Some(&profile.id), opts)?;

    println!("Fetched {} bytes", body.len());

    // Persist cookies and cache after browsing.
    engine::net::cookies::persist_cookies(&profile.id, &profile.data_dir)?;
    engine::net::cache::persist_cache(&profile.id, &profile.data_dir)?;

    Ok(())
}

fn list_profiles(mgr: &ProfileManager) {
    let profiles = mgr.list();
    if profiles.is_empty() {
        println!("No profiles found.");
        return;
    }

    let active_id = mgr.active().map(|p| p.id.as_str());
    println!("Profiles:");
    for p in profiles {
        let marker = if active_id == Some(p.id.as_str()) { "  *" } else { "   " };
        let used = p.last_used.as_deref().unwrap_or("never");
        println!(
            "{} {} | {} | id={} | last used={}",
            marker, p.name, p.id, p.id, used
        );
    }
}

fn show_config(profile: &profile_manager::Profile) {
    println!("Profile: {}", profile.name);
    println!("ID: {}", profile.id);
    println!("Data dir: {:?}", profile.data_dir);
    println!("Cookies path: {:?}", profile.data_dir.join("cookies").join("cookies.json"));
    println!("Cache path: {:?}", profile.data_dir.join("cache"));
    println!("History path: {:?}", profile.data_dir.join("history"));
    println!("Bookmarks path: {:?}", profile.data_dir.join("bookmarks"));
    println!("Extensions path: {:?}", profile.data_dir.join("extensions"));
    println!("LocalStorage path: {:?}", profile.data_dir.join("localstorage"));
    println!("Sessions path: {:?}", profile.data_dir.join("sessions"));
}

fn print_usage() {
    println!("MyBrowser — Chrome-like profile browser built on a from-scratch Rust engine");
    println!();
    println!("Usage:");
    println!("  mybrowser list                          List all profiles");
    println!("  mybrowser create <name>                 Create a new isolated profile");
    println!("  mybrowser switch <id|name>              Switch active profile");
    println!("  mybrowser delete <id>                   Delete a profile and its data");
    println!("  mybrowser active                        Show the active profile");
    println!("  mybrowser browse <url> <profile>        Browse a URL in a profile's context");
    println!("  mybrowser config <id|name>              Show profile config & paths");
    println!();
    println!("Each profile gets its own isolated data directory (cookies, cache,");
    println!("history, bookmarks, extensions, localstorage, sessions). Switching");
    println!("profiles swaps the active storage — zero session leakage between");
    println!("workloads, exactly like Chrome profiles.");
}
