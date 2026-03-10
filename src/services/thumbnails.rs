use std::io::Cursor;
use std::path::{Path, PathBuf};
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

    pub fn get(&mut self, path: &Path) -> Option<&Thumbnail> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, thumbnail: Thumbnail) {
        self.cache.insert(path, thumbnail);
    }

    pub fn remove(&mut self, path: &Path) {
        self.cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

#[derive(Debug)]
pub struct PreviewImageService {
    cache: TimedCache<PathBuf, Thumbnail>,
}

impl PreviewImageService {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<&Thumbnail> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, thumbnail: Thumbnail) {
        self.cache.insert(path, thumbnail);
    }

    pub fn remove(&mut self, path: &Path) {
        self.cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

pub fn generate_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    let image = image::open(path).ok()?;
    let thumbnail = image.thumbnail(max_size, max_size);
    let mut bytes = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .ok()?;
    Some(Thumbnail::new(bytes, Some("image/png".to_string())))
}

pub fn generate_preview(path: &Path, max_size: u32) -> Option<Thumbnail> {
    // Fast path: check extension first (free)
    if is_gif_path(path) {
        let bytes = std::fs::read(path).ok()?;
        return Some(Thumbnail::new(bytes, Some("image/gif".to_string())));
    }

    // Check 6-byte magic header before loading the entire file
    {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut header = [0u8; 6];
        if file.read_exact(&mut header).is_ok() && is_gif_header(&header) {
            drop(file);
            let bytes = std::fs::read(path).ok()?;
            return Some(Thumbnail::new(bytes, Some("image/gif".to_string())));
        }
    }

    // Non-GIF: use image::open() which streams through the decoder
    // instead of loading the entire file into memory first
    let image = image::open(path).ok()?;
    let preview = image.thumbnail(max_size, max_size);
    let mut preview_bytes = Vec::new();
    preview
        .write_to(
            &mut Cursor::new(&mut preview_bytes),
            image::ImageFormat::Png,
        )
        .ok()?;
    Some(Thumbnail::new(preview_bytes, Some("image/png".to_string())))
}

fn is_gif_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
}

fn is_gif_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}
