//! Bounded, TTL-based caches.
//! Prevents unbounded memory growth in long-running browser sessions.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A cache entry with TTL and size tracking.
struct CacheEntry<V> {
    value: V,
    cached_at: Instant,
    ttl: Duration,
    size_bytes: usize,
}

/// A bounded cache with per-entry TTL.
/// When the cache exceeds max_entries, the oldest entries are evicted.
pub struct BoundedCache<K, V> {
    entries: Mutex<HashMap<K, CacheEntry<V>>>,
    max_entries: usize,
    max_size_bytes: usize,
    current_size_bytes: Mutex<usize>,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> BoundedCache<K, V> {
    pub fn new(max_entries: usize, max_size_bytes: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries,
            max_size_bytes,
            current_size_bytes: Mutex::new(0),
        }
    }

    /// Get a value from the cache (returns None if expired or missing).
    pub fn get(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get(key)?;

        if entry.cached_at.elapsed() > entry.ttl {
            entries.remove(key);
            return None;
        }

        Some(entry.value.clone())
    }

    /// Insert a value into the cache.
    /// Evicts oldest entries if over limits.
    pub fn put(&self, key: K, value: V, ttl: Duration, size_bytes: usize) {
        let mut entries = self.entries.lock().unwrap();
        let mut current_size = self.current_size_bytes.lock().unwrap();

        // Remove existing entry if present.
        if let Some(old) = entries.get(&key) {
            *current_size = current_size.saturating_sub(old.size_bytes);
        }

        // Evict oldest entries if we're over limits.
        while entries.len() >= self.max_entries || *current_size + size_bytes > self.max_size_bytes {
            if let Some(oldest_key) = entries.keys().next().cloned() {
                if let Some(old) = entries.remove(&oldest_key) {
                    *current_size = current_size.saturating_sub(old.size_bytes);
                }
            } else {
                break;
            }
        }

        let entry = CacheEntry {
            value,
            cached_at: Instant::now(),
            ttl,
            size_bytes,
        };
        *current_size += size_bytes;
        entries.insert(key, entry);
    }

    /// Remove a key from the cache.
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.lock().unwrap();
        let mut current_size = self.current_size_bytes.lock().unwrap();
        if let Some(entry) = entries.remove(key) {
            *current_size = current_size.saturating_sub(entry.size_bytes);
            Some(entry.value)
        } else {
            None
        }
    }

    /// Clear the entire cache.
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        let mut current_size = self.current_size_bytes.lock().unwrap();
        entries.clear();
        *current_size = 0;
    }

    /// Current number of entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }
}

/// HTTP response cache — bounded by entry count and total size.
pub type HttpCache = BoundedCache<String, Vec<u8>>;

/// Cookie cache — bounded by entry count.
pub type CookieCache = BoundedCache<String, String>;

/// Image cache — bounded by entry count and total size (images are large).
pub type ImageCache = BoundedCache<String, Vec<u8>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_cache_ttl_expiry() {
        let cache: BoundedCache<String, String> = BoundedCache::new(10, 1024);
        cache.put("key1".to_string(), "value1".to_string(), Duration::from_millis(10), 10);
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(cache.get(&"key1".to_string()), None);
    }

    #[test]
    fn bounded_cache_eviction() {
        let cache: BoundedCache<String, String> = BoundedCache::new(2, 1024);
        cache.put("a".to_string(), "1".to_string(), Duration::from_secs(60), 10);
        cache.put("b".to_string(), "2".to_string(), Duration::from_secs(60), 10);
        cache.put("c".to_string(), "3".to_string(), Duration::from_secs(60), 10);

        // "a" should be evicted (oldest).
        assert_eq!(cache.get(&"a".to_string()), None);
        assert_eq!(cache.get(&"b".to_string()), Some("2".to_string()));
        assert_eq!(cache.get(&"c".to_string()), Some("3".to_string()));
    }
}
