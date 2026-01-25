use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::filesystem::{FsEntry, FsMetadata};

#[derive(Debug)]
struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
}

#[derive(Debug)]
pub struct TimedCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    order: VecDeque<K>,
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

    pub fn get(&mut self, key: &K) -> Option<&V> {
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

    pub fn remove(&mut self, key: &K) {
        self.entries.remove(key);
        self.order.retain(|existing| existing != key);
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

    pub fn get(&mut self, path: &PathBuf) -> Option<&FsMetadata> {
        self.inner.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, metadata: FsMetadata) {
        self.inner.insert(path, metadata);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[derive(Debug)]
pub struct DirectoryCache {
    inner: TimedCache<PathBuf, Vec<FsEntry>>,
}

impl DirectoryCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &PathBuf) -> Option<&Vec<FsEntry>> {
        self.inner.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, entries: Vec<FsEntry>) {
        self.inner.insert(path, entries);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::TimedCache;
    use std::time::Duration;
    use std::thread::sleep;

    #[test]
    fn timed_cache_evicts_oldest_entries() {
        let mut cache = TimedCache::new(2, Duration::from_secs(60));

        cache.insert("alpha", 1);
        cache.insert("beta", 2);
        cache.insert("gamma", 3);

        assert!(cache.get(&"alpha").is_none());
        assert_eq!(cache.get(&"beta"), Some(&2));
        assert_eq!(cache.get(&"gamma"), Some(&3));
    }

    #[test]
    fn timed_cache_expires_entries() {
        let mut cache = TimedCache::new(4, Duration::from_millis(10));

        cache.insert("alpha", 1);
        sleep(Duration::from_millis(25));

        assert!(cache.get(&"alpha").is_none());
    }
}
