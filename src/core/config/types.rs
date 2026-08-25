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

impl ViewColumn {
    /// Stable key under which this column's width is stored.
    ///
    /// Deliberately *not* the displayed label: widths used to be keyed by the
    /// French header text ("Nom", "Taille", "Modifié") while the defaults below
    /// were written in English, so three of the four defaults were never read
    /// and those columns silently started at the 150px fallback.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Type => "Type",
            Self::Size => "Size",
            Self::Modified => "Modified",
        }
    }
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

impl ThemeConfig {
    /// The Colony family and variant this choice selects.
    ///
    /// Xion used to carry its own five hand-written palettes. They are now a
    /// selection into the shared catalogue of `colony-ui` — twenty-five
    /// families, fifty-seven variants — so a colour fixed upstream reaches Xion
    /// without being re-typed here, and Xion looks like the rest of the
    /// ecosystem.
    ///
    /// `Light` and `Dark` were generic names with no family behind them; they
    /// map to the catalogue's own fallback family. `Nord` and `Solarized` map
    /// exactly. `HighContrast` is not a family at all — see
    /// [`Self::wants_high_contrast`].
    pub fn colony_keys(&self) -> (&'static str, &'static str) {
        match self {
            Self::Light => ("gruvbox", "light"),
            Self::Dark => ("gruvbox", "dark"),
            Self::Nord => ("nord", "dark"),
            Self::Solarized => ("solarized", "dark"),
            Self::HighContrast => ("gruvbox", "dark"),
        }
    }

    /// Whether the palette should be boosted for legibility.
    ///
    /// Colony treats high contrast as a modifier applied to any palette, not as
    /// a theme of its own — which is the better model, and it happens to fix a
    /// real defect of Xion's hand-written version: there, the header background
    /// and the list background were both pure black, so the two could only be
    /// told apart by a border.
    pub fn wants_high_contrast(&self) -> bool {
        matches!(self, Self::HighContrast)
    }

    /// Whether this theme uses a dark palette.
    ///
    /// `AppConfig::dark_mode` is derived from the theme, but the test was
    /// duplicated verbatim in the UI update handlers and nowhere in the config
    /// loader, so a hand-edited file could carry `theme = "Nord"` with
    /// `dark_mode = false` and keep a light syntax highlighter on a dark UI.
    pub fn is_dark(&self) -> bool {
        matches!(
            self,
            Self::Dark | Self::Nord | Self::Solarized | Self::HighContrast
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ShellConfig {
    #[default]
    Cmd,
    PowerShell,
    GitBash,
    Custom(String),
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
    /// User-added sidebar favorites (persisted across sessions).
    pub user_favorites: Vec<PathBuf>,
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
            user_favorites: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::{AppConfig, ThemeConfig};

    #[test]
    fn theme_is_dark_covers_every_variant() {
        assert!(!ThemeConfig::Light.is_dark());
        assert!(ThemeConfig::Dark.is_dark());
        assert!(ThemeConfig::Nord.is_dark());
        assert!(ThemeConfig::Solarized.is_dark());
        assert!(ThemeConfig::HighContrast.is_dark());
    }

    #[test]
    fn default_config_keeps_dark_mode_and_theme_in_sync() {
        let config = AppConfig::default();
        assert_eq!(config.dark_mode, config.theme.is_dark());
    }
}
