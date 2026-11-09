//! Vector icon set for the browser chrome.
//!
//! Every glyph is drawn on Material's 24x24 grid with 2dp strokes, the same
//! geometry Google uses for Chrome's toolbar icons (arrow_back, refresh,
//! star, more_vert, add, close, lock, tune, mic, ...). Drawing them as vectors
//! keeps them pixel-crisp at any DPI and needs no bitmap assets.

use egui::{pos2, Color32, Pos2, Rect, Shape, Stroke, Vec2};

/// Every icon the browser chrome can draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Navigation
    Back,
    Forward,
    Reload,
    Stop,
    Home,
    // Omnibox
    Lock,
    Tune,
    Globe,
    Search,
    Mic,
    Lens,
    GoogleG,
    Star,
    StarFilled,
    Share,
    Translate,
    // Tab strip
    Plus,
    Close,
    Menu,
    TabSearch,
    Speaker,
    SpeakerMuted,
    Pin,
    // Window controls
    Minimize,
    Maximize,
    Restore,
    // Menu
    NewWindow,
    Incognito,
    History,
    Download,
    Bookmark,
    Key,
    Shield,
    Device,
    MapPin,
    Zoom,
    Print,
    Palette,
    Settings,
    Help,
    Info,
    ChevronRight,
    Exit,
    Apps,
    Extension,
    Pencil,
    Sparkle,
    Folder,
    Check,
}

/// Draw `icon` centred in `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    paint_weighted(painter, rect, icon, color, 2.0);
}

/// Draw `icon` centred in `rect` with a custom Material stroke weight.
pub fn paint_weighted(
    painter: &egui::Painter,
    rect: Rect,
    icon: Icon,
    color: Color32,
    weight: f32,
) {
    let side = rect.width().min(rect.height());
    // The 24-unit grid is inset by 1 so 2dp strokes never clip.
    let scale = side / 24.0;
    let pen = Pen {
        painter,
        origin: rect.center() - Vec2::splat(12.0 * scale),
        scale,
        color,
        stroke: Stroke::new(weight * scale, color),
    };
    glyph(&pen, icon);
}

/// A tiny drawing helper that maps 24x24 grid coordinates into screen space.
struct Pen<'a> {
    painter: &'a egui::Painter,
    origin: Pos2,
    scale: f32,
    color: Color32,
    stroke: Stroke,
}

impl<'a> Pen<'a> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.scale
    }

    fn points(&self, pts: &[(f32, f32)]) -> Vec<Pos2> {
        pts.iter().map(|&(x, y)| self.p(x, y)).collect()
    }

    /// Open polyline.
    fn line(&self, pts: &[(f32, f32)]) {
        if pts.len() < 2 {
            return;
        }
        self.painter
            .add(Shape::line(self.points(pts), self.stroke));
    }

    /// Closed polyline outline.
    fn outline(&self, pts: &[(f32, f32)]) {
        if pts.len() < 2 {
            return;
        }
        self.painter
            .add(Shape::closed_line(self.points(pts), self.stroke));
    }

    /// Filled polygon.
    fn fill(&self, pts: &[(f32, f32)]) {
        if pts.len() < 3 {
            return;
        }
        self.painter
            .add(Shape::convex_polygon(self.points(pts), self.color, Stroke::NONE));
    }

    fn dot(&self, cx: f32, cy: f32, r: f32) {
        self.painter
            .circle_filled(self.p(cx, cy), r * self.scale, self.color);
    }

    fn ring(&self, cx: f32, cy: f32, r: f32) {
        self.painter
            .circle_stroke(self.p(cx, cy), r * self.scale, self.stroke);
    }

    /// Circular arc in degrees (0 = 3 o'clock, positive = clockwise).
    fn arc(&self, cx: f32, cy: f32, r: f32, from_deg: f32, to_deg: f32) {
        let steps = 28;
        let from = from_deg.to_radians();
        let to = to_deg.to_radians();
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = from + (to - from) * (i as f32 / steps as f32);
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
        self.line(&pts);
    }

    fn rrect(&self, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) {
        let rect = Rect::from_min_max(self.p(x0, y0), self.p(x1, y1));
        self.painter
            .rect_stroke(rect, egui::Rounding::same(radius * self.scale), self.stroke);
    }

    fn rrect_fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) {
        let rect = Rect::from_min_max(self.p(x0, y0), self.p(x1, y1));
        self.painter
            .rect_filled(rect, egui::Rounding::same(radius * self.scale), self.color);
    }
}

/// Google's four-colour "G" mark (omnibox leading icon, New Tab brand spot).
///
/// The mark is not monochrome, so it is painted directly rather than through
/// the single-colour `Pen` used by the rest of the set.
pub fn google_g(painter: &egui::Painter, rect: Rect) {
    let radius = rect.width().min(rect.height()) * 0.5;
    let centre = rect.center();
    let width = (radius * 0.36).max(1.5);

    let red = Color32::from_rgb(0xEA, 0x43, 0x35);
    let yellow = Color32::from_rgb(0xFB, 0xBC, 0x05);
    let green = Color32::from_rgb(0x34, 0xA8, 0x53);
    let blue = Color32::from_rgb(0x42, 0x85, 0xF4);

    let arc = |from_deg: f32, to_deg: f32, color: Color32| {
        let steps = 32;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = (from_deg + (to_deg - from_deg) * (i as f32 / steps as f32)).to_radians();
            pts.push(pos2(
                centre.x + radius * a.cos(),
                centre.y + radius * a.sin(),
            ));
        }
        painter.add(Shape::line(pts, Stroke::new(width, color)));
    };

    // Angles are screen-space (0° = 3 o'clock, 90° = 6 o'clock).
    arc(205.0, 305.0, red);
    arc(130.0, 205.0, yellow);
    arc(70.0, 130.0, green);
    arc(20.0, 70.0, blue);

    // The horizontal bar that closes the G on the right.
    painter.add(Shape::line(
        vec![
            pos2(centre.x + radius * 0.08, centre.y),
            pos2(centre.x + radius, centre.y),
        ],
        Stroke::new(width, blue),
    ));
}

fn glyph(pen: &Pen<'_>, icon: Icon) {
    match icon {
        // ── Navigation ──
        Icon::Back => {
            pen.line(&[(19.5, 12.0), (5.0, 12.0)]);
            pen.line(&[(11.5, 5.5), (5.0, 12.0), (11.5, 18.5)]);
        }
        Icon::Forward => {
            pen.line(&[(4.5, 12.0), (19.0, 12.0)]);
            pen.line(&[(12.5, 5.5), (19.0, 12.0), (12.5, 18.5)]);
        }
        Icon::Reload => {
            pen.arc(12.0, 12.0, 6.8, 40.0, 330.0);
            pen.line(&[(15.6, 2.6), (15.6, 7.1), (11.1, 7.1)]);
        }
        Icon::Stop => {
            pen.line(&[(6.0, 6.0), (18.0, 18.0)]);
            pen.line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Home => {
            pen.line(&[(4.0, 11.4), (12.0, 4.4), (20.0, 11.4)]);
            pen.line(&[(6.3, 10.2), (6.3, 19.4), (17.7, 19.4), (17.7, 10.2)]);
        }
        // ── Omnibox ──
        Icon::Lock => {
            pen.rrect(6.0, 10.6, 18.0, 20.0, 2.4);
            pen.arc(12.0, 10.4, 3.5, 180.0, 360.0);
            pen.line(&[(8.5, 10.4), (8.5, 7.4)]);
            pen.line(&[(15.5, 10.4), (15.5, 7.4)]);
            // shackle top
            pen.arc(12.0, 7.4, 3.5, 180.0, 360.0);
        }
        Icon::Tune => {
            pen.line(&[(4.5, 7.0), (19.5, 7.0)]);
            pen.line(&[(4.5, 12.0), (19.5, 12.0)]);
            pen.line(&[(4.5, 17.0), (19.5, 17.0)]);
            pen.dot(9.0, 7.0, 1.9);
            pen.dot(15.0, 12.0, 1.9);
            pen.dot(11.0, 17.0, 1.9);
        }
        Icon::Globe => {
            pen.ring(12.0, 12.0, 7.6);
            pen.line(&[(4.4, 12.0), (19.6, 12.0)]);
            pen.arc(12.0, 12.0, 7.6, 250.0, 290.0);
            let mut left = Vec::new();
            let mut right = Vec::new();
            for i in 0..=20 {
                let t = -1.0 + 2.0 * (i as f32 / 20.0);
                let y = 12.0 + t * 7.0;
                let bulge = 3.1 * (1.0 - t * t).max(0.0).sqrt();
                left.push((12.0 - bulge, y));
                right.push((12.0 + bulge, y));
            }
            pen.line(&left);
            pen.line(&right);
        }
        Icon::Search => {
            pen.ring(10.6, 10.6, 6.2);
            pen.line(&[(15.2, 15.2), (20.0, 20.0)]);
        }
        Icon::Mic => {
            pen.rrect(9.0, 3.2, 15.0, 13.4, 3.0);
            pen.arc(12.0, 13.0, 5.3, 20.0, 160.0);
            pen.line(&[(12.0, 18.3), (12.0, 21.2)]);
            pen.line(&[(8.8, 21.2), (15.2, 21.2)]);
        }
        Icon::Lens => {
            pen.ring(11.2, 11.2, 5.6);
            pen.line(&[(15.2, 15.2), (20.0, 20.0)]);
            pen.dot(11.2, 11.2, 1.4);
            pen.line(&[(17.6, 3.4), (20.6, 3.4), (20.6, 6.4)]);
        }
        Icon::GoogleG => {
            // Single-colour fallback for the brand mark.
            pen.arc(12.0, 12.0, 7.0, 30.0, 330.0);
            pen.line(&[(12.0, 12.0), (19.0, 12.0)]);
            pen.line(&[(19.0, 12.0), (19.0, 15.0)]);
        }
        Icon::Star | Icon::StarFilled => {
            let mut pts = Vec::with_capacity(10);
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { 8.3 } else { 3.55 };
                pts.push((12.0 + a.cos() * r, 12.3 + a.sin() * r));
            }
            if icon == Icon::StarFilled {
                pen.fill(&pts);
            } else {
                pen.outline(&pts);
            }
        }
        Icon::Share => {
            pen.ring(17.0, 5.6, 2.4);
            pen.ring(17.0, 18.4, 2.4);
            pen.ring(7.0, 12.0, 2.4);
            pen.line(&[(9.2, 10.9), (14.8, 6.7)]);
            pen.line(&[(9.2, 13.1), (14.8, 17.3)]);
        }
        Icon::Translate => {
            pen.line(&[(3.6, 5.6), (11.4, 5.6)]);
            pen.line(&[(7.5, 3.6), (7.5, 5.6)]);
            pen.arc(7.9, 6.6, 5.2, 30.0, 140.0);
            pen.line(&[(4.6, 12.4), (10.4, 12.4)]);
            pen.line(&[(12.6, 20.4), (16.6, 10.4), (20.6, 20.4)]);
            pen.line(&[(14.2, 16.6), (19.0, 16.6)]);
        }
        // ── Tab strip ──
        Icon::Plus => {
            pen.line(&[(12.0, 5.0), (12.0, 19.0)]);
            pen.line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Close => {
            pen.line(&[(6.4, 6.4), (17.6, 17.6)]);
            pen.line(&[(17.6, 6.4), (6.4, 17.6)]);
        }
        Icon::Menu => {
            pen.dot(12.0, 5.6, 2.0);
            pen.dot(12.0, 12.0, 2.0);
            pen.dot(12.0, 18.4, 2.0);
        }
        Icon::TabSearch => {
            pen.line(&[(6.0, 9.6), (12.0, 15.6), (18.0, 9.6)]);
        }
        Icon::Speaker => {
            pen.fill(&[(4.0, 9.4), (7.4, 9.4), (11.4, 5.6), (11.4, 18.4), (7.4, 14.6), (4.0, 14.6)]);
            pen.arc(12.8, 12.0, 3.4, -55.0, 55.0);
            pen.arc(12.8, 12.0, 6.2, -52.0, 52.0);
        }
        Icon::SpeakerMuted => {
            pen.fill(&[(4.0, 9.4), (7.4, 9.4), (11.4, 5.6), (11.4, 18.4), (7.4, 14.6), (4.0, 14.6)]);
            pen.line(&[(15.0, 9.6), (20.0, 14.6)]);
            pen.line(&[(20.0, 9.6), (15.0, 14.6)]);
        }
        Icon::Pin => {
            pen.line(&[(8.4, 3.6), (15.6, 3.6)]);
            pen.line(&[(10.2, 3.6), (10.2, 9.4), (6.6, 14.0), (17.4, 14.0), (13.8, 9.4), (13.8, 3.6)]);
            pen.line(&[(12.0, 14.0), (12.0, 20.6)]);
        }
        // ── Window controls ──
        Icon::Minimize => {
            pen.line(&[(5.5, 12.0), (18.5, 12.0)]);
        }
        Icon::Maximize => {
            pen.rrect(5.6, 5.6, 18.4, 18.4, 1.6);
        }
        Icon::Restore => {
            pen.rrect(4.6, 8.4, 15.0, 19.4, 1.6);
            pen.line(&[(8.4, 8.4), (8.4, 4.6), (19.4, 4.6), (19.4, 15.6), (15.0, 15.6)]);
        }
        // ── Menu ──
        Icon::NewWindow => {
            pen.rrect(3.8, 5.4, 20.2, 18.8, 2.2);
            pen.line(&[(3.8, 9.0), (20.2, 9.0)]);
            pen.line(&[(12.0, 12.0), (12.0, 16.4)]);
            pen.line(&[(9.8, 14.2), (14.2, 14.2)]);
        }
        Icon::Incognito => {
            pen.rrect(3.4, 9.0, 20.6, 12.6, 1.8);
            pen.ring(7.6, 15.8, 2.9);
            pen.ring(16.4, 15.8, 2.9);
            pen.line(&[(10.5, 15.8), (13.5, 15.8)]);
            pen.line(&[(8.4, 9.0), (9.8, 4.6)]);
            pen.line(&[(15.6, 9.0), (14.2, 4.6)]);
            pen.line(&[(9.8, 4.6), (14.2, 4.6)]);
        }
        Icon::History => {
            pen.arc(12.0, 12.0, 7.4, 150.0, 470.0);
            pen.line(&[(12.0, 7.6), (12.0, 12.4), (15.6, 14.4)]);
            pen.line(&[(4.6, 6.0), (4.6, 10.8), (9.4, 10.8)]);
        }
        Icon::Download => {
            pen.line(&[(12.0, 3.8), (12.0, 15.0)]);
            pen.line(&[(7.4, 10.4), (12.0, 15.0), (16.6, 10.4)]);
            pen.line(&[(4.6, 19.4), (19.4, 19.4)]);
        }
        Icon::Bookmark => {
            pen.fill(&[(7.0, 3.6), (17.0, 3.6), (17.0, 20.6), (12.0, 16.2), (7.0, 20.6)]);
        }
        Icon::Key => {
            pen.ring(8.4, 15.6, 3.6);
            pen.line(&[(11.0, 13.0), (19.4, 4.6)]);
            pen.line(&[(15.6, 8.4), (18.2, 11.0)]);
            pen.line(&[(18.0, 6.0), (20.4, 8.4)]);
        }
        Icon::Shield => {
            pen.outline(&[(12.0, 3.2), (19.6, 6.2), (19.6, 12.2), (12.0, 20.8), (4.4, 12.2), (4.4, 6.2)]);
            pen.line(&[(8.6, 12.0), (11.2, 14.6), (15.6, 9.8)]);
        }
        Icon::Device => {
            pen.rrect(3.4, 5.2, 20.6, 16.4, 1.8);
            pen.line(&[(9.0, 20.2), (15.0, 20.2)]);
            pen.line(&[(12.0, 16.4), (12.0, 20.2)]);
        }
        Icon::MapPin => {
            pen.arc(12.0, 9.8, 5.0, 150.0, 390.0);
            pen.line(&[(8.1, 13.0), (12.0, 20.8), (15.9, 13.0)]);
            pen.dot(12.0, 9.8, 1.7);
        }
        Icon::Zoom => {
            pen.ring(10.8, 10.8, 5.8);
            pen.line(&[(15.0, 15.0), (20.0, 20.0)]);
            pen.line(&[(8.4, 10.8), (13.2, 10.8)]);
            pen.line(&[(10.8, 8.4), (10.8, 13.2)]);
        }
        Icon::Print => {
            pen.rrect(5.2, 9.4, 18.8, 16.4, 1.6);
            pen.line(&[(7.8, 9.4), (7.8, 3.8), (16.2, 3.8), (16.2, 9.4)]);
            pen.rrect(7.8, 13.0, 16.2, 20.4, 1.2);
        }
        Icon::Palette => {
            pen.arc(12.0, 12.0, 8.2, -90.0, 235.0);
            pen.arc(12.0, 12.0, 3.2, -90.0, 235.0);
            pen.dot(7.4, 8.6, 1.5);
            pen.dot(6.6, 13.6, 1.5);
            pen.dot(10.0, 17.4, 1.5);
        }
        Icon::Settings => {
            pen.ring(12.0, 12.0, 3.1);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (s, c) = a.sin_cos();
                pen.line(&[(12.0 + c * 4.6, 12.0 + s * 4.6), (12.0 + c * 7.4, 12.0 + s * 7.4)]);
            }
            pen.ring(12.0, 12.0, 8.4);
        }
        Icon::Help => {
            pen.ring(12.0, 12.0, 8.2);
            pen.arc(12.0, 9.8, 2.9, 170.0, 30.0);
            pen.line(&[(12.0, 12.8), (12.0, 14.6)]);
            pen.dot(12.0, 17.2, 1.3);
        }
        Icon::Info => {
            pen.ring(12.0, 12.0, 8.2);
            pen.line(&[(12.0, 10.8), (12.0, 16.4)]);
            pen.dot(12.0, 7.9, 1.3);
        }
        Icon::ChevronRight => {
            pen.line(&[(9.8, 6.0), (15.8, 12.0), (9.8, 18.0)]);
        }
        Icon::Exit => {
            pen.line(&[(13.4, 4.0), (5.6, 4.0), (5.6, 20.0), (13.4, 20.0)]);
            pen.line(&[(10.8, 12.0), (20.4, 12.0)]);
            pen.line(&[(17.2, 8.8), (20.4, 12.0), (17.2, 15.2)]);
        }
        Icon::Apps => {
            for row in 0..3 {
                for col in 0..3 {
                    pen.dot(6.6 + col as f32 * 5.4, 6.6 + row as f32 * 5.4, 1.5);
                }
            }
        }
        Icon::Extension => {
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
        }
        Icon::Pencil => {
            pen.line(&[(4.4, 19.6), (5.4, 15.2), (15.6, 5.0), (19.0, 8.4), (8.8, 18.6), (4.4, 19.6)]);
            pen.line(&[(13.8, 6.8), (17.2, 10.2)]);
        }
        Icon::Sparkle => {
            // Gemini-style four-point star with a smaller companion.
            pen.fill(&[
                (12.0, 2.2), (13.9, 8.6), (20.3, 10.5), (13.9, 12.4),
                (12.0, 18.8), (10.1, 12.4), (3.7, 10.5), (10.1, 8.6),
            ]);
            pen.fill(&[
                (19.2, 14.4), (20.0, 17.0), (22.6, 17.8), (20.0, 18.6),
                (19.2, 21.2), (18.4, 18.6), (15.8, 17.8), (18.4, 17.0),
            ]);
        }
        Icon::Folder => {
            pen.outline(&[
                (3.6, 6.6), (10.0, 6.6), (11.8, 8.8), (20.4, 8.8),
                (20.4, 19.2), (3.6, 19.2),
            ]);
        }
        Icon::Check => {
            pen.line(&[(4.6, 12.4), (9.6, 17.4), (19.4, 7.0)]);
        }
    }
}
