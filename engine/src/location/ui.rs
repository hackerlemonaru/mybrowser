//! Location Profile UI — City selector with consistency validation.

use crate::location::{location_catalog, validate_consistency, set_location, LocationProfile};
use egui::{
    Align2, Color32, Frame, Rounding, Stroke, Vec2, Align, Layout, RichText, TextEdit,
};

/// Result of location UI interaction.
#[derive(Debug, Clone)]
pub enum LocationUIAction {
    None,
    SetLocation(LocationProfile),
    ClearLocation,
}

/// Location profile selector widget.
pub struct LocationProfileSelector {
    dialog_open: bool,
    city_search: String,
    selected_location: Option<LocationProfile>,
}

impl Default for LocationProfileSelector {
    fn default() -> Self {
        Self {
            dialog_open: false,
            city_search: String::new(),
            selected_location: None,
        }
    }
}

impl LocationProfileSelector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        config: &Option<LocationProfile>,
    ) -> LocationUIAction {
        let mut action = LocationUIAction::None;

        ui.group(|ui| self.render_group_contents(ui, config, &mut action));

        if self.dialog_open {
            self.render_city_dialog(ui.ctx(), &mut action);
        }

        action
    }

    fn render_group_contents(
        &mut self,
        ui: &mut egui::Ui,
        config: &Option<LocationProfile>,
        action: &mut LocationUIAction,
    ) {
        ui.label(egui::RichText::new("\u{1F4CD} Location Profile").strong());

        ui.horizontal(|ui| {
            let has_loc = config.is_some();
            let mut toggle = has_loc;
            if ui.checkbox(&mut toggle, "Enable Location Spoofing").clicked() {
                if toggle {
                    if let Some(ref loc) = config {
                        *action = LocationUIAction::SetLocation(loc.clone());
                    }
                } else {
                    *action = LocationUIAction::ClearLocation;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Select City…").clicked() {
                    self.dialog_open = true;
                    self.selected_location = config.clone();
                }
            });
        });

        if let Some(ref loc) = config {
            ui.separator();
            ui.label(egui::RichText::new("Active Location:").small().strong());
            ui.label(format!("  {}", loc.name));
            ui.label(format!(
                "  Country: {} ({}) | Timezone: {}",
                loc.country_name, loc.country_code, loc.timezone
            ));
            ui.label(format!(
                "  Coordinates: {:.4}, {:.4} | Locale: {} | Currency: {} {}",
                loc.latitude, loc.longitude, loc.locale, loc.currency_symbol, loc.currency
            ));
            ui.label(format!(
                "  Language: {} | Phone: +{} | Measurement: {}",
                loc.language, loc.phone_code, loc.measurement_system
            ));

            let warnings = validate_consistency(loc);
            if warnings.is_empty() {
                ui.label(
                    egui::RichText::new("\u{2705} Location is fully consistent — undetectable")
                        .small()
                        .color(Color32::from_rgb(0, 200, 0)),
                );
            } else {
                for w in &warnings {
                    ui.label(
                        egui::RichText::new(format!("\u{26A0} {}", w))
                            .small()
                            .color(Color32::from_rgb(255, 165, 0)),
                    );
                }
            }

            ui.label(
                egui::RichText::new("\u{26A0} All location signals will match this city.")
                    .small()
                    .color(Color32::from_rgb(255, 165, 0)),
            );
        } else {
            ui.label(
                egui::RichText::new("No location selected. Click 'Select City…' to choose one.")
                    .small()
                    .color(Color32::from_rgb(255, 165, 0)),
            );
        }
    }

    fn render_city_dialog(
        &mut self,
        ctx: &egui::Context,
        action: &mut LocationUIAction,
    ) {
        egui::Window::new("Select City")
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(700.0, 500.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(600.0);
                ui.label("Choose a city. All location signals will be consistent:");
                ui.separator();

                ui.label("Search (city, country, timezone):");
                ui.text_edit_singleline(&mut self.city_search);
                ui.separator();

                let catalog = location_catalog();
                let filtered: Vec<&LocationProfile> = if self.city_search.is_empty() {
                    catalog.iter().collect()
                } else {
                    let q = self.city_search.to_lowercase();
                    catalog
                        .iter()
                        .filter(|l| {
                            l.city.to_lowercase().contains(&q)
                                || l.country_name.to_lowercase().contains(&q)
                                || l.country_code.to_lowercase().contains(&q)
                                || l.timezone.to_lowercase().contains(&q)
                                || l.region.to_lowercase().contains(&q)
                        })
                        .collect()
                };

                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show_rows(ui, 24.0, filtered.len(), |ui, row_range| {
                        for idx in row_range {
                            let loc = filtered[idx];
                            let is_selected = self
                                .selected_location
                                .as_ref()
                                .map_or(false, |sl| sl.city == loc.city && sl.country_code == loc.country_code);

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
                                    ui.set_min_height(56.0);
                                    ui.horizontal(|ui| {
                                        if ui.radio(is_selected, "").clicked() {
                                            self.selected_location = Some(loc.clone());
                                        }

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&loc.name).strong().size(14.0),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} · {} · {} · {}",
                                                    loc.country_name,
                                                    loc.timezone,
                                                    loc.locale,
                                                    loc.currency
                                                ))
                                                .small()
                                                .color(Color32::from_gray(150)),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "  Coords: {:.4}, {:.4} | Phone: +{}",
                                                    loc.latitude, loc.longitude, loc.phone_code
                                                ))
                                                .small()
                                                .color(Color32::from_gray(150)),
                                            );
                                        });

                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(&loc.country_code)
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

                if let Some(ref loc) = self.selected_location {
                    ui.label(egui::RichText::new("Selected:").strong());
                    ui.label(format!("  {}", loc.name));
                    ui.label(format!(
                        "  Timezone: {} | Locale: {} | Currency: {} {}",
                        loc.timezone, loc.locale, loc.currency_symbol, loc.currency
                    ));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.dialog_open = false;
                        self.selected_location = None;
                    }
                    if ui.button("Apply Location").clicked() {
                        if let Some(loc) = self.selected_location.clone() {
                            *action = LocationUIAction::SetLocation(loc.clone());
                        }
                        self.dialog_open = false;
                        self.selected_location = None;
                    }
                });
            });
    }
}

pub fn location_status_indicator(ui: &mut egui::Ui, config: &Option<LocationProfile>) {
    if let Some(ref loc) = config {
        ui.label(
            RichText::new(format!("\u{1F4CD} {}", loc.name))
                .small()
                .color(Color32::from_rgb(0, 200, 0)),
        );
    } else {
        ui.label(
            RichText::new("\u{1F4CD} Location: Disabled")
                .small()
                .color(Color32::from_gray(150)),
        );
    }
}
