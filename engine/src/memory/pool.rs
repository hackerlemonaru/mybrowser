//! Object pools for frequently allocated types.
//! Reduces allocation overhead for DOM nodes, CSS rules, paint commands.

use std::sync::Mutex;
use std::collections::VecDeque;

/// A simple object pool.
/// Objects are recycled instead of reallocated.
pub struct ObjectPool<T> {
    pool: Mutex<VecDeque<T>>,
    creator: Box<dyn Fn() -> T + Send + Sync>,
}

impl<T> ObjectPool<T> {
    pub fn new(creator: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            pool: Mutex::new(VecDeque::new()),
            creator: Box::new(creator),
        }
    }

    /// Get an object from the pool (or create a new one).
    pub fn acquire(&self) -> T {
        self.pool.lock().unwrap().pop_front().unwrap_or_else(|| (self.creator)())
    }

    /// Return an object to the pool.
    pub fn release(&self, obj: T) {
        self.pool.lock().unwrap().push_back(obj);
    }

    pub fn len(&self) -> usize {
        self.pool.lock().unwrap().len()
    }
}

/// Pool for paint commands (allocated frequently during rendering).
pub type PaintCommandPool = ObjectPool<crate::gpu::PaintCommand>;

impl Default for PaintCommandPool {
    fn default() -> Self {
        Self::new(|| crate::gpu::PaintCommand::Rect {
            x: 0.0, y: 0.0, w: 0.0, h: 0.0, color: [0.0; 4],
        })
    }
}
