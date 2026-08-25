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

/// Ce que l'utilisateur a choisi dans l'apparence.
///
/// Xion portait cinq palettes écrites à la main derrière un `enum`. Elles sont
/// devenues une sélection dans le catalogue partagé de `colony-ui` — vingt-cinq
/// familles, cinquante-sept variantes, huit accents — donc le choix ne peut
/// plus être un ensemble fermé de cinq noms.
///
/// Les clés sont des chaînes et non un type fermé, à dessein : le socle ajoute
/// des familles sans que Xion soit recompilé, et `colony_ui::resolve` retombe
/// sur sa palette de repli pour tout couple inconnu — une famille retirée en
/// amont dégrade au lieu d'empêcher le démarrage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeChoice {
    /// Clé de famille du catalogue Colony : `gruvbox`, `nord`, `catppuccin`…
    pub family: String,
    /// Clé de variante dans cette famille : `dark`, `light`, `mocha`…
    pub variant: String,
    /// Rehausse le contraste. Colony en fait un modificateur applicable à
    /// n'importe quelle palette, pas un thème à part.
    pub high_contrast: bool,
    /// Accent choisi par l'utilisateur ; `None` signifie « celui du thème ».
    pub accent: Option<String>,
}

impl Default for ThemeChoice {
    fn default() -> Self {
        // La famille de repli du catalogue, pour que Xion démarre sur ce que
        // l'écosystème considère comme son défaut plutôt que sur un goût local.
        Self {
            family: "gruvbox".to_string(),
            variant: "dark".to_string(),
            high_contrast: false,
            accent: None,
        }
    }
}

impl ThemeChoice {
    /// Le couple que `colony_ui::resolve` attend.
    pub fn keys(&self) -> (&str, &str) {
        (&self.family, &self.variant)
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
    pub theme: ThemeChoice,
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
            // Dérivé du thème, jamais saisi à part : les deux ont divergé par
            // le passé, et un surligneur de syntaxe clair a tourné sur une
            // interface sombre jusqu'au changement de thème suivant.
            dark_mode: crate::ui::theme::resolves_dark(&ThemeChoice::default()),
            theme: ThemeChoice::default(),
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
    use super::{AppConfig, ThemeChoice};

    /// Le défaut suit le repli du catalogue Colony plutôt qu'un goût local.
    #[test]
    fn the_default_choice_is_the_ecosystem_fallback() {
        let choice = ThemeChoice::default();
        assert_eq!(choice.keys(), ("gruvbox", "dark"));
        assert!(!choice.high_contrast);
        assert_eq!(choice.accent, None, "« auto » veut dire l'accent du thème");
    }

    /// `dark_mode` est dérivé de la palette résolue, pas saisi à part : les
    /// deux ont divergé par le passé, et un surligneur de syntaxe clair sur une
    /// interface sombre a survécu jusqu'au changement de thème suivant.
    #[test]
    fn default_config_keeps_dark_mode_and_theme_in_sync() {
        let config = AppConfig::default();
        assert_eq!(
            config.dark_mode,
            crate::ui::theme::resolves_dark(&config.theme)
        );
    }
}
