//! MyBrowser — Full integrated browser.
//!
//! Usage:
//!   mybrowser <profile_id> [url]          CLI mode — fetch a URL and exit
//!   mybrowser --window <profile_id>       GUI browser window with tabs
//!   mybrowser --gui                       Profile switcher launcher
//!
//! Connects:
//!   - Profile switcher GUI (gui/)
//!   - Profile Manager (isolated data dirs)
//!   - Storage: localStorage, IndexedDB, history, bookmarks (per profile)
//!   - Engine: Falco (from-scratch HTML/CSS/JS/layout/paint)
//!   - GPU: Direct Vulkan (ash — NOT WebGPU/wgpu)
//!   - Proxy: Per-profile SOCKS5/HTTP
//!   - Fingerprint: Canvas, WebGL, Audio, Fonts, Screen (per profile, consistent)
//!   - Tab Management: Groups, pin, mute, freeze, search, vertical tabs
//!   - Dummy System Profile: 48+ real device profiles for fingerprint spoofing
//!   - Extensions: Manifest V3 (sandboxed, per profile, no system access)

use anyhow::Result;
use chrono::Utc;
use engine::fingerprint::{get_or_create, apply_to_headers};
use engine::net::{
    self, proxy::ProxyManager, init_custom_domain_registry,
    password_manager::{PasswordManager, PasswordOptions},
};
use extensions::ExtensionManager;
use profile_manager::{ProfileManager, ProfileType};
use storage::{local_storage, history, bookmarks, download};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
mod gl;
use egui_glow::winit::EguiGlow;
mod ui;
mod window;

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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--gui".to_string()) || args.len() == 1 {
        return run_profile_switcher();
    }

    // ── Custom domain management subcommands ──
    if args.contains(&"--register-domain".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        let domain = args
            .iter()
            .position(|a| a == "--register-domain")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!("Missing domain after --register-domain"))?;
        let serving_url = args
            .iter()
            .position(|a| a == "--serving-url")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!("Missing --serving-url"))?;
        return register_custom_domain(profile_id, domain, serving_url);
    }

    if args.contains(&"--list-domains".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return list_custom_domains(profile_id);
    }

    if args.contains(&"--unregister-domain".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        let domain = args
            .iter()
            .position(|a| a == "--unregister-domain")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!("Missing domain after --unregister-domain"))?;
        return unregister_custom_domain(profile_id, domain);
    }

    if args.contains(&"--domain-stats".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return domain_stats(profile_id);
    }

    // ── Download manager subcommands ──
    if args.contains(&"--download-list".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return download_list(profile_id);
    }

    if args.contains(&"--download-stats".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return download_stats(profile_id);
    }

    if args.contains(&"--download-cancel".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        let id = args
            .iter()
            .position(|a| a == "--download-cancel")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!("Missing download ID after --download-cancel"))?;
        return download_cancel(profile_id, id);
    }

    // ── Password manager subcommands ──
    if args.contains(&"--password-list".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return password_list(profile_id);
    }

    if args.contains(&"--password-gen".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return password_gen(profile_id);
    }

    if args.contains(&"--password-health".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return password_health(profile_id);
    }

    if args.contains(&"--password-export".to_string()) {
        let profile_id = args.get(1).ok_or_else(|| anyhow::anyhow!("Missing profile ID"))?;
        return password_export(profile_id);
    }

    // ── Profile management subcommands ──
    if args.get(1).map(|s| s.as_str()) == Some("create") {
        let name = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "Profile".to_string());
        let mut mgr = ProfileManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;
        let profile = mgr
            .create(name)
            .map_err(|e| anyhow::anyhow!("Failed to create profile: {}", e))?;
        println!("[Profile] Created '{}' ({})", profile.name, profile.id);
        println!("[Profile] Data dir: {:?}", profile.data_dir);
        return Ok(());
    }

    if args.get(1).map(|s| s.as_str()) == Some("list") {
        let mgr = ProfileManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;
        for profile in mgr.list() {
            println!("{} | {:?} | {}", profile.name, profile.profile_type, profile.id);
        }
        return Ok(());
    }

    if args.contains(&"--window".to_string()) {
        let profile_id = args
            .iter()
            .position(|a| a == "--window")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!("Missing profile ID after --window"))?;
        return run_browser_window(profile_id);
    }

    let profile_id = args.get(1)
        .ok_or_else(|| anyhow::anyhow!("Missing profile ID. Usage: mybrowser <profile_id> [url]"))?;
    let url = args.get(2)
        .map(|s| s.as_str())
        .unwrap_or("https://example.com");

    run_browser(profile_id, url)
}

/// Launch the profile switcher GUI, then start the browser with the selected profile.
fn run_profile_switcher() -> Result<()> {
    eprintln!("Starting MyBrowser profile switcher...");
    eprintln!("Run: cargo run -p profile-switcher");
    eprintln!("Then: cargo run -p browser -- --window <profile_id>");
    eprintln!("Or:   cargo run -p browser -- <profile_id> [url]");
    eprintln!();

    let mgr = ProfileManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;
    eprintln!("Available profiles:");
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
    eprintln!("Usage: cargo run -p browser -- --window <profile_id>");
    eprintln!("Example: cargo run -p browser -- --window <id>");

    Ok(())
}

/// Run the GUI browser window with a specific profile.
fn run_browser_window(profile_id: &str) -> Result<()> {
    let mgr = ProfileManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;

    let _profile = mgr.resolve(profile_id)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_id))?;

    eprintln!("═════════════════════════════════════════════════");
    eprintln!("  MyBrowser — Profile: {}", profile_id);
    eprintln!("  Data dir: {:?}", _profile.data_dir);
    eprintln!("═════════════════════════════════════════════════");

    // Initialize GPU context.
    // Per-profile proxy setup.
    let proxy_config = ProxyManager::get_config(profile_id);
    if let Some(ref pc) = proxy_config {
        eprintln!(
            "[Proxy] Profile using {}://{}:{}",
            match pc.proxy_type {
                engine::net::proxy::ProxyType::Http => "http",
                engine::net::proxy::ProxyType::Socks5 => "socks5",
                engine::net::proxy::ProxyType::Socks5h => "socks5h",
                engine::net::proxy::ProxyType::Direct => "direct",
            },
            pc.host,
            pc.port
        );
    } else {
        eprintln!("[Proxy] Direct connection (no proxy configured)");
    }

    // Fingerprint spoofing.
    let fp = get_or_create(profile_id);
    eprintln!("[Fingerprint] User-Agent: {}", fp.user_agent.to_string());
    eprintln!("[Fingerprint] WebGL: {} {}", fp.webgl.vendor, fp.webgl.renderer);
    eprintln!("[Fingerprint] Screen: {}x{}", fp.screen.width, fp.screen.height);
    eprintln!("[Fingerprint] Timezone: {}", fp.timezone);
    eprintln!("[Fingerprint] Canvas hash: {}", fp.canvas.expected_hash);

    // Apply fingerprint to HTTP headers globally.
    apply_to_headers(profile_id, &mut std::collections::HashMap::new());

    // Load per-profile cookies and cache.
    engine::net::cookies::set_active_profile(Some(profile_id));
    engine::net::cache::set_active_profile_cache(Some(profile_id));
    engine::net::cookies::load_cookies(profile_id, &_profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load cookies: {}", e))?;
    engine::net::cache::load_cache(profile_id, &_profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load cache: {}", e))?;

    // ── Launch egui + winit browser window ──
    run_egui_window(profile_id)
}

/// Run the browser window with a specific profile (egui + winit).
/// Accessible via VNC/noVNC on the VPS IP.
///
/// Full OpenGL rendering via egui_glow (GLX context created in gl/mod.rs).
/// The egui UI is tessellated and painted to the window every frame.
fn run_egui_window(profile_id: &str) -> Result<()> {
    let mgr = ProfileManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;

    let _profile = mgr.resolve(profile_id)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_id))?;

    eprintln!("═════════════════════════════════════════════════");
    eprintln!("  MyBrowser GUI — Profile: {}", profile_id);
    eprintln!("═════════════════════════════════════════════════");

    let event_loop = winit::event_loop::EventLoop::new()?;

    let mut app = BrowserApp {
        profile_id: profile_id.to_string(),
        window: None,
        gl_ctx: None,
        egui_glow: None,
        browser_window: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}

struct BrowserApp {
    profile_id: String,
    window: Option<winit::window::Window>,
    gl_ctx: Option<crate::gl::GlContext>,
    egui_glow: Option<EguiGlow>,
    browser_window: Option<window::BrowserWindow>,
}

impl winit::application::ApplicationHandler for BrowserApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        eprintln!("[GUI] Creating window for profile: {}", self.profile_id);

        // Chrome-style frame: the UI paints the tab strip, toolbar and window
        // controls itself, so no OS title bar is drawn over it.
        let window = event_loop
            .create_window(
                winit::window::Window::default_attributes()
                    .with_title("New Tab")
                    .with_visible(true)
                    .with_decorations(false)
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                    .with_min_inner_size(winit::dpi::LogicalSize::new(860.0, 540.0)),
            )
            .expect("failed to create window");

        // Ensure the window is mapped and visible on the X11 display (fixes black screen on VNC).
        window.set_visible(true);
        // Place the window fully on screen: the default position can leave part
        // of the client area off the left/top edge of the desktop.
        window.set_outer_position(winit::dpi::PhysicalPosition::new(40, 30));

        let mut browser_window =
            window::BrowserWindow::new(&self.profile_id).expect("failed to create browser window");

        let gl_ctx =
            unsafe { crate::gl::GlContext::new_from_window(&window) }
                .expect("failed to create OpenGL context");

        let egui_glow =
            EguiGlow::new(&event_loop, gl_ctx.gl_arc(), None, None, false);

        // egui applies `set_fonts` at the start of the next frame, so the
        // browser fonts must be installed before the first frame; otherwise
        // the first paint lays out "medium"/"brand" text against the default
        // families and panics.
        crate::ui::fonts::install(&egui_glow.egui_ctx);
        browser_window.mark_fonts_installed();

        self.window = Some(window);
        self.gl_ctx = Some(gl_ctx);
        self.egui_glow = Some(egui_glow);
        self.browser_window = Some(browser_window);

        eprintln!("[GUI] Window created with OpenGL context.");
        eprintln!("[GUI] BrowserWindow UI wired (tabs, dummy profile, location).");
        eprintln!("[GUI] egui_glow painter active. Rendering to screen.");
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };
        let egui_glow = match &mut self.egui_glow {
            Some(w) => w,
            None => return,
        };

        let egui_event = egui_glow.on_window_event(window, &event);
        if egui_event.repaint {
            window.request_redraw();
        }

        match event {
            winit::event::WindowEvent::CloseRequested => {
                _event_loop.exit();
            }
            winit::event::WindowEvent::Resized(_) => {
                window.request_redraw();
            }
            winit::event::WindowEvent::ScaleFactorChanged { .. } => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };
        let Some(gl_ctx) = self.gl_ctx.as_ref() else { return };
        let Some(egui_glow) = self.egui_glow.as_mut() else { return };

        // Disjoint-field borrows: egui_glow owns input/ctx/painter,
        // browser_window is only touched inside the egui UI callback.
        let mut browser_window = self.browser_window.as_mut();
        let mut should_close = false;
        egui_glow.run(window, |egui_ctx| {
            if let Some(bw) = browser_window.as_mut() {
                should_close = bw.render(egui_ctx);
            }
        });
        egui_glow.paint(window);

        if should_close {
            event_loop.exit();
        }

        unsafe { gl_ctx.swap_buffers() };

        // Repaint only when egui actually asked for one: the old unconditional
        // `request_redraw` kept the GPU busy at 100% while the browser was idle.
        if egui_glow.egui_ctx.has_requested_repaint() {
            window.request_redraw();
        }
    }
}

/// Run the full browser with a specific profile (CLI mode).
fn run_browser(profile_id: &str, url: &str) -> Result<()> {
    let mgr = ProfileManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to open profile manager: {}", e))?;

    let profile = mgr.resolve(profile_id)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_id))?;

    // ── Initialize custom domain registry (per-profile data dir) ──
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");
    init_custom_domain_registry(&profiles_dir)?;

    // Load this profile's existing custom domains from disk
    {
        let registry_guard = engine::net::CUSTOM_DOMAIN_REGISTRY.lock().unwrap();
        if let Some(registry) = registry_guard.as_ref() {
            let _ = registry.load_profile_domains(profile_id);
        }
    }

    eprintln!("═════════════════════════════════════════════════");
    eprintln!("  MyBrowser — Profile: {}", profile.name);
    eprintln!("  Type: {:?}", profile.profile_type);
    eprintln!("  Data dir: {:?}", profile.data_dir);
    eprintln!("═════════════════════════════════════════════════");

    // ── 1. Per-profile proxy setup ──
    let proxy_config = ProxyManager::get_config(profile_id);
    if let Some(ref pc) = proxy_config {
        eprintln!(
            "[Proxy] Profile using {}://{}:{}",
            match pc.proxy_type {
                engine::net::proxy::ProxyType::Http => "http",
                engine::net::proxy::ProxyType::Socks5 => "socks5",
                engine::net::proxy::ProxyType::Socks5h => "socks5h",
                engine::net::proxy::ProxyType::Direct => "direct",
            },
            pc.host,
            pc.port
        );
    } else {
        eprintln!("[Proxy] Direct connection (no proxy configured)");
    }

    // ── 3. Fingerprint spoofing ──
    let fp = get_or_create(profile_id);
    eprintln!("[Fingerprint] User-Agent: {}", fp.user_agent.to_string());
    eprintln!("[Fingerprint] WebGL: {} {}", fp.webgl.vendor, fp.webgl.renderer);
    eprintln!("[Fingerprint] Screen: {}x{}", fp.screen.width, fp.screen.height);
    eprintln!("[Fingerprint] Timezone: {}", fp.timezone);
    eprintln!("[Fingerprint] Canvas hash: {}", fp.canvas.expected_hash);

    // Apply fingerprint to HTTP headers globally.
    apply_to_headers(profile_id, &mut std::collections::HashMap::new());

    // ── 4. Load per-profile cookies and cache ──
    engine::net::cookies::set_active_profile(Some(profile_id));
    engine::net::cache::set_active_profile_cache(Some(profile_id));
    engine::net::cookies::load_cookies(profile_id, &profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load cookies: {}", e))?;
    engine::net::cache::load_cache(profile_id, &profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load cache: {}", e))?;

    // ── 5. Initialize extension manager for this profile ──
    let mut ext_mgr = ExtensionManager::new(
        profile_id.to_string(),
        &profile.data_dir,
    ).map_err(|e| anyhow::anyhow!("Failed to init extensions: {}", e))?;
    eprintln!("[Extensions] {} extensions loaded for profile", ext_mgr.list().len());

    // ── 6. Load per-profile storage ──
    let history_entries = history::list(&profile.data_dir);
    let bookmark_entries = bookmarks::list(&profile.data_dir);
    eprintln!(
        "[Storage] {} history entries, {} bookmarks",
        history_entries.len(),
        bookmark_entries.len()
    );

    // ── 7. Fetch URL using profile's isolated context ──
    eprintln!(
        "[Network] Fetching {} in profile '{}'...",
        url, profile.name
    );

    let body = net::fetch_bytes(url, Some(profile_id))?;

    // ── 8. Record visit in per-profile history ──
    let _ = history::add_visit(&profile.data_dir, url, "");

    // ── 9. Persist cookies and cache ──
    engine::net::cookies::persist_cookies(profile_id, &profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to persist cookies: {}", e))?;
    engine::net::cache::persist_cache(profile_id, &profile.data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to persist cache: {}", e))?;

    // ── 10. Set localStorage for this profile only ──
    let origin = extract_origin(url);
    let _ = local_storage::set(&profile.data_dir, &origin, "last_visit", &Utc::now().to_rfc3339());

    eprintln!("[Network] Fetched {} bytes", body.len());
    eprintln!(
        "[Storage] Profile data isolated at: {:?}",
        profile.data_dir
    );

    // ── 11. (GUI mode --window launches the egui window via run_egui_window.) ──
    eprintln!("[Browser] Profile context ready. Use --window to open the GUI.");

    // ── 12. If guest, wipe on exit ──
    if profile.profile_type == ProfileType::Guest {
        eprintln!("[GUEST] All data will be wiped when this session ends.");
        let mut mgr = mgr;
        let _ = mgr.wipe_guest(profile_id);
    }

    Ok(())
}

// ── Custom Domain CLI handlers ──

/// Register a custom domain → serving URL mapping for a profile.
fn register_custom_domain(profile_id: &str, domain: &str, serving_url: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    init_custom_domain_registry(&profiles_dir)?;

    let registry_guard = engine::net::CUSTOM_DOMAIN_REGISTRY.lock().unwrap();
    let registry = registry_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Custom domain registry not initialized"))?;

    registry.register_domain(profile_id, domain, serving_url)?;
    eprintln!(
        "[Custom Domain] Registered {} → {} (profile: {})",
        domain, serving_url, profile_id
    );
    eprintln!(
        "[Custom Domain] HF hits: only for this domain (not for .com/.net/.org traffic)"
    );
    Ok(())
}

/// List all custom domains for a profile.
fn list_custom_domains(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    init_custom_domain_registry(&profiles_dir)?;

    let registry_guard = engine::net::CUSTOM_DOMAIN_REGISTRY.lock().unwrap();
    let registry = registry_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Custom domain registry not initialized"))?;

    let domains = registry.list_domains(profile_id);
    if domains.is_empty() {
        eprintln!("[Custom Domain] No custom domains registered for profile '{}'.", profile_id);
    } else {
        eprintln!("[Custom Domain] Registered domains for profile '{}':", profile_id);
        for entry in &domains {
            eprintln!(
                "  {} → {} (resolved {} times, created {})",
                entry.domain, entry.serving_url, entry.resolve_count, entry.created_at
            );
        }
    }
    Ok(())
}

/// Unregister a custom domain for a profile.
fn unregister_custom_domain(profile_id: &str, domain: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    init_custom_domain_registry(&profiles_dir)?;

    let registry_guard = engine::net::CUSTOM_DOMAIN_REGISTRY.lock().unwrap();
    let registry = registry_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Custom domain registry not initialized"))?;

    registry.unregister_domain(profile_id, domain)?;
    eprintln!("[Custom Domain] Unregistered {} (profile: {})", domain, profile_id);
    Ok(())
}

/// Show custom domain resolution statistics for a profile.
fn domain_stats(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    init_custom_domain_registry(&profiles_dir)?;

    let logs = engine::net::RESOLUTION_LOG.get_for_profile(profile_id);
    let total = logs.len();
    let success = logs.iter().filter(|l| l.success).count();
    let failed = total - success;
    let hf_hits = logs.iter().filter(|l| matches!(l.source, engine::net::ResolutionSource::HfBackend)).count();
    let local_hits = logs.iter().filter(|l| matches!(l.source, engine::net::ResolutionSource::LocalRegistry)).count();

    eprintln!("[Custom Domain] Stats for profile '{}':", profile_id);
    eprintln!("  Total resolutions: {}", total);
    eprintln!("  Successful: {}", success);
    eprintln!("  Failed: {}", failed);
    eprintln!("  Local registry hits: {}", local_hits);
    eprintln!("  HF backend hits: {}", hf_hits);
    eprintln!("  HF is only contacted for registered custom domains — not for .com/.net/.org");
    Ok(())
}

// ── Download Manager CLI handlers ──

fn download_list(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let records = download::list(&profile_dir);

    if records.is_empty() {
        eprintln!("[Download] No downloads for profile '{}'.", profile_id);
    } else {
        eprintln!("[Download] Downloads for profile '{}':", profile_id);
        for r in &records {
            eprintln!(
                "  {} | {} | {} bytes | {} | {}",
                r.id,
                r.url,
                r.downloaded_bytes,
                format!("{:?}", r.state),
                r.suggested_filename
            );
        }
    }
    Ok(())
}

fn download_stats(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let stats = download::stats(&profile_dir);

    eprintln!("[Download] Stats for profile '{}':", profile_id);
    eprintln!("  Total downloads: {}", stats.total_downloads);
    eprintln!("  Completed: {}", stats.completed_downloads);
    eprintln!("  Cancelled: {}", stats.cancelled_downloads);
    eprintln!("  Interrupted: {}", stats.interrupted_downloads);
    eprintln!("  Total bytes: {}", stats.total_bytes_downloaded);
    Ok(())
}

fn download_cancel(profile_id: &str, id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    download::update_record(
        &profile_dir,
        id,
        storage::download::DownloadState::Cancelled,
        0,
        Some("Cancelled by user"),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;
    eprintln!("[Download] Cancelled download {} (profile: {})", id, profile_id);
    Ok(())
}

// ── Password Manager CLI handlers ──

fn password_list(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let mgr = PasswordManager::new(profile_id, &profile_dir)?;
    let creds = mgr.list_credentials(profile_id);

    if creds.is_empty() {
        eprintln!("[Password] No saved passwords for profile '{}'.", profile_id);
    } else {
        eprintln!("[Password] Saved credentials for profile '{}':", profile_id);
        for c in &creds {
            eprintln!(
                "  {} | {} | {} (used {}x)",
                c.origin, c.username, c.id, c.use_count
            );
        }
    }
    Ok(())
}

fn password_gen(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let mgr = PasswordManager::new(profile_id, &profile_dir)?;
    let pwd = mgr.generate_password(&PasswordOptions::default());
    eprintln!("[Password] Generated: {}", pwd);
    Ok(())
}

fn password_health(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let mgr = PasswordManager::new(profile_id, &profile_dir)?;
    let report = mgr.health_check(profile_id);

    eprintln!("[Password] Health check for profile '{}':", profile_id);
    eprintln!("  Total passwords: {}", report.total_passwords);
    eprintln!("  Weak passwords: {}", report.weak_passwords);
    eprintln!("  Reused passwords: {}", report.reused_passwords);
    eprintln!("  Compromised passwords: {}", report.compromised_passwords);
    Ok(())
}

fn password_export(profile_id: &str) -> Result<()> {
    let profiles_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("mybrowser")
        .join("profiles");

    let profile_dir = profiles_dir.join(profile_id);
    let mgr = PasswordManager::new(profile_id, &profile_dir)?;
    let csv = mgr.export_csv(profile_id)?;
    eprintln!("{}", csv);
    Ok(())
}
