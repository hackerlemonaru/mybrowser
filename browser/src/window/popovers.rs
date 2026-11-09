//! Toolbar popovers that do more than the 3-dot menu: the profile bubble
//! (switch profile / create profile) and the per-profile VPN panel.

use super::*;

impl BrowserWindow {
    /// The profile bubble, exactly like Chrome's avatar menu: the current
    /// profile, every other profile, "Add profile" and "Manage profiles".
    pub(super) fn render_profile_popover(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(anchor) = self.profile_anchor else {
            return;
        };
        let profiles: Vec<(String, String)> = match profile_manager::ProfileManager::new() {
            Ok(manager) => manager
                .list()
                .iter()
                .map(|profile| (profile.id.clone(), profile.name.clone()))
                .collect(),
            Err(_) => vec![(self.profile_id.clone(), self.profile_name.clone())],
        };
        let name = self.profile_name.clone();
        let current = self.profile_id.clone();

        let width = 320.0;
        let row_h = 42.0;
        let height = 100.0
            + profiles.len().max(1) as f32 * row_h
            + 12.0
            + row_h * 2.0
            + 8.0;
        let screen = ctx.screen_rect();
        let pos = pos2(
            (anchor.right() - width).clamp(screen.left() + 8.0, screen.right() - width - 8.0),
            (anchor.bottom() + 6.0).min((screen.bottom() - height - 8.0).max(screen.top() + 8.0)),
        ) + vec2(0.0, self.popup_slide);

        let mut switch_to: Option<String> = None;
        let mut create = false;
        let mut manage = false;

        let area = egui::Area::new(egui::Id::new("profile_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 12.0, 6.0, 26.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 12.0, p.menu_bg);
                widgets::stroke_round(ui.painter(), rect, 12.0, p.menu_separator, 1.0);

                // Current profile header.
                paint_avatar(ui.painter(), pos2(rect.left() + 32.0, rect.top() + 32.0), 18.0, &name, p);
                ui.painter().text(
                    pos2(rect.left() + 60.0, rect.top() + 24.0),
                    Align2::LEFT_CENTER,
                    &name,
                    fonts::medium(14.0),
                    p.text,
                );
                ui.painter().text(
                    pos2(rect.left() + 60.0, rect.top() + 42.0),
                    Align2::LEFT_CENTER,
                    "Local profile",
                    fonts::ui_font(11.5),
                    p.text_dim,
                );
                ui.painter().hline(
                    (rect.left() + 12.0)..=(rect.right() - 12.0),
                    rect.top() + 70.0,
                    Stroke::new(1.0, p.menu_separator),
                );
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.top() + 84.0),
                    Align2::LEFT_CENTER,
                    "Profiles",
                    fonts::medium(11.5),
                    p.text_dim,
                );

                let mut y = rect.top() + 100.0;
                for (id, profile_name) in &profiles {
                    let row = Rect::from_min_max(
                        pos2(rect.left() + 5.0, y),
                        pos2(rect.right() - 5.0, y + row_h),
                    );
                    let response =
                        ui.interact(row, egui::Id::new(("profile_row", id.clone())), Sense::click());
                    widgets::state_layer(
                        ui.ctx(),
                        ui.painter(),
                        response.id,
                        row,
                        8.0,
                        p.hover,
                        response.hovered(),
                        response.is_pointer_button_down_on(),
                    );
                    paint_avatar(
                        ui.painter(),
                        pos2(row.left() + 22.0, row.center().y),
                        13.0,
                        profile_name,
                        p,
                    );
                    widgets::text_clipped(
                        ui.painter(),
                        ui.ctx(),
                        Rect::from_min_max(
                            pos2(row.left() + 44.0, row.top()),
                            pos2(row.right() - 30.0, row.bottom()),
                        ),
                        profile_name,
                        fonts::ui_font(13.0),
                        p.text,
                    );
                    if *id == current {
                        icons::paint(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(row.right() - 20.0, row.center().y),
                                Vec2::splat(15.0),
                            ),
                            Icon::Check,
                            p.accent,
                        );
                    }
                    if response.clicked() && *id != current {
                        switch_to = Some(id.clone());
                    }
                    y += row_h;
                }

                for (icon, label, is_create) in [
                    (Icon::Plus, "Add profile", true),
                    (Icon::Settings, "Manage profiles", false),
                ] {
                    let row = Rect::from_min_max(
                        pos2(rect.left() + 5.0, y),
                        pos2(rect.right() - 5.0, y + row_h),
                    );
                    let response =
                        ui.interact(row, egui::Id::new(("profile_action", label)), Sense::click());
                    widgets::state_layer(
                        ui.ctx(),
                        ui.painter(),
                        response.id,
                        row,
                        8.0,
                        p.hover,
                        response.hovered(),
                        response.is_pointer_button_down_on(),
                    );
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(row.left() + 22.0, row.center().y),
                            Vec2::splat(17.0),
                        ),
                        icon,
                        p.icon,
                    );
                    ui.painter().text(
                        pos2(row.left() + 44.0, row.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        fonts::ui_font(13.0),
                        p.text,
                    );
                    if response.clicked() {
                        if is_create {
                            create = true;
                        } else {
                            manage = true;
                        }
                    }
                    y += row_h;
                }
                ui.rect_contains_pointer(rect)
            });

        if let Some(id) = switch_to {
            Self::open_profile_window(&id);
            self.popup = None;
        }
        if create {
            if let Some(id) = self.create_profile() {
                Self::open_profile_window(&id);
            }
            self.popup = None;
        }
        if manage {
            self.settings = Some(SettingsSection::You);
            self.popup = None;
        }
        if ctx.cumulative_pass_nr() != self.popup_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area.inner
        {
            self.popup = None;
        }
    }

    /// The per-profile VPN panel: on/off plus the provider and endpoint applied
    /// to this profile's proxy configuration.
    pub(super) fn render_vpn_popover(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(anchor) = self.vpn_anchor else {
            return;
        };
        let width = 352.0;
        let height = 268.0;
        let screen = ctx.screen_rect();
        let pos = pos2(
            (anchor.right() - width).clamp(screen.left() + 8.0, screen.right() - width - 8.0),
            (anchor.bottom() + 6.0).min((screen.bottom() - height - 8.0).max(screen.top() + 8.0)),
        ) + vec2(0.0, self.popup_slide);

        let enabled = self.vpn_enabled;
        let provider = self.vpn_provider.clone();
        let message = self.vpn_message.clone();
        let mut toggle = false;
        let mut apply = false;

        let area = egui::Area::new(egui::Id::new("vpn_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                widgets::drop_shadow(ui.painter(), rect, 12.0, 6.0, 26.0, p.shadow);
                widgets::fill_round(ui.painter(), rect, 12.0, p.menu_bg);
                widgets::stroke_round(ui.painter(), rect, 12.0, p.menu_separator, 1.0);

                // Header: shield in a tinted circle, title, state.
                let shield = pos2(rect.left() + 34.0, rect.top() + 34.0);
                ui.painter().circle_filled(
                    shield,
                    17.0,
                    if enabled { p.accent.gamma_multiply(0.22) } else { p.hover },
                );
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(shield, Vec2::splat(19.0)),
                    Icon::Shield,
                    if enabled { p.accent } else { p.icon },
                );
                ui.painter().text(
                    pos2(rect.left() + 60.0, rect.top() + 26.0),
                    Align2::LEFT_CENTER,
                    "MyBrowser VPN",
                    fonts::medium(14.0),
                    p.text,
                );
                let state = if enabled {
                    if provider.is_empty() {
                        "On".to_string()
                    } else {
                        format!("On · {}", provider)
                    }
                } else {
                    "Off".to_string()
                };
                ui.painter().text(
                    pos2(rect.left() + 60.0, rect.top() + 44.0),
                    Align2::LEFT_CENTER,
                    state,
                    fonts::ui_font(11.5),
                    if enabled { p.accent } else { p.text_dim },
                );

                // Status pill on the right.
                let pill = Rect::from_min_max(
                    pos2(rect.right() - 70.0, rect.top() + 22.0),
                    pos2(rect.right() - 14.0, rect.top() + 44.0),
                );
                widgets::fill_round(
                    ui.painter(),
                    pill,
                    11.0,
                    if enabled { p.accent.gamma_multiply(0.22) } else { p.hover },
                );
                ui.painter().text(
                    pill.center(),
                    Align2::CENTER_CENTER,
                    if enabled { "On" } else { "Off" },
                    fonts::medium(11.0),
                    if enabled { p.accent } else { p.text_dim },
                );

                ui.painter().hline(
                    (rect.left() + 12.0)..=(rect.right() - 12.0),
                    rect.top() + 66.0,
                    Stroke::new(1.0, p.menu_separator),
                );

                // Toggle row.
                let row = Rect::from_min_max(
                    pos2(rect.left() + 6.0, rect.top() + 74.0),
                    pos2(rect.right() - 6.0, rect.top() + 110.0),
                );
                let response = ui.interact(row, egui::Id::new("vpn_toggle"), Sense::click());
                widgets::state_layer(
                    ui.ctx(),
                    ui.painter(),
                    response.id,
                    row,
                    8.0,
                    p.hover,
                    response.hovered(),
                    response.is_pointer_button_down_on(),
                );
                ui.painter().text(
                    pos2(row.left() + 12.0, row.center().y),
                    Align2::LEFT_CENTER,
                    if enabled {
                        "Turn off VPN"
                    } else {
                        "Turn on VPN"
                    },
                    fonts::ui_font(13.0),
                    p.text,
                );
                paint_switch(
                    ui.painter(),
                    Rect::from_center_size(pos2(row.right() - 30.0, row.center().y), vec2(34.0, 18.0)),
                    enabled,
                    p,
                );
                if response.clicked() {
                    toggle = true;
                }

                // Provider + endpoint fields.
                for (label, top_offset, field) in [
                    ("Provider", 122.0, 0usize),
                    ("Tunnel endpoint", 172.0, 1usize),
                ] {
                    ui.painter().text(
                        pos2(rect.left() + 16.0, rect.top() + top_offset),
                        Align2::LEFT_CENTER,
                        label,
                        fonts::ui_font(11.5),
                        p.text_dim,
                    );
                    let field_rect = Rect::from_min_max(
                        pos2(rect.right() - 184.0, rect.top() + top_offset - 12.0),
                        pos2(rect.right() - 14.0, rect.top() + top_offset + 12.0),
                    );
                    widgets::fill_round(ui.painter(), field_rect, 8.0, p.omnibox_bg);
                    let target: &mut String = if field == 0 {
                        &mut self.vpn_provider
                    } else {
                        &mut self.vpn_endpoint
                    };
                    ui.put(
                        field_rect.shrink2(vec2(8.0, 4.0)),
                        TextEdit::singleline(target)
                            .frame(false)
                            .font(fonts::ui_font(12.0))
                            .text_color(p.omnibox_text)
                            .desired_width(field_rect.width() - 16.0),
                    );
                }

                // Footer note + apply.
                ui.painter().text(
                    pos2(rect.left() + 16.0, rect.bottom() - 26.0),
                    Align2::LEFT_CENTER,
                    "Applies to this profile only",
                    fonts::ui_font(10.5),
                    p.text_dim,
                );
                let apply_rect = Rect::from_min_max(
                    pos2(rect.right() - 92.0, rect.bottom() - 44.0),
                    pos2(rect.right() - 14.0, rect.bottom() - 14.0),
                );
                let apply_response = ui.interact(apply_rect, egui::Id::new("vpn_apply"), Sense::click());
                let fill = if apply_response.hovered() {
                    widgets::blend(p.accent, Color32::WHITE, 0.14)
                } else {
                    p.accent
                };
                widgets::fill_round(ui.painter(), apply_rect, 15.0, fill);
                ui.painter().text(
                    apply_rect.center(),
                    Align2::CENTER_CENTER,
                    "Apply",
                    fonts::medium(12.0),
                    Color32::WHITE,
                );
                if apply_response.clicked() {
                    apply = true;
                }

                if let Some(message) = message.clone() {
                    ui.painter().text(
                        pos2(rect.left() + 16.0, rect.bottom() - 46.0),
                        Align2::LEFT_CENTER,
                        message,
                        fonts::ui_font(10.5),
                        p.accent,
                    );
                }
                ui.rect_contains_pointer(rect)
            });

        if toggle {
            self.apply_action(Action::Vpn);
        }
        if apply {
            self.vpn_provider = self.vpn_provider.trim().to_string();
            self.vpn_endpoint = self.vpn_endpoint.trim().to_string();
            ProxyManager::set_vpn(
                &self.profile_id,
                self.vpn_enabled,
                &self.vpn_provider,
                &self.vpn_endpoint,
            );
            self.refresh_vpn_status();
        }
        if ctx.cumulative_pass_nr() != self.popup_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area.inner
        {
            self.popup = None;
        }
    }

    fn refresh_vpn_status(&mut self) {
        let (enabled, provider, endpoint, connected, message) =
            ProxyManager::vpn_status(&self.profile_id);
        self.vpn_enabled = enabled;
        self.vpn_provider = provider;
        self.vpn_endpoint = endpoint;
        self.vpn_message = message;
        let _ = connected;
    }

    /// Create a profile with a free name and return its id.
    pub(super) fn create_profile(&mut self) -> Option<String> {
        let mut manager = profile_manager::ProfileManager::new().ok()?;
        let name = format!("Profile {}", manager.list().len() + 1);
        manager.create(name).ok().map(|profile| profile.id)
    }

    /// Open a separate window for `id` (one window per profile, like Chrome).
    pub(super) fn open_profile_window(id: &str) -> bool {
        std::env::current_exe()
            .ok()
            .map(|exe| {
                std::process::Command::new(exe)
                    .arg("--window")
                    .arg(id)
                    .spawn()
                    .is_ok()
            })
            .unwrap_or(false)
    }
}

fn paint_avatar(painter: &egui::Painter, center: Pos2, radius: f32, name: &str, p: &Palette) {
    let color = if p.is_dark {
        Color32::from_rgb(0x6E, 0x9B, 0xE8)
    } else {
        Color32::from_rgb(0x1A, 0x73, 0xE8)
    };
    painter.circle_filled(center, radius, color);
    let initial = name
        .chars()
        .next()
        .unwrap_or('P')
        .to_uppercase()
        .next()
        .unwrap_or('P');
    painter.text(
        center,
        Align2::CENTER_CENTER,
        initial,
        fonts::medium(radius * 0.95),
        Color32::WHITE,
    );
}

fn paint_switch(painter: &egui::Painter, rect: Rect, on: bool, p: &Palette) {
    let radius = rect.height() / 2.0;
    if on {
        widgets::fill_round(painter, rect, radius, p.accent);
    } else {
        widgets::fill_round(painter, rect, radius, p.hover);
        widgets::stroke_round(painter, rect, radius, p.text_dim.gamma_multiply(0.5), 1.0);
    }
    let knob = pos2(
        if on {
            rect.right() - radius
        } else {
            rect.left() + radius
        },
        rect.center().y,
    );
    painter.circle_filled(
        knob,
        radius - 3.0,
        if on { Color32::WHITE } else { p.text_dim },
    );
}
