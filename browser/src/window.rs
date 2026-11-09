//! Browser window UI — a Google Chrome desktop shell.
//!
//! Layout, top to bottom, matching Chrome:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │ ⌄  [New Tab  ×]  +                                        –  □  ×   │  tab strip
//! ├──────────────────────────────────────────────────────────────────────┤
//! │ ← → ⟳  [ Search Google or type a URL   mic lens | ext vpn dl (P) ⋮ ]│  toolbar
//! ├──────────────────────────────────────────────────────────────────────┤
//! │ page (or the New Tab page)                                           │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Everything that is not standard Chrome chrome lives inside the 3-dot menu
//! (VPN, dummy system profile, location profile, appearance, panels).

use anyhow::Result;
use engine::dummy_profile::ui::{DummyProfileSelector, DummyProfileUIAction};
use engine::dummy_profile::{set_config, DummySystemConfig};
use engine::location::ui::{LocationProfileSelector, LocationUIAction};
use engine::location::{remove_location, set_location, LocationProfile};
use engine::net::fetch;
use engine::net::proxy::ProxyManager;
use engine::tab_management::{Tab, TabId, TabManager, TabState};
use egui::{
    pos2, vec2, Align2, Color32, Key, Pos2, Rect, Sense, Stroke, TextEdit, TextureOptions, Vec2,
};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use crate::ui::{
    fonts,
    icons::{self, Icon},
    theme::{Metrics, Mode, Palette},
    widgets,
};

mod page;
mod popovers;
mod settings;

/// Height of a menu row in the 3-dot menu.
const MENU_ROW: f32 = 28.0;
/// Height of a divider in the 3-dot menu.
const MENU_DIVIDER: f32 = 9.0;

/// The floating popover that is currently open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Popup {
    Menu,
    TabSearch,
    Appearance,
    Extensions,
    Profile,
    Vpn,
}

/// Sections of the full-page settings screen, mirroring chrome://settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    You,
    Autofill,
    Privacy,
    Appearance,
    Search,
    Startup,
    Downloads,
    About,
}

impl SettingsSection {
    const ALL: [SettingsSection; 8] = [
        SettingsSection::You,
        SettingsSection::Autofill,
        SettingsSection::Privacy,
        SettingsSection::Appearance,
        SettingsSection::Search,
        SettingsSection::Startup,
        SettingsSection::Downloads,
        SettingsSection::About,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsSection::You => "You and MyBrowser",
            SettingsSection::Autofill => "Autofill and passwords",
            SettingsSection::Privacy => "Privacy and security",
            SettingsSection::Appearance => "Appearance",
            SettingsSection::Search => "Search engine",
            SettingsSection::Startup => "On startup",
            SettingsSection::Downloads => "Downloads",
            SettingsSection::About => "About MyBrowser",
        }
    }

    fn icon(self) -> Icon {
        match self {
            SettingsSection::You => Icon::Device,
            SettingsSection::Autofill => Icon::Key,
            SettingsSection::Privacy => Icon::Shield,
            SettingsSection::Appearance => Icon::Palette,
            SettingsSection::Search => Icon::Search,
            SettingsSection::Startup => Icon::Home,
            SettingsSection::Downloads => Icon::Download,
            SettingsSection::About => Icon::Info,
        }
    }
}

/// Persisted per-profile preferences (`<profile>/browser_prefs.json`).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Prefs {
    engine: String,
    theme: String,
    bookmarks_bar: bool,
    startup: String,
    startup_url: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            engine: "google".into(),
            theme: "dark".into(),
            bookmarks_bar: true,
            startup: "newtab".into(),
            startup_url: String::new(),
        }
    }
}

fn prefs_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("browser_prefs.json")
}

fn load_prefs(data_dir: &std::path::Path) -> Prefs {
    std::fs::read_to_string(prefs_path(data_dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// One row of the built-in search results view.
#[derive(Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Result of rendering one page on a worker thread.
struct RenderOutcome {
    url: String,
    title: String,
    buffer: Result<engine::RenderedBuffer, String>,
    /// Non-empty when the page is a search: the browser draws these itself.
    results: Vec<SearchResult>,
}

/// A page render in flight. Pages are fetched and laid out off the UI thread so
/// switching tabs never blocks on the network.
struct PendingRender {
    tab: TabId,
    rx: Receiver<RenderOutcome>,
    /// Size the page was laid out for.
    size: Vec2,
    /// When the render started, so a stuck worker can time out.
    started: Instant,
}

/// A modal dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialog {
    History,
    Downloads,
    Bookmarks,
    Passwords,
}

/// A side panel opened from the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidePanel {
    Dummy,
    Location,
}

/// Something the user asked for from a menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    NewTab,
    NewWindow,
    Incognito,
    ReopenTab,
    History,
    Downloads,
    Bookmarks,
    Passwords,
    Vpn,
    Dummy,
    Location,
    Zoom,
    Print,
    Find,
    Settings,
    Help,
    LightMode,
    DarkMode,
    ToggleBookmarksBar,
    Customize,
}

/// A NTP / bookmarks-bar shortcut.
#[derive(Clone)]
struct Shortcut {
    title: String,
    url: String,
}

/// Rendered page texture.
struct Viewport {
    texture: Option<egui::TextureHandle>,
    size: Vec2,
    dirty: bool,
}

/// Per-tab navigation history. `TabManager` owns the tabs themselves.
#[derive(Default)]
struct NavigationHistory {
    entries: Vec<String>,
    index: usize,
}

/// One entry in a floating popover.
enum MenuEntry {
    Row {
        icon: Icon,
        label: String,
        shortcut: Option<String>,
        action: Action,
        checked: Option<bool>,
        danger: bool,
    },
    Divider,
    Section(String),
    Header(String),
}

impl MenuEntry {
    fn height(&self) -> f32 {
        match self {
            MenuEntry::Row { .. } => MENU_ROW,
            MenuEntry::Divider => MENU_DIVIDER,
            MenuEntry::Section(_) | MenuEntry::Header(_) => 26.0,
        }
    }
}

/// The browser window.
pub struct BrowserWindow {
    profile_id: String,
    profile_name: String,
    data_dir: std::path::PathBuf,
    tab_manager: TabManager,
    navigation: HashMap<TabId, NavigationHistory>,
    dragged_tab: Option<TabId>,
    address_bar: String,
    ntp_query: String,
    omnibox_focused: bool,
    address_focus_requested: bool,
    mode: Mode,
    bookmarks_bar_visible: bool,
    viewport: Viewport,
    /// Rendered page per tab, so switching back to a tab is instant.
    pages: HashMap<TabId, egui::TextureHandle>,
    /// Parsed search results per tab (drawn by the browser, not the engine).
    searches: HashMap<TabId, Vec<SearchResult>>,
    pending: Option<PendingRender>,
    /// Tab whose texture the viewport currently shows.
    shown_tab: Option<TabId>,
    /// Cached bookmark (title, url) pairs. Reading `bookmarks.json` on every
    /// frame caused visible stutter while hovering and switching tabs.
    bookmarks_cache: Vec<(String, String)>,
    bookmarks_dirty: bool,
    shortcuts_cache: Vec<Shortcut>,
    shortcuts_dirty: bool,
    /// Vertical offset of the open popover, for the slide animation.
    popup_slide: f32,
    settings: Option<SettingsSection>,
    settings_notice: Option<String>,
    prefs: Prefs,
    profile_anchor: Option<Rect>,
    vpn_anchor: Option<Rect>,
    popup: Option<Popup>,
    /// Frame in which the popup was opened: the click that opened it must not
    /// close it again on the same frame.
    popup_frame: u64,
    last_popup: Option<Popup>,
    menu_anchor: Option<Rect>,
    appearance_anchor: Option<Rect>,
    tab_search_anchor: Option<Rect>,
    dialog: Option<Dialog>,
    /// Frame the current dialog was opened in, so the click that opened it does
    /// not count as a click on the scrim.
    dialog_frame: u64,
    last_dialog: Option<Dialog>,
    side_panel: Option<SidePanel>,
    last_frame: Instant,
    fonts_installed: bool,
    // Per-profile VPN (fresh per profile).
    vpn_enabled: bool,
    vpn_provider: String,
    vpn_endpoint: String,
    vpn_message: Option<String>,
    dummy_config: DummySystemConfig,
    dummy_ui: DummyProfileSelector,
    location_config: Option<LocationProfile>,
    location_ui: LocationProfileSelector,
}

impl BrowserWindow {
    pub fn new(profile_id: &str) -> Result<Self> {
        let mgr = profile_manager::ProfileManager::new()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let profile = mgr
            .resolve(profile_id)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_id))?;
        let data_dir = profile.data_dir.clone();
        let profile_name = profile.name.clone();

        // A brand new window starts on an empty New Tab page.
        let mut tab_manager = TabManager::new();
        let first_tab = tab_manager.open_tab("about:blank".to_string(), "New Tab".to_string());
        let mut navigation = HashMap::new();
        navigation.insert(
            first_tab,
            NavigationHistory {
                entries: vec!["about:blank".to_string()],
                index: 0,
            },
        );

        let prefs = load_prefs(&data_dir);

        let (vpn_enabled, vpn_provider, vpn_endpoint, _connected, vpn_message) =
            ProxyManager::vpn_status(profile_id);

        let mut window = Self {
            profile_id: profile_id.to_string(),
            profile_name,
            data_dir,
            tab_manager,
            navigation,
            dragged_tab: None,
            address_bar: String::new(),
            ntp_query: String::new(),
            omnibox_focused: false,
            address_focus_requested: false,
            mode: if prefs.theme == "light" {
                Mode::Light
            } else {
                Mode::Dark
            },
            bookmarks_bar_visible: prefs.bookmarks_bar,
            viewport: Viewport {
                texture: None,
                size: Vec2::ZERO,
                dirty: true,
            },
            pages: HashMap::new(),
            searches: HashMap::new(),
            pending: None,
            shown_tab: None,
            bookmarks_cache: Vec::new(),
            bookmarks_dirty: true,
            shortcuts_cache: Vec::new(),
            shortcuts_dirty: true,
            popup_slide: 0.0,
            settings: None,
            settings_notice: None,
            prefs: prefs.clone(),
            profile_anchor: None,
            vpn_anchor: None,
            popup: None,
            popup_frame: 0,
            last_popup: None,
            menu_anchor: None,
            appearance_anchor: None,
            tab_search_anchor: None,
            dialog: None,
            dialog_frame: 0,
            last_dialog: None,
            side_panel: None,
            last_frame: Instant::now(),
            fonts_installed: false,
            vpn_enabled,
            vpn_provider,
            vpn_endpoint,
            vpn_message,
            dummy_config: engine::dummy_profile::get_config(profile_id),
            dummy_ui: DummyProfileSelector::new(),
            location_config: engine::location::get_location(profile_id),
            location_ui: LocationProfileSelector::new(),
        };

        // "On startup" preference: optionally open a chosen page instead of the
        // New Tab page.
        if window.prefs.startup == "url" && !window.prefs.startup_url.trim().is_empty() {
            let url = window.prefs.startup_url.clone();
            window.navigate_to(&url);
        }

        Ok(window)
    }

    /// Reload the cached bookmark list (only when something changed).
    fn refresh_bookmarks(&mut self) {
        if !self.bookmarks_dirty {
            return;
        }
        self.bookmarks_cache = storage::bookmarks::list(&self.data_dir)
            .into_iter()
            .map(|bookmark| (bookmark.title, bookmark.url))
            .collect();
        self.bookmarks_dirty = false;
    }

    fn bookmarks(&mut self) -> &[(String, String)] {
        self.refresh_bookmarks();
        &self.bookmarks_cache
    }

    fn save_prefs(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.prefs) {
            let _ = std::fs::write(prefs_path(&self.data_dir), json);
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.prefs.theme = if mode == Mode::Light { "light" } else { "dark" }.to_string();
        self.save_prefs();
    }

    /// Mark the fonts as installed, for when they were set up before the
    /// first frame (see `main.rs`).
    pub fn mark_fonts_installed(&mut self) {
        self.fonts_installed = true;
    }

    /// Render the whole window. Returns true when the window should close.
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        if !self.fonts_installed {
            fonts::install(ctx);
            self.fonts_installed = true;
        }
        self.handle_shortcuts(ctx);
        let p = Palette::for_mode(self.mode);
        self.apply_egui_visuals(ctx, &p);
        self.refresh_bookmarks();
        // Popovers slide down a few pixels as they open and back up as they
        // close, so they feel attached to the button that opened them.
        self.popup_slide = {
            let open = ctx.animate_bool_with_time(
                egui::Id::new("popover_open"),
                self.popup.is_some(),
                0.14,
            );
            -8.0 * (1.0 - open)
        };

        egui::TopBottomPanel::top("chrome_tab_strip")
            .min_height(Metrics::TAB_STRIP)
            .max_height(Metrics::TAB_STRIP)
            .frame(egui::Frame::none().fill(p.frame))
            .show(ctx, |ui| self.render_tab_strip(ui, &p));

        egui::TopBottomPanel::top("chrome_toolbar")
            .min_height(Metrics::TOOLBAR)
            .max_height(Metrics::TOOLBAR)
            .frame(egui::Frame::none().fill(p.toolbar))
            .show(ctx, |ui| self.render_toolbar(ui, &p));

        if self.bookmarks_bar_visible && !self.bookmarks_cache.is_empty() {
            egui::TopBottomPanel::top("chrome_bookmarks")
                .min_height(Metrics::BOOKMARKS)
                .max_height(Metrics::BOOKMARKS)
                .frame(egui::Frame::none().fill(p.toolbar))
                .show(ctx, |ui| self.render_bookmarks_bar(ui, &p));
        }

        if let Some(panel) = self.side_panel {
            egui::SidePanel::right("chrome_side_panel")
                .exact_width(360.0)
                .frame(
                    egui::Frame::none()
                        .fill(p.menu_bg)
                        .inner_margin(egui::Margin::same(18.0)),
                )
                .show(ctx, |ui| match panel {
                    SidePanel::Dummy => self.render_dummy_panel(ui),
                    SidePanel::Location => self.render_location_panel(ui),
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(p.page_bg))
            .show(ctx, |ui| {
                if self.settings.is_some() {
                    self.render_settings(ui, &p);
                } else if self.is_new_tab_page() {
                    self.render_new_tab(ui, &p);
                } else {
                    self.render_page(ui, &p);
                }
            });

        if self.settings.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.settings = None;
        }

        // Any popover that just appeared must ignore the click that opened it
        // (including popovers opened from a menu row).
        if self.popup != self.last_popup {
            self.popup_frame = ctx.cumulative_pass_nr();
            self.last_popup = self.popup;
        }

        // Floating layers, painted above the page.
        match self.popup {
            Some(Popup::Menu) => self.render_menu(ctx, &p),
            Some(Popup::TabSearch) => self.render_tab_search(ctx, &p),
            Some(Popup::Appearance) => self.render_appearance(ctx, &p),
            Some(Popup::Extensions) => self.render_extensions(ctx, &p),
            Some(Popup::Profile) => self.render_profile_popover(ctx, &p),
            Some(Popup::Vpn) => self.render_vpn_popover(ctx, &p),
            None => {}
        }
        if self.dialog != self.last_dialog {
            self.dialog_frame = ctx.cumulative_pass_nr();
            self.last_dialog = self.dialog;
        }
        self.render_dialog(ctx, &p);

        self.last_frame = Instant::now();
        false
    }

    /// Keep egui's own widgets (text fields, scroll areas, side panels) in the
    /// same colour world as the hand-painted chrome.
    fn apply_egui_visuals(&self, ctx: &egui::Context, p: &Palette) {
        ctx.style_mut(|style| {
            style.visuals.dark_mode = p.is_dark;
            style.visuals.override_text_color = Some(p.text);
            style.visuals.selection.bg_fill = p.accent.gamma_multiply(0.55);
            style.visuals.selection.stroke = Stroke::new(1.0, p.accent);
            style.visuals.extreme_bg_color = p.omnibox_bg;
            style.visuals.faint_bg_color = p.hover;
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline);
            style.visuals.widgets.inactive.bg_fill = p.hover;
            style.visuals.widgets.inactive.weak_bg_fill = p.hover;
            style.visuals.widgets.hovered.bg_fill = p.pressed;
            style.visuals.widgets.hovered.weak_bg_fill = p.pressed;
            style.visuals.widgets.active.bg_fill = p.pressed;
            style.visuals.window_fill = p.menu_bg;
            style.visuals.panel_fill = p.menu_bg;
        });
    }

    // ───────────────────────────── Tab strip ─────────────────────────────

    fn render_tab_strip(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        let cy = rect.center().y;

        // Window caption buttons, far right (Chrome on Windows/Linux).
        let controls_w = Metrics::WINDOW_CONTROL * 3.0;
        let controls = Rect::from_min_max(pos2(rect.right() - controls_w, rect.top()), rect.max);
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));

        // Tab search chevron at the far left.
        let search_rect = Rect::from_center_size(pos2(rect.left() + 20.0, cy), Vec2::splat(28.0));
        let search_response = ui.interact(
            search_rect,
            egui::Id::new("tab_search_button"),
            Sense::click(),
        );
        widgets::paint_icon_button(ui, p, search_rect, Icon::TabSearch, &search_response, true);
        if search_response.clicked() {
            self.tab_search_anchor = Some(search_rect);
            self.popup = if self.popup == Some(Popup::TabSearch) {
                None
            } else {
                Some(Popup::TabSearch)
            };
            self.popup_frame = ui.ctx().cumulative_pass_nr();
        }

        // Tabs occupy everything between the chevron and the new-tab button.
        let tabs_left = search_rect.right() + 2.0;
        let tabs_right = controls.left() - 8.0;
        let tabs_rect = Rect::from_min_max(
            pos2(tabs_left, rect.top() + 5.0),
            pos2(tabs_right, rect.bottom() - 5.0),
        );

        let ids = self.tab_manager.tab_order().to_vec();
        let count = ids.len().max(1) as f32;
        let gap = 2.0;
        let tab_w = ((tabs_rect.width() - (count - 1.0) * gap) / count)
            .clamp(Metrics::TAB_MIN, Metrics::TAB_MAX);

        let mut x = tabs_rect.left();
        let tabs: Vec<Tab> = ids
            .iter()
            .filter_map(|id| self.tab_manager.get_tab(*id).cloned())
            .collect();
        let tab_count = tabs.len();
        for (index, tab) in tabs.iter().enumerate() {
            let width = if tab.pinned { 46.0 } else { tab_w };
            let tab_rect = Rect::from_min_max(
                pos2(x, tabs_rect.top()),
                pos2(x + width, tabs_rect.bottom()),
            );
            if tab_rect.right() > tabs_rect.right() + 1.0 {
                break;
            }
            let active = self.tab_manager.active_tab() == Some(tab.id);
            self.render_tab_card(ui, tab, tab_rect, active, index == tab_count - 1, p);
            x += width + gap;
        }

        // New tab button, right beside the last tab: Chrome does not pin it to
        // the far right of the strip.
        let plus_rect = Rect::from_center_size(pos2(x + 13.0, cy), Vec2::splat(30.0));
        let plus_response = ui.interact(plus_rect, egui::Id::new("new_tab_button"), Sense::click());
        widgets::paint_icon_button(ui, p, plus_rect, Icon::Plus, &plus_response, true);
        if plus_response.clicked() {
            self.open_new_tab();
        }
        let _ = plus_response.on_hover_text("New tab");

        // Dragging the empty strip moves the window; double-click maximises
        // (exactly like Chrome's title bar behaviour).
        if plus_rect.right() < tabs_rect.right() {
            let empty = Rect::from_min_max(
                pos2(plus_rect.right() + 4.0, rect.top()),
                pos2(controls.left() - 8.0, rect.bottom()),
            );
            let empty_response =
                ui.interact(empty, egui::Id::new("tab_strip_drag_area"), Sense::click_and_drag());
            if empty_response.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if empty_response.double_clicked() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
        }

        // Caption buttons.
        let mut cx = controls.left();
        for (icon, kind) in [
            (Icon::Minimize, 0),
            (if maximized { Icon::Restore } else { Icon::Maximize }, 1),
            (Icon::Close, 2),
        ] {
            let button = Rect::from_min_max(
                pos2(cx, rect.top()),
                pos2(cx + Metrics::WINDOW_CONTROL, rect.bottom()),
            );
            let response = widgets::window_button(
                ui,
                p,
                button,
                icon,
                kind == 2,
                egui::Id::new(("window_control", kind)),
            );
            if response.clicked() {
                match kind {
                    0 => ui
                        .ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                    1 => ui
                        .ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
                    _ => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                }
            }
            cx += Metrics::WINDOW_CONTROL;
        }
    }

    fn render_tab_card(
        &mut self,
        ui: &mut egui::Ui,
        tab: &Tab,
        rect: Rect,
        active: bool,
        is_last: bool,
        p: &Palette,
    ) {
        let id = egui::Id::new(("chrome_tab", tab.id.as_u64()));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let hovered = response.hovered();

        if active {
            widgets::drop_shadow(ui.painter(), rect, 10.0, 1.0, 6.0, p.shadow);
            widgets::fill_round(ui.painter(), rect, 10.0, p.tab_active);
            widgets::stroke_round(ui.painter(), rect, 10.0, p.tab_active_stroke, 1.0);
        } else if hovered {
            widgets::state_layer(
                ui.ctx(),
                ui.painter(),
                id,
                rect,
                10.0,
                p.hover,
                hovered,
                response.is_pointer_button_down_on(),
            );
        }

        // Leading icon: spinner while loading, mute glyph, otherwise the site
        // icon. A New Tab has no favicon in Chrome, so it has no leading icon.
        let blank = tab.url == "about:blank";
        let icon_center = pos2(rect.left() + 16.0, rect.center().y);
        if tab.muted {
            icons::paint(
                ui.painter(),
                Rect::from_center_size(icon_center, Vec2::splat(15.0)),
                Icon::SpeakerMuted,
                p.text_dim,
            );
        } else if tab.state == TabState::Loading && !blank {
            icons::paint(
                ui.painter(),
                Rect::from_center_size(icon_center, Vec2::splat(15.0)),
                Icon::Reload,
                p.text_dim,
            );
        } else if !blank {
            icons::paint(
                ui.painter(),
                Rect::from_center_size(icon_center, Vec2::splat(15.0)),
                Icon::Globe,
                if active { p.icon } else { p.text_dim },
            );
        }

        let title = if tab.title.is_empty() {
            if tab.url == "about:blank" {
                "New Tab"
            } else {
                tab.url.as_str()
            }
        } else {
            tab.title.as_str()
        };

        if !tab.pinned {
            let show_close = tab.is_closable() && (active || hovered);
            let text_right = if tab.is_closable() {
                rect.right() - if show_close { 30.0 } else { 10.0 }
            } else {
                rect.right() - 8.0
            };
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(
                    pos2(
                        rect.left() + if blank { 14.0 } else { 30.0 },
                        rect.top(),
                    ),
                    pos2(text_right, rect.bottom()),
                ),
                title,
                fonts::ui_font(12.5),
                if active { p.text } else { p.text_dim },
            );

            if show_close {
                let close_rect =
                    Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), Vec2::splat(22.0));
                let close_id = egui::Id::new(("chrome_tab_close", tab.id.as_u64()));
                let close = ui.interact(close_rect, close_id, Sense::click());
                if close.hovered() {
                    widgets::circle_state_layer(
                        ui.ctx(),
                        ui.painter(),
                        close_id,
                        close_rect.center(),
                        10.0,
                        p.pressed,
                        true,
                        close.is_pointer_button_down_on(),
                    );
                }
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(close_rect.center(), Vec2::splat(13.0)),
                    Icon::Close,
                    if close.hovered() { p.text } else { p.text_dim },
                );
                if close.clicked() {
                    self.close_tab(tab.id);
                    return;
                }
            }
        }

        // Hairline between adjacent inactive tabs.
        if !active && !hovered && !is_last && !tab.pinned {
            ui.painter().vline(
                rect.right() + 1.0,
                (rect.center().y - 9.0)..=(rect.center().y + 9.0),
                Stroke::new(1.0, p.hairline),
            );
        }

        if response.clicked() {
            self.tab_manager.switch_to_tab(tab.id);
            self.sync_active_address();
            self.viewport.dirty = true;
        }
        if response.middle_clicked() {
            self.close_tab(tab.id);
            return;
        }

        if response.drag_started() {
            self.dragged_tab = Some(tab.id);
        }
        if let Some(dragged) = self.dragged_tab {
            if response.hovered() && dragged != tab.id && ui.input(|i| i.pointer.primary_down()) {
                if let Some(target) = self
                    .tab_manager
                    .tab_order()
                    .iter()
                    .position(|id| *id == tab.id)
                {
                    self.tab_manager.move_tab_to_position(dragged, target);
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.dragged_tab = None;
            }
        }
    }

    // ───────────────────────────── Toolbar ─────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        let cy = rect.center().y;
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, p.hairline),
        );

        let mut x = rect.left() + 6.0;
        let (can_back, can_forward) = self.navigation_availability();
        let nav_id = |name: &'static str| egui::Id::new(("nav", name));

        for (icon, enabled, tooltip, name) in [
            (Icon::Back, can_back, "Back  (Alt+Left)", "back"),
            (Icon::Forward, can_forward, "Forward  (Alt+Right)", "forward"),
            (
                Icon::Reload,
                true,
                "Reload  (Ctrl+R)",
                "reload",
            ),
        ] {
            let button = Rect::from_center_size(pos2(x + 16.0, cy), Vec2::splat(32.0));
            let response = ui.interact(button, nav_id(name), Sense::click());
            widgets::paint_icon_button(ui, p, button, icon, &response, enabled);
            if enabled {
                let response = response.on_hover_text(tooltip);
                if response.clicked() {
                    match name {
                        "back" => self.navigate_history(-1),
                        "forward" => self.navigate_history(1),
                        _ => {
                            self.viewport.texture = None;
                            self.viewport.dirty = true;
                        }
                    }
                }
            }
            x += 32.0 + 2.0;
        }

        // Right cluster: extensions, VPN, downloads, profile avatar, 3-dot menu.
        let menu_rect = Rect::from_center_size(pos2(rect.right() - 22.0, cy), Vec2::splat(32.0));
        let avatar_rect = Rect::from_center_size(pos2(menu_rect.left() - 20.0, cy), Vec2::splat(30.0));
        let downloads_rect =
            Rect::from_center_size(pos2(avatar_rect.left() - 20.0, cy), Vec2::splat(32.0));
        let vpn_rect =
            Rect::from_center_size(pos2(downloads_rect.left() - 18.0, cy), Vec2::splat(32.0));
        let extensions_rect =
            Rect::from_center_size(pos2(vpn_rect.left() - 18.0, cy), Vec2::splat(32.0));

        let omni = Rect::from_min_max(
            pos2(x + 4.0, cy - Metrics::OMNIBOX / 2.0),
            pos2(extensions_rect.left() - 8.0, cy + Metrics::OMNIBOX / 2.0),
        );
        self.vpn_anchor = Some(vpn_rect);
        self.profile_anchor = Some(avatar_rect);
        self.menu_anchor = Some(menu_rect);
        self.appearance_anchor = Some(menu_rect);
        self.render_omnibox(ui, p, omni);

        // Extensions.
        let ext_response = ui.interact(
            extensions_rect,
            egui::Id::new("toolbar_extensions"),
            Sense::click(),
        );
        widgets::paint_icon_button(ui, p, extensions_rect, Icon::Extension, &ext_response, true);
        if ext_response.clicked() {
            self.popup = if self.popup == Some(Popup::Extensions) {
                None
            } else {
                Some(Popup::Extensions)
            };
            self.popup_frame = ui.ctx().cumulative_pass_nr();
        }
        ext_response.on_hover_text("Extensions");

        // Per-profile VPN.
        let vpn_response = ui.interact(vpn_rect, egui::Id::new("toolbar_vpn"), Sense::click());
        widgets::paint_icon_button(ui, p, vpn_rect, Icon::Shield, &vpn_response, true);
        if self.vpn_enabled {
            ui.painter().circle_filled(
                pos2(vpn_rect.right() - 9.0, vpn_rect.bottom() - 9.0),
                3.0,
                p.accent,
            );
        }
        if vpn_response.clicked() {
            self.popup = if self.popup == Some(Popup::Vpn) {
                None
            } else {
                Some(Popup::Vpn)
            };
            self.popup_frame = ui.ctx().cumulative_pass_nr();
        }
        vpn_response.on_hover_text(if self.vpn_enabled {
            "VPN is on"
        } else {
            "VPN is off"
        });

        // Downloads.
        let downloads_response =
            ui.interact(downloads_rect, egui::Id::new("toolbar_downloads"), Sense::click());
        widgets::paint_icon_button(ui, p, downloads_rect, Icon::Download, &downloads_response, true);
        if downloads_response.clicked() {
            self.dialog = Some(Dialog::Downloads);
        }
        downloads_response.on_hover_text("Downloads");

        // Profile avatar.
        let avatar_response = ui.interact(avatar_rect, egui::Id::new("profile_avatar"), Sense::click());
        widgets::circle_state_layer(
            ui.ctx(),
            ui.painter(),
            avatar_response.id,
            avatar_rect.center(),
            15.0,
            p.hover,
            avatar_response.hovered(),
            avatar_response.is_pointer_button_down_on(),
        );
        let initial = self
            .profile_name
            .chars()
            .next()
            .unwrap_or('A')
            .to_uppercase()
            .next()
            .unwrap_or('A');
        let avatar_color = if p.is_dark {
            Color32::from_rgb(0x6E, 0x9B, 0xE8)
        } else {
            Color32::from_rgb(0x1A, 0x73, 0xE8)
        };
        ui.painter()
            .circle_filled(avatar_rect.center(), 13.5, avatar_color);
        ui.painter().text(
            avatar_rect.center(),
            Align2::CENTER_CENTER,
            initial,
            fonts::medium(13.0),
            Color32::WHITE,
        );
        let avatar_response = avatar_response.on_hover_text(&self.profile_name);
        if avatar_response.clicked() {
            self.popup = if self.popup == Some(Popup::Profile) {
                None
            } else {
                Some(Popup::Profile)
            };
            self.popup_frame = ui.ctx().cumulative_pass_nr();
        }

        // 3-dot menu.
        let menu_response = ui.interact(menu_rect, egui::Id::new("chrome_menu_button"), Sense::click());
        widgets::paint_icon_button(ui, p, menu_rect, Icon::Menu, &menu_response, true);
        if menu_response.clicked() {
            self.popup = if self.popup == Some(Popup::Menu) {
                None
            } else {
                Some(Popup::Menu)
            };
            self.popup_frame = ui.ctx().cumulative_pass_nr();
        }
        menu_response.on_hover_text("Customise and control MyBrowser");
    }

    fn render_omnibox(&mut self, ui: &mut egui::Ui, p: &Palette, omni: Rect) {
        let bg_response = ui.interact(omni, egui::Id::new("omnibox_bg"), Sense::click());
        if bg_response.clicked() {
            self.address_focus_requested = true;
        }

        // Only the bookmark star sits inside the pill; mic and Lens live on the
        // New Tab search box.
        let right_w = 32.0 + 10.0;
        let edit_rect = Rect::from_min_max(
            pos2(omni.left() + 34.0, omni.center().y - 11.0),
            pos2(omni.right() - right_w, omni.center().y + 11.0),
        );

        // Pill background: focus ring, hover, or resting surface.
        if self.omnibox_focused {
            widgets::fill_round(ui.painter(), omni, Metrics::OMNIBOX_RADIUS, p.omnibox_bg);
            widgets::stroke_round(ui.painter(), omni, Metrics::OMNIBOX_RADIUS, p.focus_ring, 2.0);
        } else {
            let fill = if bg_response.hovered() {
                p.omnibox_hover
            } else {
                p.omnibox_bg
            };
            widgets::fill_round(ui.painter(), omni, Metrics::OMNIBOX_RADIUS, fill);
        }

        // Leading icon: the Google mark on a new tab, otherwise the site state.
        let active_url = self.active_url();
        let icon_rect =
            Rect::from_center_size(pos2(omni.left() + 17.0, omni.center().y), Vec2::splat(16.0));
        if active_url == "about:blank" || active_url.is_empty() {
            icons::paint(ui.painter(), icon_rect, Icon::Search, p.text_dim);
        } else {
            let icon = if active_url.starts_with("https://") {
                Icon::Tune
            } else if active_url.starts_with("http://") {
                Icon::Globe
            } else {
                Icon::Info
            };
            icons::paint(ui.painter(), icon_rect, icon, p.text_dim);
        }

        ui.visuals_mut().selection.bg_fill = p.accent.gamma_multiply(0.5);
        ui.visuals_mut().selection.stroke = Stroke::NONE;
        let engine_name = match self.prefs.engine.as_str() {
            "duckduckgo" => "DuckDuckGo",
            "bing" => "Bing",
            _ => "Google",
        };
        let hint = format!("Search {} or type a URL", engine_name);
        let response = ui.put(
            edit_rect,
            TextEdit::singleline(&mut self.address_bar)
                .frame(false)
                .font(fonts::ui_font(13.0))
                .text_color(p.omnibox_text)
                .hint_text(hint)
                .hint_text_font(fonts::ui_font(13.0))
                .desired_width(edit_rect.width())
                .margin(egui::Margin::symmetric(0.0, 1.0)),
        );
        if self.address_focus_requested {
            response.request_focus();
            self.address_focus_requested = false;
        }
        self.omnibox_focused = response.has_focus();
        let submitted = ui.input(|i| i.key_pressed(Key::Enter))
            && (response.has_focus() || response.lost_focus());
        if submitted {
            let value = self.address_bar.trim().to_string();
            if !value.is_empty() {
                self.navigate_to(&value);
                response.surrender_focus();
            }
        }

        // Bookmark star, always the rightmost control inside the pill.
        let star_rect =
            Rect::from_center_size(pos2(omni.right() - 20.0, omni.center().y), Vec2::splat(30.0));
        let bookmarked = self.is_active_bookmarked();
        let response = ui.interact(star_rect, egui::Id::new("omnibox_star"), Sense::click());
        widgets::paint_icon_button(
            ui,
            p,
            star_rect,
            if bookmarked {
                Icon::StarFilled
            } else {
                Icon::Star
            },
            &response,
            true,
        );
        if response.clicked() {
            self.toggle_bookmark();
        }
        response.on_hover_text(if bookmarked {
            "Remove bookmark"
        } else {
            "Bookmark this tab"
        });
    }

    // ───────────────────────── Bookmark bar ─────────────────────────

    fn render_bookmarks_bar(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, p.hairline),
        );
        let mut x = rect.left() + 10.0;
        let entries = self.bookmarks().to_vec();
        for bookmark in &entries {
            let font = fonts::ui_font(12.0);
            let text_w = ui.ctx().fonts(|f| {
                f.layout_no_wrap(bookmark.0.clone(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            });
            let width = (14.0 + 8.0 + text_w + 16.0).min(200.0);
            if x + width > rect.right() - 40.0 {
                break;
            }
            let item = Rect::from_min_max(
                pos2(x, rect.top() + 3.0),
                pos2(x + width, rect.bottom() - 3.0),
            );
            let id = egui::Id::new(("bookmark_item", bookmark.1.clone()));
            let response = ui.interact(item, id, Sense::click());
            widgets::state_layer(
                ui.ctx(),
                ui.painter(),
                id,
                item,
                8.0,
                p.hover,
                response.hovered(),
                response.is_pointer_button_down_on(),
            );
            icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(item.left() + 15.0, item.center().y), Vec2::splat(14.0)),
                Icon::Globe,
                p.text_dim,
            );
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(
                    pos2(item.left() + 28.0, item.top()),
                    pos2(item.right() - 8.0, item.bottom()),
                ),
                &bookmark.0,
                font,
                p.text,
            );
            let url = bookmark.1.clone();
            if response.clicked() {
                self.navigate_to(&url);
            }
            response.on_hover_text(&bookmark.1);
            x += width + 2.0;
        }

        let folder = Rect::from_center_size(
            pos2(rect.right() - 20.0, rect.center().y),
            Vec2::splat(28.0),
        );
        let response = ui.interact(folder, egui::Id::new("bookmarks_folder"), Sense::click());
        widgets::paint_icon_button(ui, p, folder, Icon::Folder, &response, true);
        if response.clicked() {
            self.dialog = Some(Dialog::Bookmarks);
        }
        response.on_hover_text("Bookmarks manager");
    }

    // ─────────────────────────── New tab page ───────────────────────────

    fn render_new_tab(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        widgets::gradient(ui.painter(), rect, p.ntp_top, p.ntp_bottom);
        let cx = rect.center().x;

        // Top-right links, as on Chrome's New Tab page: Gmail, Images, the app
        // grid and the profile avatar.
        {
            let row_y = rect.top() + 34.0;
            let initial = self
                .profile_name
                .chars()
                .next()
                .unwrap_or('R')
                .to_uppercase()
                .next()
                .unwrap_or('R');
            let avatar_centre = pos2(rect.right() - 44.0, row_y);
            let gmail = Rect::from_center_size(pos2(rect.right() - 214.0, row_y), vec2(54.0, 28.0));
            let images =
                Rect::from_center_size(pos2(rect.right() - 150.0, row_y), vec2(58.0, 28.0));
            let apps = Rect::from_center_size(pos2(rect.right() - 98.0, row_y), Vec2::splat(28.0));

            for (rect, label, url) in [
                (gmail, "Gmail", "https://mail.google.com"),
                (images, "Images", "https://images.google.com"),
            ] {
                let response = ui.interact(rect, egui::Id::new(("ntp_link", label)), Sense::click());
                widgets::state_layer(
                    ui.ctx(),
                    ui.painter(),
                    response.id,
                    rect,
                    6.0,
                    p.hover,
                    response.hovered(),
                    response.is_pointer_button_down_on(),
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    fonts::ui_font(13.0),
                    p.ntp_shortcut_text,
                );
                if response.clicked() {
                    self.navigate_to(url);
                }
            }

            let apps_response = ui.interact(apps, egui::Id::new("ntp_apps"), Sense::click());
            widgets::paint_icon_button(ui, p, apps, Icon::Apps, &apps_response, true);
            ui.painter()
                .circle_filled(avatar_centre, 15.0, Color32::from_rgb(0x8A, 0x66, 0x66));
            ui.painter().text(
                avatar_centre,
                Align2::CENTER_CENTER,
                initial,
                fonts::medium(13.0),
                Color32::WHITE,
            );
            let avatar = ui.interact(
                Rect::from_center_size(avatar_centre, Vec2::splat(32.0)),
                egui::Id::new("ntp_avatar"),
                Sense::click(),
            );
            if avatar.clicked() {
                self.profile_anchor =
                    Some(Rect::from_center_size(avatar_centre, Vec2::splat(32.0)));
                self.popup = if self.popup == Some(Popup::Profile) {
                    None
                } else {
                    Some(Popup::Profile)
                };
            }
        }

        // Google wordmark.
        let logo_size = (rect.height() * 0.098).clamp(44.0, 86.0);
        let logo_y = rect.top() + (rect.height() * 0.27).max(96.0);
        ui.painter().text(
            pos2(cx, logo_y),
            Align2::CENTER_CENTER,
            "Google",
            fonts::brand(logo_size),
            p.ntp_logo,
        );

        // Search pill.
        let search_w = (rect.width() * 0.49).clamp(380.0, 624.0);
        let search_h = 54.0;
        let search = Rect::from_center_size(
            pos2(cx, logo_y + logo_size * 0.62 + 52.0),
            vec2(search_w, search_h),
        );
        widgets::drop_shadow(ui.painter(), search, search_h / 2.0, 2.0, 10.0, p.shadow);
        widgets::fill_round(ui.painter(), search, search_h / 2.0, p.ntp_search_bg);
        widgets::stroke_round(
            ui.painter(),
            search,
            search_h / 2.0,
            p.ntp_search_stroke,
            1.0,
        );

        let lens_rect =
            Rect::from_center_size(pos2(search.right() - 26.0, search.center().y), Vec2::splat(32.0));
        let mic_rect =
            Rect::from_center_size(pos2(lens_rect.left() - 14.0, search.center().y), Vec2::splat(32.0));
        let plus_rect = Rect::from_center_size(
            pos2(search.left() + 26.0, search.center().y),
            Vec2::splat(28.0),
        );
        let edit_rect = Rect::from_min_max(
            pos2(plus_rect.right() + 4.0, search.center().y - 12.0),
            pos2(mic_rect.left() - 8.0, search.center().y + 12.0),
        );

        let plus_response = ui.interact(plus_rect, egui::Id::new("ntp_plus"), Sense::click());
        widgets::paint_icon_button(ui, p, plus_rect, Icon::Plus, &plus_response, true);
        if plus_response.clicked() {
            self.address_focus_requested = true;
        }

        let hint = "Ask Google";
        let response = ui.put(
            edit_rect,
            TextEdit::singleline(&mut self.ntp_query)
                .frame(false)
                .font(fonts::ui_font(15.0))
                .text_color(p.omnibox_text)
                .hint_text(hint)
                .hint_text_font(fonts::ui_font(15.0))
                .desired_width(edit_rect.width())
                .margin(egui::Margin::symmetric(0.0, 1.0)),
        );
        // A singleline TextEdit surrenders focus when Enter is pressed, so the
        // frame that submits no longer reports `has_focus`.
        let submitted = ui.input(|i| i.key_pressed(Key::Enter))
            && (response.has_focus() || response.lost_focus());
        if submitted {
            let value = self.ntp_query.trim().to_string();
            if !value.is_empty() {
                self.ntp_query.clear();
                self.navigate_to(&value);
                response.surrender_focus();
            }
        }

        for (rect, icon, key) in [
            (mic_rect, Icon::Mic, "ntp_mic"),
            (lens_rect, Icon::Lens, "ntp_lens"),
        ] {
            let r = ui.interact(rect, egui::Id::new(key), Sense::click());
            widgets::paint_icon_button(ui, p, rect, icon, &r, true);
        }
        // "Do more" cards.
        let card = vec2(200.0, 52.0);
        let gap = 12.0;
        let cards_y = search.bottom() + 26.0;
        let left_card = Rect::from_min_size(
            pos2(cx - card.x - gap / 2.0, cards_y),
            card,
        );
        let right_card = Rect::from_min_size(pos2(cx + gap / 2.0, cards_y), card);
        for (rect, icon, title, body, key) in [
            (
                left_card,
                Icon::Search,
                "Search the web",
                "Pages, images and answers",
                "ntp_card_search",
            ),
            (
                right_card,
                Icon::Apps,
                "Browse shortcuts",
                "Your bookmarks and history",
                "ntp_card_shortcuts",
            ),
        ] {
            let id = egui::Id::new(key);
            let r = ui.interact(rect, id, Sense::click());
            let fill = if r.hovered() {
                widgets::blend(p.ntp_card, Color32::WHITE, if p.is_dark { 0.06 } else { 0.0 })
            } else {
                p.ntp_card
            };
            widgets::fill_round(ui.painter(), rect, 14.0, fill);
            widgets::stroke_round(ui.painter(), rect, 14.0, p.ntp_card_stroke, 1.0);
            icons::paint(
                ui.painter(),
                Rect::from_center_size(
                    pos2(rect.left() + 22.0, rect.center().y),
                    Vec2::splat(20.0),
                ),
                icon,
                p.ntp_card_title,
            );
            ui.painter().text(
                pos2(rect.left() + 44.0, rect.center().y - 9.0),
                Align2::LEFT_CENTER,
                title,
                fonts::medium(13.0),
                p.ntp_card_title,
            );
            ui.painter().text(
                pos2(rect.left() + 44.0, rect.center().y + 9.0),
                Align2::LEFT_CENTER,
                body,
                fonts::ui_font(12.0),
                p.ntp_card_body,
            );
            if r.clicked() {
                let _ = key;
                self.address_focus_requested = true;
            }
        }

        // Shortcuts.
        let shortcuts = self.shortcuts();
        let slot = 84.0;
        let total = (shortcuts.len() as f32 + 1.0) * slot;
        let mut sx = cx - total / 2.0 + slot / 2.0;
        let circle_y = cards_y + card.y + 46.0;
        for shortcut in &shortcuts {
            let circle = Rect::from_center_size(pos2(sx, circle_y), Vec2::splat(42.0));
            let id = egui::Id::new(("shortcut", shortcut.url.clone()));
            let r = ui.interact(circle, id, Sense::click());
            let fill = if r.hovered() {
                widgets::blend(p.ntp_shortcut_bg, Color32::WHITE, if p.is_dark { 0.08 } else { 0.0 })
            } else {
                p.ntp_shortcut_bg
            };
            ui.painter().circle_filled(circle.center(), 21.0, fill);
            icons::paint(
                ui.painter(),
                Rect::from_center_size(circle.center(), Vec2::splat(20.0)),
                Icon::Globe,
                p.ntp_shortcut_text,
            );
            let label_rect = Rect::from_min_max(
                pos2(sx - slot / 2.0 + 4.0, circle.bottom() + 8.0),
                pos2(sx + slot / 2.0 - 4.0, circle.bottom() + 26.0),
            );
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(
                    pos2(label_rect.left(), label_rect.top()),
                    pos2(label_rect.right(), label_rect.bottom()),
                ),
                &shortcut.title,
                fonts::ui_font(11.5),
                p.ntp_shortcut_text,
            );
            let url = shortcut.url.clone();
            if r.clicked() {
                self.navigate_to(&url);
            }
            r.on_hover_text(&shortcut.url);
            sx += slot;
        }

        // "Add shortcut" tile.
        let add_circle = Rect::from_center_size(pos2(sx, circle_y), Vec2::splat(42.0));
        let add = ui.interact(add_circle, egui::Id::new("add_shortcut"), Sense::click());
        ui.painter()
            .circle_filled(add_circle.center(), 21.0, p.ntp_shortcut_bg);
        icons::paint(
            ui.painter(),
            Rect::from_center_size(add_circle.center(), Vec2::splat(20.0)),
            Icon::Plus,
            p.ntp_shortcut_text,
        );
        ui.painter().text(
            pos2(sx, add_circle.bottom() + 16.0),
            Align2::CENTER_CENTER,
            "Add shortcut",
            fonts::ui_font(11.5),
            p.ntp_shortcut_text,
        );
        if add.clicked() {
            self.dialog = Some(Dialog::Bookmarks);
        }

    }

    /// Recent sites for the New Tab shortcuts (bookmarks first, then history).
    ///
    /// Cached: reading `bookmarks.json` and `history.json` for every frame was
    /// a visible source of stutter on the New Tab page.
    fn shortcuts(&mut self) -> Vec<Shortcut> {
        self.refresh_bookmarks();
        if !self.shortcuts_dirty {
            return self.shortcuts_cache.clone();
        }
        let mut out: Vec<Shortcut> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut push =
            |title: String, url: String, seen: &mut Vec<String>, out: &mut Vec<Shortcut>| {
                if url.is_empty() || url == "about:blank" {
                    return;
                }
                let domain = extract_domain(&url);
                if seen.iter().any(|known| known == &domain) {
                    return;
                }
                seen.push(domain);
                let title = if title.trim().is_empty() {
                    extract_domain(&url)
                } else {
                    title
                };
                out.push(Shortcut { title, url });
            };

        for (title, url) in self.bookmarks_cache.iter().rev() {
            push(title.clone(), url.clone(), &mut seen, &mut out);
            if out.len() >= 6 {
                break;
            }
        }
        for entry in storage::history::list(&self.data_dir).into_iter().rev() {
            push(entry.title, entry.url, &mut seen, &mut out);
            if out.len() >= 6 {
                break;
            }
        }
        self.shortcuts_cache = out.clone();
        self.shortcuts_dirty = false;
        out
    }

    // ─────────────────────────── Menus / popups ───────────────────────────

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let vpn_label = if self.vpn_enabled {
            format!(
                "VPN · {}",
                if self.vpn_provider.is_empty() {
                    "On"
                } else {
                    self.vpn_provider.as_str()
                }
            )
        } else {
            "VPN".to_string()
        };
        vec![
            MenuEntry::Header("MyBrowser".to_string()),
            MenuEntry::Row {
                icon: Icon::Plus,
                label: "New tab".into(),
                shortcut: Some("Ctrl+T".into()),
                action: Action::NewTab,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Reload,
                label: "Reopen closed tab".into(),
                shortcut: Some("Ctrl+Shift+T".into()),
                action: Action::ReopenTab,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::NewWindow,
                label: "New window".into(),
                shortcut: Some("Ctrl+N".into()),
                action: Action::NewWindow,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Incognito,
                label: "New Incognito window".into(),
                shortcut: Some("Ctrl+Shift+N".into()),
                action: Action::Incognito,
                checked: None,
                danger: false,
            },
            MenuEntry::Divider,
            MenuEntry::Row {
                icon: Icon::History,
                label: "History".into(),
                shortcut: Some("Ctrl+H".into()),
                action: Action::History,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Download,
                label: "Downloads".into(),
                shortcut: Some("Ctrl+J".into()),
                action: Action::Downloads,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Star,
                label: "Bookmarks".into(),
                shortcut: Some("Ctrl+Shift+O".into()),
                action: Action::Bookmarks,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Key,
                label: "Passwords".into(),
                shortcut: None,
                action: Action::Passwords,
                checked: None,
                danger: false,
            },
            MenuEntry::Divider,
            MenuEntry::Row {
                icon: Icon::Shield,
                label: vpn_label,
                shortcut: Some(if self.vpn_enabled { "On".into() } else { "Off".into() }),
                action: Action::Vpn,
                checked: Some(self.vpn_enabled),
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Device,
                label: "Dummy system profile".into(),
                shortcut: Some(if self.dummy_config.enabled { "On".into() } else { "Off".into() }),
                action: Action::Dummy,
                checked: Some(self.dummy_config.enabled),
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::MapPin,
                label: "Location profile".into(),
                shortcut: Some(if self.location_config.is_some() { "Set".into() } else { "None".into() }),
                action: Action::Location,
                checked: None,
                danger: false,
            },
            MenuEntry::Divider,
            MenuEntry::Row {
                icon: Icon::Zoom,
                label: "Zoom".into(),
                shortcut: Some("100%".into()),
                action: Action::Zoom,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Print,
                label: "Print".into(),
                shortcut: Some("Ctrl+P".into()),
                action: Action::Print,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Search,
                label: "Find".into(),
                shortcut: Some("Ctrl+F".into()),
                action: Action::Find,
                checked: None,
                danger: false,
            },
            MenuEntry::Section("Appearance".into()),
            MenuEntry::Row {
                icon: Icon::Palette,
                label: "Light theme".into(),
                shortcut: None,
                action: Action::LightMode,
                checked: Some(self.mode == Mode::Light),
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Palette,
                label: "Dark theme".into(),
                shortcut: None,
                action: Action::DarkMode,
                checked: Some(self.mode == Mode::Dark),
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Bookmark,
                label: "Show bookmarks bar".into(),
                shortcut: Some("Ctrl+Shift+B".into()),
                action: Action::ToggleBookmarksBar,
                checked: Some(self.bookmarks_bar_visible),
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Pencil,
                label: "Customize appearance".into(),
                shortcut: None,
                action: Action::Customize,
                checked: None,
                danger: false,
            },
            MenuEntry::Divider,
            MenuEntry::Row {
                icon: Icon::Settings,
                label: "Settings".into(),
                shortcut: None,
                action: Action::Settings,
                checked: None,
                danger: false,
            },
            MenuEntry::Row {
                icon: Icon::Help,
                label: "Help".into(),
                shortcut: None,
                action: Action::Help,
                checked: None,
                danger: false,
            },
        ]
    }

    fn render_menu(&mut self, ctx: &egui::Context, p: &Palette) {
        let anchor = match self.menu_anchor {
            Some(a) => a,
            None => return,
        };
        let width = 300.0;
        let entries = self.menu_entries();
        let rows: f32 = entries.iter().map(|e| e.height()).sum();
        let height = rows + 10.0;
        let screen = ctx.screen_rect();
        // Anchor under the 3-dot button, but never let the menu run off the
        // bottom of the window.
        let pos = pos2(
            (anchor.right() - width).clamp(screen.left() + 8.0, screen.right() - width - 8.0),
            (anchor.bottom() + 6.0).min((screen.bottom() - height - 8.0).max(screen.top() + 8.0)),
        );
        let size = vec2(width, height);
        let mut action: Option<Action> = None;
        let mut clicked_inside = false;

        let area = egui::Area::new(egui::Id::new("chrome_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos + vec2(0.0, self.popup_slide))
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter();
                widgets::drop_shadow(painter, rect, 12.0, 6.0, 26.0, p.shadow);
                widgets::fill_round(painter, rect, 12.0, p.menu_bg);
                widgets::stroke_round(painter, rect, 12.0, p.menu_separator, 1.0);

                let mut y = rect.top() + 6.0;
                for (index, entry) in entries.iter().enumerate() {
                    match entry {
                        MenuEntry::Row {
                            icon,
                            label,
                            shortcut,
                            action: row_action,
                            checked,
                            danger,
                        } => {
                            let row = Rect::from_min_max(
                                pos2(rect.left() + 5.0, y),
                                pos2(rect.right() - 5.0, y + MENU_ROW),
                            );
                            let id = egui::Id::new(("menu_row", index));
                            let response = ui.interact(row, id, Sense::click());
                            widgets::state_layer(
                                ui.ctx(),
                                ui.painter(),
                                id,
                                row,
                                6.0,
                                p.hover,
                                response.hovered(),
                                response.is_pointer_button_down_on(),
                            );
                            let fg = if *danger { p.error } else { p.text };
                            icons::paint(
                                ui.painter(),
                                Rect::from_center_size(
                                    pos2(row.left() + 18.0, row.center().y),
                                    Vec2::splat(18.0),
                                ),
                                *icon,
                                fg,
                            );
                            ui.painter().text(
                                pos2(row.left() + 40.0, row.center().y),
                                Align2::LEFT_CENTER,
                                widgets::ellipsize(ctx, label, &fonts::ui_font(13.0), row.width() - 110.0),
                                fonts::ui_font(13.0),
                                fg,
                            );
                            if let Some(true) = checked {
                                icons::paint(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        pos2(row.right() - 22.0, row.center().y),
                                        Vec2::splat(15.0),
                                    ),
                                    Icon::Check,
                                    p.accent,
                                );
                            } else if let Some(shortcut) = shortcut {
                                ui.painter().text(
                                    pos2(row.right() - 10.0, row.center().y),
                                    Align2::RIGHT_CENTER,
                                    shortcut,
                                    fonts::ui_font(11.5),
                                    p.text_dim,
                                );
                            }
                            if response.clicked() {
                                action = Some(*row_action);
                                clicked_inside = true;
                            }
                            y += MENU_ROW;
                        }
                        MenuEntry::Divider => {
                            ui.painter().hline(
                                (rect.left() + 8.0)..=(rect.right() - 8.0),
                                y + MENU_DIVIDER / 2.0,
                                Stroke::new(1.0, p.menu_separator),
                            );
                            y += MENU_DIVIDER;
                        }
                        MenuEntry::Section(text) | MenuEntry::Header(text) => {
                            ui.painter().text(
                                pos2(rect.left() + 16.0, y + 13.0),
                                Align2::LEFT_CENTER,
                                text,
                                fonts::medium(11.5),
                                p.text_dim,
                            );
                            y += 26.0;
                        }
                    }
                }
                clicked_inside || ui.rect_contains_pointer(rect)
            });

        let inside = area.inner;
        if let Some(action) = action {
            self.apply_action(action);
            return;
        }
        // Skip the click that opened the popover: it is outside the popover and
        // would otherwise close it again in the same frame.
        if ctx.cumulative_pass_nr() != self.popup_frame
            && ctx.input(|i| i.pointer.any_click())
            && !inside
        {
            self.popup = None;
        }
    }

    fn render_tab_search(&mut self, ctx: &egui::Context, p: &Palette) {
        let anchor = self.tab_search_anchor.unwrap_or(ctx.screen_rect());
        let tabs: Vec<Tab> = self
            .tab_manager
            .tab_order()
            .iter()
            .filter_map(|id| self.tab_manager.get_tab(*id).cloned())
            .collect();
        let width = 320.0;
        let row_h = 36.0;
        let height = 34.0 + tabs.len().max(1) as f32 * row_h + 8.0;
        let pos = pos2(anchor.left(), anchor.bottom() + 6.0);
        let mut switch_to: Option<TabId> = None;
        let mut close: Option<TabId> = None;

        let area = egui::Area::new(egui::Id::new("tab_search_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos + vec2(0.0, self.popup_slide))
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 12.0, 6.0, 24.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 12.0, p.menu_bg);
                widgets::stroke_round(ui.painter(), rect, 12.0, p.menu_separator, 1.0);
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 17.0),
                    Align2::LEFT_CENTER,
                    "Open tabs",
                    fonts::medium(12.0),
                    p.text_dim,
                );
                let mut y = rect.top() + 34.0;
                for tab in &tabs {
                    let row = Rect::from_min_max(
                        pos2(rect.left() + 5.0, y),
                        pos2(rect.right() - 5.0, y + row_h),
                    );
                    let id = egui::Id::new(("tab_search_row", tab.id.as_u64()));
                    let response = ui.interact(row, id, Sense::click());
                    if self.tab_manager.active_tab() == Some(tab.id) {
                        widgets::fill_round(ui.painter(), row, 8.0, p.hover);
                    } else {
                        widgets::state_layer(
                            ui.ctx(),
                            ui.painter(),
                            id,
                            row,
                            8.0,
                            p.hover,
                            response.hovered(),
                            response.is_pointer_button_down_on(),
                        );
                    }
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(row.left() + 20.0, row.center().y),
                            Vec2::splat(15.0),
                        ),
                        Icon::Globe,
                        p.text_dim,
                    );
                    widgets::text_clipped(
                        ui.painter(),
                        ctx,
                        Rect::from_min_max(
                            pos2(row.left() + 36.0, row.top()),
                            pos2(row.right() - 34.0, row.bottom()),
                        ),
                        tab.display_title(),
                        fonts::ui_font(12.5),
                        p.text,
                    );
                    let close_rect = Rect::from_center_size(
                        pos2(row.right() - 18.0, row.center().y),
                        Vec2::splat(22.0),
                    );
                    let close_response = ui.interact(
                        close_rect,
                        egui::Id::new(("tab_search_close", tab.id.as_u64())),
                        Sense::click(),
                    );
                    if close_response.clicked() {
                        close = Some(tab.id);
                    }
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(close_rect.center(), Vec2::splat(12.0)),
                        Icon::Close,
                        if close_response.hovered() { p.text } else { p.text_dim },
                    );
                    if response.clicked() {
                        switch_to = Some(tab.id);
                    }
                    y += row_h;
                }
                if tabs.is_empty() {
                    ui.painter().text(
                        pos2(rect.center().x, rect.top() + 56.0),
                        Align2::CENTER_CENTER,
                        "No open tabs",
                        fonts::ui_font(12.0),
                        p.text_dim,
                    );
                }
                ui.rect_contains_pointer(rect)
            });

        if let Some(id) = switch_to {
            self.tab_manager.switch_to_tab(id);
            self.sync_active_address();
            self.viewport.dirty = true;
            self.popup = None;
        }
        if let Some(id) = close {
            self.close_tab(id);
        }
        if ctx.input(|i| i.pointer.any_click()) && !area.inner && self.popup.is_some() {
            if self.popup == Some(Popup::TabSearch) && !area.inner {
                self.popup = None;
            }
        }
    }

    fn render_extensions(&mut self, ctx: &egui::Context, p: &Palette) {
        let anchor = match self.appearance_anchor.or(self.menu_anchor) {
            Some(a) => a,
            None => return,
        };
        let width = 312.0;
        let height = 132.0;
        let screen = ctx.screen_rect();
        let pos = pos2(
            (anchor.right() - width).clamp(screen.left() + 8.0, screen.right() - width - 8.0),
            anchor.bottom() + 6.0,
        );
        let area = egui::Area::new(egui::Id::new("extensions_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos + vec2(0.0, self.popup_slide))
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 12.0, 6.0, 24.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 12.0, p.menu_bg);
                widgets::stroke_round(ui.painter(), rect, 12.0, p.menu_separator, 1.0);
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 22.0),
                    Align2::LEFT_CENTER,
                    "Extensions",
                    fonts::medium(13.0),
                    p.text,
                );
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 54.0),
                    Align2::LEFT_CENTER,
                    "No extensions loaded in this profile.",
                    fonts::ui_font(12.0),
                    p.text_dim,
                );
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 78.0),
                    Align2::LEFT_CENTER,
                    "Manifest V3 extensions are managed per profile.",
                    fonts::ui_font(11.0),
                    p.text_dim,
                );
                ui.rect_contains_pointer(rect)
            });
        if ctx.cumulative_pass_nr() != self.popup_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area.inner
        {
            self.popup = None;
        }
    }

    fn render_appearance(&mut self, ctx: &egui::Context, p: &Palette) {
        let anchor = match self.appearance_anchor {
            Some(a) => a,
            None => return,
        };
        let width = 260.0;
        let height = 150.0;
        let pos = pos2(anchor.right() - width, anchor.top() - height - 8.0);
        let mut action: Option<Action> = None;

        let area = egui::Area::new(egui::Id::new("appearance_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos + vec2(0.0, self.popup_slide))
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 14.0, 6.0, 24.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 14.0, p.menu_bg);
                widgets::stroke_round(ui.painter(), rect, 14.0, p.menu_separator, 1.0);
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 20.0),
                    Align2::LEFT_CENTER,
                    "Appearance",
                    fonts::medium(13.0),
                    p.text,
                );
                let mut y = rect.top() + 38.0;
                for (icon, label, row_action) in [
                    (Icon::Palette, "Light theme", Action::LightMode),
                    (Icon::Palette, "Dark theme", Action::DarkMode),
                    (Icon::Bookmark, "Show bookmarks bar", Action::ToggleBookmarksBar),
                ] {
                    let row = Rect::from_min_max(
                        pos2(rect.left() + 6.0, y),
                        pos2(rect.right() - 6.0, y + 32.0),
                    );
                    let id = egui::Id::new(("appearance_row", label));
                    let response = ui.interact(row, id, Sense::click());
                    widgets::state_layer(
                        ui.ctx(),
                        ui.painter(),
                        id,
                        row,
                        8.0,
                        p.hover,
                        response.hovered(),
                        response.is_pointer_button_down_on(),
                    );
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(row.left() + 18.0, row.center().y),
                            Vec2::splat(16.0),
                        ),
                        icon,
                        p.icon,
                    );
                    ui.painter().text(
                        pos2(row.left() + 36.0, row.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        fonts::ui_font(12.5),
                        p.text,
                    );
                    let active = match row_action {
                        Action::LightMode => self.mode == Mode::Light,
                        Action::DarkMode => self.mode == Mode::Dark,
                        Action::ToggleBookmarksBar => self.bookmarks_bar_visible,
                        _ => false,
                    };
                    if active {
                        icons::paint(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(row.right() - 18.0, row.center().y),
                                Vec2::splat(14.0),
                            ),
                            Icon::Check,
                            p.accent,
                        );
                    }
                    if response.clicked() {
                        action = Some(row_action);
                    }
                    y += 34.0;
                }
                ui.rect_contains_pointer(rect)
            });

        if let Some(action) = action {
            self.apply_action(action);
            return;
        }
        if ctx.input(|i| i.pointer.any_click()) && !area.inner {
            self.popup = None;
        }
    }

    // ─────────────────────────── Dialogs ───────────────────────────

    fn render_dialog(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(dialog) = self.dialog else { return };
        let screen = ctx.screen_rect();

        let size = vec2(
            (screen.width() * 0.46).clamp(420.0, 640.0),
            (screen.height() * 0.62).clamp(320.0, 560.0),
        );
        let pos = Pos2::new(
            screen.center().x - size.x / 2.0,
            screen.center().y - size.y / 2.0,
        );
        let card_rect = Rect::from_min_size(pos, size);

        // Clicking the scrim (or pressing Escape) closes the dialog.
        let pointer = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO);
        let clicked_outside = ctx.cumulative_pass_nr() != self.dialog_frame
            && ctx.input(|i| i.pointer.any_click())
            && !card_rect.contains(pointer);
        if clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.dialog = None;
            return;
        }

        egui::Area::new(egui::Id::new("dialog_scrim"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    if p.is_dark {
                        Color32::from_black_alpha(140)
                    } else {
                        Color32::from_black_alpha(70)
                    },
                );
            });

        let title = match dialog {
            Dialog::History => "History",
            Dialog::Downloads => "Downloads",
            Dialog::Bookmarks => "Bookmarks",
            Dialog::Passwords => "Passwords",
        };

        let mut close = false;
        egui::Area::new(egui::Id::new("dialog_card"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 16.0, 8.0, 32.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 16.0, p.menu_bg);
                ui.painter().text(
                    pos2(rect.left() + 24.0, rect.top() + 30.0),
                    Align2::LEFT_CENTER,
                    title,
                    fonts::medium(16.0),
                    p.text,
                );
                let close_rect =
                    Rect::from_center_size(pos2(rect.right() - 26.0, rect.top() + 30.0), Vec2::splat(28.0));
                let id = egui::Id::new("dialog_close");
                let response = ui.interact(close_rect, id, Sense::click());
                widgets::paint_icon_button(ui, p, close_rect, Icon::Close, &response, true);
                if response.clicked() {
                    close = true;
                }
                ui.painter().hline(
                    (rect.left() + 12.0)..=(rect.right() - 12.0),
                    rect.top() + 52.0,
                    Stroke::new(1.0, p.menu_separator),
                );

                let body = Rect::from_min_max(
                    pos2(rect.left() + 12.0, rect.top() + 58.0),
                    pos2(rect.right() - 12.0, rect.bottom() - 12.0),
                );
                self.render_dialog_body(ui, p, dialog, body);
            });

        if close {
            self.dialog = None;
        }
    }

    fn render_dialog_body(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        dialog: Dialog,
        body: Rect,
    ) {
        let mut y = body.top();
        let row_h = 44.0;
        let mut navigate: Option<String> = None;

        let rows: Vec<(String, String)> = match dialog {
            Dialog::History => storage::history::list(&self.data_dir)
                .into_iter()
                .rev()
                .take(40)
                .map(|e| {
                    let title = if e.title.is_empty() {
                        e.url.clone()
                    } else {
                        e.title.clone()
                    };
                    (title, e.url)
                })
                .collect(),
            Dialog::Downloads => storage::download::list(&self.data_dir)
                .into_iter()
                .rev()
                .take(40)
                .map(|d| {
                    (
                        d.suggested_filename,
                        format!("{} bytes · {:?}", d.downloaded_bytes, d.state),
                    )
                })
                .collect(),
            Dialog::Bookmarks => storage::bookmarks::list(&self.data_dir)
                .into_iter()
                .map(|b| (b.title, b.url))
                .collect(),
            Dialog::Passwords => vec![(
                "Per-profile password vault".to_string(),
                "Credentials are encrypted at rest".to_string(),
            )],
        };

        if rows.is_empty() {
            ui.painter().text(
                body.center(),
                Align2::CENTER_CENTER,
                match dialog {
                    Dialog::History => "No browsing history yet",
                    Dialog::Downloads => "No downloads yet",
                    Dialog::Bookmarks => "No bookmarks yet",
                    Dialog::Passwords => "No saved passwords",
                },
                fonts::ui_font(13.0),
                p.text_dim,
            );
            return;
        }

        // Clip the row list to the dialog body, the way Chrome's list scrolls.
        let painter = ui.painter_at(body);
        for (index, (title, subtitle)) in rows.iter().enumerate() {
            if y + row_h > body.bottom() {
                break;
            }
            let row = Rect::from_min_max(
                pos2(body.left() + 6.0, y),
                pos2(body.right() - 6.0, y + row_h),
            );
            let id = egui::Id::new(("dialog_row", index));
            let response = ui.interact(row, id, Sense::click());
            widgets::state_layer(
                ui.ctx(),
                &painter,
                id,
                row,
                8.0,
                p.hover,
                response.hovered(),
                response.is_pointer_button_down_on(),
            );
            icons::paint(
                &painter,
                Rect::from_center_size(pos2(row.left() + 18.0, row.center().y), Vec2::splat(16.0)),
                match dialog {
                    Dialog::History => Icon::History,
                    Dialog::Downloads => Icon::Download,
                    Dialog::Bookmarks => Icon::Star,
                    Dialog::Passwords => Icon::Key,
                },
                p.icon,
            );
            widgets::text_clipped(
                &painter,
                ui.ctx(),
                Rect::from_min_max(
                    pos2(row.left() + 38.0, row.top() + 7.0),
                    pos2(row.right() - 8.0, row.top() + 25.0),
                ),
                title,
                fonts::ui_font(12.5),
                p.text,
            );
            widgets::text_clipped(
                &painter,
                ui.ctx(),
                Rect::from_min_max(
                    pos2(row.left() + 38.0, row.top() + 23.0),
                    pos2(row.right() - 8.0, row.bottom() - 4.0),
                ),
                subtitle,
                fonts::ui_font(11.0),
                p.text_dim,
            );
            if response.clicked() && matches!(dialog, Dialog::History | Dialog::Bookmarks) {
                navigate = Some(subtitle.clone());
            }
            y += row_h;
        }

        if let Some(url) = navigate {
            self.navigate_to(&url);
            self.dialog = None;
        }
    }

    // ─────────────────────────── Side panels ───────────────────────────

    fn render_dummy_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Dummy system profile")
                .font(fonts::medium(15.0))
                .size(15.0),
        );
        ui.add_space(10.0);
        let action = self.dummy_ui.render(ui, &self.dummy_config);
        self.handle_dummy_action(action);
    }

    fn render_location_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Location profile")
                .font(fonts::medium(15.0))
                .size(15.0),
        );
        ui.add_space(10.0);
        let action = self.location_ui.render(ui, &self.location_config);
        self.handle_location_action(action);
    }

    // ─────────────────────────── Actions ───────────────────────────

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::NewTab => self.open_new_tab(),
            Action::ReopenTab => self.restore_closed_tab(),
            Action::History => {
                self.dialog = Some(Dialog::History);
                self.popup = None;
            }
            Action::Downloads => {
                self.dialog = Some(Dialog::Downloads);
                self.popup = None;
            }
            Action::Bookmarks => {
                self.dialog = Some(Dialog::Bookmarks);
                self.popup = None;
            }
            Action::Passwords => {
                self.dialog = Some(Dialog::Passwords);
                self.popup = None;
            }
            Action::Vpn => {
                self.vpn_enabled = !self.vpn_enabled;
                if self.vpn_provider.is_empty() {
                    self.vpn_provider = "MyBrowser VPN".to_string();
                }
                ProxyManager::set_vpn(
                    &self.profile_id,
                    self.vpn_enabled,
                    &self.vpn_provider,
                    &self.vpn_endpoint,
                );
                let (enabled, provider, endpoint, connected, message) =
                    ProxyManager::vpn_status(&self.profile_id);
                self.vpn_enabled = enabled;
                self.vpn_provider = provider;
                self.vpn_endpoint = endpoint;
                self.vpn_message = message;
                let _ = connected;
            }
            Action::Dummy => {
                self.side_panel = Some(SidePanel::Dummy);
                self.popup = None;
            }
            Action::Location => {
                self.side_panel = Some(SidePanel::Location);
                self.popup = None;
            }
            Action::LightMode => {
                self.set_mode(Mode::Light);
                self.popup = None;
            }
            Action::DarkMode => {
                self.set_mode(Mode::Dark);
                self.popup = None;
            }
            Action::ToggleBookmarksBar => {
                self.bookmarks_bar_visible = !self.bookmarks_bar_visible;
                self.prefs.bookmarks_bar = self.bookmarks_bar_visible;
                self.save_prefs();
            }
            Action::Settings => {
                self.settings = Some(SettingsSection::You);
                self.side_panel = None;
                self.popup = None;
                self.dialog = None;
            }
            Action::Customize => {
                self.popup = Some(Popup::Appearance);
                self.dialog = None;
                self.last_popup = None;
            }
            Action::Find => {
                self.address_focus_requested = true;
                self.popup = None;
            }
            Action::Zoom | Action::Print | Action::Help | Action::NewWindow
            | Action::Incognito => {
                self.popup = None;
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let keys = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            let shift = i.modifiers.shift;
            (
                ctrl && i.key_pressed(Key::T),
                ctrl && i.key_pressed(Key::W),
                ctrl && i.key_pressed(Key::R),
                ctrl && i.key_pressed(Key::L),
                ctrl && i.key_pressed(Key::H),
                ctrl && i.key_pressed(Key::J),
                ctrl && shift && i.key_pressed(Key::T),
                ctrl && shift && i.key_pressed(Key::Tab),
                ctrl && !shift && i.key_pressed(Key::Tab),
                i.modifiers.alt && i.key_pressed(Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Escape),
                ctrl && shift && i.key_pressed(Key::B),
            )
        });
        if keys.0 {
            self.open_new_tab();
        }
        if keys.1 {
            self.close_active_tab();
        }
        if keys.2 {
            self.viewport.texture = None;
            self.viewport.dirty = true;
        }
        if keys.3 {
            self.address_bar = self.active_url();
            self.address_focus_requested = true;
        }
        if keys.4 {
            self.dialog = Some(Dialog::History);
        }
        if keys.5 {
            self.dialog = Some(Dialog::Downloads);
        }
        if keys.6 {
            self.restore_closed_tab();
        }
        if keys.7 {
            self.switch_previous_tab();
        }
        if keys.8 {
            self.switch_next_tab();
        }
        if keys.9 {
            self.navigate_history(-1);
        }
        if keys.10 {
            self.navigate_history(1);
        }
        if keys.11 {
            self.popup = None;
            self.dialog = None;
        }
        if keys.12 {
            self.bookmarks_bar_visible = !self.bookmarks_bar_visible;
        }
    }

    fn is_new_tab_page(&self) -> bool {
        match self.tab_manager.active_tab() {
            Some(id) => self
                .tab_manager
                .get_tab(id)
                .map(|tab| tab.url == "about:blank" || tab.url.is_empty())
                .unwrap_or(true),
            None => true,
        }
    }

    fn open_new_tab(&mut self) {
        let id = self
            .tab_manager
            .open_tab("about:blank".to_string(), "New Tab".to_string());
        self.navigation.insert(
            id,
            NavigationHistory {
                entries: vec!["about:blank".to_string()],
                index: 0,
            },
        );
        self.address_bar.clear();
        self.ntp_query.clear();
        self.viewport.texture = None;
        self.viewport.dirty = true;
        self.address_focus_requested = true;
    }

    fn navigate_to(&mut self, input: &str) {
        let url = resolve_address(input, &self.prefs.engine);
        let id = match self.tab_manager.active_tab() {
            Some(id) => id,
            None => {
                self.open_new_tab();
                match self.tab_manager.active_tab() {
                    Some(id) => id,
                    None => return,
                }
            }
        };

        let history = self.navigation.entry(id).or_default();
        if history.entries.is_empty() {
            history.entries.push("about:blank".to_string());
            history.index = 0;
        }
        if history.entries.get(history.index) != Some(&url) {
            history.entries.truncate(history.index.saturating_add(1));
            history.entries.push(url.clone());
            history.index = history.entries.len() - 1;
        }

        if let Some(tab) = self.tab_manager.get_tab_mut(id) {
            tab.url = url.clone();
            tab.title = "Loading…".to_string();
            tab.state = TabState::Loading;
            tab.load_progress = 0.0;
        }
        self.address_bar = url.clone();
        self.viewport.texture = None;
        self.viewport.dirty = true;
        let _ = storage::history::add_visit(&self.data_dir, &url, "");
        self.shortcuts_dirty = true;
    }

    fn navigate_history(&mut self, direction: i32) {
        let Some(id) = self.tab_manager.active_tab() else {
            return;
        };
        let Some(history) = self.navigation.get_mut(&id) else {
            return;
        };
        if direction < 0 && history.index > 0 {
            history.index -= 1;
        } else if direction > 0 && history.index + 1 < history.entries.len() {
            history.index += 1;
        } else {
            return;
        }
        let url = history.entries[history.index].clone();
        if let Some(tab) = self.tab_manager.get_tab_mut(id) {
            tab.url = url.clone();
            tab.title = if url == "about:blank" {
                "New Tab".to_string()
            } else {
                "Loading…".to_string()
            };
            tab.state = if url == "about:blank" {
                TabState::Active
            } else {
                TabState::Loading
            };
        }
        self.address_bar = if url == "about:blank" {
            String::new()
        } else {
            url
        };
        self.viewport.texture = None;
        self.viewport.dirty = true;
    }

    fn navigation_availability(&self) -> (bool, bool) {
        let Some(id) = self.tab_manager.active_tab() else {
            return (false, false);
        };
        let Some(history) = self.navigation.get(&id) else {
            return (false, false);
        };
        (
            history.index > 0,
            history.index + 1 < history.entries.len(),
        )
    }

    fn close_tab(&mut self, id: TabId) {
        let active_before_close = self.tab_manager.active_tab();
        self.navigation.remove(&id);
        self.pages.remove(&id);
        self.searches.remove(&id);
        self.tab_manager.close_tab(id);
        if let Some(active_id) = active_before_close {
            if active_id != id && self.tab_manager.get_tab(active_id).is_some() {
                self.tab_manager.switch_to_tab(active_id);
            }
        }
        if self.tab_manager.active_tab().is_none() {
            self.open_new_tab();
        } else {
            self.sync_active_address();
            self.viewport.texture = None;
            self.viewport.dirty = true;
        }
    }

    fn close_active_tab(&mut self) {
        if let Some(id) = self.tab_manager.active_tab() {
            self.close_tab(id);
        }
    }

    fn restore_closed_tab(&mut self) {
        if let Some(id) = self.tab_manager.restore_closed_tab() {
            if let Some(tab) = self.tab_manager.get_tab(id) {
                let url = tab.url.clone();
                self.navigation.insert(
                    id,
                    NavigationHistory {
                        entries: vec![url],
                        index: 0,
                    },
                );
            }
            self.sync_active_address();
            self.viewport.texture = None;
            self.viewport.dirty = true;
        }
    }

    fn switch_previous_tab(&mut self) {
        if self.tab_manager.prev_tab().is_some() {
            self.sync_active_address();
            self.viewport.texture = None;
            self.viewport.dirty = true;
        }
    }

    fn switch_next_tab(&mut self) {
        if self.tab_manager.next_tab().is_some() {
            self.sync_active_address();
            self.viewport.texture = None;
            self.viewport.dirty = true;
        }
    }

    fn sync_active_address(&mut self) {
        self.address_bar = self
            .tab_manager
            .active_tab()
            .and_then(|id| self.tab_manager.get_tab(id))
            .map(|tab| {
                if tab.url == "about:blank" {
                    String::new()
                } else {
                    tab.url.clone()
                }
            })
            .unwrap_or_default();
    }

    fn active_url(&self) -> String {
        self.tab_manager
            .active_tab()
            .and_then(|id| self.tab_manager.get_tab(id))
            .map(|tab| tab.url.clone())
            .unwrap_or_default()
    }

    fn is_active_bookmarked(&mut self) -> bool {
        let url = self.active_url();
        if url.is_empty() || url == "about:blank" {
            return false;
        }
        self.bookmarks()
            .iter()
            .any(|(_, bookmark_url)| bookmark_url == &url)
    }

    fn toggle_bookmark(&mut self) {
        let url = self.active_url();
        if url.is_empty() || url == "about:blank" {
            return;
        }
        let existing = storage::bookmarks::list(&self.data_dir)
            .into_iter()
            .find(|bookmark| bookmark.url == url);
        if let Some(bookmark) = existing {
            let _ = storage::bookmarks::remove(&self.data_dir, &bookmark.id);
        } else {
            let title = self
                .tab_manager
                .active_tab()
                .and_then(|id| self.tab_manager.get_tab(id))
                .map(|tab| tab.title.clone())
                .unwrap_or_else(|| extract_domain(&url));
            let _ = storage::bookmarks::add(&self.data_dir, &url, &title, "Bookmarks bar");
        }
        self.bookmarks_dirty = true;
        self.shortcuts_dirty = true;
    }

    fn handle_location_action(&mut self, action: LocationUIAction) {
        match action {
            LocationUIAction::None => {}
            LocationUIAction::SetLocation(loc) => {
                self.location_config = Some(loc.clone());
                set_location(&self.profile_id, loc);
            }
            LocationUIAction::ClearLocation => {
                self.location_config = None;
                remove_location(&self.profile_id);
            }
        }
    }

    fn handle_dummy_action(&mut self, action: DummyProfileUIAction) {
        match action {
            DummyProfileUIAction::None => {}
            DummyProfileUIAction::EnableDummyMode => {
                self.dummy_config.enabled = true;
                set_config(&self.profile_id, self.dummy_config.clone());
            }
            DummyProfileUIAction::DisableDummyMode => {
                self.dummy_config.enabled = false;
                set_config(&self.profile_id, self.dummy_config.clone());
            }
            DummyProfileUIAction::SelectDevice(device) => {
                self.dummy_config.device = Some(device);
                self.dummy_config.enabled = true;
                set_config(&self.profile_id, self.dummy_config.clone());
            }
            DummyProfileUIAction::ApplyDummyConfig(config) => {
                self.dummy_config = config;
                set_config(&self.profile_id, self.dummy_config.clone());
            }
        }
    }

}

fn resolve_address(input: &str, engine: &str) -> String {
    let value = input.trim();
    if value.is_empty() {
        return "about:blank".to_string();
    }
    if value.starts_with("about:")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("file://")
    {
        return value.to_string();
    }
    let local =
        value.starts_with("localhost") || value.starts_with("127.0.0.1") || value.starts_with("[::1]");
    let looks_like_url =
        value.contains('.') || value.contains(':') || local || value.starts_with('/');
    if looks_like_url {
        if local || value.starts_with('/') {
            format!("http://{}", value.trim_start_matches('/'))
        } else {
            format!("https://{}", value)
        }
    } else {
        let query = encode_query(value);
        match engine {
            "duckduckgo" => format!("https://duckduckgo.com/?q={}", query),
            "bing" => format!("https://www.bing.com/search?q={}", query),
            _ => format!("https://www.google.com/search?q={}", query),
        }
    }
}

fn encode_query(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{:02X}", byte));
        }
    }
    output
}

fn extract_page_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open = lower[start..].find('>')? + start + 1;
    let end = lower[open..].find("</title>")? + open;
    let title = html[open..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn extract_domain(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped
        .split('/')
        .next()
        .unwrap_or(url)
        .trim_start_matches("www.")
        .to_string()
}
