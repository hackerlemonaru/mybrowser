//! Full-page settings screen, modelled on `chrome://settings`: a left
//! navigation column and card-based sections.
//!
//! Every control here writes real per-profile state — theme, search engine,
//! startup page, bookmarks bar, VPN, dummy system profile, browsing data — and
//! persists it in `<profile>/browser_prefs.json`.

use super::*;

/// Width of the section navigation column.
const NAV_WIDTH: f32 = 250.0;
/// Height of one settings row.
const ROW: f32 = 56.0;
/// Widest a settings card gets on a large window.
const CONTENT_MAX: f32 = 720.0;

/// The control painted at the right edge of a settings row.
enum Trailing {
    None,
    Toggle(bool),
    Radio(bool),
    Chevron,
    Value(String),
}

impl BrowserWindow {
    pub(super) fn render_settings(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, p.page_bg);

        // Header bar: back to the page, then the title.
        let header = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 56.0));
        ui.painter().rect_filled(header, 0.0, p.toolbar);
        ui.painter().hline(
            header.x_range(),
            header.bottom() - 0.5,
            Stroke::new(1.0, p.hairline),
        );

        let back_rect =
            Rect::from_center_size(pos2(header.left() + 28.0, header.center().y), Vec2::splat(32.0));
        let back = ui.interact(back_rect, egui::Id::new("settings_back"), Sense::click());
        widgets::paint_icon_button(ui, p, back_rect, Icon::Back, &back, true);
        if back.clicked() {
            self.settings = None;
            return;
        }
        back.on_hover_text("Back to the page");
        ui.painter().text(
            pos2(back_rect.right() + 12.0, header.center().y),
            Align2::LEFT_CENTER,
            "Settings",
            fonts::medium(17.0),
            p.text,
        );

        let nav_width = NAV_WIDTH.min(rect.width() * 0.34);
        let content_width = (rect.width() - nav_width - 100.0).clamp(320.0, CONTENT_MAX);
        let max_height = (rect.bottom() - header.bottom() - 28.0).max(160.0);

        ui.add_space(56.0);
        ui.add_space(12.0);

        let this = &mut *self;
        ui.horizontal_top(|ui| {
            ui.add_space(20.0);
            ui.allocate_ui_with_layout(
                vec2(nav_width, max_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| this.render_settings_nav(ui, p),
            );
            ui.add_space(24.0);
            ui.allocate_ui_with_layout(
                vec2(content_width, max_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(max_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| this.render_settings_section(ui, p));
                },
            );
        });
    }

    fn render_settings_nav(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let width = ui.available_width();
        for section in SettingsSection::ALL {
            let selected = self.settings == Some(section);
            let (row, response) = ui.allocate_exact_size(vec2(width, 40.0), Sense::click());
            if selected {
                widgets::fill_round(ui.painter(), row, 20.0, p.hover);
            } else {
                widgets::state_layer(
                    ui.ctx(),
                    ui.painter(),
                    response.id,
                    row,
                    20.0,
                    p.hover,
                    response.hovered(),
                    response.is_pointer_button_down_on(),
                );
            }
            icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(row.left() + 24.0, row.center().y), Vec2::splat(18.0)),
                section.icon(),
                if selected { p.accent } else { p.icon },
            );
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(
                    pos2(row.left() + 44.0, row.top()),
                    pos2(row.right() - 8.0, row.bottom()),
                ),
                section.label(),
                fonts::ui_font(13.0),
                p.text,
            );
            if response.clicked() {
                self.settings = Some(section);
                self.settings_notice = None;
            }
        }
    }

    fn render_settings_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let Some(section) = self.settings else {
            return;
        };

        ui.label(
            egui::RichText::new(section.label())
                .font(fonts::medium(18.0))
                .color(p.text),
        );
        ui.add_space(4.0);
        if let Some(notice) = self.settings_notice.clone() {
            ui.label(
                egui::RichText::new(notice)
                    .font(fonts::ui_font(12.0))
                    .color(p.accent),
            );
        }
        ui.add_space(10.0);

        match section {
            SettingsSection::You => self.settings_you(ui, p),
            SettingsSection::Autofill => self.settings_autofill(ui, p),
            SettingsSection::Privacy => self.settings_privacy(ui, p),
            SettingsSection::Appearance => self.settings_appearance(ui, p),
            SettingsSection::Search => self.settings_search(ui, p),
            SettingsSection::Startup => self.settings_startup(ui, p),
            SettingsSection::Downloads => self.settings_downloads(ui, p),
            SettingsSection::About => self.settings_about(ui, p),
        }
        ui.add_space(24.0);
    }

    fn settings_you(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let name = self.profile_name.clone();
        let id = self.profile_id.clone();
        let subtitle = format!("Profile id: {}", id);
        let card = settings_card(ui, p, 2);
        let _ = settings_row(
            ui,
            p,
            card,
            0,
            "you_name",
            Icon::Device,
            &name,
            Some(&subtitle),
            Trailing::None,
        );
        let add = settings_row(
            ui,
            p,
            card,
            1,
            "you_add",
            Icon::Plus,
            "Add a profile",
            Some("Opens another window with its own data"),
            Trailing::Chevron,
        );
        if add.clicked() {
            if let Some(new_id) = self.create_profile() {
                Self::open_profile_window(&new_id);
            }
        }

        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(
                "This profile is isolated: history, cookies, bookmarks and extensions never leave it.",
            )
            .font(fonts::ui_font(11.5))
            .color(p.text_dim),
        );
        ui.add_space(12.0);

        let this = &mut *self;
        ui.horizontal(|ui| {
            if settings_button(ui, p, "you_history", "History") {
                this.dialog = Some(Dialog::History);
            }
            ui.add_space(8.0);
            if settings_button(ui, p, "you_bookmarks", "Bookmarks") {
                this.dialog = Some(Dialog::Bookmarks);
            }
            ui.add_space(8.0);
            if settings_button(ui, p, "you_downloads", "Downloads") {
                this.dialog = Some(Dialog::Downloads);
            }
        });
    }

    fn settings_autofill(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let saved = engine::net::password_manager::PasswordManager::new(
            &self.profile_id,
            &self.data_dir,
        )
        .map(|manager| manager.list_credentials(&self.profile_id).len())
        .unwrap_or(0);
        let subtitle = match saved {
            0 => "No passwords saved in this profile".to_string(),
            1 => "1 password saved in this profile".to_string(),
            count => format!("{} passwords saved in this profile", count),
        };

        let card = settings_card(ui, p, 2);
        if settings_row(
            ui,
            p,
            card,
            0,
            "autofill_passwords",
            Icon::Key,
            "Passwords",
            Some(&subtitle),
            Trailing::Chevron,
        )
        .clicked()
        {
            self.dialog = Some(Dialog::Passwords);
        }
        let _ = settings_row(
            ui,
            p,
            card,
            1,
            "autofill_payments",
            Icon::Star,
            "Payment methods",
            Some("Not supported by this build yet"),
            Trailing::None,
        );
    }

    fn settings_privacy(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let vpn_on = self.vpn_enabled;
        let dummy_on = self.dummy_config.enabled;
        let location_set = self.location_config.is_some();
        let vpn_detail = if vpn_on {
            "On — this profile's traffic is tunnelled"
        } else {
            "Off — direct connection"
        };
        let dummy_detail = if dummy_on {
            "Fingerprint spoofing enabled"
        } else {
            "Spoof a real device fingerprint"
        };
        let location_detail = if location_set {
            "Set for this profile"
        } else {
            "No location override"
        };

        let card = settings_card(ui, p, 4);
        if settings_row(
            ui,
            p,
            card,
            0,
            "privacy_vpn",
            Icon::Shield,
            "VPN",
            Some(vpn_detail),
            Trailing::Toggle(vpn_on),
        )
        .clicked()
        {
            self.apply_action(Action::Vpn);
            self.settings_notice = Some(if self.vpn_enabled {
                "VPN turned on for this profile".to_string()
            } else {
                "VPN turned off".to_string()
            });
        }

        if settings_row(
            ui,
            p,
            card,
            1,
            "privacy_dummy",
            Icon::Device,
            "Dummy system profile",
            Some(dummy_detail),
            Trailing::Toggle(dummy_on),
        )
        .clicked()
        {
            self.dummy_config.enabled = !self.dummy_config.enabled;
            set_config(&self.profile_id, self.dummy_config.clone());
            self.settings_notice = Some("Dummy system profile updated".to_string());
        }

        if settings_row(
            ui,
            p,
            card,
            2,
            "privacy_location",
            Icon::MapPin,
            "Location profile",
            Some(location_detail),
            Trailing::Chevron,
        )
        .clicked()
        {
            self.side_panel = Some(SidePanel::Location);
            self.settings = None;
        }

        if settings_row(
            ui,
            p,
            card,
            3,
            "privacy_clear",
            Icon::Reload,
            "Clear browsing data",
            Some("Removes history and cookies for this profile"),
            Trailing::Chevron,
        )
        .clicked()
        {
            let _ = storage::history::clear(&self.data_dir);
            let _ = engine::net::cookies::clear_cookies(&self.profile_id, &self.data_dir);
            self.shortcuts_dirty = true;
            self.settings_notice = Some("History and cookies cleared".to_string());
        }
    }

    fn settings_appearance(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let dark = self.mode == Mode::Dark;
        let bar = self.bookmarks_bar_visible;

        let card = settings_card(ui, p, 3);
        if settings_row(
            ui,
            p,
            card,
            0,
            "appearance_light",
            Icon::Palette,
            "Light",
            None,
            Trailing::Radio(!dark),
        )
        .clicked()
        {
            self.set_mode(Mode::Light);
            self.settings_notice = Some("Light theme applied".to_string());
        }
        if settings_row(
            ui,
            p,
            card,
            1,
            "appearance_dark",
            Icon::Palette,
            "Dark",
            None,
            Trailing::Radio(dark),
        )
        .clicked()
        {
            self.set_mode(Mode::Dark);
            self.settings_notice = Some("Dark theme applied".to_string());
        }
        if settings_row(
            ui,
            p,
            card,
            2,
            "appearance_bar",
            Icon::Bookmark,
            "Show the bookmarks bar",
            Some("Ctrl+Shift+B"),
            Trailing::Toggle(bar),
        )
        .clicked()
        {
            self.bookmarks_bar_visible = !self.bookmarks_bar_visible;
            self.prefs.bookmarks_bar = self.bookmarks_bar_visible;
            self.save_prefs();
        }
    }

    fn settings_search(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let engine = self.prefs.engine.clone();
        let options = [
            ("google", "Google", "google.com"),
            ("duckduckgo", "DuckDuckGo", "duckduckgo.com"),
            ("bing", "Bing", "bing.com"),
        ];

        let card = settings_card(ui, p, options.len());
        for (index, (key, label, host)) in options.iter().enumerate() {
            if settings_row(
                ui,
                p,
                card,
                index,
                key,
                Icon::Search,
                label,
                Some(host),
                Trailing::Radio(engine == *key),
            )
            .clicked()
            {
                self.prefs.engine = (*key).to_string();
                self.save_prefs();
                self.settings_notice = Some(format!("Searches now go through {}", label));
            }
        }
    }

    fn settings_startup(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let new_tab = self.prefs.startup != "url";

        let card = settings_card(ui, p, 2);
        if settings_row(
            ui,
            p,
            card,
            0,
            "startup_new",
            Icon::Home,
            "Open the New Tab page",
            None,
            Trailing::Radio(new_tab),
        )
        .clicked()
        {
            self.prefs.startup = "newtab".to_string();
            self.save_prefs();
        }
        if settings_row(
            ui,
            p,
            card,
            1,
            "startup_url",
            Icon::Globe,
            "Open a specific page",
            None,
            Trailing::Radio(!new_tab),
        )
        .clicked()
        {
            self.prefs.startup = "url".to_string();
            self.save_prefs();
        }

        ui.add_space(14.0);
        ui.label(
            egui::RichText::new("Page to open")
                .font(fonts::ui_font(12.0))
                .color(p.text_dim),
        );
        ui.add_space(6.0);
        let changed = ui
            .add(
                egui::TextEdit::singleline(&mut self.prefs.startup_url)
                    .font(fonts::ui_font(13.0))
                    .hint_text("https://example.com")
                    .desired_width(400.0),
            )
            .changed();
        if changed {
            self.prefs.startup = "url".to_string();
            self.save_prefs();
        }
    }

    fn settings_downloads(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let stats = storage::download::stats(&self.data_dir);
        let subtitle = format!(
            "{} downloads · {}",
            stats.total_downloads,
            human_bytes(stats.total_bytes_downloaded)
        );
        let folder = self.data_dir.display().to_string();

        let card = settings_card(ui, p, 2);
        if settings_row(
            ui,
            p,
            card,
            0,
            "downloads_open",
            Icon::Download,
            "Downloads",
            Some(&subtitle),
            Trailing::Chevron,
        )
        .clicked()
        {
            self.dialog = Some(Dialog::Downloads);
        }
        let _ = settings_row(
            ui,
            p,
            card,
            1,
            "downloads_folder",
            Icon::Folder,
            "Profile data folder",
            Some(&folder),
            Trailing::None,
        );
    }

    fn settings_about(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let profile = self.profile_id.clone();

        let card = settings_card(ui, p, 3);
        let _ = settings_row(
            ui,
            p,
            card,
            0,
            "about_version",
            Icon::Info,
            "Version",
            Some(&version),
            Trailing::None,
        );
        let _ = settings_row(
            ui,
            p,
            card,
            1,
            "about_engine",
            Icon::Sparkle,
            "Layout engine",
            Some("Falco — from-scratch HTML/CSS/layout/paint"),
            Trailing::None,
        );
        let _ = settings_row(
            ui,
            p,
            card,
            2,
            "about_profile",
            Icon::Device,
            "Profile",
            Some(&profile),
            Trailing::None,
        );

        ui.add_space(16.0);
        let this = &mut *self;
        ui.horizontal(|ui| {
            if settings_button(ui, p, "about_reset", "Reset all settings") {
                this.prefs = Prefs::default();
                this.save_prefs();
                this.bookmarks_bar_visible = true;
                this.set_mode(Mode::Dark);
                this.settings_notice = Some("Settings reset to defaults".to_string());
            }
        });
    }
}

/// A rounded card sized for `rows` settings rows. The caller paints the rows
/// into the returned rectangle.
fn settings_card(ui: &mut egui::Ui, p: &Palette, rows: usize) -> Rect {
    let width = ui.available_width().min(CONTENT_MAX);
    let height = rows as f32 * ROW + 8.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    widgets::fill_round(ui.painter(), rect, 12.0, p.menu_bg);
    widgets::stroke_round(ui.painter(), rect, 12.0, p.menu_separator, 1.0);
    rect
}

fn settings_row(
    ui: &mut egui::Ui,
    p: &Palette,
    card: Rect,
    index: usize,
    id: &str,
    icon: Icon,
    title: &str,
    subtitle: Option<&str>,
    trailing: Trailing,
) -> egui::Response {
    let top = card.top() + 4.0 + index as f32 * ROW;
    let row = Rect::from_min_max(
        pos2(card.left() + 4.0, top),
        pos2(card.right() - 4.0, top + ROW),
    );
    let response = ui.interact(row, egui::Id::new(("settings_row", id)), Sense::click());
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
        Rect::from_center_size(pos2(row.left() + 24.0, row.center().y), Vec2::splat(18.0)),
        icon,
        p.icon,
    );

    let text_left = row.left() + 46.0;
    let text_right = row.right() - 58.0;
    match subtitle {
        Some(subtitle) => {
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(pos2(text_left, row.top() + 10.0), pos2(text_right, row.top() + 30.0)),
                title,
                fonts::ui_font(13.5),
                p.text,
            );
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(
                    pos2(text_left, row.top() + 27.0),
                    pos2(text_right, row.bottom() - 6.0),
                ),
                subtitle,
                fonts::ui_font(11.5),
                p.text_dim,
            );
        }
        None => {
            widgets::text_clipped(
                ui.painter(),
                ui.ctx(),
                Rect::from_min_max(pos2(text_left, row.top()), pos2(text_right, row.bottom())),
                title,
                fonts::ui_font(13.5),
                p.text,
            );
        }
    }

    match trailing {
        Trailing::Toggle(on) => paint_toggle(
            ui.painter(),
            Rect::from_center_size(pos2(row.right() - 34.0, row.center().y), vec2(34.0, 18.0)),
            on,
            p,
        ),
        Trailing::Radio(selected) => {
            paint_radio(ui.painter(), pos2(row.right() - 28.0, row.center().y), selected, p)
        }
        Trailing::Chevron => icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(row.right() - 24.0, row.center().y), Vec2::splat(18.0)),
            Icon::ChevronRight,
            p.text_dim,
        ),
        Trailing::Value(text) => {
            ui.painter().text(
                pos2(row.right() - 20.0, row.center().y),
                Align2::RIGHT_CENTER,
                text,
                fonts::ui_font(12.0),
                p.text_dim,
            );
        }
        Trailing::None => {}
    }

    ui.painter().hline(
        (row.left() + 12.0)..=(row.right() - 12.0),
        row.bottom(),
        Stroke::new(1.0, p.menu_separator),
    );
    response
}

fn settings_button(ui: &mut egui::Ui, p: &Palette, _id: &str, label: &str) -> bool {
    let font = fonts::medium(12.5);
    let text_width = ui.ctx().fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    let (rect, response) =
        ui.allocate_exact_size(vec2(text_width + 40.0, 36.0), Sense::click());
    let fill = if response.hovered() {
        widgets::blend(p.accent, Color32::WHITE, 0.14)
    } else {
        p.accent
    };
    widgets::fill_round(ui.painter(), rect, 18.0, fill);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, Color32::WHITE);
    response.clicked()
}

fn paint_toggle(painter: &egui::Painter, rect: Rect, on: bool, p: &Palette) {
    let radius = rect.height() / 2.0;
    if on {
        widgets::fill_round(painter, rect, radius, p.accent);
    } else {
        let track = widgets::blend(p.hover, Color32::WHITE, if p.is_dark { 0.10 } else { 0.0 });
        widgets::fill_round(painter, rect, radius, track);
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

fn paint_radio(painter: &egui::Painter, center: Pos2, selected: bool, p: &Palette) {
    painter.circle_stroke(
        center,
        8.0,
        Stroke::new(1.6, if selected { p.accent } else { p.text_dim }),
    );
    if selected {
        painter.circle_filled(center, 4.2, p.accent);
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}
