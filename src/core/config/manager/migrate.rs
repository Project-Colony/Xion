//! Turning a parsed file into an `AppConfig`, and back.
//!
//! `merge_from_v0` and `merge_from_v1` are the migration path; `config_to_file`
//! is the only writer.

use crate::core::config::types::{
    AppConfig, ConfigWarning, EntryFilterConfig, ShellConfig, SortKeyConfig, SortOrderConfig,
    TabPersistConfig, ThemeConfig, ViewColumn, ViewMode,
};

use super::*;
// ── V0 / V1 merge ─────────────────────────────────────────────────────────────

pub(super) fn merge_from_v0(file: AppConfigFileV0, warnings: &mut Vec<ConfigWarning>) -> AppConfig {
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

pub(super) fn merge_from_v1(file: AppConfigFileV1, warnings: &mut Vec<ConfigWarning>) -> AppConfig {
    let mut config = AppConfig::default();
    // `theme` carries the whole truth and `dark_mode` is only derived from it.
    // The old merge propagated dark_mode -> theme but never theme -> dark_mode,
    // so `theme = "Nord"` with `dark_mode = false` survived the load and drove a
    // light syntax highlighter on a dark UI until the next theme change.
    match file.theme {
        Some(theme_str) => {
            config.theme = match theme_str.as_str() {
                "Light" => ThemeConfig::Light,
                "Dark" => ThemeConfig::Dark,
                "Nord" => ThemeConfig::Nord,
                "Solarized" => ThemeConfig::Solarized,
                "HighContrast" => ThemeConfig::HighContrast,
                other => {
                    warnings.push(ConfigWarning {
                        message: format!(
                            "Thème inconnu '{other}', utilisation du thème par défaut"
                        ),
                    });
                    ThemeConfig::default()
                }
            };
        }
        // Pre-theme files only had the boolean.
        None if file.dark_mode == Some(true) => config.theme = ThemeConfig::Dark,
        None => {}
    }
    config.dark_mode = config.theme.is_dark();
    if let Some(dark_mode) = file.dark_mode
        && dark_mode != config.dark_mode
    {
        warnings.push(ConfigWarning {
            message: format!(
                "dark_mode ({dark_mode}) incohérent avec le thème, recalculé à {}",
                config.dark_mode
            ),
        });
    }
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
                .map(|col| match col {
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
        config.user_favorites = user_favorites.into_iter().filter(|p| p.exists()).collect();
    }
    config
}

pub(super) fn config_to_file(config: &AppConfig) -> AppConfigFileV1 {
    let theme_str = match config.theme {
        ThemeConfig::Light => "Light",
        ThemeConfig::Dark => "Dark",
        ThemeConfig::Nord => "Nord",
        ThemeConfig::Solarized => "Solarized",
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
                SortKeyConfig::Name => SortKeyConfigFile::Name,
                SortKeyConfig::Modified => SortKeyConfigFile::Modified,
                SortKeyConfig::Size => SortKeyConfigFile::Size,
            }),
            sort_order: Some(match config.list.sort_order {
                SortOrderConfig::Asc => SortOrderConfigFile::Asc,
                SortOrderConfig::Desc => SortOrderConfigFile::Desc,
            }),
            directories_first: Some(config.list.directories_first),
            filter: Some(match config.list.filter {
                EntryFilterConfig::All => EntryFilterConfigFile::All,
                EntryFilterConfig::OnlyDirectories => EntryFilterConfigFile::OnlyDirectories,
                EntryFilterConfig::OnlyFiles => EntryFilterConfigFile::OnlyFiles,
            }),
        }),
        cache: Some(CacheConfigFile {
            thumbnail_entries: Some(config.cache.thumbnail_entries),
            thumbnail_ttl_seconds: Some(config.cache.thumbnail_ttl_seconds),
            directory_entries: Some(config.cache.directory_entries),
            directory_ttl_seconds: Some(config.cache.directory_ttl_seconds),
        }),
        filesystem: Some(FilesystemConfigFile {
            metadata_batch_size: Some(config.filesystem.metadata_batch_size),
            metadata_parallelism: Some(config.filesystem.metadata_parallelism),
        }),
        view: Some(ViewConfigFile {
            mode: Some(match config.view.mode {
                ViewMode::List => ViewModeConfigFile::List,
                ViewMode::Grid => ViewModeConfigFile::Grid,
            }),
            thumbnail_size: Some(config.view.thumbnail_size),
            row_height: Some(config.view.row_height),
            grid_columns: Some(config.view.grid_columns),
            grid_row_height: Some(config.view.grid_row_height),
            overscan: Some(config.view.overscan),
            columns: Some(
                config
                    .view
                    .columns
                    .iter()
                    .map(|col| match col {
                        ViewColumn::Name => ViewColumnConfigFile::Name,
                        ViewColumn::Type => ViewColumnConfigFile::Type,
                        ViewColumn::Size => ViewColumnConfigFile::Size,
                        ViewColumn::Modified => ViewColumnConfigFile::Modified,
                    })
                    .collect(),
            ),
        }),
        paging: Some(PagingConfigFile {
            page_size: Some(config.paging.page_size),
        }),
        tabs: Some(
            config
                .tabs
                .iter()
                .map(|t| TabPersistConfigFile {
                    path: Some(t.path.clone()),
                })
                .collect(),
        ),
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
            let map: std::collections::HashMap<String, String> = config
                .labels
                .iter()
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
            move_up: Some(chord_to_string(&config.shortcuts.move_up)),
            move_down: Some(chord_to_string(&config.shortcuts.move_down)),
            move_home: Some(chord_to_string(&config.shortcuts.move_home)),
            move_end: Some(chord_to_string(&config.shortcuts.move_end)),
            activate: Some(chord_to_string(&config.shortcuts.activate)),
            clear_selection: Some(chord_to_string(&config.shortcuts.clear_selection)),
            cycle_pane_focus: Some(chord_to_string(&config.shortcuts.cycle_pane_focus)),
            back: Some(chord_to_string(&config.shortcuts.back)),
            forward: Some(chord_to_string(&config.shortcuts.forward)),
            refresh: Some(chord_to_string(&config.shortcuts.refresh)),
            select_all: Some(chord_to_string(&config.shortcuts.select_all)),
            toggle_context_menu: Some(chord_to_string(&config.shortcuts.toggle_context_menu)),
            rename: Some(chord_to_string(&config.shortcuts.rename)),
            delete: Some(chord_to_string(&config.shortcuts.delete)),
            new_folder: Some(chord_to_string(&config.shortcuts.new_folder)),
            focus_search: Some(chord_to_string(&config.shortcuts.focus_search)),
        }),
    }
}
