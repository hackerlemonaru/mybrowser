"""One-shot patcher for the Chrome-shell wiring.

Kept in the tree as a record of the mechanical edits: every replacement asserts
its expected occurrence count, so a mismatch fails loudly instead of silently
producing a half-patched file.
"""

import pathlib

w = pathlib.Path("browser/src/window.rs")
s = w.read_text(encoding="utf-8")


def sub(old, new, count=1):
    global s
    found = s.count(old)
    if found != count:
        raise SystemExit(
            "FAIL expected %d found %d: %r"
            % (count, found, old.splitlines()[0][:80] if old else "")
        )
    s = s.replace(old, new, count)


def line_start(i):
    return s.rfind("\n", 0, i) + 1


def cut(start_text, end_text):
    global s
    i = line_start(s.index(start_text))
    j = line_start(s.index(end_text, i))
    s = s[:i] + s[j:]


def insert_before(marker, text):
    global s
    i = line_start(s.index(marker))
    s = s[:i] + text + s[i:]


# 1. new child modules
sub(
    """    widgets,
};

/// Height of a menu row in the 3-dot menu.""",
    """    widgets,
};

mod page;
mod popovers;
mod settings;

/// Height of a menu row in the 3-dot menu.""",
)

# 2. PendingRender carries the size it renders for
sub(
    """struct PendingRender {
    tab: TabId,
    rx: Receiver<RenderOutcome>,
}""",
    """struct PendingRender {
    tab: TabId,
    rx: Receiver<RenderOutcome>,
    /// Size the page was laid out for.
    size: Vec2,
}""",
)

# 3. new window fields
sub(
    """    pending: Option<PendingRender>,
    settings: Option<SettingsSection>,""",
    """    pending: Option<PendingRender>,
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
    settings: Option<SettingsSection>,""",
)

sub(
    """            pending: None,
            settings: None,""",
    """            pending: None,
            shown_tab: None,
            bookmarks_cache: Vec::new(),
            bookmarks_dirty: true,
            shortcuts_cache: Vec::new(),
            shortcuts_dirty: true,
            popup_slide: 0.0,
            settings: None,""",
)

# 4. bookmark cache helpers
insert_before(
    "    fn save_prefs(&self) {",
    """    /// Reload the cached bookmark list (only when something changed).
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

""",
)

# 5. per-frame popover animation + cached bookmarks in render()
sub(
    """        self.apply_egui_visuals(ctx, &p);
""",
    """        self.apply_egui_visuals(ctx, &p);
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
""",
)

sub(
    "        if self.bookmarks_bar_visible && !storage::bookmarks::list(&self.data_dir).is_empty() {",
    "        if self.bookmarks_bar_visible && !self.bookmarks_cache.is_empty() {",
)

# 6. New Tab shortcuts from the cache
cut(
    "    /// Recent sites for the New Tab shortcuts (bookmarks first, then history).",
    "Page viewport",
)
insert_before(
    "Page viewport",
    """    /// Recent sites for the New Tab shortcuts (bookmarks first, then history).
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

""",
)

# 7. drop the old synchronous page renderer (page.rs replaces it)
cut("Page viewport", "Menus / popups")

# 8. bookmarks bar reads the cache
sub(
    "        let entries = storage::bookmarks::list(&self.data_dir);",
    "        let entries = self.bookmarks().to_vec();",
)
sub(
    "f.layout_no_wrap(bookmark.title.clone(), font.clone(), Color32::WHITE)",
    "f.layout_no_wrap(bookmark.0.clone(), font.clone(), Color32::WHITE)",
)
sub(
    'let id = egui::Id::new(("bookmark_item", bookmark.id.clone()));',
    'let id = egui::Id::new(("bookmark_item", bookmark.1.clone()));',
)
sub(
    "                &bookmark.title,\n                font,\n                p.text,",
    "                &bookmark.0,\n                font,\n                p.text,",
)
sub(
    "            let url = bookmark.url.clone();",
    "            let url = bookmark.1.clone();",
)
sub(
    "            response.on_hover_text(&bookmark.url);",
    "            response.on_hover_text(&bookmark.1);",
)

sub(
    """    fn is_active_bookmarked(&self) -> bool {
        let url = self.active_url();
        !url.is_empty()
            && url != "about:blank"
            && storage::bookmarks::list(&self.data_dir)
                .iter()
                .any(|bookmark| bookmark.url == url)
    }""",
    """    fn is_active_bookmarked(&mut self) -> bool {
        let url = self.active_url();
        if url.is_empty() || url == "about:blank" {
            return false;
        }
        self.bookmarks()
            .iter()
            .any(|(_, bookmark_url)| bookmark_url == &url)
    }""",
)

sub(
    """            let _ = storage::bookmarks::add(&self.data_dir, &url, &title, "Bookmarks bar");
        }
    }""",
    """            let _ = storage::bookmarks::add(&self.data_dir, &url, &title, "Bookmarks bar");
        }
        self.bookmarks_dirty = true;
        self.shortcuts_dirty = true;
    }""",
)

# 9. navigation: search engine from prefs, shortcut cache invalidation
sub(
    "        let url = resolve_address(input);",
    "        let url = resolve_address(input, &self.prefs.engine);",
)
sub(
    '        let _ = storage::history::add_visit(&self.data_dir, &url, "");',
    '        let _ = storage::history::add_visit(&self.data_dir, &url, "");\n        self.shortcuts_dirty = true;',
)

sub(
    "fn resolve_address(input: &str) -> String {",
    "fn resolve_address(input: &str, engine: &str) -> String {",
)
sub(
    """    } else {
        format!("https://www.google.com/search?q={}", encode_query(value))
    }
}""",
    """    } else {
        let query = encode_query(value);
        match engine {
            "duckduckgo" => format!("https://duckduckgo.com/?q={}", query),
            "bing" => format!("https://www.bing.com/search?q={}", query),
            _ => format!("https://www.google.com/search?q={}", query),
        }
    }
}""",
)

# 10. actions: settings page, persisted theme/bookmarks bar, no Exit
sub(
    """            Action::LightMode => {
                self.mode = Mode::Light;
                self.viewport.dirty = true;
                self.popup = None;
            }
            Action::DarkMode => {
                self.mode = Mode::Dark;
                self.viewport.dirty = true;
                self.popup = None;
            }
            Action::ToggleBookmarksBar => self.bookmarks_bar_visible = !self.bookmarks_bar_visible,
            Action::Settings => {
                self.side_panel = Some(SidePanel::Dummy);
                self.popup = None;
            }""",
    """            Action::LightMode => {
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
            }""",
)

sub(
    """                let (enabled, _, _, connected, message) =
                    ProxyManager::vpn_status(&self.profile_id);
                self.vpn_enabled = enabled;
                self.vpn_message = message;
                let _ = connected;""",
    """                let (enabled, provider, endpoint, connected, message) =
                    ProxyManager::vpn_status(&self.profile_id);
                self.vpn_enabled = enabled;
                self.vpn_provider = provider;
                self.vpn_endpoint = endpoint;
                self.vpn_message = message;
                let _ = connected;""",
)

sub(
    """            Action::Exit => {
                std::process::exit(0);
            }
""",
    "",
)

sub(
    """            MenuEntry::Row {
                icon: Icon::Exit,
                label: "Exit".into(),
                shortcut: None,
                action: Action::Exit,
                checked: None,
                danger: true,
            },
""",
    "",
)

# 11. popover slide offsets
for anchor in [
    '"chrome_menu"',
    '"tab_search_popup"',
    '"extensions_popup"',
    '"appearance_popup"',
]:
    sub(
        "egui::Id::new(%s))\n            .order(egui::Order::Foreground)\n            .fixed_pos(pos)"
        % anchor,
        "egui::Id::new(%s))\n            .order(egui::Order::Foreground)\n            .fixed_pos(pos + vec2(0.0, self.popup_slide))"
        % anchor,
    )

# 12. New Tab avatar opens the profile bubble
sub(
    """            if avatar.clicked() {
                self.popup = Some(Popup::Menu);
            }""",
    """            if avatar.clicked() {
                self.profile_anchor =
                    Some(Rect::from_center_size(avatar_centre, Vec2::splat(32.0)));
                self.popup = if self.popup == Some(Popup::Profile) {
                    None
                } else {
                    Some(Popup::Profile)
                };
            }""",
)

# 13. VPN state dot on the toolbar shield
sub(
    "        widgets::paint_icon_button(ui, p, vpn_rect, Icon::Shield, &vpn_response, true);",
    """        widgets::paint_icon_button(ui, p, vpn_rect, Icon::Shield, &vpn_response, true);
        if self.vpn_enabled {
            ui.painter().circle_filled(
                pos2(vpn_rect.right() - 9.0, vpn_rect.bottom() - 9.0),
                3.0,
                p.accent,
            );
        }""",
)

# 14. omnibox hint follows the chosen search engine
sub(
    """        let hint = "Search Google or type a URL";
        let response = ui.put(
            edit_rect,
            TextEdit::singleline(&mut self.address_bar)
                .frame(false)
                .font(fonts::ui_font(13.0))
                .text_color(p.omnibox_text)
                .hint_text(hint)""",
    """        let engine_name = match self.prefs.engine.as_str() {
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
                .hint_text(hint)""",
)

w.write_text(s, encoding="utf-8", newline="\r\n")
print("window.rs patched:", len(s.splitlines()), "lines")

# main.rs: stop the idle repaint loop
m = pathlib.Path("browser/src/main.rs")
t = m.read_text(encoding="utf-8")
old = """        unsafe { gl_ctx.swap_buffers() };
        window.request_redraw();"""
new = """        unsafe { gl_ctx.swap_buffers() };

        // Repaint only when egui actually asked for one: the old unconditional
        // `request_redraw` kept the GPU busy at 100% while the browser was idle.
        if egui_glow.egui_ctx.has_requested_repaint() {
            window.request_redraw();
        }"""
if t.count(old) != 1:
    raise SystemExit("FAIL main.rs repaint loop")
m.write_text(t.replace(old, new), encoding="utf-8", newline="\r\n")
print("main.rs patched")

# icons.rs: a cleaner Material "extension" piece
i = pathlib.Path("browser/src/ui/icons.rs")
c = i.read_text(encoding="utf-8")
old_icon = """        Icon::Extension => {
            // Puzzle piece: body plus the interlocking tab.
            pen.rrect(4.4, 9.0, 19.6, 20.4, 2.2);
            pen.arc(12.0, 9.0, 2.7, 180.0, 360.0);
            pen.line(&[(6.2, 9.0), (9.3, 9.0)]);
            pen.line(&[(14.7, 9.0), (17.8, 9.0)]);
        }"""
new_icon = """        Icon::Extension => {
            // Material "extension": a filled puzzle piece with a knob on the
            // top edge, which stays readable at toolbar size.
            pen.fill(&[
                (4.2, 9.2),
                (8.3, 9.2),
                (9.0, 7.0),
                (10.1, 5.4),
                (12.0, 4.8),
                (13.9, 5.4),
                (15.0, 7.0),
                (15.7, 9.2),
                (19.8, 9.2),
                (19.8, 17.8),
                (17.8, 19.8),
                (4.2, 19.8),
            ]);
        }"""
if c.count(old_icon) != 1:
    raise SystemExit("FAIL extensions glyph")
i.write_text(c.replace(old_icon, new_icon), encoding="utf-8", newline="\r\n")
print("icons.rs patched")
