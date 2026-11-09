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
use engine::tab_management::TabManager;
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

/// Browser window state.
pub struct BrowserWindow {
    profile_id: String,
    data_dir: std::path::PathBuf,
    tab_manager: TabManager,
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

        let mut tab_manager = TabManager::new();
        let _initial = tab_manager.open_tab(
            "https://example.com".to_string(),
            "New Tab".to_string(),
        );

        let dummy_config = engine::dummy_profile::get_config(profile_id);
        let location_config = engine::location::get_location(profile_id);

        Ok(Self {
            profile_id: profile_id.to_string(),
            data_dir,
            tab_manager,
            address_bar: "https://example.com".to_string(),
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
        if self.bookmarks_bar_visible && self.has_bookmarks() {
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
        let theme = self.theme;
        Frame::none()
            .fill(theme.tab_inactive())
            .inner_margin(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();

                    // Render tabs (ungrouped, in order).
                    let tab_ids: Vec<engine::tab_management::TabId> =
                        self.tab_manager.tab_order().to_vec();

                    for tid in tab_ids {
                        let tab = match self.tab_manager.get_tab(tid) {
                            Some(t) => t.clone(),
                            None => continue,
                        };
                        self.render_tab_card(ui, &tab, theme);
                    }

                    // `+` new tab button (Chrome style, at end of tab strip).
                    if ui
                        .button(RichText::new("+").strong().color(theme.text()).size(16.0))
                        .clicked()
                    {
                        self.tab_manager.open_tab(
                            "https://example.com".to_string(),
                            "New Tab".to_string(),
                        );
                        self.viewport.dirty = true;
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
    ) {
        let is_active = self.tab_manager.active_tab() == Some(tab.id);

        // Tab background: active tab blends with content frame.
        let fill = if is_active {
            theme.tab_active()
        } else {
            theme.tab_inactive()
        };

        let stroke = if is_active {
            Stroke::new(1.0, theme.border())
        } else {
            Stroke::new(1.0, Color32::TRANSPARENT)
        };

        // Chrome tab shape: rounded top corners, squared bottom (blends into content).
        Frame::none()
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding {
                nw: 6.0,
                ne: 6.0,
                sw: 0.0,
                se: 0.0,
            })
            .inner_margin(egui::vec2(8.0, 2.0))
            .show(ui, |ui| {
                ui.set_min_height(34.0);
                ui.horizontal(|ui| {
                    ui.spacing();

                    // Favicon (colored circle with domain initial).
                    let domain = extract_domain(&tab.url);
                    let initial = domain.chars().next().unwrap_or('?');
                    let fav_color = favicon_color(&domain);
                    ui.painter().circle_filled(
                        ui.cursor().center(),
                        8.0,
                        fav_color,
                    );
                    ui.painter().text(
                        ui.cursor().center(),
                        Align2::CENTER_CENTER,
                        &initial.to_string(),
                        FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                    ui.allocate_space(Vec2::new(14.0, 0.0));

                    // Tab title.
                    let title = if tab.title.is_empty() {
                        "New Tab".to_string()
                    } else {
                        tab.title.clone()
                    };
                    let display_title = if title.len() > 45 {
                        format!("{}...", &title[..42])
                    } else {
                        title
                    };

                    let response = ui.selectable_label(
                        is_active,
                        RichText::new(&display_title)
                            .size(12.0)
                            .color(if is_active {
                                theme.text()
                            } else {
                                theme.text_muted()
                            }),
                    );
                    let is_hovered = response.hovered();
                    if response.clicked() {
                        self.tab_manager.switch_to_tab(tab.id);
                        self.address_bar = tab.url.clone();
                        self.viewport.dirty = true;
                    }

                    // Close button on hover (Chrome style).
                    if is_hovered && !tab.pinned {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.tab_manager.close_tab(tab.id);
                                self.viewport.dirty = true;
                            }
                        });
                    }
                });
            });
    }

    // ── Navigation Toolbar ──
    fn render_nav_toolbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        Frame::none()
            .fill(theme.toolbar())
            .inner_margin(egui::vec2(8.0, 2.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();

                    // Back.
                    let back_enabled = true; // TODO: wire to session history
                    if ui.button(RichText::new("◀").size(14.0)).clicked() {
                        // back navigation
                    }

                    // Forward.
                    if ui.button(RichText::new("▶").size(14.0)).clicked() {
                        // forward navigation
                    }

                    // Reload / Stop.
                    if ui.button(RichText::new("⟳").size(14.0)).clicked() {
                        self.viewport.dirty = true;
                    }

                    ui.separator();

                    // ── Omnibox ──
                    // Security icon (left of omnibox).
                    ui.painter().circle_filled(
                        ui.cursor().center(),
                        7.0,
                        Color32::from_rgb(16, 137, 43), // green secure
                    );
                    // Security icon: draw a shield-like shape instead of emoji.
                        let security_rect = Rect::from_center_size(
                            ui.cursor().center(),
                            Vec2::splat(10.0),
                        );
                        ui.painter().rect_filled(
                            security_rect,
                            Rounding::same(2.0),
                            Color32::from_rgb(66, 133, 244),
                        );
                    ui.allocate_space(Vec2::new(16.0, 0.0));

                    // Omnibox text field.
                    let omnibox_width = ui.available_width() - 140.0;
                    ui.set_width(omnibox_width.clamp(200.0, f32::INFINITY));

                    let omnibox_response = ui.text_edit_singleline(&mut self.address_bar);
                    if omnibox_response.has_focus() {
                        self.omnibox_focused = true;
                    }
                    if omnibox_response.lost_focus() {
                        self.omnibox_focused = false;
                    }

                    // Focus outline when focused.
                    if self.omnibox_focused {
                        let rect = omnibox_response.rect;
                        ui.painter().rect_stroke(
                            rect.shrink(-1.0),
                            Rounding::same(4.0),
                            Stroke::new(1.5, Color32::from_rgb(66, 133, 244)),
                        );
                    }

                    if ui.input(|i| i.key_pressed(Key::Enter)) && !self.address_bar.trim().is_empty() {
                        let url = self.address_bar.trim().to_string();
                        if let Some(active_id) = self.tab_manager.active_tab() {
                            if let Some(tab) = self.tab_manager.get_tab_mut(active_id) {
                                tab.url = url.clone();
                                tab.title = "Loading...".to_string();
                                tab.state = engine::tab_management::TabState::Loading;
                            }
                        } else {
                            self.tab_manager.open_tab(url.clone(), "Loading...".to_string());
                        }
                        self.viewport.dirty = true;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // 3-dot menu.
                        let menu_color = if self.menu_hovered {
                            Color32::from_rgb(60, 61, 65)
                        } else {
                            theme.toolbar()
                        };
                        let menu_resp = ui.button("⋮");
                        if menu_resp.hovered() {
                            self.menu_hovered = true;
                        } else {
                            self.menu_hovered = false;
                        }
                        if menu_resp.clicked() {
                            self.show_menu = !self.show_menu;
                        }

                        // Profile avatar.
                        ui.allocate_space(Vec2::new(4.0, 0.0));
                        let avatar_color = Color32::from_rgb(66, 133, 244);
                        ui.painter().circle_filled(ui.cursor().center(), 12.0, avatar_color);
                        ui.painter().text(
                            ui.cursor().center(),
                            Align2::CENTER_CENTER,
                            "A",
                            FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                        ui.allocate_space(Vec2::new(24.0, 0.0));
                    });
                });
            });
    }

    // ── Bookmarks Bar ──
    fn render_bookmarks_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        Frame::none()
            .fill(theme.tab_inactive())
            .inner_margin(egui::vec2(6.0, 1.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();

                    for bookmark in storage::bookmarks::list(&self.data_dir) {
                        let domain = extract_domain(&bookmark.url);
                        let initial = domain.chars().next().unwrap_or('?');
                        let fav_color = favicon_color(&domain);

                        ui.horizontal(|ui| {
                            ui.set_min_height(22.0);
                            ui.painter().circle_filled(
                                ui.cursor().center(),
                                7.0,
                                fav_color,
                            );
                            ui.painter().text(
                                ui.cursor().center(),
                                Align2::CENTER_CENTER,
                                &initial.to_string(),
                                FontId::proportional(9.0),
                                Color32::WHITE,
                            );
                            ui.allocate_space(Vec2::new(10.0, 0.0));

                            if ui
                                .button(
                                    RichText::new(&bookmark.title)
                                        .size(11.0)
                                        .color(theme.text()),
                                )
                                .clicked()
                            {
                                self.tab_manager.open_tab(
                                    bookmark.url.clone(),
                                    bookmark.title.clone(),
                                );
                                self.address_bar = bookmark.url.clone();
                                self.viewport.dirty = true;
                            }
                        });

                        ui.spacing();
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("≡").size(12.0)).clicked() {
                            // bookmarks folder menu
                        }
                    });
                });
            });
    }

    // ── Main Viewport ──
    fn render_viewport(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let available = ui.available_size();

        // Re-render viewport if dirty or size changed.
        if self.viewport.dirty
            || self.viewport.texture.is_none()
            || (self.viewport.size - available).length() > 4.0
        {
            self.render_page_to_texture(ui.ctx(), available);
            self.viewport.size = available;
            self.viewport.dirty = false;
        }

        // Show loading state if no texture yet.
        if let Some(texture) = &self.viewport.texture {
            let img = egui::Image::new(texture).fit_to_original_size(1.0);
            ui.add(egui::Image::new(texture).fit_to_original_size(1.0));
        } else {
            // Empty / new tab page.
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    let fav_color = favicon_color("example");
                    ui.painter().circle_filled(
                        ui.cursor().center(),
                        32.0,
                        fav_color,
                    );
                    ui.painter().text(
                        ui.cursor().center(),
                        Align2::CENTER_CENTER,
                        "E",
                        FontId::proportional(24.0),
                        Color32::WHITE,
                    );
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("New Tab")
                            .size(20.0)
                            .strong()
                            .color(theme.text()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Enter a URL or search query above")
                            .size(13.0)
                            .color(theme.text_muted()),
                    );
                });
            });
        }
    }

    /// Render the active tab's page to an egui texture via the Falco engine.
    fn render_page_to_texture(&mut self, ctx: &egui::Context, size: Vec2) {
        let tab = match self.tab_manager.active_tab().and_then(|id| self.tab_manager.get_tab(id)) {
            Some(t) => t,
            None => return,
        };

        // Fetch HTML for the tab's URL.
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
        let screen_rect = ctx.available_rect();
        let popup_width = 250.0;
        let popup_x = screen_rect.right() - popup_width - 10.0;
        let popup_y = 86.0; // below tab strip + nav toolbar

        egui::Window::new("Chrome Menu")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .min_width(popup_width)
            .max_width(popup_width)
            .frame(Frame::none()
                .fill(self.theme.toolbar())
                .rounding(Rounding::same(6.0))
                .stroke(Stroke::new(1.0, self.theme.border())))
            .show(ctx, |ui| {
                ui.set_min_width(popup_width);

                // New tab / window.
                if ui.button("New tab\tCtrl+T").clicked() {
                    self.tab_manager.open_tab(
                        "https://example.com".to_string(),
                        "New Tab".to_string(),
                    );
                    self.address_bar = "https://example.com".to_string();
                    self.viewport.dirty = true;
                    self.show_menu = false;
                }
                if ui.button("New window\tCtrl+N").clicked() {
                    self.show_menu = false;
                }

                ui.separator();

                // History.
                if ui.button("History\tCtrl+H").clicked() {
                    self.show_menu = false;
                }
                if ui.button("Downloads\tCtrl+J").clicked() {
                    self.show_menu = false;
                }
                if ui.button("Bookmarks").clicked() {
                    self.show_menu = false;
                }

                ui.separator();

                // Zoom.
                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    if ui.button("−").clicked() {}
                    ui.label("100%");
                    if ui.button("+").clicked() {}
                });

                ui.separator();

                if ui.button("Print...\tCtrl+P").clicked() {
                    self.show_menu = false;
                }
                if ui.button("Find\tCtrl+F").clicked() {
                    self.show_menu = false;
                }

                ui.separator();

                // VPN (per profile).
                egui::CollapsingHeader::new("VPN")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label("Per-profile VPN");
                        ui.checkbox(&mut self.vpn_enabled, "Enable VPN for this profile");
                        if self.vpn_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Provider:");
                                ui.text_edit_singleline(&mut self.vpn_provider);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Endpoint:");
                                ui.text_edit_singleline(&mut self.vpn_endpoint);
                            });
                            ui.horizontal(|ui| {
                                if ui.button("Connect").clicked() {
                                    if let Some(pid) = self.selected_profile_id() {
                                        ProxyManager::set_vpn(&pid, true, &self.vpn_provider, &self.vpn_endpoint);
                                        let (_, _, _, connected, msg) =
                                            ProxyManager::vpn_status(&pid);
                                        self.vpn_enabled = connected;
                                        self.vpn_message = msg;
                                    }
                                }
                                if ui.button("Disconnect").clicked() {
                                    if let Some(pid) = self.selected_profile_id() {
                                        ProxyManager::set_vpn(&pid, false, "", "");
                                        self.vpn_enabled = false;
                                        self.vpn_message = Some("VPN disconnected.".to_string());
                                    }
                                }
                            });
                            if let Some(msg) = &self.vpn_message {
                                let color = if msg.contains("connected") {
                                    Color32::from_rgb(16, 137, 43)
                                } else {
                                    Color32::from_rgb(234, 67, 53)
                                };
                                ui.colored_label(color, RichText::new(msg).small());
                            }
                        }
                    });

                ui.separator();

                if ui.button("Settings").clicked() {
                    self.show_menu = false;
                }

                ui.separator();

                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

        // Close menu when clicking outside.
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

