use std::path::PathBuf;
use std::time::Duration;

use crate::filesystem::TimedCache;

#[derive(Debug, Clone)]
pub struct Thumbnail {
    pub bytes: Vec<u8>,
    pub mime: Option<String>,
}

impl Thumbnail {
    pub fn new(bytes: Vec<u8>, mime: Option<String>) -> Self {
        Self { bytes, mime }
    }
}

#[derive(Debug)]
pub struct ThumbnailService {
    cache: TimedCache<PathBuf, Thumbnail>,
}

impl ThumbnailService {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &PathBuf) -> Option<&Thumbnail> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, thumbnail: Thumbnail) {
        self.cache.insert(path, thumbnail);
    }

    pub fn remove(&mut self, path: &PathBuf) {
        self.cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
