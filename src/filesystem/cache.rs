use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::filesystem::{FsEntry, FsMetadata};

#[derive(Debug)]
pub(crate) struct CacheEntry<V> {
    pub(crate) value: V,
    inserted_at: Instant,
}

#[derive(Debug)]
pub struct TimedCache<K, V> {
    pub(crate) entries: HashMap<K, CacheEntry<V>>,
    pub(crate) order: VecDeque<K>,
    max_entries: usize,
    ttl: Duration,
}

impl<K, V> TimedCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            ttl,
        }
    }

    pub fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let expired = self
            .entries
            .get(key)
            .map(|entry| entry.inserted_at.elapsed() > self.ttl)
            .unwrap_or(false);

        if expired {
            self.entries.remove(key);
            return None;
        }

        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.contains_key(&key) {
            self.order.retain(|existing| existing != &key);
        }

        self.entries.insert(
            key.clone(),
            CacheEntry {
                value,
                inserted_at: Instant::now(),
            },
        );
        self.order.push_back(key);
        self.evict_if_needed();
    }

    pub fn remove<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.entries.remove(key);
        self.order.retain(|existing| existing.borrow() != key);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries {
            if let Some(key) = self.order.pop_front() {
                self.entries.remove(&key);
            }
        }
    }
}

#[derive(Debug)]
pub struct MetadataCache {
    inner: TimedCache<PathBuf, FsMetadata>,
}

impl MetadataCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<&FsMetadata> {
        self.inner.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, metadata: FsMetadata) {
        self.inner.insert(path, metadata);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

/// Estimated memory cost per FsEntry (PathBuf ~60 bytes + name ~30 bytes + metadata ~80 bytes).
const ESTIMATED_BYTES_PER_ENTRY: usize = 170;
/// Default maximum memory budget for directory cache: 64 MB.
const DEFAULT_MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct DirectoryCache {
    inner: TimedCache<PathBuf, Vec<FsEntry>>,
    estimated_bytes: usize,
    max_bytes: usize,
}

impl DirectoryCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: TimedCache::new(max_entries, ttl),
            estimated_bytes: 0,
            max_bytes: DEFAULT_MAX_CACHE_BYTES,
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<&Vec<FsEntry>> {
        self.inner.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, entries: Vec<FsEntry>) {
        let entry_bytes = entries.len() * ESTIMATED_BYTES_PER_ENTRY;

        // Evict oldest entries until we're under the byte budget
        while self.estimated_bytes + entry_bytes > self.max_bytes
            && !self.inner.order.is_empty()
        {
            if let Some(key) = self.inner.order.pop_front() {
                if let Some(removed) = self.inner.entries.remove(&key) {
                    self.estimated_bytes = self
                        .estimated_bytes
                        .saturating_sub(removed.value.len() * ESTIMATED_BYTES_PER_ENTRY);
                }
            }
        }

        // If the single entry itself exceeds the budget, still insert it
        // (it will be the only entry) but don't track negative
        self.estimated_bytes += entry_bytes;
        self.inner.insert(path, entries);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
        self.estimated_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::TimedCache;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn timed_cache_evicts_oldest_entries() {
        let mut cache = TimedCache::new(2, Duration::from_secs(60));

        cache.insert("alpha", 1);
        cache.insert("beta", 2);
        cache.insert("gamma", 3);

        assert!(cache.get("alpha").is_none());
        assert_eq!(cache.get("beta"), Some(&2));
        assert_eq!(cache.get("gamma"), Some(&3));
    }

    #[test]
    fn timed_cache_expires_entries() {
        let mut cache = TimedCache::new(4, Duration::from_millis(10));

        cache.insert("alpha", 1);
        sleep(Duration::from_millis(25));

        assert!(cache.get("alpha").is_none());
    }
}
