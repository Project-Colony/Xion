use std::fmt;
use std::path::PathBuf;

pub type AppResult<T> = Result<T, XionError>;

#[derive(Debug)]
pub enum XionError {
    Io(std::io::Error),
    InvalidPath(PathBuf),
    NotFound(PathBuf),
}

impl fmt::Display for XionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::InvalidPath(path) => write!(formatter, "Invalid path: {}", path.display()),
            Self::NotFound(path) => write!(formatter, "Not found: {}", path.display()),
        }
    }
}

impl std::error::Error for XionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for XionError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub start_path: PathBuf,
    pub show_hidden: bool,
    pub thumbnail_size: u32,
    pub thumbnail_cache_entries: usize,
    pub thumbnail_cache_ttl_seconds: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        let start_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            start_path,
            show_hidden: false,
            thumbnail_size: 48,
            thumbnail_cache_entries: 256,
            thumbnail_cache_ttl_seconds: 300,
        }
    }
}
