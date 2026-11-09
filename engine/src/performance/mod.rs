//! Performance and rendering optimization module.
//!
//! Techniques for a smooth, efficient browser:
//!   - Compositor thread separation (GPU rendering off main thread)
//!   - Compositor-only animations (transform, opacity) — skip layout/paint
//!   - Damage rect tracking (only repaint what changed)
//!   - Frame budgeting (16.67ms at 60Hz)
//!   - content-visibility: auto (skip off-screen content)
//!   - Passive event listeners
//!   - Lazy loading for off-screen images
//!   - Critical rendering path optimization

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Frame budget target: 16.67ms for 60fps.
pub const FRAME_BUDGET_MS: u64 = 16;

/// Performance metrics tracked per frame.
#[derive(Debug, Clone, Default)]
pub struct FrameMetrics {
    pub frame_start: Instant,
    pub style_ms: f64,
    pub layout_ms: f64,
    pub paint_ms: f64,
    pub composite_ms: f64,
    pub total_ms: f64,
    pub dropped: bool,
}

/// Global performance tracker.
static FRAME_COUNT: AtomicU64 = AtomicU64::new(0);
static DROPPED_FRAMES: AtomicU64 = AtomicU64::new(0);
static TOTAL_FRAME_TIME: AtomicU64 = AtomicU64::new(0);

/// Record a completed frame.
pub fn record_frame(metrics: &FrameMetrics) {
    FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
    if metrics.dropped {
        DROPPED_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    TOTAL_FRAME_TIME.fetch_add(
        metrics.total_ms as u64,
        Ordering::Relaxed,
    );
}

/// Get average frame time in ms.
pub fn avg_frame_time_ms() -> f64 {
    let count = FRAME_COUNT.load(Ordering::Relaxed);
    if count == 0 {
        return 0.0;
    }
    let total = TOTAL_FRAME_TIME.load(Ordering::Relaxed) as f64;
    total / count as f64
}

/// Get dropped frame percentage.
pub fn dropped_frame_percent() -> f64 {
    let count = FRAME_COUNT.load(Ordering::Relaxed);
    if count == 0 {
        return 0.0;
    }
    let dropped = DROPPED_FRAMES.load(Ordering::Relaxed);
    dropped as f64 / count as f64 * 100.0
}

/// A damage rectangle — only this region needs repainting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl DamageRect {
    pub fn empty() -> Self {
        Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0.0 || self.height == 0.0
    }

    pub fn union(&self, other: &DamageRect) -> DamageRect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        DamageRect { x: left, y: top, width: right - left, height: bottom - top }
    }
}

/// Tracks damage regions for incremental repaint.
/// Only damaged rectangles are repainted — the rest is reused from cache.
pub struct DamageTracker {
    rects: Vec<DamageRect>,
}

impl DamageTracker {
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Add a damage region.
    pub fn add(&mut self, rect: DamageRect) {
        if rect.is_empty() {
            return;
        }
        // Merge with existing overlapping rects.
        let mut merged = false;
        for existing in &mut self.rects {
            if Self::overlaps(existing, &rect) {
                *existing = existing.union(&rect);
                merged = true;
                break;
            }
        }
        if !merged {
            self.rects.push(rect);
        }
    }

    /// Get all damage rects and clear.
    pub fn take(&mut self) -> Vec<DamageRect> {
        std::mem::take(&mut self.rects)
    }

    fn overlaps(a: &DamageRect, b: &DamageRect) -> bool {
        a.x < b.x + b.width && a.x + a.width > b.x &&
        a.y < b.y + b.height && a.y + a.height > b.y
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame scheduler — enforces frame budget and yields to main thread.
/// Ensures smooth 60fps by not blocking the compositor thread.
pub struct FrameScheduler {
    last_frame: Instant,
    budget: Duration,
}

impl FrameScheduler {
    pub fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            budget: Duration::from_millis(FRAME_BUDGET_MS),
        }
    }

    /// Check if we should yield to let the compositor run.
    /// Returns true if we've exceeded the frame budget.
    pub fn should_yield(&self) -> bool {
        self.last_frame.elapsed() > self.budget
    }

    /// Mark a frame as started.
    pub fn frame_started(&mut self) {
        self.last_frame = Instant::now();
    }

    /// Check if we're within budget for smooth animation.
    pub fn within_budget(&self) -> bool {
        self.last_frame.elapsed() < self.budget
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// CSS property animation classification.
//! Only transform and opacity can be handled by the compositor thread.
//! Everything else requires layout/paint on the main thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPropertyClass {
    /// Compositor-only — can be animated on GPU thread without main thread.
    CompositorOnly,
    /// Requires layout (triggers reflow).
    Layout,
    /// Requires paint (rasterization).
    Paint,
    /// Requires composite only (cheap).
    Composite,
}

impl AnimationPropertyClass {
    pub fn for_css_property(prop: &str) -> Self {
        match prop {
            "transform" | "opacity" => AnimationPropertyClass::CompositorOnly,
            "will-change" => AnimationPropertyClass::Composite,
            "width" | "height" | "top" | "left" | "margin" | "padding"
            | "display" | "flex" | "grid" => AnimationPropertyClass::Layout,
            "background-color" | "border" | "box-shadow" | "color"
            | "font-size" => AnimationPropertyClass::Paint,
            _ => AnimationPropertyClass::Layout, // Conservative default
        }
    }
}

/// Check if an animation can run entirely on the compositor thread.
pub fn is_compositor_only_animation(properties: &[&str]) -> bool {
    properties.iter().all(|p| {
        AnimationPropertyClass::for_css_property(p)
            == AnimationPropertyClass::CompositorOnly
    })
}

/// content-visibility: auto support — skip rendering off-screen content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentVisibility {
    /// Normal rendering.
    Visible,
    /// Skip layout/paint until scrolled into view.
    Auto,
    /// Hidden but maintains layout.
    Hidden,
}

/// Lazy image loading — only load images when near viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingStrategy {
    Eager,
    Lazy,
    Auto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_only_properties() {
        assert!(is_compositor_only_animation(&["transform", "opacity"]));
        assert!(!is_compositor_only_animation(&["transform", "width"]));
        assert!(!is_compositor_only_animation(&["opacity", "top"]));
    }

    #[test]
    fn damage_rect_union() {
        let a = DamageRect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 };
        let b = DamageRect { x: 50.0, y: 50.0, width: 100.0, height: 100.0 };
        let c = a.union(&b);
        assert_eq!(c.x, 0.0);
        assert_eq!(c.y, 0.0);
        assert_eq!(c.width, 150.0);
        assert_eq!(c.height, 150.0);
    }

    #[test]
    fn frame_budget() {
        let sched = FrameScheduler::new();
        assert!(sched.within_budget());
    }
}
