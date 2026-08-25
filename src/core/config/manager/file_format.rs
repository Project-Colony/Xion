//! The on-disk shapes: the V0 and V1 TOML documents.
//!
//! Deliberately separate from `AppConfig`: the file format has to stay
//! backward compatible, the runtime type does not.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Private file format structs (V0 / V1) ────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct VersionHeader {
    pub(super) version: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AppConfigFileV0 {
    pub(super) start_path: Option<PathBuf>,
    pub(super) show_hidden: Option<bool>,
    pub(super) thumbnail_size: Option<u32>,
    pub(super) thumbnail_cache_entries: Option<usize>,
    pub(super) thumbnail_cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct TabPersistConfigFile {
    pub(super) path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct AppConfigFileV1 {
    pub(super) version: Option<u32>,
    pub(super) dark_mode: Option<bool>,
    /// Ancien nom de thème : `Light`, `Nord`, `HighContrast`… Toujours lu, pour
    /// migrer les fichiers écrits avant l'adoption du catalogue Colony. Plus
    /// jamais écrit.
    pub(super) theme: Option<String>,
    /// Clé de famille du catalogue Colony.
    pub(super) theme_family: Option<String>,
    /// Clé de variante dans cette famille.
    pub(super) theme_variant: Option<String>,
    /// Rehaussement du contraste, applicable à n'importe quelle palette.
    pub(super) high_contrast: Option<bool>,
    /// Accent choisi ; absent signifie « celui du thème ».
    pub(super) accent: Option<String>,
    pub(super) start_path: Option<PathBuf>,
    pub(super) list: Option<ListConfigFile>,
    pub(super) cache: Option<CacheConfigFile>,
    pub(super) filesystem: Option<FilesystemConfigFile>,
    pub(super) view: Option<ViewConfigFile>,
    pub(super) paging: Option<PagingConfigFile>,
    pub(super) shortcuts: Option<ShortcutBindingsFile>,
    pub(super) tabs: Option<Vec<TabPersistConfigFile>>,
    pub(super) active_tab_index: Option<usize>,
    pub(super) compact_mode: Option<bool>,
    pub(super) terminal_shell: Option<String>,
    pub(super) respect_gitignore: Option<bool>,
    pub(super) labels: Option<std::collections::HashMap<String, String>>,
    pub(super) column_widths: Option<std::collections::HashMap<String, f32>>,
    pub(super) user_favorites: Option<Vec<PathBuf>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ListConfigFile {
    pub(super) show_hidden: Option<bool>,
    pub(super) sort_key: Option<SortKeyConfigFile>,
    pub(super) sort_order: Option<SortOrderConfigFile>,
    pub(super) directories_first: Option<bool>,
    pub(super) filter: Option<EntryFilterConfigFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SortKeyConfigFile {
    Name,
    Modified,
    Size,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SortOrderConfigFile {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum EntryFilterConfigFile {
    All,
    OnlyDirectories,
    OnlyFiles,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ViewColumnConfigFile {
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ViewModeConfigFile {
    List,
    Grid,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct CacheConfigFile {
    pub(super) thumbnail_entries: Option<usize>,
    pub(super) thumbnail_ttl_seconds: Option<u64>,
    pub(super) directory_entries: Option<usize>,
    pub(super) directory_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct FilesystemConfigFile {
    pub(super) metadata_batch_size: Option<usize>,
    pub(super) metadata_parallelism: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ViewConfigFile {
    pub(super) mode: Option<ViewModeConfigFile>,
    pub(super) thumbnail_size: Option<u32>,
    pub(super) row_height: Option<f32>,
    pub(super) grid_columns: Option<usize>,
    pub(super) grid_row_height: Option<f32>,
    pub(super) overscan: Option<usize>,
    pub(super) columns: Option<Vec<ViewColumnConfigFile>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct PagingConfigFile {
    pub(super) page_size: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ShortcutBindingsFile {
    pub(super) move_up: Option<String>,
    pub(super) move_down: Option<String>,
    pub(super) move_home: Option<String>,
    pub(super) move_end: Option<String>,
    pub(super) activate: Option<String>,
    pub(super) clear_selection: Option<String>,
    pub(super) cycle_pane_focus: Option<String>,
    pub(super) back: Option<String>,
    pub(super) forward: Option<String>,
    pub(super) refresh: Option<String>,
    pub(super) select_all: Option<String>,
    pub(super) toggle_context_menu: Option<String>,
    pub(super) rename: Option<String>,
    pub(super) delete: Option<String>,
    pub(super) new_folder: Option<String>,
    pub(super) focus_search: Option<String>,
}
