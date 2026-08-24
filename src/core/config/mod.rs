//! Application configuration — types, loading, validation, and keyboard shortcuts.
//!
//! Organised into three sub-modules:
//!
//! - [`shortcuts`] — [`KeyChord`], [`KeyInput`], [`ShortcutBindings`] and related key types
//! - [`types`]     — all public config data structs and enums
//! - [`manager`]   — [`ConfigManager`], file-format structs, loading, and validation

pub mod manager;
pub mod shortcuts;
pub mod types;

pub use manager::{ConfigError, ConfigManager};
pub use shortcuts::{KeyChord, KeyInput, KeyKind, NamedKey, ShortcutBindings};
pub use types::{
    AppConfig, AppConfigLoad, CacheConfig, ConfigSource, ConfigWarning, EntryFilterConfig,
    FilesystemConfig, ListConfig, PagingConfig, ShellConfig, SortKeyConfig, SortOrderConfig,
    TabPersistConfig, ThemeConfig, ViewColumn, ViewConfig, ViewMode,
};
