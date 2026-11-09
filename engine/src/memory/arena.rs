//! Re-export bumpalo as the arena allocator with browser-specific wrappers.
//!
//! Usage: Create a PageArena per tab/page. All DOM nodes, CSS stylesheets,
//! layout boxes, and paint commands for that page are allocated in the arena.
//! When the tab closes, the entire arena is freed in O(1).

pub use bumpalo::Bump;

/// Typed arena for homogeneous collections (e.g., DOM nodes of one type).
pub struct TypedArena<'a, T> {
    arena: &'a Bump,
    items: Vec<*mut T>,
}

impl<'a, T> TypedArena<'a, T> {
    pub fn new(arena: &'a Bump) -> Self {
        Self {
            arena,
            items: Vec::new(),
        }
    }

    pub fn alloc(&'a mut self, value: T) -> &'a mut T {
        let item = self.arena.alloc(value);
        self.items.push(item as *mut T);
        item
    }
}
