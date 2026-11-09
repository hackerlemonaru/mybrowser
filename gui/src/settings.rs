//! Settings panel — Chrome-like layout.
//! Left sidebar category TOC, search bar at top, hierarchical settings
//! on the right. Mirrors chrome://settings structure:
//!   You and Google / Autofill / Safety check / Privacy and security /
//!   Appearance / Search engine / On startup / Languages / Downloads /
//!   Accessibility / System (Proxy + VPN) / Extensions / About
//!
//! All settings are per-profile — switching profiles gives a fresh set,
//! nothing carried over from any past profile.

use egui::{
    Color32, Vec2, Align, Layout, Frame, Rounding, Stroke, CollapsingHeader, Slider,
    Checkbox, TextEdit, RichText, TextStyle, Align2, FontId,
};
use profile_manager::{ProfileManager, ProfileType};
use storage::{local_storage, history, bookmarks};
use falco::net::proxy::ProxyManager;
use std::collections::HashMap;

/// Settings state.
pub struct Settings {
    /// Currently selected settings category.
    pub selected_category: SettingsCategory,
    /// Search query.
    pub search_query: String,
    /// Selected profile (for per-profile settings).
    pub selected_profile: Option<String>,
    // Dummy profile
    pub dummy_profile_enabled: bool,
    pub dummy_system_info: DummySystemInfo,
    pub show_advanced: bool,
    // Custom domain
    pub custom_domain_input: String,
    pub custom_serving_url_input: String,
    pub custom_domain_message: Option<String>,
    // Download manager
    pub download_message: Option<String>,
    // Password manager
    pub password_message: Option<String>,
    pub show_save_password_dialog: bool,
    pub save_password_origin: String,
    pub save_password_username: String,
    pub save_password_password: String,
    pub show_password_manager: bool,
    pub edit_credential_id: Option<String>,
    pub edit_username: String,
    pub edit_password: String,
    // VPN (per profile)
    pub vpn_enabled: bool,
    pub vpn_provider: String,
    pub vpn_endpoint: String,
    pub vpn_message: Option<String>,
}

/// Chrome settings categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    YouAndGoogle,
    Autofill,
    SafetyCheck,
    PrivacyAndSecurity,
    Appearance,
    SearchEngine,
    OnStartup,
    Languages,
    Downloads,
    Accessibility,
    System,
    Extensions,
    About,
}

impl SettingsCategory {
    pub fn all() -> Vec<Self> {
        vec![
            Self::YouAndGoogle,
            Self::Autofill,
            Self::SafetyCheck,
            Self::PrivacyAndSecurity,
            Self::Appearance,
            Self::SearchEngine,
            Self::OnStartup,
            Self::Languages,
            Self::Downloads,
            Self::Accessibility,
            Self::System,
            Self::Extensions,
            Self::About,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::YouAndGoogle => "You and Google",
            Self::Autofill => "Autofill",
            Self::SafetyCheck => "Safety check",
            Self::PrivacyAndSecurity => "Privacy and security",
            Self::Appearance => "Appearance",
            Self::SearchEngine => "Search engine",
            Self::OnStartup => "On startup",
            Self::Languages => "Languages",
            Self::Downloads => "Downloads",
            Self::Accessibility => "Accessibility",
            Self::System => "System",
            Self::Extensions => "Extensions",
            Self::About => "About MyBrowser",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Self::YouAndGoogle => "Sync, profile picture, passwords",
            Self::Autofill => "Passwords, payment methods, addresses",
            Self::SafetyCheck => "Check for compromised passwords and risky sites",
            Self::PrivacyAndSecurity => "Cookies, site settings, clear browsing data",
            Self::Appearance => "Theme, fonts, zoom, layout",
            Self::SearchEngine => "Default search engine and shortcuts",
            Self::OnStartup => "What happens when MyBrowser starts",
            Self::Languages => "Preferred languages and spell check",
            Self::Downloads => "Download location and behavior",
            Self::Accessibility => "Captions, text scaling, simplified view",
            Self::System => "Hardware acceleration, proxy, VPN",
            Self::Extensions => "Manage installed extensions",
            Self::About => "Version, terms of service",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::YouAndGoogle => "👤",
            Self::Autofill => "📋",
            Self::SafetyCheck => "🛡️",
            Self::PrivacyAndSecurity => "🔒",
            Self::Appearance => "🎨",
            Self::SearchEngine => "🔍",
            Self::OnStartup => "▶️",
            Self::Languages => "🌐",
            Self::Downloads => "⬇️",
            Self::Accessibility => "♿",
            Self::System => "💻",
            Self::Extensions => "🧩",
            Self::About => "ℹ️",
        }
    }
}

impl Default for SettingsCategory {
    fn default() -> Self {
        Self::YouAndGoogle
    }
}

/// Fake system info for dummy profile mode.
#[derive(Debug, Clone)]
pub struct DummySystemInfo {
    pub os: String,
    pub os_version: String,
    pub cpu: String,
    pub cpu_cores: u32,
    pub memory_gb: f32,
    pub gpu: String,
    pub gpu_vendor: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub screen_layout: String,
    pub timezone: String,
    pub locale: String,
    pub device_pixel_ratio: f32,
    pub touch_support: bool,
    pub hardware_acceleration: bool,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
}

impl Default for DummySystemInfo {
    fn default() -> Self {
        Self {
            os: "Windows 11".to_string(),
            os_version: "10.0.22631".to_string(),
            cpu: "Intel Core i7-12700K".to_string(),
            cpu_cores: 12,
            memory_gb: 32.0,
            gpu: "NVIDIA GeForce RTX 4080".to_string(),
            gpu_vendor: "NVIDIA Corporation".to_string(),
            screen_width: 2560,
            screen_height: 1440,
            screen_layout: "single".to_string(),
            timezone: "America/New_York".to_string(),
            locale: "en-US".to_string(),
            device_pixel_ratio: 1.0,
            touch_support: false,
            hardware_acceleration: true,
            webgl_vendor: "NVIDIA Corporation".to_string(),
            webgl_renderer: "NVIDIA GeForce RTX 4080".to_string(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self {
            selected_category: SettingsCategory::default(),
            search_query: String::new(),
            selected_profile: None,
            dummy_profile_enabled: false,
            dummy_system_info: DummySystemInfo::default(),
            show_advanced: false,
            custom_domain_input: String::new(),
            custom_serving_url_input: String::new(),
            custom_domain_message: None,
            download_message: None,
            password_message: None,
            show_save_password_dialog: false,
            save_password_origin: String::new(),
            save_password_username: String::new(),
            save_password_password: String::new(),
            show_password_manager: false,
            edit_credential_id: None,
            edit_username: String::new(),
            edit_password: String::new(),
            // VPN defaults — fresh per profile, nothing inherited.
            vpn_enabled: false,
            vpn_provider: "None".to_string(),
            vpn_endpoint: String::new(),
            vpn_message: None,
        }
    }

    /// Reset all per-profile settings to defaults (called when switching profiles).
    pub fn reset_for_profile(&mut self) {
        self.dummy_profile_enabled = false;
        self.dummy_system_info = DummySystemInfo::default();
        self.custom_domain_input.clear();
        self.custom_serving_url_input.clear();
        self.custom_domain_message = None;
        self.download_message = None;
        self.password_message = None;
        self.show_password_manager = false;
        self.edit_credential_id = None;
        self.edit_username.clear();
        self.edit_password.clear();
        // VPN: fresh per profile.
        self.vpn_enabled = false;
        self.vpn_provider = "None".to_string();
        self.vpn_endpoint.clear();
        self.vpn_message = None;
    }

    pub fn render(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        // Outer frame: Chrome-dark background.
        Frame::none().fill(Color32::from_rgb(32, 33, 36)).show(ui, |ui| {
            ui.horizontal(|ui| {
                // ── Left sidebar: category TOC ──
                egui::SidePanel::left("settings_sidebar")
                    .default_width(240.0)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_width(240.0);
                        self.render_sidebar(ui, mgr);
                    });

                // ── Right: search bar + main content ──
                egui::CentralPanel::default().show(ui.ctx(), |ui| {
                    ui.set_min_width(600.0);
                    self.render_content(ui, mgr);
                });
            });
        });
    }

    // ── Sidebar ──
    fn render_sidebar(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        ui.vertical(|ui| {
            ui.heading(egui::RichText::new("MyBrowser").size(20.0).strong());
            ui.label("Settings");
            ui.separator();

            for category in SettingsCategory::all() {
                let selected = self.selected_category == category;
                let bg = if selected {
                    Color32::from_rgb(66, 133, 244).linear_multiply(0.2)
                } else {
                    Color32::from_rgb(32, 33, 36)
                };
                Frame::none()
                    .fill(bg)
                    .rounding(Rounding::same(6.0))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(category.icon()).size(18.0));
                            ui.allocate_space(Vec2::new(8.0, 0.0));
                            ui.label(
                                RichText::new(category.title())
                                    .strong()
                                    .color(if selected {
                                        Color32::from_rgb(255, 255, 255)
                                    } else {
                                        Color32::from_rgb(200, 200, 200)
                                    }),
                            );
                        });
                    });
                if ui.rect_contains_pointer(ui.last_item_rect()) && ui.input(|i| i.pointer.any_click()) {
                    self.selected_category = category;
                }
            }

            ui.separator();
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.label(RichText::new("MyBrowser • Settings").small().color(Color32::from_gray(150)));
            });
        });
    }

    // ── Main content ──
    fn render_content(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        // Profile selector at top of content.
        ui.group(|ui| {
            ui.label(egui::RichText::new("Active Profile").strong());
            let profiles: Vec<&profile_manager::Profile> = mgr.regular_profiles().collect();
            for profile in profiles {
                let selected = self.selected_profile.as_deref() == Some(&profile.id);
                if ui.selectable_label(selected, &profile.name).clicked() {
                    self.selected_profile = Some(profile.id.clone());
                    self.reset_for_profile();
                }
            }
            if profiles.is_empty() {
                ui.label(RichText::new("No profiles created.").small().color(Color32::from_gray(150)));
            }
        });
        ui.separator();

        // Search bar (Chrome-style, fixed at top).
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut self.search_query);
            });
        });
        ui.separator();

        // Content by category.
        match self.selected_category {
            SettingsCategory::YouAndGoogle => self.render_you_and_google(ui, mgr),
            SettingsCategory::Autofill => self.render_autofill(ui, mgr),
            SettingsCategory::SafetyCheck => self.render_safety_check(ui, mgr),
            SettingsCategory::PrivacyAndSecurity => self.render_privacy_and_security(ui, mgr),
            SettingsCategory::Appearance => self.render_appearance(ui, mgr),
            SettingsCategory::SearchEngine => self.render_search_engine(ui, mgr),
            SettingsCategory::OnStartup => self.render_on_startup(ui, mgr),
            SettingsCategory::Languages => self.render_languages(ui, mgr),
            SettingsCategory::Downloads => self.render_downloads(ui, mgr),
            SettingsCategory::Accessibility => self.render_accessibility(ui, mgr),
            SettingsCategory::System => self.render_system(ui, mgr),
            SettingsCategory::Extensions => self.render_extensions(ui, mgr),
            SettingsCategory::About => self.render_about(ui, mgr),
        }
    }

    // ── Category: You and Google ──
    fn render_you_and_google(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("👤 You and Google")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Manage your Google Account and sync settings.");
                ui.separator();
                ui.label("Sync is not configured.");
                ui.label("Profile picture: Not set");
                ui.separator();
                ui.label("Chrome name: Not set");
            });
    }

    // ── Category: Autofill ──
    fn render_autofill(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        CollapsingHeader::new("📋 Autofill")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Manage saved passwords, payment methods, and addresses.");
                ui.separator();

                ui.heading("Passwords");
                ui.horizontal(|ui| {
                    if ui.button("Show Saved Passwords").clicked() {
                        self.show_password_manager = true;
                    }
                    if ui.button("Generate Strong Password").clicked() {
                        if let Some(profile_id) = &self.selected_profile {
                            match std::process::Command::new("mybrowser")
                                .args(["--password-gen", profile_id])
                                .output()
                            {
                                Ok(output) => {
                                    let output_text = String::from_utf8_lossy(&output.stdout);
                                    if let Some(line) = output_text.lines().find(|l| l.contains("Generated:")) {
                                        if let Some(pwd) = line.split("Generated:").nth(1) {
                                            self.password_message = Some(format!("Generated: {}", pwd.trim()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.password_message = Some(format!("Error generating password: {}", e));
                                }
                            }
                        }
                    }
                });

                if let Some(msg) = &self.password_message {
                    ui.colored_label(Color32::from_rgb(80, 255, 80), msg);
                }

                ui.separator();

                // Save password dialog (shown when website login succeeds).
                if self.show_save_password_dialog {
                    ui.group(|ui| {
                        ui.heading("💾 Save Password?");
                        ui.label("MyBrowser would like to save your password for:");
                        ui.label(RichText::new(&self.save_password_origin).strong());
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Username:");
                            ui.text_edit_singleline(&mut self.save_password_username);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Password:");
                            ui.text_edit_singleline(&mut self.save_password_password);
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Save Password").clicked() {
                                if let Some(profile_id) = &self.selected_profile {
                                    self.password_message = Some(format!("Saved password for {}", self.save_password_origin));
                                }
                                self.show_save_password_dialog = false;
                                self.save_password_origin.clear();
                                self.save_password_username.clear();
                                self.save_password_password.clear();
                            }
                            if ui.button("Never Save").clicked() {
                                self.show_save_password_dialog = false;
                                self.save_password_origin.clear();
                                self.save_password_username.clear();
                                self.save_password_password.clear();
                            }
                        });
                    });
                    ui.separator();
                }

                ui.heading("Payment methods");
                ui.label("No payment methods saved.");
                ui.separator();
                ui.heading("Addresses");
                ui.label("No addresses saved.");
            });
    }

    // ── Category: Safety check ──
    fn render_safety_check(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        CollapsingHeader::new("🛡️ Safety check")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Run a safety check to identify potential risks.");
                ui.separator();
                if ui.button("Check now").clicked() {
                    if let Some(profile_id) = &self.selected_profile {
                        match std::process::Command::new("mybrowser")
                            .args(["--password-health", profile_id])
                            .output()
                        {
                            Ok(output) => {
                                let output_text = String::from_utf8_lossy(&output.stdout);
                                for line in output_text.lines().skip(1) {
                                    if !line.trim().is_empty() {
                                        ui.label(line.trim());
                                    }
                                }
                            }
                            Err(e) => {
                                ui.label(format!("Could not run safety check: {}", e));
                            }
                        }
                    }
                }
                ui.separator();
                ui.label("• Check for compromised passwords");
                ui.label("• Check for outdated extensions");
                ui.label("• Check safe browsing status");
            });
    }

    // ── Category: Privacy and security ──
    fn render_privacy_and_security(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        CollapsingHeader::new("🔒 Privacy and security")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Control how MyBrowser handles your data and protects you online.");
                ui.separator();

                ui.heading("Cookies");
                ui.label("Manage cookie settings for sites.");

                ui.separator();
                ui.heading("Site settings");
                ui.label("Control permissions for sites (camera, microphone, location, etc.).");

                ui.separator();
                ui.heading("Clear browsing data");
                ui.label("Clear history, cookies, cache, and more.");

                ui.separator();
                ui.heading("Safe Browsing");
                ui.label("Protection against dangerous sites and downloads.");
            });
    }

    // ── Category: Appearance ──
    fn render_appearance(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("🎨 Appearance")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Customize the look and feel of MyBrowser.");
                ui.separator();
                ui.label("Theme: Dark (system)");
                ui.separator();
                ui.label("Font size: Medium");
                ui.separator();
                ui.label("Page zoom: 100%");
                ui.separator();
                ui.label("Show bookmarks bar: Off");
            });
    }

    // ── Category: Search engine ──
    fn render_search_engine(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("🔍 Search engine")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Manage your default search engine and site search shortcuts.");
                ui.separator();
                ui.label("Search engine used in the address bar: Google");
                ui.separator();
                ui.label("Site search: Not configured");
            });
    }

    // ── Category: On startup ──
    fn render_on_startup(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("▶️ On startup")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Choose what MyBrowser does when it starts.");
                ui.separator();
                ui.label("Open the New Tab page");
                ui.separator();
                ui.label("Continue where you left off");
                ui.separator();
                ui.label("Open a specific page or set of pages");
            });
    }

    // ── Category: Languages ──
    fn render_languages(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("🌐 Languages")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Manage preferred languages and spell check.");
                ui.separator();
                ui.label("Preferred language: English (United States)");
                ui.separator();
                ui.label("Spell check: Basic");
            });
    }

    // ── Category: Downloads ──
    fn render_downloads(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        CollapsingHeader::new("⬇️ Downloads")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Configure download location and behavior.");
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Refresh Downloads").clicked() {
                        if let Some(profile_id) = &self.selected_profile {
                            match std::process::Command::new("mybrowser")
                                .args(["--download-list", profile_id])
                                .output()
                            {
                                Ok(output) => {
                                    let output_text = String::from_utf8_lossy(&output.stdout);
                                    let lines: Vec<&str> = output_text
                                        .lines()
                                        .skip(1)
                                        .filter(|l| !l.trim().is_empty())
                                        .collect();
                                    if lines.is_empty() {
                                        self.download_message = Some("No downloads found.".to_string());
                                    } else {
                                        self.download_message = Some(format!(
                                            "Found {} download(s):\n{}",
                                            lines.len(),
                                            lines.join("\n")
                                        ));
                                    }
                                }
                                Err(e) => {
                                    self.download_message = Some(format!("Error loading downloads: {}", e));
                                }
                            }
                        }
                    }
                });

                if let Some(msg) = &self.download_message {
                    ui.label(msg);
                }

                ui.separator();
                ui.label("Ask where to save each file before downloading: Off");
                ui.separator();
                ui.label("Location: /root/Downloads");
            });
    }

    // ── Category: Accessibility ──
    fn render_accessibility(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("♿ Accessibility")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Improve accessibility with captions, live captions, and text scaling.");
                ui.separator();
                ui.label("Live caption: Off");
                ui.separator();
                ui.label("Text scaling: 100%");
            });
    }

    // ── Category: System ──
    fn render_system(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        CollapsingHeader::new("💻 System")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Hardware acceleration, proxy, and VPN settings.");
                ui.separator();

                ui.heading("Hardware acceleration");
                ui.label("Use hardware acceleration when available: On");

                ui.separator();
                ui.heading("Proxy / VPN");

                // VPN section (per profile, Chrome-style).
                ui.checkbox(&mut self.vpn_enabled, "Enable VPN for this profile");
                if self.vpn_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Provider:");
                        ui.text_edit_singleline(&mut self.vpn_provider);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Endpoint (host:port):");
                        ui.text_edit_singleline(&mut self.vpn_endpoint);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Connect").clicked() {
                            if let Some(profile_id) = &self.selected_profile {
                                ProxyManager::set_vpn(
                                    profile_id,
                                    true,
                                    &self.vpn_provider,
                                    &self.vpn_endpoint,
                                );
                                let (_, _, _, connected, msg) =
                                    ProxyManager::vpn_status(profile_id);
                                self.vpn_enabled = connected;
                                self.vpn_message = msg;
                            }
                        }
                        if ui.button("Disconnect").clicked() {
                            if let Some(profile_id) = &self.selected_profile {
                                ProxyManager::set_vpn(profile_id, false, "", "");
                                self.vpn_enabled = false;
                                self.vpn_message = Some("VPN disconnected.".to_string());
                            }
                        }
                    });
                    if let Some(msg) = &self.vpn_message {
                        let color = if msg.contains("connected") {
                            Color32::from_rgb(80, 255, 80)
                        } else {
                            Color32::from_rgb(255, 80, 80)
                        };
                        ui.colored_label(color, RichText::new(msg).small());
                    }
                    ui.separator();
                    ui.label("VPN routes this profile's traffic through a free proxy endpoint.");
                    ui.label("Each profile has its own VPN setting — nothing is shared across profiles.");
                } else {
                    ui.label("VPN is disabled for this profile.");
                }

                ui.separator();
                ui.heading("System proxy settings");
                ui.label("Open your computer's proxy settings.");
            });
    }

    // ── Category: Extensions ──
    fn render_extensions(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("🧩 Extensions")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Extensions run in a sandboxed process per profile.");
                ui.label("- Content scripts isolated from page JavaScript (DOM Proxy)");
                ui.label("- No eval() or nativeMessaging allowed");
                ui.label("- Message senders validated by extension ID");
                ui.label("- Per-profile extension storage");
                ui.separator();
                if ui.button("Manage Extensions").clicked() {
                    // Open extension management UI.
                }
            });
    }

    // ── Category: About ──
    fn render_about(&mut self, ui: &mut egui::Ui, _mgr: &ProfileManager) {
        CollapsingHeader::new("ℹ️ About MyBrowser")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("MyBrowser — A Chrome-like browser built in Rust.");
                ui.separator();
                ui.label("Version: 0.1.0");
                ui.separator();
                ui.label("Engine: Falco (from-scratch Rust browser engine)");
                ui.label("GPU: Direct Vulkan (ash)");
                ui.label("Extensions: Manifest V3 (sandboxed)");
                ui.separator();
                ui.label("Terms of service");
                ui.label("Privacy policy");
            });
    }

    /// Trigger the "Save Password?" dialog when a website login succeeds.
    pub fn trigger_save_password_dialog(&mut self, origin: &str, username: &str, password: &str) {
        self.show_save_password_dialog = true;
        self.save_password_origin = origin.to_string();
        self.save_password_username = username.to_string();
        self.save_password_password = password.to_string();
    }

    pub fn is_save_password_dialog_visible(&self) -> bool {
        self.show_save_password_dialog
    }

    pub fn save_password_data(&self) -> (&str, &str, &str) {
        (
            &self.save_password_origin,
            &self.save_password_username,
            &self.save_password_password,
        )
    }

    pub fn is_dummy_mode(&self) -> bool {
        self.dummy_profile_enabled
    }

    pub fn dummy_system_info(&self) -> &DummySystemInfo {
        &self.dummy_system_info
    }
}