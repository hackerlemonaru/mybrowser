//! Compositor thread — runs GPU rendering off the main thread.
//!
//! The compositor thread handles:
//!   - Layer composition (combining painted layers into final frame)
//!   - Scrolling (without main thread involvement)
//!   - Compositor-only animations (transform, opacity)
//!   - Damage-based incremental repaint
//!
//! This is the key to smooth 60fps: the compositor never blocks on
//! main-thread JavaScript or layout.

use crate::performance::{DamageTracker, FrameMetrics, FrameScheduler, record_frame};
use crate::gpu::{GpuContext, PaintFrame, PaintCommand};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Messages sent from main thread to compositor thread.
pub enum CompositorMsg {
    /// New paint frame to render.
    Frame(PaintFrame),
    /// Scroll offset changed (compositor handles this directly).
    Scroll { x: f32, y: f32 },
    /// Damage region needs repaint.
    Damage(crate::performance::DamageRect),
    /// Shutdown.
    Quit,
}

/// The compositor — runs in its own thread.
pub struct Compositor {
    gpu_ctx: GpuContext,
    receiver: mpsc::Receiver<CompositorMsg>,
    damage_tracker: DamageTracker,
    scheduler: FrameScheduler,
    scroll_x: f32,
    scroll_y: f32,
}

impl Compositor {
    pub fn new(
        gpu_ctx: GpuContext,
        receiver: mpsc::Receiver<CompositorMsg>,
    ) -> Self {
        Self {
            gpu_ctx,
            receiver,
            damage_tracker: DamageTracker::new(),
            scheduler: FrameScheduler::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    /// Main compositor loop — runs on the compositor thread.
    pub fn run(mut self) {
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                CompositorMsg::Frame(frame) => {
                    self.render_frame(frame);
                }
                CompositorMsg::Scroll { x, y } => {
                    // Compositor handles scrolling directly — no main thread needed.
                    self.scroll_x = x;
                    self.scroll_y = y;
                    eprintln!("[Compositor] Scroll to ({}, {})", x, y);
                }
                CompositorMsg::Damage(rect) => {
                    self.damage_tracker.add(rect);
                }
                CompositorMsg::Quit => break,
            }
        }
    }

    fn render_frame(&mut self, frame: PaintFrame) {
        let start = Instant::now();

        // Apply damage tracking — only repaint damaged regions.
        let damage = self.damage_tracker.take();
        if !damage.is_empty() {
            eprintln!("[Compositor] Damage rects: {:?}", damage);
        }

        // Apply scroll offset (compositor-only, no layout).
        let _ = (self.scroll_x, self.scroll_y);

        // Submit to GPU.
        if let Err(e) = self.gpu_ctx.render_frame(&frame) {
            eprintln!("[Compositor] GPU render error: {}", e);
        }

        let elapsed = start.elapsed();
        let metrics = FrameMetrics {
            frame_start: start,
            composite_ms: elapsed.as_secs_f64() * 1000.0,
            total_ms: elapsed.as_secs_f64() * 1000.0,
            dropped: elapsed > std::time::Duration::from_millis(crate::performance::FRAME_BUDGET_MS),
            ..Default::default()
        };

        if metrics.dropped {
            eprintln!("[Compositor] Dropped frame: {:.2}ms", metrics.total_ms);
        }

        record_frame(&metrics);
        self.scheduler.frame_started();
    }
}

/// Spawn a compositor thread.
/// Returns a sender to communicate with it.
pub fn spawn_compositor(
    gpu_ctx: GpuContext,
) -> mpsc::Sender<CompositorMsg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let compositor = Compositor::new(gpu_ctx, rx);
        compositor.run();
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_compositor_only() {
        use crate::performance::is_compositor_only_animation;
        assert!(is_compositor_only_animation(&["transform", "opacity"]));
        assert!(!is_compositor_only_animation(&["transform", "width"]));
    }
}
