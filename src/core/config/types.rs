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

/// What the user chose in the appearance.
///
/// Xion used to carry five hand-written palettes behind an `enum`. They became
/// a selection in `colony-ui`'s shared catalogue (twenty-five families,
/// fifty-seven variants, eight accents), so the choice can no longer be a
/// closed set of five names.
///
/// The keys are strings and not a closed type, on purpose: the foundation adds
/// families without Xion being recompiled, and `colony_ui::resolve` falls back
/// to its fallback palette for any unknown pair, so a family removed upstream
/// degrades instead of preventing startup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeChoice {
    /// Family key in the Colony catalogue: `gruvbox`, `nord`, `catppuccin`...
    pub family: String,
    /// Variant key within that family: `dark`, `light`, `mocha`...
    pub variant: String,
    /// Raise the contrast. Colony makes it a modifier for any palette, not a
    /// separate theme.
    pub high_contrast: bool,
    /// Accent chosen by the user; `None` means "the theme's".
    pub accent: Option<String>,
}

impl Default for ThemeChoice {
    fn default() -> Self {
        // The catalogue's fallback family, so Xion starts on what the ecosystem
        // considers its default rather than on a local taste.
        Self {
            family: "gruvbox".to_string(),
            variant: "dark".to_string(),
            high_contrast: false,
            accent: None,
        }
    }
}

impl ThemeChoice {
    /// The pair `colony_ui::resolve` expects.
    pub fn keys(&self) -> (&str, &str) {
        (&self.family, &self.variant)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ShellConfig {
    /// The user's login shell.
    ///
    /// An intent, not a binary: `$SHELL` on Unix, `cmd` on Windows. That is
    /// what lets the same configuration file stay valid from one machine to
    /// another; the variant used to be called `Cmd`, which named a Windows
    /// executable while starting zsh on Linux.
    #[default]
    System,
    PowerShell,
    /// Bash: Git Bash on Windows, the system bash elsewhere.
    Bash,
    /// An executable named by its absolute path.
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

/// One restored tab, as stored in `session.toml` (see [`super::Session`]).
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
    pub theme: ThemeChoice,
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
            // Derived from the theme, never entered separately: the two drifted
            // apart in the past, and a light syntax highlighter ran on a dark
            // interface until the next theme change.
            dark_mode: crate::ui::theme::resolves_dark(&ThemeChoice::default()),
            theme: ThemeChoice::default(),
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
    use super::{AppConfig, ThemeChoice};

    /// The default follows the Colony catalogue's fallback rather than a local taste.
    #[test]
    fn the_default_choice_is_the_ecosystem_fallback() {
        let choice = ThemeChoice::default();
        assert_eq!(choice.keys(), ("gruvbox", "dark"));
        assert!(!choice.high_contrast);
        assert_eq!(choice.accent, None, "\"auto\" means the theme's accent");
    }

    /// `dark_mode` is derived from the resolved palette, not entered
    /// separately: the two drifted apart in the past, and a light syntax
    /// highlighter on a dark interface survived until the next theme change.
    #[test]
    fn default_config_keeps_dark_mode_and_theme_in_sync() {
        let config = AppConfig::default();
        assert_eq!(
            config.dark_mode,
            crate::ui::theme::resolves_dark(&config.theme)
        );
    }
}
