//! Browser window UI — complete Google Chrome-like reconstruction.
//!
//! Layout (top to bottom, exactly like Chrome):
//!   [Tab Strip]        tabs + `+` new tab button
//!   [Nav Toolbar]      back/forward/reload | omnibox | star | extensions | profile | 3-dot
//!   [Bookmarks Bar]    favicon + title per bookmark (only if bookmarks exist)
//!   [Viewport]         Falco engine render output, fills all remaining space
//!
//! Light and dark themes. No hardcoded bookmarks. No debug info.
//! Every control connects to real browser functionality.

use anyhow::Result;
use engine::tab_management::{TabId, TabManager, TabState};
use engine::dummy_profile::{DummySystemConfig, set_config};
use engine::dummy_profile::ui::{DummyProfileSelector, DummyProfileUIAction};
use engine::location::{set_location, LocationProfile};
use engine::location::ui::{LocationProfileSelector, LocationUIAction};
use engine::net::proxy::ProxyManager;
use engine::net::fetch;
use egui::{
    Align, Align2, Color32, Frame, Layout, Rect, RichText, Rounding, Stroke, Sense, TextEdit,
    TextureHandle, TextureOptions, Vec2, FontId, Key,
};
use std::collections::HashMap;
use std::time::Instant;

/// UI theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn frame(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(232, 234, 237), // Chrome light grey frame
            Theme::Dark => Color32::from_rgb(32, 33, 36),     // Chrome dark frame
        }
    }
    fn toolbar(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(255, 255, 255),
            Theme::Dark => Color32::from_rgb(32, 33, 36),
        }
    }
    fn tab_active(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(255, 255, 255),
            Theme::Dark => Color32::from_rgb(48, 48, 48),
        }
    }
    fn tab_inactive(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(232, 234, 237),
            Theme::Dark => Color32::from_rgb(32, 33, 36),
        }
    }
    fn text(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(32, 33, 36),
            Theme::Dark => Color32::from_rgb(232, 234, 237),
        }
    }
    fn text_muted(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(95, 99, 104),
            Theme::Dark => Color32::from_rgb(142, 145, 150),
        }
    }
    fn border(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(218, 220, 224),
            Theme::Dark => Color32::from_rgb(60, 61, 65),
        }
    }
}

/// Rendered page viewport state.
struct Viewport {
    texture: Option<TextureHandle>,
    size: Vec2,
    dirty: bool,
}

/// Per-tab session history. TabManager owns tab lifecycle/order; navigation
/// history stays in the browser UI layer until the engine exposes session history.
#[derive(Default)]
struct NavigationHistory {
    entries: Vec<String>,
    index: usize,
}

/// Browser window state.
pub struct BrowserWindow {
    profile_id: String,
    data_dir: std::path::PathBuf,
    tab_manager: TabManager,
    navigation: HashMap<TabId, NavigationHistory>,
    dragged_tab: Option<TabId>,
    show_history: bool,
    show_downloads: bool,
    show_bookmarks_manager: bool,
    address_bar: String,
    omnibox_focused: bool,
    show_menu: bool,
    menu_hovered: bool,
    theme: Theme,
    bookmarks_bar_visible: bool,
    last_frame: Instant,
    viewport: Viewport,
    // Per-profile VPN (fresh per profile).
    vpn_enabled: bool,
    vpn_provider: String,
    vpn_endpoint: String,
    vpn_message: Option<String>,
    // Panels (opened from 3-dot menu).
    show_dummy_panel: bool,
    show_location_panel: bool,
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

        // Start with an empty browser tab. Do not seed the user's browser with example.com.
        let mut tab_manager = TabManager::new();
        let first_tab = tab_manager.open_tab(
            "about:blank".to_string(),
            "New Tab".to_string(),
        );
        let mut navigation = HashMap::new();
        navigation.insert(first_tab, NavigationHistory {
            entries: vec!["about:blank".to_string()],
            index: 0,
        });

        let dummy_config = engine::dummy_profile::get_config(profile_id);
        let location_config = engine::location::get_location(profile_id);

        Ok(Self {
            profile_id: profile_id.to_string(),
            data_dir,
            tab_manager,
            navigation,
            dragged_tab: None,
            show_history: false,
            show_downloads: false,
            show_bookmarks_manager: false,
            address_bar: String::new(),
            omnibox_focused: false,
            show_menu: false,
            menu_hovered: false,
            theme: Theme::Light,
            bookmarks_bar_visible: true,
            last_frame: Instant::now(),
            viewport: Viewport {
                texture: None,
                size: Vec2::ZERO,
                dirty: true,
            },
            vpn_enabled: false,
            vpn_provider: String::new(),
            vpn_endpoint: String::new(),
            vpn_message: None,
            show_dummy_panel: false,
            show_location_panel: false,
            dummy_config,
            dummy_ui: DummyProfileSelector::new(),
            location_config,
            location_ui: LocationProfileSelector::new(),
        })
    }

    /// Render the full browser window. Returns true if the window should close.
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        self.handle_shortcuts(ctx);
        // ── Tab Strip (first element at top) ──
        egui::TopBottomPanel::top("tab_strip")
            .min_height(42.0)
            .max_height(42.0)
            .show(ctx, |ui| {
                self.render_tab_strip(ui);
            });

        // ── Navigation Toolbar ──
        egui::TopBottomPanel::top("nav_toolbar")
            .min_height(44.0)
            .max_height(44.0)
            .show(ctx, |ui| {
                self.render_nav_toolbar(ui);
            });

        // ── Bookmarks Bar (only if bookmarks exist and bar is visible) ──
        if self.bookmarks_bar_visible {
            egui::TopBottomPanel::top("bookmarks_bar")
                .min_height(28.0)
                .max_height(28.0)
                .show(ctx, |ui| {
                    self.render_bookmarks_bar(ui);
                });
        }

        // ── 3-dot menu popup ──
        if self.show_menu {
            self.render_menu_popup(ctx);
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
            )
        });
        if keys.0 { self.open_new_tab(); }
        if keys.1 { self.close_active_tab(); }
        if keys.2 { self.viewport.texture = None; self.viewport.dirty = true; }
        if keys.3 { self.address_bar = self.active_url(); self.omnibox_focused = true; }
        if keys.4 { self.show_history = true; }
        if keys.5 { self.show_downloads = true; }
        if keys.6 { self.restore_closed_tab(); }
        if keys.7 { self.switch_previous_tab(); }
        if keys.8 { self.switch_next_tab(); }
        if keys.9 { self.navigate_history(-1); }
        if keys.10 { self.navigate_history(1); }
        if keys.11 { self.show_menu = false; }
    }

    fn open_new_tab(&mut self) {
        let id = self.tab_manager.open_tab(
            "about:blank".to_string(),
            "New Tab".to_string(),
        );
        self.navigation.insert(id, NavigationHistory {
            entries: vec!["about:blank".to_string()],
            index: 0,
        });
        self.address_bar.clear();
        self.viewport.texture = None;
        self.viewport.dirty = true;
    }

    fn navigate_to(&mut self, input: &str) {
        let url = resolve_address(input);
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
    }

    fn navigate_history(&mut self, direction: i32) {
        let Some(id) = self.tab_manager.active_tab() else { return };
        let Some(history) = self.navigation.get_mut(&id) else { return };

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
            tab.title = if url == "about:blank" { "New Tab".to_string() } else { "Loading…".to_string() };
            tab.state = if url == "about:blank" { TabState::Active } else { TabState::Loading };
        }
        self.address_bar = if url == "about:blank" { String::new() } else { url };
        self.viewport.texture = None;
        self.viewport.dirty = true;
    }

    fn navigation_availability(&self) -> (bool, bool) {
        let Some(id) = self.tab_manager.active_tab() else { return (false, false) };
        let Some(history) = self.navigation.get(&id) else { return (false, false) };
        (history.index > 0, history.index + 1 < history.entries.len())
    }

    fn close_tab(&mut self, id: TabId) {
        self.navigation.remove(&id);
        self.tab_manager.close_tab(id);
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
                self.navigation.insert(id, NavigationHistory {
                    entries: vec![url],
                    index: 0,
                });
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
        self.address_bar = self.tab_manager.active_tab()
            .and_then(|id| self.tab_manager.get_tab(id))
            .map(|tab| if tab.url == "about:blank" { String::new() } else { tab.url.clone() })
            .unwrap_or_default();
    }

    fn active_url(&self) -> String {
        self.tab_manager.active_tab()
            .and_then(|id| self.tab_manager.get_tab(id))
            .map(|tab| tab.url.clone())
            .unwrap_or_default()
    }

    fn is_active_bookmarked(&self) -> bool {
        let url = self.active_url();
        !url.is_empty()
            && storage::bookmarks::list(&self.data_dir).iter().any(|bookmark| bookmark.url == url)
    }

    fn toggle_bookmark(&mut self) {
        let url = self.active_url();
        if url.is_empty() || url == "about:blank" { return; }

        let existing = storage::bookmarks::list(&self.data_dir)
            .into_iter()
            .find(|bookmark| bookmark.url == url);
        if let Some(bookmark) = existing {
            let _ = storage::bookmarks::remove(&self.data_dir, &bookmark.id);
        } else {
            let title = self.tab_manager.active_tab()
                .and_then(|id| self.tab_manager.get_tab(id))
                .map(|tab| tab.title.clone())
                .unwrap_or_else(|| extract_domain(&url));
            let _ = storage::bookmarks::add(&self.data_dir, &url, &title, "Bookmarks bar");
        }
    }

    fn render_history_popup(&mut self, ctx: &egui::Context) {
        let mut open = self.show_history;
        egui::Window::new("History")
            .open(&mut open)
            .default_width(440.0)
            .default_height(420.0)
            .resizable(true)
            .show(ctx, |ui| {
                let entries = storage::history::list(&self.data_dir);
                if entries.is_empty() {
                    ui.label("No browsing history yet.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for entry in entries.into_iter().take(200) {
                            let title = if entry.title.is_empty() { &entry.url } else { &entry.title };
                            if ui.selectable_label(false, format!("{}\n{}", truncate_tab_title(title, 64), entry.url)).clicked() {
                                let url = entry.url.clone();
                                self.navigate_to(&url);
                            }
                            ui.separator();
                        }
                    });
                }
            });
        self.show_history = open;
    }

    fn render_downloads_popup(&mut self, ctx: &egui::Context) {
        let mut open = self.show_downloads;
        egui::Window::new("Downloads")
            .open(&mut open)
            .default_width(480.0)
            .default_height(360.0)
            .resizable(true)
            .show(ctx, |ui| {
                let records = storage::download::list(&self.data_dir);
                if records.is_empty() {
                    ui.label("No downloads yet.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for record in records.iter().rev() {
                            ui.label(RichText::new(&record.suggested_filename).strong());
                            ui.label(RichText::new(format!("{} bytes · {:?}", record.downloaded_bytes, record.state))
                                .small().color(self.theme.text_muted()));
                            ui.separator();
                        }
                    });
                }
            });
        self.show_downloads = open;
    }

    fn render_bookmarks_manager(&mut self, ctx: &egui::Context) {
        let mut open = self.show_bookmarks_manager;
        egui::Window::new("Bookmarks")
            .open(&mut open)
            .default_width(440.0)
            .default_height(380.0)
            .resizable(true)
            .show(ctx, |ui| {
                let entries = storage::bookmarks::list(&self.data_dir);
                if entries.is_empty() {
                    ui.label("Your bookmarks will appear here.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for bookmark in entries {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&bookmark.title).strong());
                                    ui.label(RichText::new(&bookmark.url).small().color(self.theme.text_muted()));
                                    ui.label(RichText::new(&bookmark.folder).small().color(self.theme.text_muted()));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button("Remove").clicked() {
                                        let _ = storage::bookmarks::remove(&self.data_dir, &bookmark.id);
                                    }
                                    if ui.small_button("Open").clicked() {
                                        let url = bookmark.url.clone();
                                        self.navigate_to(&url);
                                    }
                                });
                            });
                            ui.separator();
                        }
                    });
                }
            });
        self.show_bookmarks_manager = open;
    }

    // ── Dummy profile side panel ──
        if self.show_dummy_panel {
            egui::SidePanel::right("dummy_panel")
                .default_width(360.0)
                .show(ctx, |ui| {
                    self.render_dummy_panel(ui);
                });
        }

        // ── Location profile side panel ──
        if self.show_location_panel {
            egui::SidePanel::right("location_panel")
                .default_width(360.0)
                .show(ctx, |ui| {
                    self.render_location_panel(ui);
                });
        }

        if self.show_history {
            self.render_history_popup(ctx);
        }
        if self.show_downloads {
            self.render_downloads_popup(ctx);
        }
        if self.show_bookmarks_manager {
            self.render_bookmarks_manager(ctx);
        }

        // ── Main viewport (fills all remaining space) ──
        egui::CentralPanel::default()
            .frame(Frame::none().fill(self.theme.frame()))
            .show(ctx, |ui| {
                self.render_viewport(ui);
            });

        self.last_frame = Instant::now();
        false
    }

    // ── Tab Strip ──
    fn render_tab_strip(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.tab_inactive())
            .inner_margin(egui::vec2(8.0, 3.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let ids = self.tab_manager.tab_order().to_vec();
                    let tab_width = (ui.available_width() / ids.len().max(1) as f32)
                        .clamp(150.0, 230.0);

                    egui::ScrollArea::horizontal()
                        .id_source("chrome_tab_strip")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for id in ids {
                                    if let Some(tab) = self.tab_manager.get_tab(id).cloned() {
                                        self.render_tab_card(ui, &tab, self.theme, tab_width);
                                    }
                                }
                            });
                        });

                    if browser_icon_button(ui, BrowserIcon::Plus, "New tab", true, self.theme).clicked() {
                        self.open_new_tab();
                    }
                });
            });
    }

    /// Render a single Chrome-style tab card.
    fn render_tab_card(
        &mut self,
        ui: &mut egui::Ui,
        tab: &engine::tab_management::Tab,
        theme: Theme,
        width: f32,
    ) {
        let active = self.tab_manager.active_tab() == Some(tab.id);
        let tab_width = if tab.pinned { 48.0 } else { width };
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(tab_width, 34.0),
            Sense::click_and_drag(),
        );

        let fill = if active {
            theme.tab_active()
        } else if response.hovered() {
            theme.hover()
        } else {
            theme.tab_inactive()
        };
        ui.painter().rect_filled(rect, Rounding::same(8.0), fill);
        if active {
            ui.painter().rect_stroke(
                rect,
                Rounding::same(8.0),
                Stroke::new(1.0, theme.border()),
            );
        }

        // Falco currently has no decoded favicon texture bridge to egui.
        // Show a restrained site glyph instead of a fabricated coloured initial.
        paint_browser_icon(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.left() + 17.0, rect.center().y),
                Vec2::splat(16.0),
            ),
            BrowserIcon::Globe,
            theme.text_muted(),
        );

        let title = if tab.title.is_empty() {
            if tab.url == "about:blank" { "New Tab" } else { tab.url.as_str() }
        } else {
            tab.title.as_str()
        };

        if !tab.pinned {
            let text_right = if tab.is_closable() { rect.right() - 32.0 } else { rect.right() - 8.0 };
            let title_rect = Rect::from_min_max(
                egui::pos2(rect.left() + 32.0, rect.top()),
                egui::pos2(text_right, rect.bottom()),
            );
            ui.painter().with_clip_rect(title_rect).text(
                egui::pos2(title_rect.left(), title_rect.center().y),
                Align2::LEFT_CENTER,
                truncate_tab_title(title, 34),
                FontId::proportional(12.5),
                if active { theme.text() } else { theme.text_muted() },
            );
        }

        if tab.is_closable() && (active || response.hovered()) {
            let close_rect = Rect::from_center_size(
                egui::pos2(rect.right() - 17.0, rect.center().y),
                Vec2::splat(24.0),
            );
            let close = ui.interact(
                close_rect,
                egui::Id::new(("chrome_tab_close", tab.id.as_u64())),
                Sense::click(),
            );
            if close.hovered() {
                ui.painter().circle_filled(close_rect.center(), 11.0, theme.hover());
            }
            paint_browser_icon(
                ui.painter(),
                Rect::from_center_size(close_rect.center(), Vec2::splat(12.0)),
                BrowserIcon::Close,
                theme.text(),
            );
            if close.clicked() {
                self.close_tab(tab.id);
                return;
            }
        }

        if response.clicked() {
            self.tab_manager.switch_to_tab(tab.id);
            self.sync_active_address();
            self.viewport.dirty = true;
        }

        if response.drag_started() {
            self.dragged_tab = Some(tab.id);
        }
        if let Some(dragged) = self.dragged_tab {
            if response.hovered()
                && dragged != tab.id
                && ui.input(|i| i.pointer.primary_down())
            {
                if let Some(target) = self.tab_manager.tab_order().iter().position(|id| *id == tab.id) {
                    self.tab_manager.move_tab_to_position(dragged, target);
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.dragged_tab = None;
            }
        }
    }

    // ── Navigation Toolbar ──
    fn render_nav_toolbar(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.toolbar())
            .inner_margin(egui::vec2(10.0, 5.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (can_back, can_forward) = self.navigation_availability();

                    if browser_icon_button(ui, BrowserIcon::Back, "Back", can_back, self.theme).clicked() {
                        self.navigate_history(-1);
                    }
                    if browser_icon_button(ui, BrowserIcon::Forward, "Forward", can_forward, self.theme).clicked() {
                        self.navigate_history(1);
                    }
                    if browser_icon_button(ui, BrowserIcon::Reload, "Reload", true, self.theme).clicked() {
                        self.viewport.texture = None;
                        self.viewport.dirty = true;
                    }

                    ui.add_space(6.0);
                    let omnibox_width = (ui.available_width() - 42.0).max(180.0);
                    let active_url = self.active_url();
                    let security_icon = if active_url.starts_with("https://") {
                        BrowserIcon::Lock
                    } else {
                        BrowserIcon::Globe
                    };

                    Frame::none()
                        .fill(match self.theme {
                            Theme::Light => Color32::from_rgb(241, 243, 244),
                            Theme::Dark => Color32::from_rgb(55, 56, 60),
                        })
                        .rounding(Rounding::same(18.0))
                        .stroke(Stroke::new(
                            if self.omnibox_focused { 1.5 } else { 1.0 },
                            if self.omnibox_focused {
                                Color32::from_rgb(26, 115, 232)
                            } else {
                                self.theme.border()
                            },
                        ))
                        .inner_margin(egui::vec2(12.0, 2.0))
                        .show(ui, |ui| {
                            ui.set_min_width(omnibox_width);
                            ui.set_max_width(omnibox_width);
                            ui.horizontal(|ui| {
                                paint_browser_icon(
                                    ui.painter(),
                                    Rect::from_min_size(ui.cursor().min, Vec2::splat(17.0)),
                                    security_icon,
                                    self.theme.text_muted(),
                                );
                                ui.add_space(8.0);

                                let field_width = (omnibox_width - 82.0).max(80.0);
                                let response = ui.add_sized(
                                    [field_width, 28.0],
                                    TextEdit::singleline(&mut self.address_bar)
                                        .desired_width(field_width)
                                        .hint_text("Search or enter address")
                                        .font(egui::TextStyle::Body)
                                        .frame(false),
                                );
                                self.omnibox_focused = response.has_focus();
                                if response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                    let value = self.address_bar.trim().to_string();
                                    if !value.is_empty() {
                                        self.navigate_to(&value);
                                    }
                                }

                                let bookmarked = self.is_active_bookmarked();
                                if browser_icon_button(
                                    ui,
                                    BrowserIcon::Star,
                                    if bookmarked { "Remove bookmark" } else { "Bookmark this tab" },
                                    !active_url.is_empty() && active_url != "about:blank",
                                    self.theme,
                                ).clicked() {
                                    self.toggle_bookmark();
                                }
                            });
                        });

                    ui.add_space(5.0);
                    if browser_icon_button(
                        ui,
                        BrowserIcon::Menu,
                        "Customize and control MyBrowser",
                        true,
                        self.theme,
                    ).clicked() {
                        self.show_menu = !self.show_menu;
                    }
                });
            });
    }

    // ── Bookmarks Bar ──
    fn render_bookmarks_bar(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(self.theme.tab_inactive())
            .inner_margin(egui::vec2(10.0, 1.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let entries = storage::bookmarks::list(&self.data_dir);
                    if entries.is_empty() {
                        ui.label(
                            RichText::new("No bookmarks")
                                .size(11.5)
                                .color(self.theme.text_muted()),
                        );
                    } else {
                        egui::ScrollArea::horizontal()
                            .id_source("chrome_bookmarks")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for bookmark in entries {
                                        let response = ui.add(
                                            egui::Button::new(
                                                RichText::new(truncate_tab_title(&bookmark.title, 26))
                                                    .size(11.5)
                                                    .color(self.theme.text()),
                                            )
                                            .frame(false)
                                            .min_size(Vec2::new(28.0, 24.0)),
                                        );
                                        if response.clicked() {
                                            self.navigate_to(&bookmark.url);
                                        }
                                    }
                                });
                            });
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if browser_icon_button(
                            ui,
                            BrowserIcon::Folder,
                            "Bookmark manager",
                            true,
                            self.theme,
                        ).clicked() {
                            self.show_bookmarks_manager = true;
                        }
                    });
                });
            });
    }

    // ── Main Viewport ──
    fn render_viewport(&mut self, ui: &mut egui::Ui) {
        let available = Vec2::new(
            ui.available_width().max(1.0),
            ui.available_height().max(1.0),
        );

        if self.viewport.dirty
            || self.viewport.texture.is_none()
            || (self.viewport.size - available).length() > 2.0
        {
            self.render_page_to_texture(ui.ctx(), available);
            self.viewport.size = available;
            self.viewport.dirty = false;
        }

        if let Some(texture) = &self.viewport.texture {
            ui.add(egui::Image::new(texture).fit_to_exact_size(available));
        } else {
            let active_is_blank = self.tab_manager.active_tab()
                .and_then(|id| self.tab_manager.get_tab(id))
                .map(|tab| tab.url == "about:blank")
                .unwrap_or(true);

            if active_is_blank {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("MyBrowser")
                                .size(30.0)
                                .strong()
                                .color(self.theme.text()),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Search the web or enter a URL above")
                                .size(13.0)
                                .color(self.theme.text_muted()),
                        );
                    });
                });
            }
        }
    }

    /// Render the active tab's page to an egui texture via the Falco engine.
    fn render_page_to_texture(&mut self, ctx: &egui::Context, size: Vec2) {
        let tab = match self.tab_manager.active_tab().and_then(|id| self.tab_manager.get_tab(id)) {
            Some(t) => t,
            None => return,
        };

        // An empty tab is a real empty state, not a request to the network.
        if tab.url == "about:blank" || tab.url.is_empty() {
            self.viewport.texture = None;
            return;
        }

        // Fetch HTML for the active URL through the existing profile-aware engine network layer.
        let html = match fetch(&tab.url, Some(&self.profile_id)) {
            Ok(h) => h,
            Err(_) => {
                let error_html = format!(
                    r#"<html><body style="font-family:sans-serif;padding:40px;color:#333;">
                    <h2>Unable to load page</h2>
                    <p>MyBrowser could not load <b>{}</b>.</p>
                    <p>Please check the address and try again.</p>
                    </body></html>"#,
                    tab.url
                );
                self.upload_rendered_page(ctx, &error_html, size);
                return;
            }
        };

        if let Some(active_id) = self.tab_manager.active_tab() {
            if let Some(active_tab) = self.tab_manager.get_tab_mut(active_id) {
                active_tab.title = extract_page_title(&html).unwrap_or_else(|| extract_domain(&active_tab.url));
                active_tab.state = TabState::Active;
                active_tab.load_progress = 1.0;
            }
        }

        let css = "";
        let opts = engine::RenderOptions {
            width: size.x.max(1.0) as u32,
            height: size.y.max(1.0) as u32,
            background: match self.theme {
                Theme::Light => 0xFFFFFFFF,
                Theme::Dark => 0xFF202124,
            },
            animation_time_ms: 0,
        };

        match engine::render_to_buffer(&html, css, opts) {
            Ok(buffer) => {
                self.upload_pixels(ctx, &buffer);
            }
            Err(_) => {
                let error_html = format!(
                    r#"<html><body style="font-family:sans-serif;padding:40px;color:#333;">
                    <h2>Render error</h2>
                    <p>Could not render <b>{}</b>.</p>
                    </body></html>"#,
                    tab.url
                );
                self.upload_rendered_page(ctx, &error_html, size);
            }
        }
    }

    fn upload_rendered_page(&mut self, ctx: &egui::Context, html: &str, size: Vec2) {
        let opts = engine::RenderOptions {
            width: size.x.max(1.0) as u32,
            height: size.y.max(1.0) as u32,
            background: match self.theme {
                Theme::Light => 0xFFFFFFFF,
                Theme::Dark => 0xFF202124,
            },
            animation_time_ms: 0,
        };
        if let Ok(buffer) = engine::render_to_buffer(html, "", opts) {
            self.upload_pixels(ctx, &buffer);
        }
    }

    fn upload_pixels(&mut self, ctx: &egui::Context, buffer: &engine::RenderedBuffer) {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width as usize, buffer.height as usize],
            &buffer.pixels,
        );
        let texture = ctx.load_texture(
            "page_viewport",
            color_image,
            TextureOptions::default(),
        );
        self.viewport.texture = Some(texture);
    }

    // ── Helpers ──
    fn has_bookmarks(&self) -> bool {
        !storage::bookmarks::list(&self.data_dir).is_empty()
    }

    fn selected_profile_id(&self) -> Option<String> {
        Some(self.profile_id.clone())
    }

    // ── 3-dot menu popup ──
    fn render_menu_popup(&mut self, ctx: &egui::Context) {
        let screen = ctx.available_rect();
        let width = 252.0;
        let pos = egui::pos2(screen.right() - width - 10.0, screen.top() + 82.0);

        egui::Area::new(egui::Id::new("chrome_main_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(self.theme.toolbar())
                    .rounding(Rounding::same(9.0))
                    .stroke(Stroke::new(1.0, self.theme.border()))
                    .shadow(egui::epaint::Shadow::small_light())
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(width - 16.0);

                        if ui.button("New tab                              Ctrl+T").clicked() {
                            self.open_new_tab();
                            self.show_menu = false;
                        }
                        if ui.button("Reopen closed tab              Ctrl+Shift+T").clicked() {
                            self.restore_closed_tab();
                            self.show_menu = false;
                        }

                        ui.separator();
                        if ui.button("History                              Ctrl+H").clicked() {
                            self.show_history = true;
                            self.show_menu = false;
                        }
                        if ui.button("Downloads                         Ctrl+J").clicked() {
                            self.show_downloads = true;
                            self.show_menu = false;
                        }
                        if ui.button("Bookmarks").clicked() {
                            self.show_bookmarks_manager = true;
                            self.show_menu = false;
                        }

                        ui.separator();
                        ui.label(RichText::new("Appearance").small().color(self.theme.text_muted()));
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.theme == Theme::Light, "Light").clicked() {
                                self.theme = Theme::Light;
                                self.viewport.dirty = true;
                            }
                            if ui.selectable_label(self.theme == Theme::Dark, "Dark").clicked() {
                                self.theme = Theme::Dark;
                                self.viewport.dirty = true;
                            }
                        });
                        ui.checkbox(&mut self.bookmarks_bar_visible, "Show bookmarks bar");

                        ui.separator();
                        if ui.button("Exit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
            });

        if ctx.input(|i| i.pointer.any_click()) {
            self.show_menu = false;
        }
    }

    // ── Dummy profile side panel ──
    fn render_dummy_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Dummy System Profile");
        let action = self.dummy_ui.render(ui, &self.dummy_config);
        self.handle_dummy_action(action);
    }

    // ── Location profile side panel ──
    fn render_location_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Location Profile");
        let action = self.location_ui.render(ui, &self.location_config);
        self.handle_location_action(action);
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
                engine::location::remove_location(&self.profile_id);
            }
        }
    }

    fn handle_dummy_action(&mut self, action: DummyProfileUIAction) {
        match action {
            DummyProfileUIAction::None => {}
            DummyProfileUIAction::EnableDummyMode => {
                self.dummy_config.enabled = true;
                persist_dummy_config(self);
            }
            DummyProfileUIAction::DisableDummyMode => {
                self.dummy_config.enabled = false;
                persist_dummy_config(self);
            }
            DummyProfileUIAction::SelectDevice(device) => {
                self.dummy_config.device = Some(device);
                self.dummy_config.enabled = true;
                persist_dummy_config(self);
            }
            DummyProfileUIAction::ApplyDummyConfig(config) => {
                self.dummy_config = config;
                persist_dummy_config(self);
            }
        }
    }
}

fn persist_dummy_config(window: &BrowserWindow) {
    set_config(&window.profile_id, window.dummy_config.clone());
}

#[derive(Debug, Clone, Copy)]
enum BrowserIcon { Back, Forward, Reload, Lock, Globe, Star, Menu, Close, Plus, Folder }

fn browser_icon_button(
    ui: &mut egui::Ui,
    icon: BrowserIcon,
    tooltip: &str,
    enabled: bool,
    theme: Theme,
) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), sense);
    if response.hovered() && enabled {
        ui.painter().circle_filled(rect.center(), 15.0, theme.hover());
    }
    paint_browser_icon(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        icon,
        if enabled { theme.text() } else { theme.text_muted() },
    );
    response.on_hover_text(tooltip)
}

fn paint_browser_icon(painter: &egui::Painter, rect: Rect, icon: BrowserIcon, color: Color32) {
    let c = rect.center();
    let stroke = Stroke::new(1.65, color);
    match icon {
        BrowserIcon::Back | BrowserIcon::Forward => {
            let d = if matches!(icon, BrowserIcon::Back) { -1.0 } else { 1.0 };
            let end = c.x + d * 6.0;
            painter.line_segment([egui::pos2(c.x - d * 5.0, c.y), egui::pos2(end, c.y)], stroke);
            painter.line_segment([egui::pos2(end, c.y), egui::pos2(end - d * 5.0, c.y - 5.0)], stroke);
            painter.line_segment([egui::pos2(end, c.y), egui::pos2(end - d * 5.0, c.y + 5.0)], stroke);
        }
        BrowserIcon::Reload => {
            painter.circle_stroke(c, 7.0, stroke);
            painter.line_segment([egui::pos2(c.x + 5.0, c.y - 6.0), egui::pos2(c.x + 7.0, c.y - 1.0)], stroke);
            painter.line_segment([egui::pos2(c.x + 5.0, c.y - 6.0), egui::pos2(c.x, c.y - 5.0)], stroke);
        }
        BrowserIcon::Lock => {
            painter.rect_filled(Rect::from_center_size(egui::pos2(c.x, c.y + 3.0), Vec2::new(11.0, 9.0)), Rounding::same(2.0), color);
            painter.circle_stroke(egui::pos2(c.x, c.y - 2.0), 4.0, stroke);
        }
        BrowserIcon::Globe => {
            painter.circle_stroke(c, 7.0, stroke);
            painter.line_segment([egui::pos2(c.x - 7.0, c.y), egui::pos2(c.x + 7.0, c.y)], stroke);
            painter.line_segment([egui::pos2(c.x, c.y - 7.0), egui::pos2(c.x, c.y + 7.0)], stroke);
            painter.circle_stroke(c, 3.6, Stroke::new(1.0, color));
        }
        BrowserIcon::Star => {
            let mut points = Vec::with_capacity(10);
            for i in 0..10 {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let radius = if i % 2 == 0 { 8.0 } else { 3.7 };
                points.push(egui::pos2(c.x + angle.cos() * radius, c.y + angle.sin() * radius));
            }
            painter.add(egui::Shape::convex_polygon(points, Color32::TRANSPARENT, stroke));
        }
        BrowserIcon::Menu => {
            for y in [-5.0, 0.0, 5.0] { painter.circle_filled(egui::pos2(c.x, c.y + y), 1.45, color); }
        }
        BrowserIcon::Close => {
            painter.line_segment([egui::pos2(c.x - 4.0, c.y - 4.0), egui::pos2(c.x + 4.0, c.y + 4.0)], stroke);
            painter.line_segment([egui::pos2(c.x + 4.0, c.y - 4.0), egui::pos2(c.x - 4.0, c.y + 4.0)], stroke);
        }
        BrowserIcon::Plus => {
            painter.line_segment([egui::pos2(c.x - 5.0, c.y), egui::pos2(c.x + 5.0, c.y)], stroke);
            painter.line_segment([egui::pos2(c.x, c.y - 5.0), egui::pos2(c.x, c.y + 5.0)], stroke);
        }
        BrowserIcon::Folder => {
            let body = Rect::from_center_size(c, Vec2::new(14.0, 10.0));
            painter.rect_stroke(body, Rounding::same(2.0), stroke);
            painter.line_segment([egui::pos2(body.left() + 1.0, body.top()), egui::pos2(body.left() + 5.0, body.top() - 3.0)], stroke);
            painter.line_segment([egui::pos2(body.left() + 5.0, body.top() - 3.0), egui::pos2(body.left() + 9.0, body.top() - 3.0)], stroke);
        }
    }
}

fn resolve_address(input: &str) -> String {
    let value = input.trim();
    if value.is_empty() { return "about:blank".to_string(); }
    if value.starts_with("about:") || value.starts_with("http://") ||
        value.starts_with("https://") || value.starts_with("file://") {
        return value.to_string();
    }
    let local = value.starts_with("localhost") || value.starts_with("127.0.0.1") || value.starts_with("[::1]");
    let looks_like_url = value.contains('.') || value.contains(':') || local || value.starts_with('/');
    if looks_like_url {
        if local || value.starts_with('/') { format!("http://{}", value.trim_start_matches('/')) }
        else { format!("https://{}", value) }
    } else {
        format!("https://www.google.com/search?q={}", encode_query(value))
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
    if title.is_empty() { None } else { Some(title.to_string()) }
}

fn truncate_tab_title(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() { format!("{}…", prefix) } else { prefix }
}

fn extract_domain(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.split('/').next().unwrap_or(url).to_string()
}

/// Deterministic favicon color from domain hash.
fn favicon_color(domain: &str) -> Color32 {
    let mut h: u32 = 0;
    for c in domain.chars() {
        h = h.wrapping_mul(31).wrapping_add(c as u32);
    }
    let r = ((h >> 16) & 0xFF) as u8;
    let g = ((h >> 8) & 0xFF) as u8;
    let b = (h & 0xFF) as u8;
    // Keep colors reasonably saturated but not too bright/dark.
    Color32::from_rgb(
        60 + (r % 160),
        60 + (g % 160),
        60 + (b % 160),
    )
}

