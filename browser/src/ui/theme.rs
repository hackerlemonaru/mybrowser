//! Chrome design tokens: colours, metrics and typography.
//!
//! Values follow Google Chrome's desktop UI (the 2024 "Refresh" look):
//! floating rounded tabs, a pill omnibox inside the toolbar, an 8% white
//! state layer for hover/pressed, and the Material dark/light surfaces.
//!
//! The dark palette is the tinted dark theme (a custom Chrome theme colour)
//! that the reference screenshot uses, so the browser chrome, the tab strip
//! and the New Tab page all sit in one coherent surface family.

use egui::Color32;

/// Colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Light => Mode::Dark,
            Mode::Dark => Mode::Light,
        }
    }
}

/// Layout metrics, in logical pixels. These match Chrome's DIP sizes.
pub struct Metrics;

impl Metrics {
    /// Tab strip height (Chrome: 40-42).
    pub const TAB_STRIP: f32 = 42.0;
    /// Height of a tab card including its rounded body.
    pub const TAB: f32 = 32.0;
    /// Tab width bounds.
    pub const TAB_MIN: f32 = 88.0;
    pub const TAB_MAX: f32 = 232.0;
    /// Toolbar row height.
    pub const TOOLBAR: f32 = 48.0;
    /// Omnibox pill height.
    pub const OMNIBOX: f32 = 34.0;
    /// Omnibox corner radius (fully rounded).
    pub const OMNIBOX_RADIUS: f32 = Self::OMNIBOX / 2.0;
    /// Square toolbar button hit area.
    pub const BUTTON: f32 = 32.0;
    /// Icon glyph size inside a button.
    pub const ICON: f32 = 18.0;
    /// Window control hit area.
    pub const WINDOW_CONTROL: f32 = 46.0;
    /// Bookmarks bar height.
    pub const BOOKMARKS: f32 = 32.0;
    /// Gaps.
    pub const GAP_XS: f32 = 4.0;
    pub const GAP_S: f32 = 8.0;
    pub const GAP_M: f32 = 12.0;
    pub const GAP_L: f32 = 16.0;
}

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The complete palette for one mode.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub mode: Mode,
    pub is_dark: bool,

    // ── Window chrome ──
    /// Tab strip background.
    pub frame: Color32,
    /// Toolbar background.
    pub toolbar: Color32,
    /// 1px hairline under the toolbar.
    pub hairline: Color32,
    /// Active tab body.
    pub tab_active: Color32,
    /// Active tab hairline.
    pub tab_active_stroke: Color32,
    /// State layer over the frame (hover on an inactive tab, buttons).
    pub hover: Color32,
    /// State layer for pressed controls.
    pub pressed: Color32,
    /// Text on the chrome.
    pub text: Color32,
    /// Secondary text (tab titles that are not focused, hints).
    pub text_dim: Color32,
    /// Icons.
    pub icon: Color32,
    /// Disabled icons (back/forward at the ends of history).
    pub icon_disabled: Color32,

    // ── Omnibox ──
    pub omnibox_bg: Color32,
    pub omnibox_hover: Color32,
    pub focus_ring: Color32,
    pub omnibox_text: Color32,
    pub omnibox_hint: Color32,

    // ── Menus / popups ──
    pub menu_bg: Color32,
    pub menu_hover: Color32,
    pub menu_separator: Color32,
    pub shadow: Color32,
    pub scrim: Color32,

    // ── Page / New Tab ──
    pub page_bg: Color32,
    pub ntp_top: Color32,
    pub ntp_bottom: Color32,
    pub ntp_logo: Color32,
    pub ntp_search_bg: Color32,
    pub ntp_search_stroke: Color32,
    pub ntp_search_hint: Color32,
    pub ntp_card: Color32,
    pub ntp_card_stroke: Color32,
    pub ntp_card_title: Color32,
    pub ntp_card_body: Color32,
    pub ntp_chip_bg: Color32,
    pub ntp_chip_text: Color32,
    pub ntp_shortcut_bg: Color32,
    pub ntp_shortcut_text: Color32,

    // ── Accents ──
    pub accent: Color32,
    pub error: Color32,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            mode: Mode::Dark,
            is_dark: true,

            frame: rgb(0x2B1B1B),
            toolbar: rgb(0x2B1B1B),
            hairline: rgba(255, 255, 255, 14),
            tab_active: rgb(0x3B2626),
            tab_active_stroke: rgba(255, 255, 255, 18),
            hover: rgba(255, 255, 255, 18),
            pressed: rgba(255, 255, 255, 30),
            text: rgb(0xF5EDED),
            text_dim: rgb(0xD3BEBE),
            icon: rgb(0xEADDDD),
            icon_disabled: rgba(234, 221, 221, 90),

            omnibox_bg: rgb(0x3A2626),
            omnibox_hover: rgb(0x452D2D),
            focus_ring: rgb(0x9BC3FF),
            omnibox_text: rgb(0xF7F1F1),
            omnibox_hint: rgb(0xBFA9A9),

            menu_bg: rgb(0x2C1D1D),
            menu_hover: rgba(255, 255, 255, 18),
            menu_separator: rgba(255, 255, 255, 22),
            shadow: rgba(0, 0, 0, 110),
            scrim: rgba(0, 0, 0, 20),

            page_bg: rgb(0x6B4747),
            ntp_top: rgb(0x6F4848),
            ntp_bottom: rgb(0x654141),
            ntp_logo: rgb(0xF6CDCD),
            ntp_search_bg: rgb(0x1F1414),
            ntp_search_stroke: rgba(255, 255, 255, 18),
            ntp_search_hint: rgb(0xD9C9C9),
            ntp_card: rgba(0, 0, 0, 48),
            ntp_card_stroke: rgba(255, 255, 255, 12),
            ntp_card_title: rgb(0xFBF3F3),
            ntp_card_body: rgb(0xE6D6D6),
            ntp_chip_bg: rgb(0x2C1C1C),
            ntp_chip_text: rgb(0xFBF3F3),
            ntp_shortcut_bg: rgba(0, 0, 0, 52),
            ntp_shortcut_text: rgb(0xF1E6E6),

            accent: rgb(0x9BC3FF),
            error: rgb(0xFFB4AB),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: Mode::Light,
            is_dark: false,

            frame: rgb(0xDEE1E6),
            toolbar: rgb(0xFFFFFF),
            hairline: rgb(0xE4E6EA),
            tab_active: rgb(0xFFFFFF),
            tab_active_stroke: rgba(0, 0, 0, 20),
            hover: rgba(0, 0, 0, 14),
            pressed: rgba(0, 0, 0, 24),
            text: rgb(0x1F1F1F),
            text_dim: rgb(0x5F6368),
            icon: rgb(0x3C4043),
            icon_disabled: rgba(60, 64, 67, 90),

            omnibox_bg: rgb(0xF1F3F4),
            omnibox_hover: rgb(0xE9ECEF),
            focus_ring: rgb(0x1A73E8),
            omnibox_text: rgb(0x1F1F1F),
            omnibox_hint: rgb(0x5F6368),

            menu_bg: rgb(0xFFFFFF),
            menu_hover: rgb(0xF1F3F4),
            menu_separator: rgb(0xE8EAED),
            shadow: rgba(0, 0, 0, 45),
            scrim: rgba(0, 0, 0, 8),

            page_bg: rgb(0xFFFFFF),
            ntp_top: rgb(0xFFFFFF),
            ntp_bottom: rgb(0xF6F7F9),
            ntp_logo: rgb(0x3C4043),
            ntp_search_bg: rgb(0xFFFFFF),
            ntp_search_stroke: rgb(0xE0E3E7),
            ntp_search_hint: rgb(0x5F6368),
            ntp_card: rgb(0xF1F3F4),
            ntp_card_stroke: rgb(0xE8EAED),
            ntp_card_title: rgb(0x1F1F1F),
            ntp_card_body: rgb(0x5F6368),
            ntp_chip_bg: rgb(0x0B57D0),
            ntp_chip_text: rgb(0xFFFFFF),
            ntp_shortcut_bg: rgb(0xF1F3F4),
            ntp_shortcut_text: rgb(0x1F1F1F),

            accent: rgb(0x0B57D0),
            error: rgb(0xB3261E),
        }
    }

    pub fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Dark => Self::dark(),
            Mode::Light => Self::light(),
        }
    }

    /// Semi-transparent version of a colour (used for hover state layers).
    pub fn alpha(color: Color32, a: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
    }

    /// The page background used by the engine when rendering a document.
    pub fn engine_background(&self) -> u32 {
        if self.is_dark {
            0xFF1F1F1F
        } else {
            0xFFFFFFFF
        }
    }
}
