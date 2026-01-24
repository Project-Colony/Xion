use std::fmt;
use std::path::PathBuf;

pub mod config;
pub use config::{
    AppConfig, AppConfigLoad, CacheConfig, ConfigManager, ConfigSource, ConfigWarning,
    EntryFilterConfig, KeyChord, KeyInput, KeyKind, ListConfig, NamedKey, PagingConfig,
    ShortcutBindings, SortKeyConfig, SortOrderConfig, ViewColumn, ViewConfig,
};

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

pub type AppConfigPath = PathBuf;
