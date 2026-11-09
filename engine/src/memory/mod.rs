//! Memory management for the browser.
//!
//! Long-running browsers must not leak memory. This module provides:
//!   - Arena allocators for per-page/tab allocations (freed all at once on close)
//!   - Bounded caches with TTL (cookies, HTTP cache, image cache)
//!   - Object pooling for frequently allocated types
//!   - jemalloc/mimalloc recommendation for the global allocator
//!
//! Security: All arena allocations are freed when a tab/profile closes,
//! preventing memory leaks across sessions.

pub mod arena;
pub mod cache;
pub mod pool;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Global memory tracker — counts allocations across the browser.
static MEMORY_TRACKER: AtomicUsize = AtomicUsize::new(0);

/// Get current tracked memory usage in bytes.
pub fn current_memory_usage() -> usize {
    MEMORY_TRACKER.load(Ordering::Relaxed)
}

/// Record an allocation.
pub fn record_alloc(bytes: usize) {
    MEMORY_TRACKER.fetch_add(bytes, Ordering::Relaxed);
}

/// Record a deallocation.
pub fn record_dealloc(bytes: usize) {
    MEMORY_TRACKER.fetch_sub(bytes, Ordering::Relaxed);
}

/// A scoped arena for a page/tab lifetime.
/// All allocations are freed when the scope ends.
pub struct PageArena {
    arena: bumpalo::Bump,
    bytes_allocated: AtomicUsize,
}

impl PageArena {
    pub fn new() -> Self {
        Self {
            arena: bumpalo::Bump::new(),
            bytes_allocated: AtomicUsize::new(0),
        }
    }

    /// Allocate space for T in the arena.
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let bytes = std::mem::size_of::<T>();
        record_alloc(bytes);
        self.bytes_allocated.fetch_add(bytes, Ordering::Relaxed);
        self.arena.alloc(value)
    }

    /// Allocate a string in the arena.
    pub fn alloc_str(&self, s: &str) -> &mut str {
        let bytes = s.len();
        record_alloc(bytes);
        self.bytes_allocated.fetch_add(bytes, Ordering::Relaxed);
        let mut buf = self.arena.alloc_slice_fill_copy(bytes, 0u8);
        buf.copy_from_slice(s.as_bytes());
        unsafe { std::str::from_utf8_unchecked_mut(buf) }
    }

    /// Get bytes allocated by this arena.
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated.load(Ordering::Relaxed)
    }
}

impl Drop for PageArena {
    fn drop(&mut self) {
        let bytes = self.bytes_allocated.load(Ordering::Relaxed);
        record_dealloc(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_alloc_and_drop() {
        let arena = PageArena::new();
        let val = arena.alloc(42i32);
        assert_eq!(*val, 42);

        let s = arena.alloc_str("hello");
        assert_eq!(*s, "hello");

        let bytes = arena.bytes_allocated();
        assert!(bytes > 0);

        // When arena drops, memory is freed.
        drop(arena);
    }

    #[test]
    fn memory_tracker() {
        let before = current_memory_usage();
        let arena = PageArena::new();
        let _ = arena.alloc(100i32);
        let after = current_memory_usage();
        assert!(after > before);
        drop(arena);
        let after_drop = current_memory_usage();
        assert_eq!(after_drop, before);
    }
}
