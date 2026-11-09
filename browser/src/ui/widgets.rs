//! Reusable Chrome-flavoured drawing helpers.
//!
//! Chrome's chrome is a flat surface with Material state layers: an 8% white
//! overlay on hover and a 12% overlay while pressed, on rounded rects or
//! circles. Everything here paints those layers and returns an egui `Response`
//! so callers stay in immediate mode.

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};

use super::fonts;
use super::icons::{self, Icon};
use super::theme::{Metrics, Palette};

/// Rounded rect filled with `color`.
pub fn fill_round(painter: &egui::Painter, rect: Rect, radius: f32, color: Color32) {
    painter.rect_filled(rect, egui::Rounding::same(radius), color);
}

/// Rounded rect outline.
pub fn stroke_round(painter: &egui::Painter, rect: Rect, radius: f32, color: Color32, width: f32) {
    painter.rect_stroke(
        rect,
        egui::Rounding::same(radius),
        Stroke::new(width, color),
    );
}

/// Soft drop shadow, like Chrome's menus, dialogs and the active tab.
pub fn drop_shadow(painter: &egui::Painter, rect: Rect, radius: f32, offset_y: f32, blur: f32, color: Color32) {
    let shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, offset_y),
        blur,
        spread: 0.0,
        color,
    };
    painter.add(shadow.as_shape(rect, radius));
}

/// Animated Material state layer over `rect`.
pub fn state_layer(
    ctx: &egui::Context,
    painter: &egui::Painter,
    id: egui::Id,
    rect: Rect,
    radius: f32,
    color: Color32,
    hovered: bool,
    pressed: bool,
) {
    if !hovered && !pressed {
        return;
    }
    let hover_t = ctx.animate_bool_with_time(id.with("hover"), hovered, 0.08);
    let press_t = ctx.animate_bool_with_time(id.with("press"), pressed, 0.05);
    let strength = (hover_t * 0.55 + press_t * 0.75).clamp(0.0, 1.0);
    let alpha = (color.a() as f32 * strength / 0.55).clamp(0.0, 255.0) as u8;
    fill_round(
        painter,
        rect,
        radius,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
    );
}

/// Circular state layer (toolbar buttons).
pub fn circle_state_layer(
    ctx: &egui::Context,
    painter: &egui::Painter,
    id: egui::Id,
    center: Pos2,
    radius: f32,
    color: Color32,
    hovered: bool,
    pressed: bool,
) {
    if !hovered && !pressed {
        return;
    }
    let hover_t = ctx.animate_bool_with_time(id.with("hover"), hovered, 0.08);
    let press_t = ctx.animate_bool_with_time(id.with("press"), pressed, 0.05);
    let strength = (hover_t * 0.6 + press_t * 0.8).clamp(0.0, 1.0);
    let alpha = (color.a() as f32 * strength / 0.6).clamp(0.0, 255.0) as u8;
    painter.circle_filled(
        center,
        radius,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
    );
}

/// Interact with an absolute rect without moving the layout cursor.
pub fn clickable(ui: &mut egui::Ui, rect: Rect, id: egui::Id, sense: Sense) -> Response {
    ui.interact(rect, id, sense)
}

/// Square toolbar icon button (32x32, circular hover, 18dp glyph).
pub fn icon_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    tooltip: &str,
    enabled: bool,
    key: impl std::hash::Hash,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(Metrics::BUTTON),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    paint_icon_button(ui, palette, rect, icon, &response, enabled);
    if enabled {
        response.on_hover_text(tooltip)
    } else {
        response
    }
}

/// Paint an icon button inside an already-allocated rect.
pub fn paint_icon_button(
    ui: &egui::Ui,
    palette: &Palette,
    rect: Rect,
    icon: Icon,
    response: &Response,
    enabled: bool,
) {
    let hovered = enabled && response.hovered();
    circle_state_layer(
        ui.ctx(),
        ui.painter(),
        response.id,
        rect.center(),
        15.0,
        palette.hover,
        hovered,
        enabled && response.is_pointer_button_down_on(),
    );
    let color = if !enabled {
        palette.icon_disabled
    } else if hovered {
        palette.text
    } else {
        palette.icon
    };
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(Metrics::ICON)),
        icon,
        color,
    );
}

/// Window caption button (minimise / maximise / close).
pub fn window_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    rect: Rect,
    icon: Icon,
    is_close: bool,
    id: egui::Id,
) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    let hovered = response.hovered();
    if hovered {
        let fill = if is_close {
            Color32::from_rgb(0xC4, 0x2B, 0x1C)
        } else if palette.is_dark {
            Color32::from_rgba_unmultiplied(255, 255, 255, 26)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 18)
        };
        fill_round(ui.painter(), rect, 0.0, fill);
    }
    let color = if hovered && is_close {
        Color32::WHITE
    } else {
        palette.icon
    };
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        icon,
        color,
    );
    response
}

/// Chrome-style pill chip with optional leading icon.
pub fn chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    rect: Rect,
    icon: Option<Icon>,
    label: &str,
    bg: Color32,
    fg: Color32,
    font_size: f32,
    key: impl std::hash::Hash,
) -> Response {
    let response = ui.interact(rect, egui::Id::new(key), Sense::click());
    let hovered = response.hovered();
    let bg = if hovered {
        blend(bg, if palette.is_dark { Color32::WHITE } else { Color32::BLACK }, 0.08)
    } else {
        bg
    };
    fill_round(ui.painter(), rect, rect.height() / 2.0, bg);

    let font = fonts::ui_font(font_size);
    let icon_w = if icon.is_some() { 16.0 } else { 0.0 };
    let gap = if icon.is_some() { 6.0 } else { 0.0 };
    let text_w = ui
        .ctx()
        .fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), fg).size().x);
    let content_w = icon_w + gap + text_w;
    let start = rect.center().x - content_w / 2.0;
    if let Some(icon) = icon {
        icons::paint(
            ui.painter(),
            Rect::from_center_size(
                Pos2::new(start + icon_w / 2.0, rect.center().y),
                Vec2::splat(icon_w),
            ),
            icon,
            fg,
        );
    }
    ui.painter().text(
        Pos2::new(start + icon_w + gap, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );
    response
}

/// Blend `b` into `a` by `t` (0..1).
pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

/// Vertical gradient painted as horizontal bands (cheap, no shader).
pub fn gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let bands = 48;
    let h = rect.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1).max(1) as f32;
        let color = blend(top, bottom, t);
        let band = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + i as f32 * h),
            Pos2::new(rect.right(), rect.top() + (i as f32 + 1.0) * h + 0.5),
        );
        painter.rect_filled(band, egui::Rounding::ZERO, color);
    }
}

/// Truncate `text` with an ellipsis so it fits `max_width`.
pub fn ellipsize(ctx: &egui::Context, text: &str, font: &FontId, max_width: f32) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    let width_of = |s: &str| {
        ctx.fonts(|f| {
            f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
    };
    if width_of(text) <= max_width {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate = format!("{}…", chars.iter().collect::<String>().trim_end());
        if width_of(&candidate) <= max_width {
            return candidate;
        }
    }
    String::new()
}

/// Draw `text` clipped to `rect`, vertically centred, with ellipsis.
pub fn text_clipped(
    painter: &egui::Painter,
    ctx: &egui::Context,
    rect: Rect,
    text: &str,
    font: FontId,
    color: Color32,
) {
    let value = ellipsize(ctx, text, &font, rect.width());
    painter.with_clip_rect(rect).text(
        Pos2::new(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        value,
        font,
        color,
    );
}
