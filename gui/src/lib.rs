//! Chrome-like profile switcher GUI.
//! Shows profile cards, "Add New", "Browse as Guest", and lets the user
//! select a profile to launch the browser with.
//!
//! Built with egui (immediate-mode GUI) + winit — pure Rust, no Electron,
//! no Tauri, no system webview. Just a window and GPU drawing.

use egui::{Color32, Vec2, Align2, Align, Layout, Frame, Rounding, Stroke};
use profile_manager::{ProfileManager, Profile, ProfileType};
use std::sync::mpsc;
use std::time::Instant;

/// Message sent from the GUI to the launcher when the user makes a choice.
#[derive(Debug, Clone)]
pub enum GuiChoice {
    /// Launch the browser with this profile ID.
    LaunchProfile(String),
    /// Create a new regular profile, then launch it.
    CreateProfile,
    /// Launch as guest (temporary profile).
    LaunchGuest,
    /// Quit the application.
    Quit,
}

/// Result of running the profile switcher GUI.
pub struct SwitcherResult {
    pub choice: GuiChoice,
    /// If the user created a new profile, this is the suggested name.
    pub new_profile_name: Option<String>,
}

pub struct ProfileSwitcher {
    mgr: ProfileManager,
    new_profile_name: String,
    show_add_dialog: bool,
    hovered_profile: Option<String>,
    result: Option<GuiChoice>,
}

impl ProfileSwitcher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mgr = ProfileManager::new()?;
        Ok(Self {
            mgr,
            new_profile_name: String::new(),
            show_add_dialog: false,
            hovered_profile: None,
            result: None,
        })
    }

    pub fn run_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_min_size(Vec2::new(600.0, 500.0));
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                // Header
                ui.heading(egui::RichText::new("MyBrowser").size(32.0).strong());
                ui.label("Choose a profile to continue");
                ui.separator();

                // Profile grid
                egui::Grid::new("profile_grid")
                    .min_col_width(170.0)
                    .min_row_height(170.0)
                    .spacing(Vec2::splat(24.0))
                    .show(ui, |ui| {
                        let profiles: Vec<Profile> = self.mgr.regular_profiles().into_iter().cloned().collect();
                        let cols = 3;

                        for (i, profile) in profiles.iter().enumerate() {
                            if i > 0 && i % cols == 0 {
                                ui.end_row();
                            }
                            self.render_profile_card(ui, profile);
                        }

                        // Empty cell for "Add New" if grid alignment needed
                        while profiles.len() % cols != 0 {
                            ui.end_row();
                            // add empty cells
                            let _ = profiles.len(); // avoid unused warning
                            break;
                        }

                        // Add New card
                        self.render_add_card(ui);

                        // Guest card
                        self.render_guest_card(ui);
                    });

                ui.separator();

                // Footer buttons
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        self.result = Some(GuiChoice::Quit);
                    }
                });
            });
        });

        // Handle add profile dialog
        if self.show_add_dialog {
            egui::Window::new("Add New Profile")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Profile name:");
                    ui.text_edit_singleline(&mut self.new_profile_name);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_add_dialog = false;
                            self.new_profile_name.clear();
                        }
                        if ui.button("Create & Launch").clicked() {
                            if !self.new_profile_name.trim().is_empty() {
                                self.result = Some(GuiChoice::CreateProfile);
                            }
                        }
                    });
                });
        }
    }

    fn render_profile_card(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let theme_color = profile
            .theme_color
            .as_deref()
            .and_then(|c| parse_hex_color(c))
            .unwrap_or(Color32::from_rgb(66, 133, 244));

        let is_hovered = self.hovered_profile.as_deref() == Some(&profile.id);

        let card = Frame::none()
            .fill(if is_hovered { theme_color.linear_multiply(0.15) } else { Color32::from_gray(28) })
            .rounding(Rounding::same(12.0))
            .stroke(Stroke::new(1.0, theme_color))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(160.0, 160.0));
                ui.vertical_centered(|ui| {
                    // Avatar
                    let avatar_size = 64.0;
                    let avatar_rect = egui::Rect::from_center_size(
                        ui.cursor().center(),
                        Vec2::splat(avatar_size),
                    );
                    ui.painter().circle_filled(avatar_rect.center(), avatar_size / 2.0, theme_color);
                    ui.painter().text(
                        avatar_rect.center(),
                        Align2::CENTER_CENTER,
                        &profile.name.chars().next().unwrap_or('?').to_string(),
                        egui::FontId::proportional(26.0),
                        Color32::WHITE,
                    );
                    ui.allocate_space(Vec2::splat(avatar_size));

                    // Name
                    ui.label(egui::RichText::new(&profile.name).strong().size(16.0));

                    // Last used
                    if let Some(used) = &profile.last_used {
                        ui.label(
                            egui::RichText::new(format!("Last used: {}", &used[..10]))
                                .small()
                                .color(Color32::from_gray(150)),
                        );
                    }

                    ui.allocate_space(Vec2::new(0.0, 8.0));

                    // Launch button
                    if ui.button("Launch").clicked() {
                        self.result = Some(GuiChoice::LaunchProfile(profile.id.clone()));
                    }
                });
            });

        // Detect hover
        let rect = card.response.rect;
        if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
            self.hovered_profile = Some(profile.id.clone());
        } else if self.hovered_profile.as_deref() == Some(&profile.id) {
            self.hovered_profile = None;
        }
    }

    fn render_add_card(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(Color32::from_gray(45))
            .rounding(Rounding::same(12.0))
            .stroke(Stroke::new(1.0, Color32::from_gray(120)))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(160.0, 160.0));
                ui.vertical_centered(|ui| {
                    ui.painter().circle_filled(
                        ui.cursor().center(),
                        32.0,
                        Color32::from_gray(120),
                    );
                    ui.painter().text(
                        ui.cursor().center(),
                        Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::proportional(26.0),
                        Color32::WHITE,
                    );
                    ui.allocate_space(Vec2::splat(64.0));

                    ui.label(egui::RichText::new("Add New").strong().size(16.0));
                    ui.allocate_space(Vec2::new(0.0, 8.0));

                    if ui.button("Create").clicked() {
                        self.show_add_dialog = true;
                    }
                });
            });
    }

    fn render_guest_card(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(Color32::from_rgb(40, 40, 45))
            .rounding(Rounding::same(12.0))
            .stroke(Stroke::new(1.0, Color32::from_gray(100)))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(160.0, 160.0));
                ui.vertical_centered(|ui| {
                    ui.painter().circle_filled(
                        ui.cursor().center(),
                        32.0,
                        Color32::from_gray(158),
                    );
                    ui.painter().text(
                        ui.cursor().center(),
                        Align2::CENTER_CENTER,
                        "G",
                        egui::FontId::proportional(26.0),
                        Color32::WHITE,
                    );
                    ui.allocate_space(Vec2::splat(64.0));

                    ui.label(egui::RichText::new("Guest").strong().size(16.0));
                    ui.label(
                        egui::RichText::new("Temporary — data wiped on exit")
                            .small()
                            .color(Color32::from_gray(150)),
                    );
                    ui.allocate_space(Vec2::new(0.0, 8.0));

                    if ui.button("Launch").clicked() {
                        self.result = Some(GuiChoice::LaunchGuest);
                    }
                });
            });
    }

    pub fn take_result(&mut self) -> Option<GuiChoice> {
        self.result.take()
    }

    pub fn new_profile_name(&self) -> &str {
        &self.new_profile_name
    }

    pub fn profile_manager(&self) -> &ProfileManager {
        &self.mgr
    }

    pub fn profile_manager_mut(&mut self) -> &mut ProfileManager {
        &mut self.mgr
    }
}

fn parse_hex_color(hex: &str) -> Option<Color32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}
