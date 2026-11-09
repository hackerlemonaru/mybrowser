mod browser;
mod profile;

use profile::ProfileManager;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut mgr = ProfileManager::default();

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
        Some("config") => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if let Some(p) = mgr.resolve(target) {
                let cfg = browser::BrowserConfig::for_profile(&p.id, &p.name, p.data_dir.clone());
                println!("{:#?}", cfg);
            } else {
                eprintln!("Profile '{}' not found", target);
            }
        }
        _ => {
            println!("MyBrowser - Chrome-like profile manager in Rust");
            println!();
            println!("Usage:");
            println!("  mybrowser list                          List all profiles");
            println!("  mybrowser create <name>                 Create a new isolated profile");
            println!("  mybrowser switch <id|name>              Switch active profile");
            println!("  mybrowser delete <id>                   Delete a profile and its data");
            println!("  mybrowser active                        Show the active profile");
            println!("  mybrowser config <id|name>              Show browser config for a profile");
            println!();
            println!("Each profile gets its own isolated data directory (cookies,");
            println!("history, bookmarks, extensions, cache) so workloads stay separate.");
        }
    }
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
        let used = p
            .last_used
            .as_deref()
            .unwrap_or("never");
        println!(
            "{} {} | {} | id={} | last used={}",
            marker, p.name, p.id, p.id, used
        );
    }
}
