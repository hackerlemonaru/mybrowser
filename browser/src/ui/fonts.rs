//! Typography: Roboto for the UI, Roboto Medium for emphasis, Poppins for the
//! New Tab wordmark (the closest freely-licensed stand-in for Product Sans).
//!
//! Chrome renders its chrome in Roboto at 12-13dp; matching the typeface is a
//! large part of matching the look.

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};

/// Family used for the "Google"-style wordmark on the New Tab page.
pub const BRAND: &str = "brand";
/// Medium-weight UI family (tab titles when active, buttons, menu labels).
pub const MEDIUM: &str = "medium";

/// Install the browser fonts and base text styles into `ctx`. Idempotent.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "roboto".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/Roboto-Regular.ttf")),
    );
    fonts.font_data.insert(
        "roboto_medium".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/Roboto-Medium.ttf")),
    );
    fonts.font_data.insert(
        "poppins".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/Poppins-Medium.ttf")),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "roboto".to_owned());

    fonts.families.insert(
        FontFamily::Name(MEDIUM.into()),
        vec!["roboto_medium".to_owned(), "roboto".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name(BRAND.into()),
        vec!["poppins".to_owned(), "roboto_medium".to_owned()],
    );

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(0.0, 0.0);
    style.spacing.menu_margin = egui::Margin::same(0.0);
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    style.visuals.window_fill = egui::Color32::TRANSPARENT;
    style.visuals.panel_fill = egui::Color32::TRANSPARENT;
    ctx.set_style(style);
}

/// Convenience: a proportional font at `size`.
pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Convenience: the medium-weight family at `size`.
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

/// Convenience: the brand family at `size`.
pub fn brand(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BRAND.into()))
}
