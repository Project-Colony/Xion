use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Deserialize;

const CURRENT_CONFIG_VERSION: u32 = 1;
const MIN_THUMBNAIL_SIZE: u32 = 24;
const MAX_THUMBNAIL_SIZE: u32 = 256;
const MIN_CACHE_ENTRIES: usize = 32;
const MAX_CACHE_ENTRIES: usize = 8192;
const MIN_CACHE_TTL_SECONDS: u64 = 30;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Enter,
    Escape,
    Tab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyKind {
    Named(NamedKey),
    Character(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChord {
    pub key: KeyKind,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyChord {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut key_token = None;

        for token in raw
            .split('+')
            .map(|token| token.trim())
            .filter(|t| !t.is_empty())
        {
            let lower = token.to_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                _ => {
                    key_token = Some(token.to_string());
                }
            }
        }

        let key_token = key_token.ok_or_else(|| "Touche manquante".to_string())?;
        let key = match key_token.to_lowercase().as_str() {
            "arrowup" => KeyKind::Named(NamedKey::ArrowUp),
            "arrowdown" => KeyKind::Named(NamedKey::ArrowDown),
            "arrowleft" => KeyKind::Named(NamedKey::ArrowLeft),
            "arrowright" => KeyKind::Named(NamedKey::ArrowRight),
            "home" => KeyKind::Named(NamedKey::Home),
            "end" => KeyKind::Named(NamedKey::End),
            "enter" => KeyKind::Named(NamedKey::Enter),
            "escape" | "esc" => KeyKind::Named(NamedKey::Escape),
            "tab" => KeyKind::Named(NamedKey::Tab),
            other => {
                if other.chars().count() == 1 {
                    KeyKind::Character(other.to_string())
                } else {
                    return Err(format!("Touche inconnue: {other}"));
                }
            }
        };

        Ok(Self {
            key,
            ctrl,
            alt,
            shift,
        })
    }

    pub fn matches(&self, input: &KeyInput, allow_shift_override: bool) -> bool {
        let shift_matches = if allow_shift_override && !self.shift {
            true
        } else {
            self.shift == input.shift
        };

        self.ctrl == input.ctrl && self.alt == input.alt && shift_matches && self.key == input.key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    pub key: KeyKind,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBindings {
    pub move_up: KeyChord,
    pub move_down: KeyChord,
    pub move_home: KeyChord,
    pub move_end: KeyChord,
    pub activate: KeyChord,
    pub clear_selection: KeyChord,
    pub cycle_pane_focus: KeyChord,
    pub back: KeyChord,
    pub forward: KeyChord,
    pub refresh: KeyChord,
    pub select_all: KeyChord,
    pub toggle_context_menu: KeyChord,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            move_up: KeyChord::parse("ArrowUp").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::ArrowUp),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            move_down: KeyChord::parse("ArrowDown").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::ArrowDown),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            move_home: KeyChord::parse("Home").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::Home),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            move_end: KeyChord::parse("End").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::End),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            activate: KeyChord::parse("Enter").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::Enter),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            clear_selection: KeyChord::parse("Escape").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::Escape),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            cycle_pane_focus: KeyChord::parse("Tab").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::Tab),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            back: KeyChord::parse("Alt+ArrowLeft").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::ArrowLeft),
                ctrl: false,
                alt: true,
                shift: false,
            }),
            forward: KeyChord::parse("Alt+ArrowRight").unwrap_or(KeyChord {
                key: KeyKind::Named(NamedKey::ArrowRight),
                ctrl: false,
                alt: true,
                shift: false,
            }),
            refresh: KeyChord::parse("Ctrl+R").unwrap_or_else(|_| KeyChord {
                key: KeyKind::Character("r".to_string()),
                ctrl: true,
                alt: false,
                shift: false,
            }),
            select_all: KeyChord::parse("Ctrl+A").unwrap_or_else(|_| KeyChord {
                key: KeyKind::Character("a".to_string()),
                ctrl: true,
                alt: false,
                shift: false,
            }),
            toggle_context_menu: KeyChord::parse("Ctrl+M").unwrap_or_else(|_| KeyChord {
                key: KeyKind::Character("m".to_string()),
                ctrl: true,
                alt: false,
                shift: false,
            }),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagingConfig {
    pub page_size: usize,
}

impl Default for PagingConfig {
    fn default() -> Self {
        Self { page_size: 120 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub start_path: PathBuf,
    pub list: ListConfig,
    pub cache: CacheConfig,
    pub filesystem: FilesystemConfig,
    pub view: ViewConfig,
    pub paging: PagingConfig,
    pub shortcuts: ShortcutBindings,
}

impl Default for AppConfig {
    fn default() -> Self {
        let start_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            start_path,
            list: ListConfig::default(),
            cache: CacheConfig::default(),
            filesystem: FilesystemConfig::default(),
            view: ViewConfig::default(),
            paging: PagingConfig::default(),
            shortcuts: ShortcutBindings::default(),
        }
    }
}

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

impl fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfigLoad {
    pub config: AppConfig,
    pub source: ConfigSource,
    pub warnings: Vec<ConfigWarning>,
}

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
}

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

#[derive(Debug, Deserialize)]
struct AppConfigFileV1 {
    #[allow(dead_code)]
    version: Option<u32>,
    start_path: Option<PathBuf>,
    list: Option<ListConfigFile>,
    cache: Option<CacheConfigFile>,
    filesystem: Option<FilesystemConfigFile>,
    view: Option<ViewConfigFile>,
    paging: Option<PagingConfigFile>,
    shortcuts: Option<ShortcutBindingsFile>,
}

#[derive(Debug, Deserialize)]
struct ListConfigFile {
    show_hidden: Option<bool>,
    sort_key: Option<SortKeyConfigFile>,
    sort_order: Option<SortOrderConfigFile>,
    directories_first: Option<bool>,
    filter: Option<EntryFilterConfigFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SortKeyConfigFile {
    Name,
    Modified,
    Size,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SortOrderConfigFile {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EntryFilterConfigFile {
    All,
    OnlyDirectories,
    OnlyFiles,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ViewColumnConfigFile {
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ViewModeConfigFile {
    List,
    Grid,
}

#[derive(Debug, Deserialize)]
struct CacheConfigFile {
    thumbnail_entries: Option<usize>,
    thumbnail_ttl_seconds: Option<u64>,
    directory_entries: Option<usize>,
    directory_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct FilesystemConfigFile {
    metadata_batch_size: Option<usize>,
    metadata_parallelism: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ViewConfigFile {
    mode: Option<ViewModeConfigFile>,
    thumbnail_size: Option<u32>,
    row_height: Option<f32>,
    grid_columns: Option<usize>,
    grid_row_height: Option<f32>,
    overscan: Option<usize>,
    columns: Option<Vec<ViewColumnConfigFile>>,
}

#[derive(Debug, Deserialize)]
struct PagingConfigFile {
    page_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
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
}

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
                message: format!("Version de config inconnue: {other}"),
            });
        }
    };

    Ok(AppConfigLoad {
        config,
        source: ConfigSource::File(path.to_path_buf()),
        warnings,
    })
}

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
            entries,
            config.cache.thumbnail_entries,
            "cache.thumbnail_entries",
            warnings,
        );
    }
    if let Some(ttl) = file.thumbnail_cache_ttl_seconds {
        config.cache.thumbnail_ttl_seconds = validated_cache_ttl(
            ttl,
            config.cache.thumbnail_ttl_seconds,
            "cache.thumbnail_ttl_seconds",
            warnings,
        );
    }
    config
}

fn merge_from_v1(file: AppConfigFileV1, warnings: &mut Vec<ConfigWarning>) -> AppConfig {
    let mut config = AppConfig::default();
    if let Some(start_path) = file.start_path {
        config.start_path = validated_path(start_path, &config.start_path, warnings);
    }
    if let Some(list) = file.list {
        if let Some(show_hidden) = list.show_hidden {
            config.list.show_hidden = show_hidden;
        }
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
                entries,
                config.cache.thumbnail_entries,
                "cache.thumbnail_entries",
                warnings,
            );
        }
        if let Some(ttl) = cache.thumbnail_ttl_seconds {
            config.cache.thumbnail_ttl_seconds = validated_cache_ttl(
                ttl,
                config.cache.thumbnail_ttl_seconds,
                "cache.thumbnail_ttl_seconds",
                warnings,
            );
        }
        if let Some(entries) = cache.directory_entries {
            config.cache.directory_entries = validated_cache_entries(
                entries,
                config.cache.directory_entries,
                "cache.directory_entries",
                warnings,
            );
        }
        if let Some(ttl) = cache.directory_ttl_seconds {
            config.cache.directory_ttl_seconds = validated_cache_ttl(
                ttl,
                config.cache.directory_ttl_seconds,
                "cache.directory_ttl_seconds",
                warnings,
            );
        }
    }
    if let Some(filesystem) = file.filesystem {
        if let Some(batch_size) = filesystem.metadata_batch_size {
            config.filesystem.metadata_batch_size = validated_metadata_batch_size(
                batch_size,
                config.filesystem.metadata_batch_size,
                warnings,
            );
        }
        if let Some(parallelism) = filesystem.metadata_parallelism {
            config.filesystem.metadata_parallelism = validated_metadata_parallelism(
                parallelism,
                config.filesystem.metadata_parallelism,
                warnings,
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
            let columns = columns
                .into_iter()
                .map(|column| match column {
                    ViewColumnConfigFile::Name => ViewColumn::Name,
                    ViewColumnConfigFile::Type => ViewColumn::Type,
                    ViewColumnConfigFile::Size => ViewColumn::Size,
                    ViewColumnConfigFile::Modified => ViewColumn::Modified,
                })
                .collect::<Vec<_>>();
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
    config
}

fn validated_path(path: PathBuf, fallback: &Path, warnings: &mut Vec<ConfigWarning>) -> PathBuf {
    if path.is_dir() {
        return path;
    }
    warnings.push(ConfigWarning {
        message: format!("start_path invalide, fallback sur {}", fallback.display()),
    });
    fallback.to_path_buf()
}

fn validated_thumbnail_size(value: u32, fallback: u32, warnings: &mut Vec<ConfigWarning>) -> u32 {
    if (MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "thumbnail_size hors limites ({}-{}), fallback sur {fallback}",
                MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE
            ),
        });
        fallback
    }
}

fn validated_cache_entries(
    value: usize,
    fallback: usize,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_CACHE_ENTRIES..=MAX_CACHE_ENTRIES).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "{label} hors limites ({}-{}), fallback sur {fallback}",
                MIN_CACHE_ENTRIES, MAX_CACHE_ENTRIES
            ),
        });
        fallback
    }
}

fn validated_cache_ttl(
    value: u64,
    fallback: u64,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> u64 {
    if value >= MIN_CACHE_TTL_SECONDS {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "{label} trop bas (min {MIN_CACHE_TTL_SECONDS}), fallback sur {fallback}"
            ),
        });
        fallback
    }
}

fn validated_metadata_batch_size(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_METADATA_BATCH_SIZE..=MAX_METADATA_BATCH_SIZE).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "filesystem.metadata_batch_size hors limites ({}-{}), fallback sur {fallback}",
                MIN_METADATA_BATCH_SIZE, MAX_METADATA_BATCH_SIZE
            ),
        });
        fallback
    }
}

fn validated_metadata_parallelism(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_METADATA_PARALLELISM..=MAX_METADATA_PARALLELISM).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "filesystem.metadata_parallelism hors limites ({}-{}), fallback sur {fallback}",
                MIN_METADATA_PARALLELISM, MAX_METADATA_PARALLELISM
            ),
        });
        fallback
    }
}

fn validated_page_size(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if (MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "page_size hors limites ({}-{}), fallback sur {fallback}",
                MIN_PAGE_SIZE, MAX_PAGE_SIZE
            ),
        });
        fallback
    }
}

fn validated_row_height(value: f32, fallback: f32, warnings: &mut Vec<ConfigWarning>) -> f32 {
    if (MIN_ROW_HEIGHT..=MAX_ROW_HEIGHT).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "row_height hors limites ({}-{}), fallback sur {fallback}",
                MIN_ROW_HEIGHT, MAX_ROW_HEIGHT
            ),
        });
        fallback
    }
}

fn validated_grid_columns(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_GRID_COLUMNS..=MAX_GRID_COLUMNS).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "grid_columns hors limites ({}-{}), fallback sur {fallback}",
                MIN_GRID_COLUMNS, MAX_GRID_COLUMNS
            ),
        });
        fallback
    }
}

fn validated_grid_row_height(value: f32, fallback: f32, warnings: &mut Vec<ConfigWarning>) -> f32 {
    if (MIN_GRID_ROW_HEIGHT..=MAX_GRID_ROW_HEIGHT).contains(&value) {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!(
                "grid_row_height hors limites ({}-{}), fallback sur {fallback}",
                MIN_GRID_ROW_HEIGHT, MAX_GRID_ROW_HEIGHT
            ),
        });
        fallback
    }
}

fn validated_overscan(value: usize, fallback: usize, warnings: &mut Vec<ConfigWarning>) -> usize {
    if value <= MAX_OVERSCAN {
        value
    } else {
        warnings.push(ConfigWarning {
            message: format!("overscan hors limites (max {MAX_OVERSCAN}), fallback sur {fallback}"),
        });
        fallback
    }
}

fn validated_view_columns(
    value: Vec<ViewColumn>,
    fallback: Vec<ViewColumn>,
    warnings: &mut Vec<ConfigWarning>,
) -> Vec<ViewColumn> {
    if value.is_empty() {
        warnings.push(ConfigWarning {
            message: "view.columns vide, fallback sur la configuration par défaut".to_string(),
        });
        fallback
    } else {
        value
    }
}

fn merge_shortcuts(
    shortcuts: ShortcutBindingsFile,
    mut fallback: ShortcutBindings,
    warnings: &mut Vec<ConfigWarning>,
) -> ShortcutBindings {
    if let Some(value) = shortcuts.move_up {
        fallback.move_up = parse_shortcut(value, fallback.move_up, "shortcuts.move_up", warnings);
    }
    if let Some(value) = shortcuts.move_down {
        fallback.move_down =
            parse_shortcut(value, fallback.move_down, "shortcuts.move_down", warnings);
    }
    if let Some(value) = shortcuts.move_home {
        fallback.move_home =
            parse_shortcut(value, fallback.move_home, "shortcuts.move_home", warnings);
    }
    if let Some(value) = shortcuts.move_end {
        fallback.move_end =
            parse_shortcut(value, fallback.move_end, "shortcuts.move_end", warnings);
    }
    if let Some(value) = shortcuts.activate {
        fallback.activate =
            parse_shortcut(value, fallback.activate, "shortcuts.activate", warnings);
    }
    if let Some(value) = shortcuts.clear_selection {
        fallback.clear_selection = parse_shortcut(
            value,
            fallback.clear_selection,
            "shortcuts.clear_selection",
            warnings,
        );
    }
    if let Some(value) = shortcuts.cycle_pane_focus {
        fallback.cycle_pane_focus = parse_shortcut(
            value,
            fallback.cycle_pane_focus,
            "shortcuts.cycle_pane_focus",
            warnings,
        );
    }
    if let Some(value) = shortcuts.back {
        fallback.back = parse_shortcut(value, fallback.back, "shortcuts.back", warnings);
    }
    if let Some(value) = shortcuts.forward {
        fallback.forward = parse_shortcut(value, fallback.forward, "shortcuts.forward", warnings);
    }
    if let Some(value) = shortcuts.refresh {
        fallback.refresh = parse_shortcut(value, fallback.refresh, "shortcuts.refresh", warnings);
    }
    if let Some(value) = shortcuts.select_all {
        fallback.select_all =
            parse_shortcut(value, fallback.select_all, "shortcuts.select_all", warnings);
    }
    if let Some(value) = shortcuts.toggle_context_menu {
        fallback.toggle_context_menu = parse_shortcut(
            value,
            fallback.toggle_context_menu,
            "shortcuts.toggle_context_menu",
            warnings,
        );
    }
    fallback
}

fn parse_shortcut(
    raw: String,
    fallback: KeyChord,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> KeyChord {
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
