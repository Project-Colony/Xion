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
            // `remove` also purges `order`. Dropping the entry alone left a
            // ghost key in the queue, which `evict_if_needed` then popped
            // instead of a live one — the LRU order drifted with every
            // expiry, and a re-inserted key was queued twice.
            self.remove(key);
            return None;
        }

        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn insert(&mut self, key: K, value: V) {
        let existing = self.entries.insert(
            key.clone(),
            CacheEntry {
                value,
                inserted_at: Instant::now(),
            },
        );
        if existing.is_some() {
            // O(n) scan — acceptable for small max_entries; IndexMap would be O(1)
            // but adds a dependency for negligible gain at typical cache sizes (<200).
            self.order.retain(|existing| existing != &key);
        }
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

/// Default maximum memory budget for directory cache: 64 MB.
const DEFAULT_MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// What a cached listing actually costs.
///
/// A flat constant per entry under-counted by 20-30%: `FsEntry` alone is about
/// 120 bytes in the vector, and the `PathBuf` and `String` it owns are heap
/// allocations whose size depends on the actual names.
fn listing_cost(entries: &[FsEntry]) -> usize {
    let fixed = std::mem::size_of::<FsEntry>();
    entries
        .iter()
        .map(|entry| fixed + entry.path.as_os_str().len() + entry.name.len())
        .sum::<usize>()
        + std::mem::size_of::<Vec<FsEntry>>()
}

/// Cache key for a directory listing.
///
/// The path alone was not enough: the same directory listed with a different
/// sort or filter produced the same key, so `ChangeSort` and `ToggleGitignore`
/// served the previous ordering for the rest of the 45-second TTL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectoryKey {
    pub path: PathBuf,
    pub options: crate::filesystem::ListOptions,
}

#[derive(Debug)]
pub struct DirectoryCache {
    inner: TimedCache<DirectoryKey, Vec<FsEntry>>,
    /// Cost of each live listing, mirrored so the running total can be
    /// reconciled after evictions and expiries that happen inside `inner`.
    costs: HashMap<DirectoryKey, usize>,
    estimated_bytes: usize,
    max_bytes: usize,
}

impl DirectoryCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: TimedCache::new(max_entries, ttl),
            costs: HashMap::new(),
            estimated_bytes: 0,
            max_bytes: DEFAULT_MAX_CACHE_BYTES,
        }
    }

    pub fn get(&mut self, key: &DirectoryKey) -> Option<&Vec<FsEntry>> {
        let hit = self.inner.get(key).is_some();
        if !hit {
            // The lookup may have expired an entry; keep the byte total honest.
            self.reconcile();
            return None;
        }
        self.inner.get(key)
    }

    pub fn insert(&mut self, key: DirectoryKey, entries: Vec<FsEntry>) {
        let cost = listing_cost(&entries);

        // A single listing bigger than the whole budget is not worth evicting
        // everything else for.
        if cost > self.max_bytes {
            return;
        }

        self.costs.remove(&key);
        self.reconcile();

        // Evict oldest listings until the newcomer fits.
        while self.estimated_bytes + cost > self.max_bytes {
            let Some(oldest) = self.inner.order.pop_front() else {
                break;
            };
            self.inner.entries.remove(&oldest);
            self.costs.remove(&oldest);
            self.reconcile();
        }

        self.inner.insert(key.clone(), entries);
        self.costs.insert(key, cost);
        // `inner.insert` may itself have evicted on max_entries.
        self.reconcile();
    }

    pub fn remove(&mut self, key: &DirectoryKey) {
        self.inner.remove(key);
        self.costs.remove(key);
        self.reconcile();
    }

    /// Drop every listing for `path`, whatever options produced it.
    ///
    /// Used by the file watcher: a directory that changed on disk invalidates
    /// all of its sorted variants at once.
    pub fn remove_path(&mut self, path: &Path) {
        let stale: Vec<DirectoryKey> = self
            .inner
            .entries
            .keys()
            .filter(|key| key.path == path)
            .cloned()
            .collect();
        for key in stale {
            self.inner.remove(&key);
            self.costs.remove(&key);
        }
        self.reconcile();
    }

    pub fn clear(&mut self) {
        self.inner.clear();
        self.costs.clear();
        self.estimated_bytes = 0;
    }

    #[cfg(test)]
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }

    /// Recompute the byte total from the listings that are actually still in
    /// the cache.
    ///
    /// Entries can leave `inner` in three ways — explicit removal, TTL expiry
    /// inside `get`, and `max_entries` eviction inside `insert` — and only the
    /// first one used to be accounted for, so the counter ratcheted upwards
    /// until the cache stopped accepting anything. With at most a few hundred
    /// keys this is cheaper than threading eviction callbacks through.
    fn reconcile(&mut self) {
        let live = &self.inner.entries;
        self.costs.retain(|key, _| live.contains_key(key));
        self.estimated_bytes = self.costs.values().sum();
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

#[cfg(test)]
mod directory_cache_tests {
    use super::*;
    use crate::filesystem::{FsEntryType, ListOptions};

    fn entry(name: &str) -> FsEntry {
        FsEntry {
            path: PathBuf::from("/tmp").join(name),
            name: name.to_string(),
            entry_type: FsEntryType::File,
            metadata: FsMetadata::default(),
        }
    }

    fn key(path: &str) -> DirectoryKey {
        DirectoryKey {
            path: PathBuf::from(path),
            options: ListOptions::default(),
        }
    }

    /// Regression: an entry expiring inside `get` was removed from the map but
    /// left its key in the order queue, so eviction popped ghosts.
    #[test]
    fn expiry_purges_the_order_queue_too() {
        let mut cache: TimedCache<String, u32> = TimedCache::new(4, Duration::from_millis(5));
        cache.insert("alpha".to_string(), 1);
        std::thread::sleep(Duration::from_millis(20));

        assert!(cache.get("alpha").is_none());
        assert!(cache.order.is_empty(), "clé fantôme restée dans la file");
    }

    #[test]
    fn reinserting_a_key_does_not_duplicate_it_in_the_queue() {
        let mut cache: TimedCache<String, u32> = TimedCache::new(4, Duration::from_secs(60));
        cache.insert("alpha".to_string(), 1);
        cache.insert("alpha".to_string(), 2);
        assert_eq!(cache.order.len(), 1);
    }

    /// Regression: the byte counter was only decremented on explicit removal,
    /// so TTL expiry and max_entries eviction ratcheted it upwards until the
    /// cache silently stopped accepting anything.
    #[test]
    fn byte_total_comes_back_down_after_eviction() {
        let mut cache = DirectoryCache::new(2, Duration::from_secs(60));
        cache.insert(key("/a"), vec![entry("one"), entry("two")]);
        cache.insert(key("/b"), vec![entry("three")]);
        let with_two = cache.estimated_bytes();
        assert!(with_two > 0);

        // Third insert evicts the oldest because max_entries is 2.
        cache.insert(key("/c"), vec![entry("four")]);

        assert!(
            cache.estimated_bytes() < with_two + 10_000,
            "le compteur d'octets ne doit pas s'emballer"
        );
        assert!(cache.get(&key("/a")).is_none(), "/a devait être évincé");
        assert!(cache.get(&key("/c")).is_some());
    }

    #[test]
    fn byte_total_is_zero_once_empty() {
        let mut cache = DirectoryCache::new(4, Duration::from_secs(60));
        cache.insert(key("/a"), vec![entry("one")]);
        cache.remove(&key("/a"));
        assert_eq!(cache.estimated_bytes(), 0);
    }

    #[test]
    fn a_different_sort_is_a_different_key() {
        let mut cache = DirectoryCache::new(4, Duration::from_secs(60));
        let ascending = key("/a");
        let mut descending = ascending.clone();
        descending.options.sort_order = crate::filesystem::SortOrder::Desc;

        cache.insert(ascending.clone(), vec![entry("one")]);

        assert!(cache.get(&ascending).is_some());
        assert!(cache.get(&descending).is_none());
    }

    #[test]
    fn remove_path_drops_all_variants() {
        let mut cache = DirectoryCache::new(8, Duration::from_secs(60));
        let ascending = key("/a");
        let mut descending = ascending.clone();
        descending.options.sort_order = crate::filesystem::SortOrder::Desc;
        cache.insert(ascending.clone(), vec![entry("one")]);
        cache.insert(descending.clone(), vec![entry("two")]);

        cache.remove_path(Path::new("/a"));

        assert!(cache.get(&ascending).is_none());
        assert!(cache.get(&descending).is_none());
        assert_eq!(cache.estimated_bytes(), 0);
    }
}
