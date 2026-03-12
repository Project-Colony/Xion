//! Public configuration data types and their default values.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::shortcuts::ShortcutBindings;

// ── Sorting / filtering enums ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKeyConfig {
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrderConfig {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFilterConfig {
    All,
    OnlyDirectories,
    OnlyFiles,
}

// ── View enums ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewColumn {
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::List => Self::Grid,
            Self::Grid => Self::List,
        }
    }
}

// ── Theme / shell enums ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ThemeConfig {
    #[default]
    Light,
    Dark,
    Nord,
    Solarized,
    HighContrast,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShellConfig {
    Cmd,
    PowerShell,
    GitBash,
    Custom(String),
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig::Cmd
    }
}

// ── Per-subsystem config structs ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListConfig {
    pub show_hidden: bool,
    pub sort_key: SortKeyConfig,
    pub sort_order: SortOrderConfig,
    pub directories_first: bool,
    pub filter: EntryFilterConfig,
}

impl Default for ListConfig {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort_key: SortKeyConfig::Name,
            sort_order: SortOrderConfig::Asc,
            directories_first: true,
            filter: EntryFilterConfig::All,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfig {
    pub thumbnail_entries: usize,
    pub thumbnail_ttl_seconds: u64,
    pub directory_entries: usize,
    pub directory_ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            thumbnail_entries: 256,
            thumbnail_ttl_seconds: 300,
            directory_entries: 256,
            directory_ttl_seconds: 45,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemConfig {
    pub metadata_batch_size: usize,
    pub metadata_parallelism: usize,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            metadata_batch_size: 256,
            metadata_parallelism: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewConfig {
    pub mode: ViewMode,
    pub thumbnail_size: u32,
    pub row_height: f32,
    pub grid_columns: usize,
    pub grid_row_height: f32,
    pub overscan: usize,
    pub columns: Vec<ViewColumn>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            mode: ViewMode::List,
            thumbnail_size: 48,
            row_height: 32.0,
            grid_columns: 4,
            grid_row_height: 140.0,
            overscan: 6,
            columns: vec![
                ViewColumn::Name,
                ViewColumn::Type,
                ViewColumn::Size,
                ViewColumn::Modified,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagingConfig {
    pub page_size: usize,
}

impl Default for PagingConfig {
    fn default() -> Self {
        Self { page_size: 120 }
    }
}

// ── Persistence helpers ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TabPersistConfig {
    pub path: PathBuf,
}

// ── Top-level AppConfig ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub start_path: PathBuf,
    pub list: ListConfig,
    pub cache: CacheConfig,
    pub filesystem: FilesystemConfig,
    pub view: ViewConfig,
    pub paging: PagingConfig,
    pub shortcuts: ShortcutBindings,
    pub dark_mode: bool,
    pub theme: ThemeConfig,
    pub tabs: Vec<TabPersistConfig>,
    pub active_tab_index: usize,
    pub terminal_shell: ShellConfig,
    pub respect_gitignore: bool,
    pub labels: std::collections::HashMap<PathBuf, crate::ui::FileLabel>,
    pub column_widths: std::collections::HashMap<String, f32>,
    /// Compact list rows (22 px) instead of normal (32 px).
    pub compact_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let start_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut column_widths = std::collections::HashMap::new();
        column_widths.insert("Name".to_string(), 300.0f32);
        column_widths.insert("Type".to_string(), 80.0f32);
        column_widths.insert("Size".to_string(), 100.0f32);
        column_widths.insert("Modified".to_string(), 150.0f32);
        Self {
            start_path,
            list: ListConfig::default(),
            cache: CacheConfig::default(),
            filesystem: FilesystemConfig::default(),
            view: ViewConfig::default(),
            paging: PagingConfig::default(),
            shortcuts: ShortcutBindings::default(),
            dark_mode: false,
            theme: ThemeConfig::default(),
            tabs: Vec::new(),
            active_tab_index: 0,
            terminal_shell: ShellConfig::default(),
            respect_gitignore: false,
            labels: std::collections::HashMap::new(),
            column_widths,
            compact_mode: false,
        }
    }
}

// ── Config loading result ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ConfigSource {
    Default,
    File(PathBuf),
    Migrated(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ConfigWarning {
    pub message: String,
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfigLoad {
    pub config: AppConfig,
    pub source: ConfigSource,
    pub warnings: Vec<ConfigWarning>,
}
