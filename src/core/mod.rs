//! Core types, configuration, and error handling for Xion.
//!
//! This module provides the foundational types used throughout the application:
//!
//! - [`XionError`] - Unified error type for all Xion operations
//! - [`AppResult`] - Result type alias using `XionError`
//! - [`AppConfig`] - Application configuration with validation
//! - [`ConfigManager`] - Configuration loading and management

use std::fmt;
use std::path::PathBuf;

pub mod config;
pub use config::{
    AppConfig, AppConfigLoad, CacheConfig, ConfigManager, ConfigSource, ConfigWarning,
    EntryFilterConfig, FilesystemConfig, KeyChord, KeyInput, KeyKind, ListConfig, NamedKey,
    PagingConfig, ShortcutBindings, ShellConfig, SortKeyConfig, SortOrderConfig, TabPersistConfig, ThemeConfig,
    ViewColumn, ViewConfig, ViewMode,
};

pub type AppResult<T> = Result<T, XionError>;

#[derive(Debug)]
pub enum XionError {
    Io(std::io::Error),
    InvalidPath(PathBuf),
    NotFound(PathBuf),
    Watcher(String),
}

impl fmt::Display for XionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::InvalidPath(path) => write!(formatter, "Invalid path: {}", path.display()),
            Self::NotFound(path) => write!(formatter, "Not found: {}", path.display()),
            Self::Watcher(error) => write!(formatter, "File watcher error: {error}"),
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
