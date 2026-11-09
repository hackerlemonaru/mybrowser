//! Page rendering.
//!
//! Fetching and layout run on a worker thread: the UI thread only uploads a
//! finished pixel buffer, so typing, hovering and switching tabs stay smooth
//! while a page downloads. Finished pages are cached per tab so going back to a
//! tab is instant instead of a re-render.
//!
//! Some pages (search results, single-page apps) lay out blank in this engine
//! because their content is produced by JavaScript. When that happens the
//! worker renders a readable text view of the fetched document instead, so a
//! page never shows up as an empty canvas.

use super::*;
use std::time::Duration;

/// How long a render may run before the tab is released again. A worker that
/// hangs (huge inline scripts, pathological CSS) must not pin a tab on
/// "Loading" forever.
const RENDER_TIMEOUT: Duration = Duration::from_secs(20);

/// Finished page textures kept in memory. Each one is width × height × 4 bytes
/// (a 1600×900 tab is ~5.7 MB), so a small cache is the difference between tens
/// of megabytes and hundreds.
const PAGE_CACHE_LIMIT: usize = 4;

impl BrowserWindow {
    /// Draw the active tab's page: a cached texture when available, a progress
    /// bar while the worker thread renders it.
    pub(super) fn render_page(&mut self, ui: &mut egui::Ui, p: &Palette) {
        self.pump_render(ui.ctx());

        let Some(id) = self.tab_manager.active_tab() else {
            return;
        };

        // Release a tab whose render never came back.
        if let Some(pending) = &self.pending {
            if pending.started.elapsed() > RENDER_TIMEOUT {
                self.pending = None;
                self.viewport.texture = None;
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.title = "Page took too long".to_string();
                    tab.state = TabState::Active;
                    tab.load_progress = 1.0;
                }
            }
        }

        let avail = Vec2::new(
            ui.available_width().max(1.0),
            ui.available_height().max(1.0),
        );

        // Switching tabs reuses that tab's finished texture.
        if self.shown_tab != Some(id) {
            self.shown_tab = Some(id);
            self.viewport.texture = self.pages.get(&id).cloned();
            self.viewport.size = Vec2::ZERO;
            self.viewport.dirty = false;
        }
        // A navigation or a reload invalidates just this tab.
        if self.viewport.dirty {
            self.viewport.dirty = false;
            self.viewport.texture = None;
            self.pages.remove(&id);
            self.searches.remove(&id);
            self.viewport.size = Vec2::ZERO;
        }

        // Searches are drawn by the browser itself, so they stay readable and
        // clickable (search engines render nothing without JavaScript).
        if self.searches.contains_key(&id) {
            self.render_search_results(ui, p, id);
            return;
        }

        let resized = self.viewport.texture.is_some()
            && (self.viewport.size - avail).length() > 24.0;
        let busy = self.pending.as_ref().map(|pending| pending.tab) == Some(id);
        if (self.viewport.texture.is_none() || resized) && !busy {
            let url = self
                .tab_manager
                .get_tab(id)
                .map(|tab| tab.url.clone())
                .unwrap_or_default();
            if url.is_empty() || url == "about:blank" {
                self.viewport.texture = None;
            } else {
                self.start_render(id, url, avail);
            }
        }

        let area = ui.max_rect();
        if let Some(texture) = &self.viewport.texture {
            ui.add(egui::Image::new(texture).fit_to_exact_size(avail));
        } else if self.pending.is_some() {
            self.render_progress(ui, p, area);
            // Poll the worker: one cheap frame per tick instead of a blocked UI.
            ui.ctx().request_repaint();
        }
    }

    /// Results for a search URL, drawn by the browser so they stay readable
    /// and clickable — the engines' own pages are JavaScript-only and render
    /// as an empty canvas in this engine.
    fn render_search_results(&mut self, ui: &mut egui::Ui, p: &Palette, tab: TabId) {
        let Some(results) = self.searches.get(&tab).cloned() else {
            return;
        };
        let query = self
            .tab_manager
            .get_tab(tab)
            .and_then(|tab| search_query(&tab.url))
            .unwrap_or_default();
        let area = ui.max_rect();
        ui.painter().rect_filled(area, 0.0, p.page_bg);

        let left = area.left() + 96.0;
        ui.painter().text(
            pos2(left, area.top() + 30.0),
            Align2::LEFT_CENTER,
            format!("\"{}\"", query),
            fonts::medium(16.0),
            p.text,
        );
        ui.painter().text(
            pos2(left, area.top() + 50.0),
            Align2::LEFT_CENTER,
            format!("{} results · text view", results.len()),
            fonts::ui_font(11.5),
            p.text_dim,
        );

        let mut open: Option<String> = None;
        ui.add_space(74.0);
        egui::ScrollArea::vertical()
            .max_height((area.height() - 86.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in &results {
                    let (row, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 78.0), Sense::click());
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
                    widgets::text_clipped(
                        ui.painter(),
                        ui.ctx(),
                        Rect::from_min_max(
                            pos2(row.left() + 96.0, row.top() + 8.0),
                            pos2(row.right() - 96.0, row.top() + 30.0),
                        ),
                        &item.title,
                        fonts::medium(14.0),
                        p.accent,
                    );
                    widgets::text_clipped(
                        ui.painter(),
                        ui.ctx(),
                        Rect::from_min_max(
                            pos2(row.left() + 96.0, row.top() + 29.0),
                            pos2(row.right() - 96.0, row.top() + 47.0),
                        ),
                        &item.url,
                        fonts::ui_font(11.5),
                        p.text_dim,
                    );
                    if !item.snippet.is_empty() {
                        widgets::text_clipped(
                            ui.painter(),
                            ui.ctx(),
                            Rect::from_min_max(
                                pos2(row.left() + 96.0, row.top() + 45.0),
                                pos2(row.right() - 96.0, row.bottom() - 4.0),
                            ),
                            &item.snippet,
                            fonts::ui_font(11.5),
                            p.text,
                        );
                    }
                    if response.clicked() {
                        open = Some(item.url.clone());
                    }
                }
            });

        if let Some(url) = open {
            self.navigate_to(&url);
        }
    }

    fn start_render(&mut self, tab: TabId, url: String, size: Vec2) {
        let profile_id = self.profile_id.clone();
        let opts = engine::RenderOptions {
            width: size.x.max(1.0) as u32,
            height: size.y.max(1.0) as u32,
            background: Palette::for_mode(self.mode).engine_background(),
            animation_time_ms: 0,
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(render_document(&url, &profile_id, opts));
        });
        self.pending = Some(PendingRender {
            tab,
            rx,
            size,
            started: Instant::now(),
        });
        if let Some(active) = self.tab_manager.get_tab_mut(tab) {
            active.state = TabState::Loading;
        }
    }

    fn pump_render(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let outcome = match pending.rx.try_recv() {
            Ok(outcome) => outcome,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                return;
            }
        };
        let tab = pending.tab;
        let size = pending.size;
        self.pending = None;

        if let Some(active) = self.tab_manager.get_tab_mut(tab) {
            active.state = TabState::Active;
            active.load_progress = 1.0;
            if !outcome.title.trim().is_empty() {
                active.title = outcome.title;
            }
        }

        // A search: keep the parsed rows for the browser's own results view.
        if !outcome.results.is_empty() {
            self.searches.insert(tab, outcome.results);
            if self.tab_manager.active_tab() == Some(tab) {
                self.viewport.texture = None;
                self.viewport.size = size;
            }
            ctx.request_repaint();
            return;
        }

        match outcome.buffer {
            Ok(buffer) => {
                // Evict an old tab before inserting, so the cache stays bounded.
                if self.pages.len() >= PAGE_CACHE_LIMIT {
                    let active = self.tab_manager.active_tab();
                    let evict = self
                        .pages
                        .keys()
                        .copied()
                        .find(|cached| Some(*cached) != active);
                    if let Some(evict) = evict {
                        self.pages.remove(&evict);
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [buffer.width as usize, buffer.height as usize],
                    &buffer.pixels,
                );
                let texture = ctx.load_texture(
                    format!("page_{}", tab.as_u64()),
                    image,
                    TextureOptions::LINEAR,
                );
                self.pages.insert(tab, texture.clone());
                if self.tab_manager.active_tab() == Some(tab) {
                    self.viewport.texture = Some(texture);
                    self.viewport.size = size;
                }
            }
            Err(_) => {
                if let Some(active) = self.tab_manager.get_tab_mut(tab) {
                    active.title = "Problem loading page".to_string();
                }
            }
        }
        ctx.request_repaint();
    }

    /// Chrome's indeterminate loading bar, drawn along the top of the page.
    fn render_progress(&self, ui: &egui::Ui, p: &Palette, area: Rect) {
        let time = ui.ctx().input(|i| i.time) as f32;
        let t = (time * 0.8).fract();
        let width = (area.width() * 0.28).max(140.0);
        let x = area.left() - width + (area.width() + width) * t;
        let bar = Rect::from_min_max(
            pos2(x.max(area.left()), area.top()),
            pos2((x + width).min(area.right()), area.top() + 3.0),
        );
        if bar.width() > 1.0 {
            ui.painter().rect_filled(bar, 2.0, p.accent);
        }
    }
}

/// Fetch + lay out one page. Runs on a worker thread.
fn render_document(url: &str, profile_id: &str, opts: engine::RenderOptions) -> RenderOutcome {
    // Searches get the browser's own results view: the engines' result pages
    // need JavaScript, so rendering them here would only show an empty canvas.
    if let Some(query) = search_query(url) {
        return render_search(url, &query, profile_id, opts);
    }

    let html = match fetch(url, Some(profile_id)) {
        Ok(html) => html,
        Err(_) => {
            let page = error_page(
                url,
                "No internet connection",
                "MyBrowser could not reach this address. Check the connection and try again.",
            );
            return RenderOutcome {
                url: url.to_string(),
                title: extract_domain(url),
                buffer: engine::render_to_buffer(&page, "", opts).map_err(|e| e.to_string()),
                results: Vec::new(),
            };
        }
    };

    // External scripts are not executed. The engine would download and
    // interpret whole framework bundles on this thread (megabytes of JS for a
    // single page), which is what left tabs loading for minutes; inline scripts
    // still run.
    let html = strip_external_scripts(&html);

    let title = extract_page_title(&html).unwrap_or_else(|| extract_domain(url));
    let mut buffer =
        engine::render_to_buffer(&html, "", opts.clone()).map_err(|e| e.to_string());

    let blank = match &buffer {
        Ok(rendered) => buffer_is_blank(rendered),
        Err(_) => true,
    };
    if blank {
        let reader = reader_page(url, &html, &title);
        buffer = match engine::render_to_buffer(&reader, "", opts.clone()) {
            Ok(rendered) => Ok(rendered),
            Err(_) => {
                let page = error_page(
                    url,
                    "This page could not be rendered",
                    "The document uses features this build does not support yet.",
                );
                engine::render_to_buffer(&page, "", opts).map_err(|e| e.to_string())
            }
        };
    }

    RenderOutcome {
        url: url.to_string(),
        title,
        buffer,
        results: Vec::new(),
    }
}

/// Fetch a search page and read its results. Runs on a worker thread.
fn render_search(
    url: &str,
    query: &str,
    profile_id: &str,
    opts: engine::RenderOptions,
) -> RenderOutcome {
    let mut results = Vec::new();
    if let Ok(html) = fetch(url, Some(profile_id)) {
        results = parse_results(&html);
    }
    // Google and Bing build their result list with JavaScript. Fall back to the
    // no-JS DuckDuckGo endpoint when the chosen engine returned little text.
    if results.len() < 3 {
        let endpoint = format!("https://html.duckduckgo.com/html/?q={}", encode_query(query));
        if let Ok(html) = fetch(&endpoint, Some(profile_id)) {
            let fallback = parse_results(&html);
            if fallback.len() > results.len() {
                results = fallback;
            }
        }
    }

    let title = format!("{} - Search", query);
    let buffer = if results.is_empty() {
        let page = format!(
            "<html><head><title>{}</title></head><body><h2>{}</h2><p>No results could be read for this search.</p></body></html>",
            escape_html(query),
            escape_html(query)
        );
        engine::render_to_buffer(&page, "", opts).map_err(|e| e.to_string())
    } else {
        // Results are drawn by the browser, so no pixels are needed.
        Err(String::new())
    };

    RenderOutcome {
        url: url.to_string(),
        title,
        buffer,
        results,
    }
}

/// Result rows from a search page. Title anchors become rows; anchors marked
/// `result__snippet` become the snippet of the row above them.
fn parse_results(html: &str) -> Vec<SearchResult> {
    let lower = html.to_ascii_lowercase();
    let mut out: Vec<SearchResult> = Vec::new();
    let mut pos = 0usize;
    while let Some(found) = lower[pos..].find("<a ") {
        let start = pos + found;
        let Some(tag_len) = lower[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_len;
        let tag = &html[start..tag_end];
        let close = lower[tag_end..]
            .find("</a>")
            .map(|end| tag_end + end)
            .unwrap_or(lower.len());
        let inner = html.get(tag_end + 1..close).unwrap_or("");
        let text = html_to_text(inner).trim().to_string();
        let class = attribute(tag, "class");

        if class.contains("result__snippet") {
            if let Some(last) = out.last_mut() {
                if last.snippet.is_empty() {
                    last.snippet = text;
                }
            }
        } else if let Some(href) = normalise_href(&attribute(tag, "href")) {
            if text.len() > 8
                && !is_boilerplate(&href)
                && !out.iter().any(|row| row.url == href)
            {
                out.push(SearchResult {
                    title: text,
                    url: href,
                    snippet: String::new(),
                });
            }
        }

        pos = if close < lower.len() {
            close + 4
        } else {
            lower.len()
        };
        if out.len() > 30 || pos >= lower.len() {
            break;
        }
    }
    out
}

/// Remove `<script src=…>` elements, keeping inline scripts. External bundles
/// are what make layout take minutes in this engine.
fn strip_external_scripts(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut pos = 0usize;
    while let Some(found) = lower[pos..].find("<script") {
        let start = pos + found;
        out.push_str(&html[pos..start]);
        let close = lower[start..]
            .find("</script>")
            .map(|end| start + end + 9)
            .unwrap_or(lower.len());
        let tag_end = lower[start..]
            .find('>')
            .map(|end| start + end)
            .unwrap_or(close);
        let tag = &lower[start..tag_end.min(lower.len())];
        if !tag.contains("src=") {
            out.push_str(html.get(start..close).unwrap_or(""));
        }
        pos = close;
        if pos >= lower.len() {
            break;
        }
    }
    out.push_str(html.get(pos..).unwrap_or(""));
    out
}

/// True when the rendered canvas holds essentially one flat colour, i.e. the
/// engine produced nothing visible. A page that paints a background but no text
/// (every JavaScript-only page) counts as blank.
fn buffer_is_blank(buffer: &engine::RenderedBuffer) -> bool {
    let pixels = &buffer.pixels;
    let total = pixels.len() / 4;
    if total == 0 {
        return true;
    }
    let step = (total / 20_000).max(1);
    let mut base: Option<(u8, u8, u8)> = None;
    let mut different = 0usize;
    let mut index = 0usize;
    while index < total {
        let offset = index * 4;
        let pixel = (pixels[offset], pixels[offset + 1], pixels[offset + 2]);
        match base {
            None => base = Some(pixel),
            Some(base) => {
                let delta = (pixel.0 as i16 - base.0 as i16).abs()
                    + (pixel.1 as i16 - base.1 as i16).abs()
                    + (pixel.2 as i16 - base.2 as i16).abs();
                if delta > 24 {
                    different += 1;
                    if different > 12 {
                        return false;
                    }
                }
            }
        }
        index += step;
    }
    true
}

/// A plain, readable view of a document the engine laid out blank.
fn reader_page(url: &str, html: &str, title: &str) -> String {
    let mut body = String::new();
    if let Some(query) = search_query(url) {
        body.push_str(&format!("<h1>{}</h1>", escape_html(&query)));
        body.push_str("<p>Text view of the search results.</p>");
        let links = extract_links(html);
        for (text, href) in links.iter().take(20) {
            body.push_str(&format!(
                "<h2><a href=\"{}\">{}</a></h2><p>{}</p>",
                escape_html(href),
                escape_html(text),
                escape_html(&extract_domain(href))
            ));
        }
        if links.is_empty() {
            body.push_str("<p>No readable results were found on this page.</p>");
        }
    } else {
        body.push_str(&format!("<h1>{}</h1>", escape_html(title)));
        body.push_str(&format!("<p>{}</p>", escape_html(url)));
        let text = html_to_text(html);
        for line in text
            .split('\n')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(150)
        {
            body.push_str(&format!("<p>{}</p>", escape_html(line)));
        }
    }
    format!(
        "<html><head><title>{}</title></head><body>{}</body></html>",
        escape_html(title),
        body
    )
}

fn error_page(url: &str, heading: &str, detail: &str) -> String {
    format!(
        "<html><head><title>{}</title></head><body><h1>{}</h1><p>{}</p><p>{}</p></body></html>",
        escape_html(heading),
        escape_html(heading),
        escape_html(detail),
        escape_html(url)
    )
}

/// The `q` value of a search URL, if this is one.
fn search_query(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "q" || key == "query" || key == "p" {
                let decoded = percent_decode(value).replace('+', " ");
                if !decoded.trim().is_empty() {
                    return Some(decoded);
                }
            }
        }
    }
    None
}

/// Outgoing links from a document, as (visible text, absolute URL).
fn extract_links(html: &str) -> Vec<(String, String)> {
    let lower = html.to_ascii_lowercase();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut pos = 0usize;
    while let Some(found) = lower[pos..].find("<a ") {
        let start = pos + found;
        let Some(tag_len) = lower[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_len;
        let tag = &html[start..tag_end];
        let close = lower[tag_end..]
            .find("</a>")
            .map(|end| tag_end + end)
            .unwrap_or(lower.len());
        let inner = html.get(tag_end + 1..close).unwrap_or("");
        let text = html_to_text(inner).trim().to_string();
        if let Some(href) = normalise_href(&attribute(tag, "href")) {
            if text.len() > 2 && !is_boilerplate(&href) && !seen.contains(&href) {
                seen.push(href.clone());
                out.push((text, href));
            }
        }
        pos = if close < lower.len() { close + 4 } else { lower.len() };
        if out.len() > 60 || pos >= lower.len() {
            break;
        }
    }
    out
}

/// Chrome/Google navigation chrome is not a search result.
fn is_boilerplate(href: &str) -> bool {
    const SKIP: [&str; 14] = [
        "google.com/search",
        "accounts.google",
        "support.google",
        "policies.google",
        "preferences",
        "translate.google",
        "maps.google",
        "youtube.com",
        "gstatic.com",
        "googleusercontent",
        "duckduckgo.com/settings",
        "bing.com/account",
        "javascript:",
        "mailto:",
    ];
    SKIP.iter().any(|needle| href.contains(needle))
}

fn normalise_href(href: &str) -> Option<String> {
    let href = decode_entities(href.trim());
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    // DuckDuckGo and Google wrap outbound links in redirect URLs.
    if let Some(at) = href.find("uddg=") {
        let target = href[at + 5..].split('&').next().unwrap_or("");
        let target = percent_decode(target);
        if target.starts_with("http") {
            return Some(target);
        }
    }
    if let Some(rest) = href.strip_prefix("/url?q=") {
        let target = rest.split('&').next().unwrap_or("");
        let target = percent_decode(target);
        if target.starts_with("http") {
            return Some(target);
        }
        return None;
    }
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("https://{}", rest));
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href);
    }
    None
}

fn attribute(tag: &str, name: &str) -> String {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{}=", name);
    let Some(at) = lower.find(&needle) else {
        return String::new();
    };
    let rest = tag[at + needle.len()..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next().unwrap_or("").to_string()
    } else if let Some(stripped) = rest.strip_prefix('\'') {
        stripped.split('\'').next().unwrap_or("").to_string()
    } else {
        rest.split(|c: char| c.is_whitespace() || c == '>')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

/// Visible text of a document: tags removed, scripts and styles dropped, one
/// line per block element.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip = 0usize;
    let mut chars = html.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '<' {
            let end = html[index..]
                .find('>')
                .map(|at| index + at)
                .unwrap_or(html.len());
            let raw = html.get(index + 1..end).unwrap_or("");
            let tag = raw.trim().to_ascii_lowercase();
            let closing = tag.starts_with('/');
            let name = tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("")
                .to_string();
            if matches!(name.as_str(), "script" | "style" | "noscript" | "head") {
                if closing {
                    skip = skip.saturating_sub(1);
                } else if !tag.ends_with('/') {
                    skip += 1;
                }
            } else if skip == 0
                && matches!(
                    name.as_str(),
                    "p" | "div"
                        | "br"
                        | "li"
                        | "tr"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "section"
                        | "article"
                        | "title"
                )
                && !out.ends_with('\n')
            {
                out.push('\n');
            }
            while let Some(&(next, _)) = chars.peek() {
                if next > end {
                    break;
                }
                chars.next();
            }
            continue;
        }
        if skip == 0 {
            out.push(ch);
        }
    }
    // Squash runs of spaces so paragraphs stay readable.
    let mut text = String::with_capacity(out.len());
    let mut space = false;
    for ch in out.chars() {
        if ch == '\n' {
            text.push('\n');
            space = false;
        } else if ch.is_whitespace() {
            if !space {
                text.push(' ');
                space = true;
            }
        } else {
            text.push(ch);
            space = false;
        }
    }
    decode_entities(&text)
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(semicolon) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        if semicolon > 10 {
            out.push('&');
            rest = &tail[1..];
            continue;
        }
        let entity = &tail[1..semicolon];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" | "#x27" => Some('\''),
            "nbsp" => Some(' '),
            "ndash" => Some('–'),
            "mdash" => Some('—'),
            "hellip" => Some('…'),
            "rsquo" => Some('’'),
            "lsquo" => Some('‘'),
            "ldquo" => Some('“'),
            "rdquo" => Some('”'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|dec| dec.parse::<u32>().ok())
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => out.push(ch),
            None => out.push_str(&tail[..=semicolon]),
        }
        rest = &tail[semicolon + 1..];
    }
    out.push_str(rest);
    out
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = (bytes[index + 1] as char).to_digit(16);
            let lo = (bytes[index + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
