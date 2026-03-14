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
    // Use ImageReader to auto-detect format and apply EXIF orientation
    let reader = image::ImageReader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let image = reader.decode().ok()?;
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

    // Check 6-byte magic header; if GIF, read file once and return
    {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut header = [0u8; 6];
        if file.read_exact(&mut header).is_ok() && is_gif_header(&header) {
            // Read remaining bytes after the header we already consumed
            let mut bytes = header.to_vec();
            file.read_to_end(&mut bytes).ok()?;
            return Some(Thumbnail::new(bytes, Some("image/gif".to_string())));
        }
    }

    // Non-GIF: use ImageReader for EXIF orientation support
    let reader = image::ImageReader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let image = reader.decode().ok()?;
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

/// #13: Generate video thumbnail using ffmpeg CLI (if available).
/// Extracts a single frame at 1 second into the video.
pub fn generate_video_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "webm" | "mov" | "wmv" | "flv" | "m4v") {
        return None;
    }
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-ss", "1",
            "-i", &path.to_string_lossy(),
            "-vframes", "1",
            "-vf", &format!("scale='min({max_size},iw)':min'({max_size},ih)':force_original_aspect_ratio=decrease"),
            "-f", "image2pipe",
            "-vcodec", "png",
            "-",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if output.status.success() && !output.stdout.is_empty() {
        Some(Thumbnail::new(output.stdout, Some("image/png".to_string())))
    } else {
        None
    }
}

/// #14: Generate PDF thumbnail using mutool CLI (MuPDF) if available,
/// falling back to pdftoppm (Poppler) as secondary option.
/// Renders the first page of the PDF to a PNG image.
pub fn generate_pdf_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext != "pdf" {
        return None;
    }

    // Try mutool (MuPDF) first — outputs PNG to stdout
    let output = std::process::Command::new("mutool")
        .args([
            "draw",
            "-o", "-",       // output to stdout
            "-F", "png",     // PNG format
            "-w", &max_size.to_string(),
            "-h", &max_size.to_string(),
            &path.to_string_lossy(),
            "1",             // first page only
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    if let Ok(output) = output {
        if output.status.success() && !output.stdout.is_empty() {
            return Some(Thumbnail::new(output.stdout, Some("image/png".to_string())));
        }
    }

    // Fallback: pdftoppm (Poppler) — writes to temp file
    let temp_dir = std::env::temp_dir();
    let temp_prefix = temp_dir.join("xion_pdf_preview");
    let output = std::process::Command::new("pdftoppm")
        .args([
            "-png",
            "-f", "1",
            "-l", "1",
            "-scale-to", &max_size.to_string(),
            &path.to_string_lossy(),
            &temp_prefix.to_string_lossy(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            // pdftoppm creates <prefix>-1.png
            let png_path = temp_dir.join("xion_pdf_preview-1.png");
            if let Ok(bytes) = std::fs::read(&png_path) {
                let _ = std::fs::remove_file(&png_path);
                if !bytes.is_empty() {
                    return Some(Thumbnail::new(bytes, Some("image/png".to_string())));
                }
            }
        }
    }

    None
}

fn is_gif_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
}

fn is_gif_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}
