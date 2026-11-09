//! Browser window UI — full Chrome-like rework.
//!
//! Layout mirrors Google Chrome exactly:
//!   [Top] OS window controls (min/max/close) — handled by the WM
//!   [Bookmarks bar] Gmail, YouTube, Maps, Translate, Apple, iCloud, Google, Yahoo, Facebook, Zomato, NDTV, Hotstar + "All Bookmarks"
//!   [Omnibox] back/forward/reload | address bar | profile | 3-dot menu
//!   [Tab strip] tabs + `+` button at the end
//!   [Content] the active page (Falco render placeholder)
//!
//! Dark theme throughout (#202124 navbar, #2d2d2d tabs, #333 content).
//! Nothing surfaced outside the navbar — all controls live in the 3-dot menu.

use anyhow::Result;
use crate::extract_origin;
use engine::tab_management::TabManager;
use engine::dummy_profile::{DummySystemConfig, set_config};
use engine::dummy_profile::ui::{DummyProfileSelector, DummyProfileUIAction};
use engine::location::{set_location, LocationProfile};
use engine::location::ui::{LocationProfileSelector, LocationUIAction};
use engine::net::proxy::ProxyManager;
use egui::{
    Key, Align, Align2, Layout, RichText, TextEdit, Color32, Frame, Rounding, Stroke,
    Vec2, FontId, Sense, Pos2,
};
use std::time::Instant;

/// Chrome-like bookmark shortcuts (exactly as in image-1).
const BOOKMARKS: &[(&str, &str)] = &[
    ("Gmail", "https://gmail.com"),
    ("YouTube", "https://youtube.com"),
    ("Maps", "https://maps.google.com"),
    ("Translate", "https://translate.google.com"),
    ("Apple", "https://apple.com"),
    ("iCloud", "https://icloud.com"),
    ("Google", "https://google.com"),
    ("Yahoo", "https://yahoo.com"),
    ("Facebook", "https://facebook.com"),
    ("Zomato", "https://zomato.com"),
    ("NDTV", "https://ndtv.com"),
    ("Hotstar", "https://hotstar.com"),
];

/// Browser window state.
pub struct BrowserWindow {
    profile_id: String,
    tab_manager: TabManager,
    dummy_config: DummySystemConfig,
    dummy_ui: DummyProfileSelector,
    location_config: Option<LocationProfile>,
    location_ui: LocationProfileSelector,
    address_bar: String,
    show_dummy_panel: bool,
    show_location_panel: bool,
    show_menu: bool,              // 3-dot menu open?
    menu_hovered: bool,
    last_frame: Instant,
    // Per-profile VPN settings (fresh per profile).
    vpn_enabled: bool,
    vpn_provider: String,
    vpn_endpoint: String,
    vpn_message: Option<String>,
}

impl BrowserWindow {
    pub fn new(profile_id: &str) -> Result<Self> {
        let mut tab_manager = TabManager::new();
        let _initial = tab_manager.open_tab(
            "https://example.com".to_string(),
            "New Tab".to_string(),
        );

        let dummy_config = engine::dummy_profile::get_config(profile_id);
        let location_config = engine::location::get_location(profile_id);

        Ok(Self {
            profile_id: profile_id.to_string(),
            tab_manager,
            dummy_config,
            dummy_ui: DummyProfileSelector::new(),
            location_config,
            location_ui: LocationProfileSelector::new(),
            address_bar: "https://example.com".to_string(),
            show_dummy_panel: false,
            show_location_panel: false,
            show_menu: false,
            menu_hovered: false,
            last_frame: Instant::now(),
            vpn_enabled: false,
            vpn_provider: "None".to_string(),
            vpn_endpoint: String::new(),
            vpn_message: None,
        })
    }

    /// Render the full browser window. Returns true if the window should close.
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        // ── Bookmarks bar (always visible, Chrome style) ──
        egui::TopBottomPanel::top("bookmarks_bar")
            .min_height(30.0)
            .show(ctx, |ui| {
                self.render_bookmarks_bar(ui);
            });

        // ── Navbar: back/forward/reload | omnibox | profile | 3-dot ──
        egui::TopBottomPanel::top("navbar")
            .min_height(44.0)
            .show(ctx, |ui| {
                self.render_navbar(ui);
            });

        // ── Tab strip (tabs + `+` at the end) ──
        egui::TopBottomPanel::top("tab_strip")
            .min_height(34.0)
            .show(ctx, |ui| {
                self.render_tab_strip(ui);
            });

        // ── 3-dot menu popup ──
        if self.show_menu {
            self.render_menu_popup(ctx);
        }

        // ── Dummy profile side panel ──
        if self.show_dummy_panel {
            egui::SidePanel::right("dummy_panel")
                .default_width(350.0)
                .show(ctx, |ui| {
                    self.render_dummy_panel(ui);
                });
        }

        // ── Location profile side panel ──
        if self.show_location_panel {
            egui::SidePanel::right("location_panel")
                .default_width(350.0)
                .show(ctx, |ui| {
                    self.render_location_panel(ui);
                });
        }

        // ── Central panel: page content ──
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_content(ui);
        });

        self.last_frame = Instant::now();
        false
    }

    // ── Bookmarks bar ──
    fn render_bookmarks_bar(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(Color32::from_rgb(32, 33, 36))
            .inner_margin(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();
                    for (name, url) in BOOKMARKS {
                        if ui.button(RichText::new(*name).small().color(Color32::from_rgb(200, 200, 200))).clicked() {
                            self.tab_manager.open_tab(url.to_string(), name.to_string());
                        }
                    }
                    ui.separator();
                    if ui.button("All Bookmarks").clicked() {
                        // open bookmarks manager
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("★").color(Color32::from_gray(120)));
                    });
                });
            });
    }

    // ── Navbar ──
    fn render_navbar(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(Color32::from_rgb(32, 33, 36))
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Navigation buttons.
                    if ui.button("◀").clicked() { /* back */ }
                    if ui.button("▶").clicked() { /* forward */ }
                    if ui.button("⟳").clicked() { /* reload */ }

                    ui.separator();

                    // Omnibox / address bar.
                    ui.set_width(ui.available_width() - 160.0);
                    ui.text_edit_singleline(&mut self.address_bar);
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        let url = self.address_bar.trim().to_string();
                        if !url.is_empty() {
                            self.tab_manager.open_tab(url, "New Tab".to_string());
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // 3-dot menu.
                        let menu_response = ui.button("⋮");
                        if menu_response.hovered() {
                            self.menu_hovered = true;
                        } else {
                            self.menu_hovered = false;
                        }
                        if menu_response.clicked() {
                            self.show_menu = !self.show_menu;
                        }

                        // Profile avatar.
                        let avatar_color = Color32::from_rgb(66, 133, 244);
                        ui.painter().circle_filled(ui.cursor().center(), 14.0, avatar_color);
                        ui.painter().text(
                            ui.cursor().center(),
                            Align2::CENTER_CENTER,
                            "A",
                            FontId::default(),
                            Color32::WHITE,
                        );
                        ui.allocate_space(Vec2::splat(32.0));
                    });
                });
            });
    }

    // ── Tab strip: tabs + `+` at the end (Chrome style) ──
    fn render_tab_strip(&mut self, ui: &mut egui::Ui) {
        Frame::none()
            .fill(Color32::from_rgb(32, 33, 36))
            .inner_margin(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();

                    // Groups.
                    let group_order: Vec<engine::tab_management::GroupId> =
                        self.tab_manager.group_order().to_vec();
                    for gid in group_order {
                        let group = match self.tab_manager.groups().get(&gid) {
                            Some(g) => g.clone(),
                            None => continue,
                        };
                        if group.tab_ids.is_empty() {
                            continue;
                        }
                        let collapsed = group.collapsed;
                        let color = parse_hex_color(group.color.as_hex());
                        let header_text = if collapsed {
                            format!("▶ {} ({})", group.name, group.tab_ids.len())
                        } else {
                            format!("▼ {} ({})", group.name, group.tab_ids.len())
                        };
                        if ui.button(RichText::new(&header_text).color(color).strong()).clicked() {
                            let _ = self.tab_manager.toggle_group_collapse(gid);
                        }
                        if !collapsed {
                            self.render_tab_list_in_group(ui, &group);
                        }
                    }

                    // Ungrouped tabs.
                    let ungrouped: Vec<engine::tab_management::TabId> =
                        self.tab_manager.tab_order().iter()
                            .filter(|tid| self.tab_manager.get_tab(**tid).map_or(false, |t| t.group_id.is_none()))
                            .copied()
                            .collect();
                    if !ungrouped.is_empty() {
                        self.render_tab_list(ui, &ungrouped);
                    }

                    // `+` beside open tabs.
                    if ui.button("+").clicked() {
                        self.tab_manager.open_tab("https://example.com".to_string(), "New Tab".to_string());
                    }
                });
            });
    }

    // ── Tab list ──
    fn render_tab_list(&mut self, ui: &mut egui::Ui, tab_ids: &[engine::tab_management::TabId]) {
        for tid in tab_ids {
            let tab = match self.tab_manager.get_tab(*tid) {
                Some(t) => t.clone(),
                None => continue,
            };
            self.render_tab_card(ui, &tab);
        }
    }

    fn render_tab_list_in_group(
        &mut self,
        ui: &mut egui::Ui,
        group: &engine::tab_management::TabGroup,
    ) {
        for tid in &group.tab_ids {
            let tab = match self.tab_manager.get_tab(*tid) {
                Some(t) => t.clone(),
                None => continue,
            };
            self.render_tab_card(ui, &tab);
        }
    }

    // ── Individual tab card (Chrome style) ──
    fn render_tab_card(&mut self, ui: &mut egui::Ui, tab: &engine::tab_management::Tab) {
        let is_active = self.tab_manager.active_tab() == Some(tab.id);
        let theme_color = if let Some(gid) = tab.group_id {
            self.tab_manager.groups().get(&gid).map_or(Color32::from_rgb(66, 133, 244), |g| parse_hex_color(g.color.as_hex()))
        } else {
            Color32::from_gray(60)
        };

        Frame::none()
            .fill(if is_active { Color32::from_rgb(235, 235, 235) } else { Color32::from_rgb(32, 33, 36) })
            .rounding(Rounding::same(4.0))
            .stroke(Stroke::new(1.0, if is_active { theme_color } else { Color32::from_gray(50) }))
            .show(ui, |ui| {
                ui.set_min_height(30.0);
                ui.horizontal(|ui| {
                    ui.painter().circle_filled(ui.cursor().center(), 7.0, Color32::from_gray(100));
                    ui.allocate_space(Vec2::new(10.0, 0.0));

                    if tab.muted { ui.label(RichText::new("🔇").small()); }
                    if tab.frozen { ui.label(RichText::new("❄️").small()); }

                    let title = tab.display_title();
                    let display_title = if title.len() > 60 { format!("{}...", &title[..57]) } else { title.to_string() };

                    if ui.selectable_label(is_active, RichText::new(&display_title)).clicked() {
                        self.tab_manager.switch_to_tab(tab.id);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !tab.pinned {
                            if ui.button("✕").clicked() { self.tab_manager.close_tab(tab.id); }
                        }
                        if tab.muted { if ui.button("🔊").clicked() { self.tab_manager.unmute_tab(tab.id); } }
                        else { if ui.button("🔇").clicked() { self.tab_manager.mute_tab(tab.id); } }
                        if tab.frozen { if ui.button("❄️").clicked() { self.tab_manager.unfreeze_tab(tab.id); } }
                        else if !tab.pinned { if ui.button("❄️").clicked() { self.tab_manager.freeze_tab(tab.id); } }
                    });
                });
            });
    }

    // ── 3-dot menu popup ──
    fn render_menu_popup(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.available_rect();
        let popup_width = 260.0;
        let popup_x = screen_rect.right() - popup_width - 12.0;
        let popup_y = 74.0; // below navbar + bookmarks

        egui::Window::new("Chrome Menu")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .min_width(popup_width)
            .max_width(popup_width)
            .frame(Frame::none()
                .fill(Color32::from_rgb(32, 33, 36))
                .rounding(Rounding::same(6.0))
                .stroke(Stroke::new(1.0, Color32::from_gray(60))))
            .show(ctx, |ui| {
                ui.set_min_width(popup_width);

                // New tab / window.
                if ui.button("New tab\tCtrl+T").clicked() {
                    self.tab_manager.open_tab("https://example.com".to_string(), "New Tab".to_string());
                    self.show_menu = false;
                }
                if ui.button("New window\tCtrl+N").clicked() { self.show_menu = false; }
                if ui.button("New Incognito window\tCtrl+Shift+N").clicked() { self.show_menu = false; }

                ui.separator();

                // Profile.
                ui.horizontal(|ui| {
                    let avatar_color = Color32::from_rgb(66, 133, 244);
                    ui.painter().circle_filled(ui.cursor().center(), 12.0, avatar_color);
                    ui.painter().text(ui.cursor().center(), Align2::CENTER_CENTER, "A", FontId::default(), Color32::WHITE);
                    ui.allocate_space(Vec2::new(26.0, 0.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Aryan (User)").strong());
                        ui.label(RichText::new("Signed in").small().color(Color32::from_gray(150)));
                    });
                });

                ui.separator();

                // Menu items.
                menu_item(ui, "Passwords and autofill", |_| {});
                menu_item(ui, "History\tCtrl+H", |_| {});
                menu_item(ui, "Downloads\tCtrl+J", |_| {});
                menu_item(ui, "Bookmarks and lists", |_| {});
                menu_item(ui, "Tab groups", |_| {});
                menu_item(ui, "Extensions", |_| {});
                menu_item(ui, "Delete browsing data...\tCtrl+Shift+Del", |_| {});

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    if ui.button("−").clicked() {}
                    ui.label("100%");
                    if ui.button("+").clicked() {}
                });

                ui.separator();

                menu_item(ui, "Print...\tCtrl+P", |_| {});
                menu_item(ui, "Open Gemini in Chrome", |_| {});
                menu_item(ui, "Search this tab with Google Lens", |_| {});
                menu_item(ui, "Translate...", |_| {});
                menu_item(ui, "Find and edit", |_| {});
                menu_item(ui, "Cast, save, and share", |_| {});
                menu_item(ui, "More tools", |_| {});

                ui.separator();

                // VPN (per profile).
                egui::CollapsingHeader::new("🔒 VPN")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label("Per-profile VPN — isolated per profile.");
                        ui.checkbox(&mut self.vpn_enabled, "Enable VPN for this profile");
                        if self.vpn_enabled {
                            ui.horizontal(|ui| { ui.label("Provider:"); ui.text_edit_singleline(&mut self.vpn_provider); });
                            ui.horizontal(|ui| { ui.label("Endpoint (host:port):"); ui.text_edit_singleline(&mut self.vpn_endpoint); });
                            ui.horizontal(|ui| {
                                    if ui.button("Connect").clicked() {
                                    if let Some(pid) = self.selected_profile_id() {
                                        ProxyManager::set_vpn(&pid, true, &self.vpn_provider, &self.vpn_endpoint);
                                        let (_, _, _, connected, msg) = ProxyManager::vpn_status(&pid);
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
                                let color = if msg.contains("connected") { Color32::from_rgb(80, 255, 80) } else { Color32::from_rgb(255, 80, 80) };
                                ui.colored_label(color, RichText::new(msg).small());
                            }
                        }
                    });

                menu_item(ui, "Settings", |_| {});

                ui.separator();

                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

        if ctx.input(|i| i.pointer.any_click()) {
            self.show_menu = false;
        }
    }

    fn selected_profile_id(&self) -> Option<String> {
        Some(self.profile_id.clone())
    }

    // ── Central content ──
    fn render_content(&mut self, ui: &mut egui::Ui) {
        let tab_info = self.tab_manager.active_tab().and_then(|tab_id| {
            self.tab_manager.get_tab(tab_id).map(|tab| (tab.url.clone(), tab.title.clone(), tab.state))
        });

        if let Some((url, title, state)) = tab_info {
            ui.label(RichText::new("Active tab:").small().strong());
            ui.label(RichText::new(&url).monospace());
            ui.label(format!("Origin: {}", extract_origin(&url)));
            if !title.is_empty() { ui.label(format!("Title: {}", title)); }
            ui.label(format!("State: {:?}", state));
            ui.separator();
            ui.group(|ui| {
                ui.label("Tab controls:");
                ui.horizontal(|ui| {
                    if ui.button("Pin").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.pin_tab(id); } }
                    if ui.button("Unpin").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.unpin_tab(id); } }
                    if ui.button("Mute").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.mute_tab(id); } }
                    if ui.button("Unmute").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.unmute_tab(id); } }
                    if ui.button("Freeze").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.freeze_tab(id); } }
                    if ui.button("Unfreeze").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.unfreeze_tab(id); } }
                    if ui.button("Close").clicked() { if let Some(id) = self.tab_manager.active_tab() { self.tab_manager.close_tab(id); } }
                });
            });
        } else {
            ui.label("No tabs open. Click + beside a tab to open a new one.");
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
            DummyProfileUIAction::EnableDummyMode => { self.dummy_config.enabled = true; persist_dummy_config(self); }
            DummyProfileUIAction::DisableDummyMode => { self.dummy_config.enabled = false; persist_dummy_config(self); }
            DummyProfileUIAction::SelectDevice(device) => { self.dummy_config.device = Some(device); self.dummy_config.enabled = true; persist_dummy_config(self); }
            DummyProfileUIAction::ApplyDummyConfig(config) => { self.dummy_config = config; persist_dummy_config(self); }
        }
    }
}

fn menu_item<F>(ui: &mut egui::Ui, label: &str, on_click: F) where F: FnOnce(&mut egui::Ui) {
    if ui.button(label).clicked() { on_click(ui); }
}

fn persist_dummy_config(window: &BrowserWindow) {
    set_config(&window.profile_id, window.dummy_config.clone());
}

fn parse_hex_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 { return Color32::from_rgb(66, 133, 244); }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(66);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(133);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(244);
    Color32::from_rgb(r, g, b)
}
