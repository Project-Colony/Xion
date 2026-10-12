//! Public configuration data types and their default values.

use std::path::PathBuf;

use colony_ui::preferences::StandardPreferences;
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

// ── Language / shell enums ────────────────────────────────────────────────────

/// The interface language the user picked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    /// The session's language: `LC_ALL`, `LC_MESSAGES` or `LANG` on Unix, the
    /// display language on Windows. French when it is French, English
    /// otherwise.
    #[default]
    Auto,
    English,
    French,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::Auto, Self::English, Self::French];

    /// How the choice is written in `config.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en",
            Self::French => "fr",
        }
    }

    /// The choice a `config.toml` value stands for; `None` when it is unknown.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.key() == key)
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
    /// Derived from the theme, never entered separately.
    pub dark_mode: bool,
    /// Theme, accent, text sizes, high contrast, dyslexia font and motion: the
    /// Appearance and Accessibility settings every Colony program shares,
    /// stored flat at the top of `config.toml` under colony-ui's field names.
    pub standard: StandardPreferences,
    pub terminal_shell: ShellConfig,
    pub language: Language,
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
            dark_mode: crate::ui::theme::resolves_dark(&StandardPreferences::default()),
            standard: StandardPreferences::default(),
            terminal_shell: ShellConfig::default(),
            language: Language::default(),
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

/// A problem found while loading the configuration, shown to the user.
///
/// Kept as a locale key and its values rather than as text: the file is read
/// before its own `language` setting is applied, so the message is only worded
/// when it is displayed.
#[derive(Debug, Clone)]
pub struct ConfigWarning {
    key: &'static str,
    args: Vec<(&'static str, String)>,
}

impl ConfigWarning {
    pub fn new(key: &'static str, args: &[(&'static str, &dyn std::fmt::Display)]) -> Self {
        Self {
            key,
            args: args
                .iter()
                .map(|(name, value)| (*name, value.to_string()))
                .collect(),
        }
    }
}

/// Every warning of a load on one line, for the status bar.
pub fn config_warnings_summary(warnings: &[ConfigWarning]) -> String {
    let joined = warnings
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ");
    crate::i18n::tf("config_warnings", &[("warnings", &joined)])
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args: Vec<(&str, &dyn std::fmt::Display)> = self
            .args
            .iter()
            .map(|(name, value)| (*name, value as &dyn std::fmt::Display))
            .collect();
        formatter.write_str(&crate::i18n::tf(self.key, &args))
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
    use super::AppConfig;

    /// The default follows the Colony catalogue's fallback rather than a local
    /// taste.
    #[test]
    fn the_default_theme_is_the_ecosystem_fallback() {
        let standard = AppConfig::default().standard;
        assert_eq!(
            (
                standard.selected_theme.as_str(),
                standard.selected_variant.as_str()
            ),
            colony_ui::preferences::DEFAULT_THEME
        );
        assert!(!standard.high_contrast);
        assert_eq!(standard.selected_accent, None, "the theme's own accent");
    }

    /// `dark_mode` is derived from the resolved palette, not entered
    /// separately: the two drifted apart in the past, and a light syntax
    /// highlighter on a dark interface survived until the next theme change.
    #[test]
    fn default_config_keeps_dark_mode_and_theme_in_sync() {
        let config = AppConfig::default();
        assert_eq!(
            config.dark_mode,
            crate::ui::theme::resolves_dark(&config.standard)
        );
    }
}
