//! Settings panel — includes "Dummy System Profile" for peak privacy.
//! When enabled, the browser reports fake system info, GPU info, screen
//! layout, and hardware — completely different from the real machine.
//! This is the "privacy at peak" mode.

use egui::{Color32, Vec2, Align, Layout, Frame, Rounding, Stroke, CollapsingHeader, Slider, Checkbox, TextEdit};
use profile_manager::{ProfileManager, ProfileType};
use storage::{local_storage, history, bookmarks};
use std::collections::HashMap;

/// Settings state.
pub struct Settings {
    pub dummy_profile_enabled: bool,
    pub dummy_system_info: DummySystemInfo,
    pub show_advanced: bool,
    selected_profile: Option<String>,
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
    pub screen_layout: String, // e.g., "single", "dual", "ultrawide"
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
            dummy_profile_enabled: false,
            dummy_system_info: DummySystemInfo::default(),
            show_advanced: false,
            selected_profile: None,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, mgr: &ProfileManager) {
        ui.heading(egui::RichText::new("Settings").size(24.0).strong());
        ui.separator();

        // Profile selection.
        ui.group(|ui| {
            ui.label(egui::RichText::new("Active Profile").strong());
            let profiles: Vec<&profile_manager::Profile> = mgr.regular_profiles().collect();
            for profile in profiles {
                let selected = self.selected_profile.as_deref() == Some(&profile.id);
                if ui.selectable_label(selected, &profile.name).clicked() {
                    self.selected_profile = Some(profile.id.clone());
                }
            }
        });

        ui.separator();

        // Dummy profile toggle (privacy peak).
        CollapsingHeader::new("🔒 Privacy Peak — Dummy System Profile")
            .default_open(self.dummy_profile_enabled)
            .show(ui, |ui| {
                ui.label("When enabled, MyBrowser reports fake system information to websites,");
                ui.label("making your browser appear as a completely different machine.");
                ui.separator();

                ui.checkbox(&mut self.dummy_profile_enabled, "Enable Dummy System Profile");

                if self.dummy_profile_enabled {
                    ui.separator();
                    ui.label(egui::RichText::new("Fake System Configuration").strong());

                    ui.horizontal(|ui| {
                        ui.label("OS:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.os);
                    });
                    ui.horizontal(|ui| {
                        ui.label("OS Version:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.os_version);
                    });
                    ui.horizontal(|ui| {
                        ui.label("CPU:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.cpu);
                    });
                    ui.horizontal(|ui| {
                        ui.label("CPU Cores:");
                        ui.add(Slider::new(&mut self.dummy_system_info.cpu_cores, 1..=64));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Memory (GB):");
                        ui.add(Slider::new(&mut self.dummy_system_info.memory_gb, 1.0..=128.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("GPU:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.gpu);
                    });
                    ui.horizontal(|ui| {
                        ui.label("GPU Vendor:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.gpu_vendor);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Screen Width:");
                        ui.add(Slider::new(&mut self.dummy_system_info.screen_width, 640..=7680));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Screen Height:");
                        ui.add(Slider::new(&mut self.dummy_system_info.screen_height, 480..=4320));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Screen Layout:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.screen_layout);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Timezone:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.timezone);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Locale:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.locale);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Device Pixel Ratio:");
                        ui.add(Slider::new(&mut self.dummy_system_info.device_pixel_ratio, 1.0..=5.0));
                    });
                    ui.checkbox(&mut self.dummy_system_info.touch_support, "Touch Support");
                    ui.checkbox(&mut self.dummy_system_info.hardware_acceleration, "Hardware Acceleration");
                    ui.horizontal(|ui| {
                        ui.label("WebGL Vendor:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.webgl_vendor);
                    });
                    ui.horizontal(|ui| {
                        ui.label("WebGL Renderer:");
                        ui.text_edit_singleline(&mut self.dummy_system_info.webgl_renderer);
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("⚠️ All fingerprint values will match this dummy configuration.")
                        .small()
                        .color(Color32::from_rgb(255, 165, 0)));
                }
            });

        ui.separator();

        // Extensions settings.
        CollapsingHeader::new("🧩 Extensions")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Extensions run in a sandboxed process per profile.");
                ui.label("- Content scripts isolated from page JavaScript (DOM Proxy)");
                ui.label("- No eval() or nativeMessaging allowed");
                ui.label("- Message senders validated by extension ID");
                ui.label("- Per-profile extension storage");
                ui.separator();

                if ui.button("Manage Extensions").clicked() {
                    // In production, open extension management UI.
                }
            });

        ui.separator();

        // Proxy settings.
        CollapsingHeader::new("🌐 Per-Profile Proxy")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Route each profile's traffic through its own proxy for IP isolation.");
                ui.separator();

                // This would integrate with engine::net::proxy::ProxyManager.
                ui.label("Proxy settings are configured per profile in the profile manager.");
            });

        ui.separator();

        // Advanced.
        CollapsingHeader::new("🔧 Advanced")
            .default_open(self.show_advanced)
            .show(ui, |ui| {
                ui.checkbox(&mut self.show_advanced, "Show Advanced Options");

                if self.show_advanced {
                    ui.label("Engine: Falco (from-scratch Rust browser engine)");
                    ui.label("GPU: Direct Vulkan (ash)");
                    ui.label("Extensions: Manifest V3 (sandboxed)");
                    ui.label("Proxy: SOCKS5 / HTTP per profile");
                    ui.label("Fingerprint: Canvas, WebGL, Audio, Fonts, Screen");
                }
            });
    }

    /// Returns whether dummy profile mode is active.
    pub fn is_dummy_mode(&self) -> bool {
        self.dummy_profile_enabled
    }

    /// Get the dummy system info to apply to fingerprint spoofing.
    pub fn dummy_system_info(&self) -> &DummySystemInfo {
        &self.dummy_system_info
    }
}
