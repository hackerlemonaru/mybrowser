//! Dummy System Profile UI — egui-based device profile selector.
//! Users choose from 48+ real device profiles (GPU, CPU, screen, etc.)
//! rather than writing their own values, ensuring fingerprint coherence.

use crate::dummy_profile::{DeviceProfile, DummySystemConfig, device_catalog};
use egui::{
    Align2, Color32, Frame, Rounding, Stroke, Vec2, Align, Layout, RichText, Checkbox, Slider,
};

/// Result of dummy profile UI interaction.
#[derive(Debug, Clone)]
pub enum DummyProfileUIAction {
    None,
    EnableDummyMode,
    DisableDummyMode,
    SelectDevice(DeviceProfile),
    ApplyDummyConfig(DummySystemConfig),
}

/// Dummy profile selector widget.
pub struct DummyProfileSelector {
    /// Whether the device selection dialog is open.
    device_dialog_open: bool,
    /// Search/filter for device profiles.
    device_search: String,
    /// Currently selected device (in dialog).
    selected_device: Option<DeviceProfile>,
    /// Scroll position.
    scroll_offset: f32,
}

impl Default for DummyProfileSelector {
    fn default() -> Self {
        Self {
            device_dialog_open: false,
            device_search: String::new(),
            selected_device: None,
            scroll_offset: 0.0,
        }
    }
}

impl DummyProfileSelector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the dummy profile UI. Returns any actions to take.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        config: &DummySystemConfig,
    ) -> DummyProfileUIAction {
        let mut action = DummyProfileUIAction::None;

        ui.group(|ui| {
            ui.label(egui::RichText::new("🔒 Dummy System Profile").strong());

            ui.horizontal(|ui| {
                let enabled = config.enabled;
                let mut toggle = enabled;
                if ui.checkbox(&mut toggle, "Enable Dummy System Profile").clicked() {
                    if toggle {
                        action = DummyProfileUIAction::EnableDummyMode;
                    } else {
                        action = DummyProfileUIAction::DisableDummyMode;
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Select Device…").clicked() {
                        self.device_dialog_open = true;
                        self.selected_device = config.device.clone();
                    }
                });
            });

            if let Some(ref device) = config.device {
                ui.separator();
                ui.label(egui::RichText::new("Active Device Profile:").small().strong());
                ui.label(format!("  {}", device.name));
                ui.label(format!(
                    "  OS: {} {} | CPU: {} | GPU: {} {}",
                    device.os, device.os_version, device.cpu_brand, device.gpu_vendor, device.gpu_renderer
                ));
                ui.label(format!(
                    "  Screen: {}×{} @ {}x | RAM: {}GB | Cores: {}",
                    device.screen_width, device.screen_height,
                    device.device_pixel_ratio, device.device_memory_gb, device.hardware_concurrency
                ));
                ui.label(format!(
                    "  Timezone: {} | Locale: {} | Touch: {}",
                    device.timezone, device.locale, device.touch_support
                ));

                ui.label(
                    egui::RichText::new("⚠️ All fingerprint values (User-Agent, Client Hints, WebGL, Canvas, Audio, Screen) will match this device.")
                        .small()
                        .color(Color32::from_rgb(255, 165, 0)),
                );
            } else if config.enabled {
                ui.label(
                    egui::RichText::new("No device selected. Click 'Select Device…' to choose one.")
                        .small()
                        .color(Color32::from_rgb(255, 165, 0)),
                );
            }
        });

        // ── Device selection dialog ──
        if self.device_dialog_open {
            self.render_device_dialog(ui.ctx(), &mut action);
        }

        action
    }

    /// Render the device selection dialog.
    fn render_device_dialog(
        &mut self,
        ctx: &egui::Context,
        action: &mut DummyProfileUIAction,
    ) {
        egui::Window::new("Select Device Profile")
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(700.0, 500.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(600.0);
                ui.label("Choose a real device profile. All fingerprint values will be coherent:");
                ui.separator();

                // Search box.
                ui.label("Search (name, OS, GPU, CPU):");
                ui.text_edit_singleline(&mut self.device_search);
                ui.separator();

                // Device list.
                let catalog = device_catalog();
                let filtered: Vec<&DeviceProfile> = if self.device_search.is_empty() {
                    catalog.iter().collect()
                } else {
                    let q = self.device_search.to_lowercase();
                    catalog
                        .iter()
                        .filter(|d| {
                            d.name.to_lowercase().contains(&q)
                                || d.os.to_lowercase().contains(&q)
                                || d.gpu_vendor.to_lowercase().contains(&q)
                                || d.gpu_renderer.to_lowercase().contains(&q)
                                || d.cpu_brand.to_lowercase().contains(&q)
                        })
                        .collect()
                };

                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show_rows(ui, 24.0, filtered.len(), |ui, row_range| {
                        for idx in row_range {
                            let device = filtered[idx];
                            let is_selected = self
                                .selected_device
                                .as_ref()
                                .map_or(false, |sd| sd.id == device.id);

                            Frame::none()
                                .fill(if is_selected {
                                    Color32::from_rgb(66, 133, 244).linear_multiply(0.15)
                                } else {
                                    Color32::from_gray(30)
                                })
                                .rounding(Rounding::same(6.0))
                                .stroke(Stroke::new(
                                    1.0,
                                    if is_selected {
                                        Color32::from_rgb(66, 133, 244)
                                    } else {
                                        Color32::from_gray(50)
                                    },
                                ))
                                .show(ui, |ui| {
                                    ui.set_min_height(60.0);
                                    ui.horizontal(|ui| {
                                        // Radio selection.
                                        if ui.radio(is_selected, "").clicked() {
                                            self.selected_device = Some(device.clone());
                                        }

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&device.name)
                                                    .strong()
                                                    .size(14.0),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} {} · {} · {} · {}×{}",
                                                    device.os, device.os_version,
                                                    device.cpu_brand,
                                                    device.gpu_vendor,
                                                    device.screen_width, device.screen_height
                                                ))
                                                .small()
                                                .color(Color32::from_gray(150)),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "GPU: {} | RAM: {}GB | Cores: {} | Pixel Ratio: {}x",
                                                    device.gpu_renderer,
                                                    device.device_memory_gb,
                                                    device.hardware_concurrency,
                                                    device.device_pixel_ratio
                                                ))
                                                .small()
                                                .color(Color32::from_gray(150)),
                                            );
                                        });

                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(
                                                        &device.locale,
                                                    )
                                                    .small()
                                                    .color(Color32::from_gray(120)),
                                                );
                                            },
                                        );
                                    });
                                });
                        }
                    });

                ui.separator();

                // Show selected device summary.
                if let Some(ref device) = self.selected_device {
                    ui.label(egui::RichText::new("Selected:").strong());
                    ui.label(format!("  {}", device.name));
                    ui.label(format!(
                        "  WebGL: {} / {}",
                        device.gpu_vendor, device.gpu_renderer
                    ));
                    ui.label(format!(
                        "  UA: {}",
                        device.user_agent.chars().take(60).collect::<String>()
                    ));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.device_dialog_open = false;
                        self.selected_device = None;
                    }
                    if ui.button("Apply Device").clicked() {
                        if let Some(device) = self.selected_device.clone() {
                            *action = DummyProfileUIAction::SelectDevice(device.clone());
                            *action = DummyProfileUIAction::ApplyDummyConfig(DummySystemConfig {
                                enabled: true,
                                device: Some(device),
                            });
                        }
                        self.device_dialog_open = false;
                        self.selected_device = None;
                    }
                });
            });
    }
}

/// Compact dummy profile status indicator for the top bar.
pub fn dummy_status_indicator(ui: &mut egui::Ui, config: &DummySystemConfig) {
    if config.enabled {
        if let Some(ref device) = config.device {
            ui.label(
                RichText::new(format!("🎭 Dummy: {}", device.name))
                    .small()
                    .color(Color32::from_rgb(255, 165, 0)),
            );
        } else {
            ui.label(
                RichText::new("🎭 Dummy: Enabled")
                    .small()
                    .color(Color32::from_rgb(255, 165, 0)),
            );
        }
    }
}
