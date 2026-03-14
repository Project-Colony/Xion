//! Config file loading, saving, validation, and the V0/V1 file format structures.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::shortcuts::{KeyChord, KeyKind, NamedKey, ShortcutBindings};
use super::types::{
    AppConfig, AppConfigLoad, ConfigSource, ConfigWarning, EntryFilterConfig,
    ShellConfig, SortKeyConfig, SortOrderConfig,
    TabPersistConfig, ThemeConfig, ViewColumn, ViewMode,
};

// ── Validation constants ──────────────────────────────────────────────────────

const CURRENT_CONFIG_VERSION: u32 = 1;
const MIN_THUMBNAIL_SIZE: u32 = 24;
const MAX_THUMBNAIL_SIZE: u32 = 256;
const MIN_CACHE_ENTRIES: usize = 32;
const MAX_CACHE_ENTRIES: usize = 8192;
const MIN_CACHE_TTL_SECONDS: u64 = 30;
const MAX_CACHE_TTL_SECONDS: u64 = 86400; // 24h max
const MIN_PAGE_SIZE: usize = 24;
const MAX_PAGE_SIZE: usize = 2048;
const MIN_ROW_HEIGHT: f32 = 20.0;
const MAX_ROW_HEIGHT: f32 = 72.0;
const MIN_GRID_COLUMNS: usize = 1;
const MAX_GRID_COLUMNS: usize = 12;
const MIN_GRID_ROW_HEIGHT: f32 = 72.0;
const MAX_GRID_ROW_HEIGHT: f32 = 240.0;
const MAX_OVERSCAN: usize = 128;
const MIN_METADATA_BATCH_SIZE: usize = 16;
const MAX_METADATA_BATCH_SIZE: usize = 4096;
const MIN_METADATA_PARALLELISM: usize = 1;
const MAX_METADATA_PARALLELISM: usize = 32;

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Manages loading and saving the application config file.
#[derive(Debug, Clone)]
pub struct ConfigManager {
    path: PathBuf,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    /// Creates a new configuration manager with the default config path.
    pub fn new() -> Self {
        let path = default_config_path();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> AppConfigLoad {
        match load_from_path(&self.path) {
            Ok(load) => load,
            Err(error) => AppConfigLoad {
                config: AppConfig::default(),
                source: ConfigSource::Default,
                warnings: vec![ConfigWarning {
                    message: error.to_string(),
                }],
            },
        }
    }

    /// Saves the current config to disk. Errors are logged via tracing.
    pub fn save(&self, config: &AppConfig) {
        let file = config_to_file(config);
        match toml::to_string_pretty(&file) {
            Ok(contents) => {
                if let Some(parent) = self.path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        tracing::warn!("Config: impossible de créer {}: {e}", parent.display());
                    }
                }
                if let Err(e) = fs::write(&self.path, contents) {
                    tracing::warn!("Config: impossible d'écrire {}: {e}", self.path.display());
                }
            }
            Err(e) => {
                tracing::warn!("Config: erreur sérialisation TOML: {e}");
            }
        }
    }
}

// ── Private file format structs (V0 / V1) ────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VersionHeader {
    version: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AppConfigFileV0 {
    start_path: Option<PathBuf>,
    show_hidden: Option<bool>,
    thumbnail_size: Option<u32>,
    thumbnail_cache_entries: Option<usize>,
    thumbnail_cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TabPersistConfigFile {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AppConfigFileV1 {
    version: Option<u32>,
    dark_mode: Option<bool>,
    theme: Option<String>,
    start_path: Option<PathBuf>,
    list: Option<ListConfigFile>,
    cache: Option<CacheConfigFile>,
    filesystem: Option<FilesystemConfigFile>,
    view: Option<ViewConfigFile>,
    paging: Option<PagingConfigFile>,
    shortcuts: Option<ShortcutBindingsFile>,
    tabs: Option<Vec<TabPersistConfigFile>>,
    active_tab_index: Option<usize>,
    compact_mode: Option<bool>,
    terminal_shell: Option<String>,
    respect_gitignore: Option<bool>,
    labels: Option<std::collections::HashMap<String, String>>,
    column_widths: Option<std::collections::HashMap<String, f32>>,
    user_favorites: Option<Vec<PathBuf>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ListConfigFile {
    show_hidden: Option<bool>,
    sort_key: Option<SortKeyConfigFile>,
    sort_order: Option<SortOrderConfigFile>,
    directories_first: Option<bool>,
    filter: Option<EntryFilterConfigFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SortKeyConfigFile { Name, Modified, Size }

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SortOrderConfigFile { Asc, Desc }

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EntryFilterConfigFile { All, OnlyDirectories, OnlyFiles }

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ViewColumnConfigFile { Name, Type, Size, Modified }

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ViewModeConfigFile { List, Grid }

#[derive(Debug, Deserialize, Serialize)]
struct CacheConfigFile {
    thumbnail_entries: Option<usize>,
    thumbnail_ttl_seconds: Option<u64>,
    directory_entries: Option<usize>,
    directory_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FilesystemConfigFile {
    metadata_batch_size: Option<usize>,
    metadata_parallelism: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ViewConfigFile {
    mode: Option<ViewModeConfigFile>,
    thumbnail_size: Option<u32>,
    row_height: Option<f32>,
    grid_columns: Option<usize>,
    grid_row_height: Option<f32>,
    overscan: Option<usize>,
    columns: Option<Vec<ViewColumnConfigFile>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PagingConfigFile {
    page_size: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShortcutBindingsFile {
    move_up: Option<String>,
    move_down: Option<String>,
    move_home: Option<String>,
    move_end: Option<String>,
    activate: Option<String>,
    clear_selection: Option<String>,
    cycle_pane_focus: Option<String>,
    back: Option<String>,
    forward: Option<String>,
    refresh: Option<String>,
    select_all: Option<String>,
    toggle_context_menu: Option<String>,
    rename: Option<String>,
    delete: Option<String>,
    new_folder: Option<String>,
    focus_search: Option<String>,
}

// ── Loading logic ─────────────────────────────────────────────────────────────

fn default_config_path() -> PathBuf {
    ProjectDirs::from("io", "xion", "Xion")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

fn load_from_path(path: &Path) -> Result<AppConfigLoad, ConfigError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppConfigLoad {
                config: AppConfig::default(),
                source: ConfigSource::Default,
                warnings: Vec::new(),
            });
        }
        Err(error) => {
            return Err(ConfigError {
                message: format!("Lecture config impossible: {error}"),
            });
        }
    };

    let header: VersionHeader = toml::from_str(&contents).map_err(|error| ConfigError {
        message: format!("Header de config invalide: {error}"),
    })?;
    let version = header.version.unwrap_or(0);

    let mut warnings = Vec::new();
    let config = match version {
        0 => {
            let file: AppConfigFileV0 = toml::from_str(&contents).map_err(|error| ConfigError {
                message: format!("Config V0 invalide: {error}"),
            })?;
            let config = merge_from_v0(file, &mut warnings);
            return Ok(AppConfigLoad {
                config,
                source: ConfigSource::Migrated(path.to_path_buf()),
                warnings,
            });
        }
        CURRENT_CONFIG_VERSION => {
            let file: AppConfigFileV1 = toml::from_str(&contents).map_err(|error| ConfigError {
                message: format!("Config V{CURRENT_CONFIG_VERSION} invalide: {error}"),
            })?;
            merge_from_v1(file, &mut warnings)
        }
        other => {
            return Err(ConfigError {
                message: format!(
                    "Version de config {other} non supportée (max: {CURRENT_CONFIG_VERSION}). \
                     Mettez à jour Xion ou supprimez le fichier de config pour le recréer."
                ),
            });
        }
    };

    Ok(AppConfigLoad {
        config,
        source: ConfigSource::File(path.to_path_buf()),
        warnings,
    })
}

// ── V0 / V1 merge ─────────────────────────────────────────────────────────────

fn merge_from_v0(file: AppConfigFileV0, warnings: &mut Vec<ConfigWarning>) -> AppConfig {
    let mut config = AppConfig::default();
    if let Some(start_path) = file.start_path {
        config.start_path = validated_path(start_path, &config.start_path, warnings);
    }
    if let Some(show_hidden) = file.show_hidden {
        config.list.show_hidden = show_hidden;
    }
    if let Some(thumbnail_size) = file.thumbnail_size {
        config.view.thumbnail_size =
            validated_thumbnail_size(thumbnail_size, config.view.thumbnail_size, warnings);
    }
    if let Some(entries) = file.thumbnail_cache_entries {
        config.cache.thumbnail_entries = validated_cache_entries(
            entries, config.cache.thumbnail_entries, "cache.thumbnail_entries", warnings,
        );
    }
    if let Some(ttl) = file.thumbnail_cache_ttl_seconds {
        config.cache.thumbnail_ttl_seconds = validated_cache_ttl(
            ttl, config.cache.thumbnail_ttl_seconds, "cache.thumbnail_ttl_seconds", warnings,
        );
    }
    config
}

fn merge_from_v1(file: AppConfigFileV1, warnings: &mut Vec<ConfigWarning>) -> AppConfig {
    let mut config = AppConfig::default();
    if let Some(dark_mode) = file.dark_mode {
        config.dark_mode = dark_mode;
        if dark_mode && file.theme.is_none() {
            config.theme = ThemeConfig::Dark;
        }
    }
    if let Some(theme_str) = file.theme {
        config.theme = match theme_str.as_str() {
            "Light" => ThemeConfig::Light,
            "Dark" => ThemeConfig::Dark,
            "Nord" => ThemeConfig::Nord,
            "Solarized" => ThemeConfig::Solarized,
            "HighContrast" => ThemeConfig::HighContrast,
            other => {
                warnings.push(ConfigWarning {
                    message: format!("Thème inconnu '{}', utilisation du thème par défaut", other),
                });
                ThemeConfig::default()
            }
        };
    }
    if let Some(start_path) = file.start_path {
        config.start_path = validated_path(start_path, &config.start_path, warnings);
    }
    if let Some(list) = file.list {
        if let Some(show_hidden) = list.show_hidden { config.list.show_hidden = show_hidden; }
        if let Some(sort_key) = list.sort_key {
            config.list.sort_key = match sort_key {
                SortKeyConfigFile::Name => SortKeyConfig::Name,
                SortKeyConfigFile::Modified => SortKeyConfig::Modified,
                SortKeyConfigFile::Size => SortKeyConfig::Size,
            };
        }
        if let Some(sort_order) = list.sort_order {
            config.list.sort_order = match sort_order {
                SortOrderConfigFile::Asc => SortOrderConfig::Asc,
                SortOrderConfigFile::Desc => SortOrderConfig::Desc,
            };
        }
        if let Some(directories_first) = list.directories_first {
            config.list.directories_first = directories_first;
        }
        if let Some(filter) = list.filter {
            config.list.filter = match filter {
                EntryFilterConfigFile::All => EntryFilterConfig::All,
                EntryFilterConfigFile::OnlyDirectories => EntryFilterConfig::OnlyDirectories,
                EntryFilterConfigFile::OnlyFiles => EntryFilterConfig::OnlyFiles,
            };
        }
    }
    if let Some(cache) = file.cache {
        if let Some(entries) = cache.thumbnail_entries {
            config.cache.thumbnail_entries = validated_cache_entries(
                entries, config.cache.thumbnail_entries, "cache.thumbnail_entries", warnings,
            );
        }
        if let Some(ttl) = cache.thumbnail_ttl_seconds {
            config.cache.thumbnail_ttl_seconds = validated_cache_ttl(
                ttl, config.cache.thumbnail_ttl_seconds, "cache.thumbnail_ttl_seconds", warnings,
            );
        }
        if let Some(entries) = cache.directory_entries {
            config.cache.directory_entries = validated_cache_entries(
                entries, config.cache.directory_entries, "cache.directory_entries", warnings,
            );
        }
        if let Some(ttl) = cache.directory_ttl_seconds {
            config.cache.directory_ttl_seconds = validated_cache_ttl(
                ttl, config.cache.directory_ttl_seconds, "cache.directory_ttl_seconds", warnings,
            );
        }
    }
    if let Some(filesystem) = file.filesystem {
        if let Some(batch_size) = filesystem.metadata_batch_size {
            config.filesystem.metadata_batch_size = validated_metadata_batch_size(
                batch_size, config.filesystem.metadata_batch_size, warnings,
            );
        }
        if let Some(parallelism) = filesystem.metadata_parallelism {
            config.filesystem.metadata_parallelism = validated_metadata_parallelism(
                parallelism, config.filesystem.metadata_parallelism, warnings,
            );
        }
    }
    if let Some(view) = file.view {
        if let Some(mode) = view.mode {
            config.view.mode = match mode {
                ViewModeConfigFile::List => ViewMode::List,
                ViewModeConfigFile::Grid => ViewMode::Grid,
            };
        }
        if let Some(thumbnail_size) = view.thumbnail_size {
            config.view.thumbnail_size =
                validated_thumbnail_size(thumbnail_size, config.view.thumbnail_size, warnings);
        }
        if let Some(row_height) = view.row_height {
            config.view.row_height =
                validated_row_height(row_height, config.view.row_height, warnings);
        }
        if let Some(grid_columns) = view.grid_columns {
            config.view.grid_columns =
                validated_grid_columns(grid_columns, config.view.grid_columns, warnings);
        }
        if let Some(grid_row_height) = view.grid_row_height {
            config.view.grid_row_height =
                validated_grid_row_height(grid_row_height, config.view.grid_row_height, warnings);
        }
        if let Some(overscan) = view.overscan {
            config.view.overscan = validated_overscan(overscan, config.view.overscan, warnings);
        }
        if let Some(columns) = view.columns {
            let columns = columns.into_iter().map(|col| match col {
                ViewColumnConfigFile::Name => ViewColumn::Name,
                ViewColumnConfigFile::Type => ViewColumn::Type,
                ViewColumnConfigFile::Size => ViewColumn::Size,
                ViewColumnConfigFile::Modified => ViewColumn::Modified,
            }).collect::<Vec<_>>();
            config.view.columns =
                validated_view_columns(columns, config.view.columns.clone(), warnings);
        }
    }
    if let Some(paging) = file.paging {
        if let Some(page_size) = paging.page_size {
            config.paging.page_size =
                validated_page_size(page_size, config.paging.page_size, warnings);
        }
    }
    if let Some(shortcuts) = file.shortcuts {
        config.shortcuts = merge_shortcuts(shortcuts, config.shortcuts, warnings);
    }
    if let Some(tabs) = file.tabs {
        let restored: Vec<TabPersistConfig> = tabs
            .into_iter()
            .filter_map(|t| t.path.map(|p| TabPersistConfig { path: p }))
            .collect();
        if !restored.is_empty() {
            config.tabs = restored;
        }
    }
    if let Some(idx) = file.active_tab_index {
        if !config.tabs.is_empty() {
            config.active_tab_index = idx.min(config.tabs.len() - 1);
        }
    }
    if let Some(compact_mode) = file.compact_mode {
        config.compact_mode = compact_mode;
        // Keep row_height in sync with persisted compact_mode
        if compact_mode {
            config.view.row_height = 22.0;
        }
    }
    if let Some(shell_str) = file.terminal_shell {
        config.terminal_shell = match shell_str.as_str() {
            "Cmd" => ShellConfig::Cmd,
            "PowerShell" => ShellConfig::PowerShell,
            "GitBash" => ShellConfig::GitBash,
            other => ShellConfig::Custom(other.to_string()),
        };
    }
    if let Some(respect_gitignore) = file.respect_gitignore {
        config.respect_gitignore = respect_gitignore;
    }
    if let Some(labels_map) = file.labels {
        use std::path::PathBuf as LabelPath;
        for (path_str, label_str) in labels_map {
            let label = match label_str.as_str() {
                "Red" => crate::ui::FileLabel::Red,
                "Orange" => crate::ui::FileLabel::Orange,
                "Yellow" => crate::ui::FileLabel::Yellow,
                "Green" => crate::ui::FileLabel::Green,
                "Blue" => crate::ui::FileLabel::Blue,
                "Purple" => crate::ui::FileLabel::Purple,
                "Gray" => crate::ui::FileLabel::Gray,
                _ => continue,
            };
            config.labels.insert(LabelPath::from(path_str), label);
        }
    }
    if let Some(column_widths) = file.column_widths {
        // Validate: only keep positive widths
        let valid: std::collections::HashMap<String, f32> = column_widths
            .into_iter()
            .filter(|(_, w)| *w > 0.0 && w.is_finite())
            .collect();
        if !valid.is_empty() {
            config.column_widths = valid;
        }
    }
    if let Some(user_favorites) = file.user_favorites {
        // Keep only paths that still exist on disk
        config.user_favorites = user_favorites
            .into_iter()
            .filter(|p| p.exists())
            .collect();
    }
    config
}

// ── Validation helpers ────────────────────────────────────────────────────────

fn validated_path(path: PathBuf, fallback: &std::path::Path, warnings: &mut Vec<ConfigWarning>) -> PathBuf {
    if path.is_dir() {
        return path;
    }
    warnings.push(ConfigWarning {
        message: format!("start_path invalide, fallback sur {}", fallback.display()),
    });
    fallback.to_path_buf()
}

fn validated_thumbnail_size(value: u32, fallback: u32, warnings: &mut Vec<ConfigWarning>) -> u32 {
    if (MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("thumbnail_size hors limites ({MIN_THUMBNAIL_SIZE}-{MAX_THUMBNAIL_SIZE}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_cache_entries(value: usize, fallback: usize, label: &str, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_CACHE_ENTRIES..=MAX_CACHE_ENTRIES).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("{label} hors limites ({MIN_CACHE_ENTRIES}-{MAX_CACHE_ENTRIES}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_cache_ttl(value: u64, fallback: u64, label: &str, warnings: &mut Vec<ConfigWarning>) -> u64 {
    if (MIN_CACHE_TTL_SECONDS..=MAX_CACHE_TTL_SECONDS).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("{label} hors limites ({MIN_CACHE_TTL_SECONDS}-{MAX_CACHE_TTL_SECONDS}s), fallback sur {fallback}"),
    });
    fallback
}

fn validated_metadata_batch_size(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_METADATA_BATCH_SIZE..=MAX_METADATA_BATCH_SIZE).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("filesystem.metadata_batch_size hors limites ({MIN_METADATA_BATCH_SIZE}-{MAX_METADATA_BATCH_SIZE}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_metadata_parallelism(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_METADATA_PARALLELISM..=MAX_METADATA_PARALLELISM).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("filesystem.metadata_parallelism hors limites ({MIN_METADATA_PARALLELISM}-{MAX_METADATA_PARALLELISM}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_page_size(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("page_size hors limites ({MIN_PAGE_SIZE}-{MAX_PAGE_SIZE}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_row_height(value: f32, fallback: f32, warnings: &mut Vec<ConfigWarning>) -> f32 {
    if (MIN_ROW_HEIGHT..=MAX_ROW_HEIGHT).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("row_height hors limites ({MIN_ROW_HEIGHT}-{MAX_ROW_HEIGHT}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_grid_columns(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_GRID_COLUMNS..=MAX_GRID_COLUMNS).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("grid_columns hors limites ({MIN_GRID_COLUMNS}-{MAX_GRID_COLUMNS}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_grid_row_height(value: f32, fallback: f32, warnings: &mut Vec<ConfigWarning>) -> f32 {
    if (MIN_GRID_ROW_HEIGHT..=MAX_GRID_ROW_HEIGHT).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("grid_row_height hors limites ({MIN_GRID_ROW_HEIGHT}-{MAX_GRID_ROW_HEIGHT}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_overscan(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    const MIN_OVERSCAN: usize = 1;
    if (MIN_OVERSCAN..=MAX_OVERSCAN).contains(&value) { return value; }
    warnings.push(ConfigWarning {
        message: format!("overscan hors limites ({MIN_OVERSCAN}-{MAX_OVERSCAN}), fallback sur {fallback}"),
    });
    fallback
}

fn validated_view_columns(value: Vec<ViewColumn>, fallback: Vec<ViewColumn>, warnings: &mut Vec<ConfigWarning>) -> Vec<ViewColumn> {
    if value.is_empty() {
        warnings.push(ConfigWarning {
            message: "view.columns vide, fallback sur la configuration par défaut".to_string(),
        });
        return fallback;
    }
    value
}

// ── Shortcut merge / parse ────────────────────────────────────────────────────

fn merge_shortcuts(shortcuts: ShortcutBindingsFile, mut fallback: ShortcutBindings, warnings: &mut Vec<ConfigWarning>) -> ShortcutBindings {
    macro_rules! merge_field {
        ($field:ident, $label:expr) => {
            if let Some(value) = shortcuts.$field {
                fallback.$field = parse_shortcut(value, fallback.$field, $label, warnings);
            }
        };
    }
    merge_field!(move_up,            "shortcuts.move_up");
    merge_field!(move_down,          "shortcuts.move_down");
    merge_field!(move_home,          "shortcuts.move_home");
    merge_field!(move_end,           "shortcuts.move_end");
    merge_field!(activate,           "shortcuts.activate");
    merge_field!(clear_selection,    "shortcuts.clear_selection");
    merge_field!(cycle_pane_focus,   "shortcuts.cycle_pane_focus");
    merge_field!(back,               "shortcuts.back");
    merge_field!(forward,            "shortcuts.forward");
    merge_field!(refresh,            "shortcuts.refresh");
    merge_field!(select_all,         "shortcuts.select_all");
    merge_field!(toggle_context_menu,"shortcuts.toggle_context_menu");
    merge_field!(rename,             "shortcuts.rename");
    merge_field!(delete,             "shortcuts.delete");
    merge_field!(new_folder,         "shortcuts.new_folder");
    merge_field!(focus_search,       "shortcuts.focus_search");
    fallback
}

fn parse_shortcut(raw: String, fallback: KeyChord, label: &str, warnings: &mut Vec<ConfigWarning>) -> KeyChord {
    match KeyChord::parse(&raw) {
        Ok(chord) => chord,
        Err(error) => {
            warnings.push(ConfigWarning {
                message: format!("{label} invalide ({error}), fallback sur valeur par défaut"),
            });
            fallback
        }
    }
}

// ── Serialisation ─────────────────────────────────────────────────────────────

fn chord_to_string(chord: &KeyChord) -> String {
    let mut parts: Vec<String> = Vec::new();
    if chord.ctrl  { parts.push("Ctrl".to_string()); }
    if chord.alt   { parts.push("Alt".to_string()); }
    if chord.shift { parts.push("Shift".to_string()); }
    let key_str = match &chord.key {
        KeyKind::Named(named) => match named {
            NamedKey::ArrowUp    => "ArrowUp",
            NamedKey::ArrowDown  => "ArrowDown",
            NamedKey::ArrowLeft  => "ArrowLeft",
            NamedKey::ArrowRight => "ArrowRight",
            NamedKey::Home       => "Home",
            NamedKey::End        => "End",
            NamedKey::Enter      => "Enter",
            NamedKey::Escape     => "Escape",
            NamedKey::Tab        => "Tab",
            NamedKey::Space      => "Space",
            NamedKey::F2         => "F2",
            NamedKey::F3         => "F3",
            NamedKey::F5         => "F5",
            NamedKey::Delete     => "Delete",
        }.to_string(),
        KeyKind::Character(c) => c.clone(),
    };
    parts.push(key_str);
    parts.join("+")
}

fn config_to_file(config: &AppConfig) -> AppConfigFileV1 {
    let theme_str = match config.theme {
        ThemeConfig::Light        => "Light",
        ThemeConfig::Dark         => "Dark",
        ThemeConfig::Nord         => "Nord",
        ThemeConfig::Solarized    => "Solarized",
        ThemeConfig::HighContrast => "HighContrast",
    };
    AppConfigFileV1 {
        version: Some(CURRENT_CONFIG_VERSION),
        dark_mode: Some(config.dark_mode),
        theme: Some(theme_str.to_string()),
        start_path: Some(config.start_path.clone()),
        list: Some(ListConfigFile {
            show_hidden: Some(config.list.show_hidden),
            sort_key: Some(match config.list.sort_key {
                SortKeyConfig::Name     => SortKeyConfigFile::Name,
                SortKeyConfig::Modified => SortKeyConfigFile::Modified,
                SortKeyConfig::Size     => SortKeyConfigFile::Size,
            }),
            sort_order: Some(match config.list.sort_order {
                SortOrderConfig::Asc  => SortOrderConfigFile::Asc,
                SortOrderConfig::Desc => SortOrderConfigFile::Desc,
            }),
            directories_first: Some(config.list.directories_first),
            filter: Some(match config.list.filter {
                EntryFilterConfig::All             => EntryFilterConfigFile::All,
                EntryFilterConfig::OnlyDirectories => EntryFilterConfigFile::OnlyDirectories,
                EntryFilterConfig::OnlyFiles       => EntryFilterConfigFile::OnlyFiles,
            }),
        }),
        cache: Some(CacheConfigFile {
            thumbnail_entries:     Some(config.cache.thumbnail_entries),
            thumbnail_ttl_seconds: Some(config.cache.thumbnail_ttl_seconds),
            directory_entries:     Some(config.cache.directory_entries),
            directory_ttl_seconds: Some(config.cache.directory_ttl_seconds),
        }),
        filesystem: Some(FilesystemConfigFile {
            metadata_batch_size:  Some(config.filesystem.metadata_batch_size),
            metadata_parallelism: Some(config.filesystem.metadata_parallelism),
        }),
        view: Some(ViewConfigFile {
            mode: Some(match config.view.mode {
                ViewMode::List => ViewModeConfigFile::List,
                ViewMode::Grid => ViewModeConfigFile::Grid,
            }),
            thumbnail_size:  Some(config.view.thumbnail_size),
            row_height:      Some(config.view.row_height),
            grid_columns:    Some(config.view.grid_columns),
            grid_row_height: Some(config.view.grid_row_height),
            overscan:        Some(config.view.overscan),
            columns: Some(config.view.columns.iter().map(|col| match col {
                ViewColumn::Name     => ViewColumnConfigFile::Name,
                ViewColumn::Type     => ViewColumnConfigFile::Type,
                ViewColumn::Size     => ViewColumnConfigFile::Size,
                ViewColumn::Modified => ViewColumnConfigFile::Modified,
            }).collect()),
        }),
        paging: Some(PagingConfigFile {
            page_size: Some(config.paging.page_size),
        }),
        tabs: Some(config.tabs.iter()
            .map(|t| TabPersistConfigFile { path: Some(t.path.clone()) })
            .collect()),
        active_tab_index: Some(config.active_tab_index),
        compact_mode: Some(config.compact_mode),
        terminal_shell: Some(match &config.terminal_shell {
            ShellConfig::Cmd => "Cmd".to_string(),
            ShellConfig::PowerShell => "PowerShell".to_string(),
            ShellConfig::GitBash => "GitBash".to_string(),
            ShellConfig::Custom(s) => s.clone(),
        }),
        respect_gitignore: Some(config.respect_gitignore),
        labels: if config.labels.is_empty() {
            None
        } else {
            let map: std::collections::HashMap<String, String> = config.labels.iter()
                .map(|(path, label)| {
                    let label_str = match label {
                        crate::ui::FileLabel::Red => "Red",
                        crate::ui::FileLabel::Orange => "Orange",
                        crate::ui::FileLabel::Yellow => "Yellow",
                        crate::ui::FileLabel::Green => "Green",
                        crate::ui::FileLabel::Blue => "Blue",
                        crate::ui::FileLabel::Purple => "Purple",
                        crate::ui::FileLabel::Gray => "Gray",
                    };
                    (path.display().to_string(), label_str.to_string())
                })
                .collect();
            Some(map)
        },
        column_widths: if config.column_widths.is_empty() {
            None
        } else {
            Some(config.column_widths.clone())
        },
        user_favorites: if config.user_favorites.is_empty() {
            None
        } else {
            Some(config.user_favorites.clone())
        },
        shortcuts: Some(ShortcutBindingsFile {
            move_up:              Some(chord_to_string(&config.shortcuts.move_up)),
            move_down:            Some(chord_to_string(&config.shortcuts.move_down)),
            move_home:            Some(chord_to_string(&config.shortcuts.move_home)),
            move_end:             Some(chord_to_string(&config.shortcuts.move_end)),
            activate:             Some(chord_to_string(&config.shortcuts.activate)),
            clear_selection:      Some(chord_to_string(&config.shortcuts.clear_selection)),
            cycle_pane_focus:     Some(chord_to_string(&config.shortcuts.cycle_pane_focus)),
            back:                 Some(chord_to_string(&config.shortcuts.back)),
            forward:              Some(chord_to_string(&config.shortcuts.forward)),
            refresh:              Some(chord_to_string(&config.shortcuts.refresh)),
            select_all:           Some(chord_to_string(&config.shortcuts.select_all)),
            toggle_context_menu:  Some(chord_to_string(&config.shortcuts.toggle_context_menu)),
            rename:               Some(chord_to_string(&config.shortcuts.rename)),
            delete:               Some(chord_to_string(&config.shortcuts.delete)),
            new_folder:           Some(chord_to_string(&config.shortcuts.new_folder)),
            focus_search:         Some(chord_to_string(&config.shortcuts.focus_search)),
        }),
    }
}
