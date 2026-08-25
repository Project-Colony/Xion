//! Building a `XionApp`: the real constructor and the test one.
//!
//! Moved verbatim out of the single 809-line `impl XionApp` block in `mod.rs`.

use super::types::*;
use crate::core::ConfigManager;
use crate::filesystem::{NativeFileWatcher, NoopFileWatcher};
use crate::services::{
    DirectoryLoader, HistoryService, NetworkDiscoveryService, PreviewImageService, ThumbnailService,
};
use crate::ui::{AppState, ModifiersState, UiMessage};
use directories::UserDirs;
use iced::Task;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::XionApp;

/// The tabs to open with, and which one is active.
///
/// `cli_path` is the directory named on the command line. It has to win: it
/// used to be parsed, resolved and validated, then silently dropped, because
/// the restored session overwrote the route further down `new()`. `xion
/// /un/dossier` opened whichever tab happened to be active last time, which
/// also broke every desktop entry point, `xdg-open` included.
///
/// It is appended and activated rather than replacing a tab: asking for a
/// folder should not cost the user their saved session. Startup does not
/// persist the tab list, so repeated launches do not accumulate tabs.
fn initial_tabs(
    persisted: &[crate::core::TabPersistConfig],
    active_index: usize,
    fallback: std::path::PathBuf,
    cli_path: Option<&std::path::Path>,
) -> (Vec<TabState>, usize) {
    let mut tabs: Vec<TabState> = if persisted.is_empty() {
        vec![TabState {
            title: "Ce PC".to_string(),
            path: fallback,
        }]
    } else {
        persisted
            .iter()
            .enumerate()
            .map(|(index, tab)| TabState {
                title: if index == 0 {
                    "Ce PC".to_string()
                } else {
                    format!("Ce PC {}", index + 1)
                },
                path: tab.path.clone(),
            })
            .collect()
    };

    let active = active_index.min(tabs.len().saturating_sub(1));

    match cli_path {
        Some(path) => {
            // Déjà ouvert : on y va. `open_in_new_tab` fait la même chose pour
            // l'instance déjà lancée ; ce chemin-ci, celui du démarrage à froid,
            // avait été oublié. Sans ça, `xion ~/Images` alors qu'un onglet
            // Images est enregistré en ouvrait un second, identique.
            if let Some(index) = tabs.iter().position(|tab| tab.path == path) {
                return (tabs, index);
            }
            tabs.push(TabState {
                title: crate::ui::app::helpers::tree_label_for_path(path),
                path: path.to_path_buf(),
            });
            let last = tabs.len() - 1;
            (tabs, last)
        }
        None => (tabs, active),
    }
}

impl XionApp {
    pub(in crate::ui::app) fn new() -> (Self, Task<UiMessage>) {
        let config_manager = ConfigManager::new();
        let config_load = config_manager.load();
        let config = config_load.config;

        // Override start_path if a CLI path was provided
        let cli_path = super::CLI_START_PATH
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        // `config.start_path` is deliberately left alone. Overwriting it here
        // looked harmless — `AppState::new` reads it to pick the route — but the
        // same object is what gets written back to disk, so a single
        // `xion /un/dossier` permanently replaced the user's configured start
        // directory. A one-off argument must not rewrite a preference. The route
        // is set from `cli_path` after the app is built instead.

        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.key());

        let (tabs, active_tab_init) = initial_tabs(
            &state.config.tabs,
            state.config.active_tab_index,
            state.route.key(),
            cli_path.as_deref(),
        );

        let directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
            state.config.cache.directory_entries,
            Duration::from_secs(state.config.cache.directory_ttl_seconds),
            state.config.paging.page_size,
        )));
        let page_size = directory_loader
            .lock()
            .map(|loader| loader.page_size())
            .unwrap_or(state.config.paging.page_size);
        let thumbnails = ThumbnailService::new(
            state.config.cache.thumbnail_entries,
            Duration::from_secs(state.config.cache.thumbnail_ttl_seconds),
        );
        let preview_cache_entries = state.config.cache.thumbnail_entries.clamp(1, 8);
        let preview_images = PreviewImageService::new(
            preview_cache_entries,
            Duration::from_secs(state.config.cache.thumbnail_ttl_seconds),
        );
        let entries = PagedEntries::new(0, page_size);
        let address_input = state.route.address_label();
        let cached_home_dir = UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
        let favorites = build_default_favorites(&state.config.user_favorites);
        let mut watcher_error = None;
        let file_watcher: FileWatcherHandle = match NativeFileWatcher::new() {
            Ok(watcher) => FileWatcherHandle::new(Box::new(watcher)),
            Err(error) => {
                watcher_error = Some(error.to_string());
                FileWatcherHandle::new(Box::new(NoopFileWatcher::new()))
            }
        };
        let mut app = Self {
            state,
            history,
            directory_loader,
            network_discovery: NetworkDiscoveryService::new(),
            media: MediaState::new(thumbnails, preview_images),
            entries: Arc::new(entries),
            stale_entries: None,
            selection_snapshot: None,
            pending_pages: HashSet::new(),
            is_loading: false,
            is_refreshing: false,
            is_user_selecting: false,
            pending_refresh: false,
            pending_refresh_reload_config: false,
            show_loading_indicator: false,
            loading_generation: 0,
            scroll: ScrollState {
                offset: 0.0,
                height: 480.0,
                content_height: 0.0,
                tree_offset: 0.0,
                tree_height: 240.0,
            },
            error: None,
            modifiers: ModifiersState::default(),
            menus: MenuState::default(),
            cursor_position: None,
            last_action: None,
            address_input,
            search: SearchState::default(),
            config_manager,
            favorites,
            tab_manager: TabManager::new(tabs, active_tab_init),
            clipboard: ClipboardState::default(),
            operation_progress: None,
            rename_dialog: None,
            last_click_time: None,
            last_clicked_path: None,
            drag_state: None,
            drag_candidate: None,
            drag_start_position: None,
            selection_box_start: None,
            selection_box_current: None,
            list_viewport_bounds: None,
            mouse_pressed: false,
            ignore_next_navigation: None,
            deferred_messages: Vec::new(),
            pane_resize: PaneResizeState::default(),
            file_watcher,
            watched_path: None,
            cached_tree_nodes: Vec::new(),
            cached_home_dir,
            address_validation_cache: AddressValidationCache::default(),
            address_editing: false,
            cached_text_preview: None,
            preview_anim_progress: 0.0,
            preview_anim_target: 0.0,
            terminal: TerminalState::default(),
            terminal_anim_progress: 0.0,
            terminal_anim_target: 0.0,
            properties_dialog: None,
            bulk_rename: None,
            cached_highlighted_preview: None,
            git_statuses: std::collections::HashMap::new(),
            dir_sizes: std::collections::HashMap::new(),
            dir_sizes_loading: std::collections::HashSet::new(),
            archive_browser: None,
            dual_pane: DualPaneState::default(),
            diff_view: None,
            quick_filter: String::new(),
            quick_filter_active: false,
            recents: RecentsService::default(),
            sidebar_collapsed: HashSet::new(),
            column_resize_state: None,
            hex_view: None,
            preview_encoding: None,
            grep_state: None,
            permissions_view: None,
            confirm_dialog: None,
            window_size: (1280.0, 800.0),
            undo_stack: UndoStack::default(),
            pending_undo_context: std::collections::HashMap::new(),
            next_operation_id: 0,
            breadcrumb_dropdown: None,
            breadcrumb_dropdown_items: Vec::new(),
            breadcrumb_dropdown_has_more: false,
            tab_drag_source: None,
            single_instance: super::PRIMARY_CLAIM.lock().ok().and_then(|mut c| c.take()),
        };
        // The route follows whichever tab `initial_tabs` made active — the one
        // the argument asked for, or the one the saved session had.
        if let Some(path) = app
            .tab_manager
            .tabs
            .get(active_tab_init)
            .map(|tab| tab.path.clone())
        {
            app.update_active_tab_path(path.clone());
            app.history.record(path);
        }
        if !config_load.warnings.is_empty() {
            app.last_action = Some(format!(
                "Config: {}",
                config_load
                    .warnings
                    .iter()
                    .map(|warning| warning.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        if let Some(error) = watcher_error {
            app.last_action = Some(format!("Observateur FS indisponible: {error}"));
        }
        app.sync_watcher();
        let task = app.refresh_entries();
        (app, task)
    }

    /// Builds a minimal `XionApp` for unit/integration testing.
    ///
    /// Uses default config and a no-op file watcher; does not start async tasks.
    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn new_for_test() -> Self {
        let config = crate::core::AppConfig::default();
        // Integration tests drive `update()` with messages that persist the
        // config (SetTheme, ToggleCompactMode, SetShell, tab reordering...).
        // Left on the default path, a test run rewrites the developer's real
        // ~/.config/xion/config.toml. Point the manager at a throwaway file
        // instead; the process id keeps parallel test binaries apart.
        let config_path =
            std::env::temp_dir().join(format!("xion-test-{}-config.toml", std::process::id()));
        let state = AppState::new(config);
        let start_path = state.route.key();
        let mut history = HistoryService::default();
        history.record(start_path.clone());
        let page_size = state.config.paging.page_size;
        let thumbnails = ThumbnailService::new(32, std::time::Duration::from_secs(60));
        let preview_images = PreviewImageService::new(4, std::time::Duration::from_secs(60));
        let directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
            100,
            std::time::Duration::from_secs(300),
            page_size,
        )));
        let address_input = state.route.address_label();
        let favorites = build_default_favorites(&state.config.user_favorites);
        let file_watcher = FileWatcherHandle::new(Box::new(NoopFileWatcher::new()));
        let tabs = vec![TabState {
            title: "Ce PC".to_string(),
            path: start_path,
        }];
        let mut app = Self {
            state,
            history,
            directory_loader,
            network_discovery: NetworkDiscoveryService::new(),
            media: MediaState::new(thumbnails, preview_images),
            entries: Arc::new(PagedEntries::new(0, page_size)),
            stale_entries: None,
            selection_snapshot: None,
            pending_pages: HashSet::new(),
            is_loading: false,
            is_refreshing: false,
            is_user_selecting: false,
            pending_refresh: false,
            pending_refresh_reload_config: false,
            show_loading_indicator: false,
            loading_generation: 0,
            scroll: ScrollState {
                offset: 0.0,
                height: 480.0,
                content_height: 0.0,
                tree_offset: 0.0,
                tree_height: 240.0,
            },
            error: None,
            modifiers: ModifiersState::default(),
            menus: MenuState::default(),
            cursor_position: None,
            last_action: None,
            address_input,
            search: SearchState::default(),
            config_manager: crate::core::ConfigManager::with_path(config_path),
            favorites,
            tab_manager: TabManager::new(tabs, 0),
            clipboard: ClipboardState::default(),
            operation_progress: None,
            rename_dialog: None,
            last_click_time: None,
            last_clicked_path: None,
            drag_state: None,
            drag_candidate: None,
            drag_start_position: None,
            selection_box_start: None,
            selection_box_current: None,
            list_viewport_bounds: None,
            mouse_pressed: false,
            ignore_next_navigation: None,
            deferred_messages: Vec::new(),
            pane_resize: PaneResizeState::default(),
            file_watcher,
            watched_path: None,
            cached_tree_nodes: Vec::new(),
            cached_home_dir: None,
            address_validation_cache: AddressValidationCache::default(),
            address_editing: false,
            cached_text_preview: None,
            preview_anim_progress: 0.0,
            preview_anim_target: 0.0,
            terminal: TerminalState::default(),
            terminal_anim_progress: 0.0,
            terminal_anim_target: 0.0,
            properties_dialog: None,
            bulk_rename: None,
            cached_highlighted_preview: None,
            git_statuses: std::collections::HashMap::new(),
            dir_sizes: std::collections::HashMap::new(),
            dir_sizes_loading: std::collections::HashSet::new(),
            archive_browser: None,
            dual_pane: DualPaneState::default(),
            diff_view: None,
            quick_filter: String::new(),
            quick_filter_active: false,
            recents: RecentsService::default(),
            sidebar_collapsed: HashSet::new(),
            column_resize_state: None,
            hex_view: None,
            preview_encoding: None,
            grep_state: None,
            permissions_view: None,
            confirm_dialog: None,
            window_size: (1280.0, 800.0),
            undo_stack: UndoStack::default(),
            pending_undo_context: std::collections::HashMap::new(),
            next_operation_id: 0,
            breadcrumb_dropdown: None,
            breadcrumb_dropdown_items: Vec::new(),
            breadcrumb_dropdown_has_more: false,
            tab_drag_source: None,
            single_instance: None,
        };
        app.rebuild_tree_cache();
        app
    }
}

#[cfg(test)]
mod initial_tabs_tests {
    use super::initial_tabs;
    use crate::core::TabPersistConfig;
    use std::path::{Path, PathBuf};

    fn persisted(paths: &[&str]) -> Vec<TabPersistConfig> {
        paths
            .iter()
            .map(|path| TabPersistConfig {
                path: PathBuf::from(path),
            })
            .collect()
    }

    #[test]
    fn without_an_argument_the_saved_session_decides() {
        let (tabs, active) = initial_tabs(
            &persisted(&["/a", "/b", "/c"]),
            1,
            PathBuf::from("/repli"),
            None,
        );
        assert_eq!(tabs.len(), 3);
        assert_eq!(active, 1);
        assert_eq!(tabs[1].path, Path::new("/b"));
    }

    /// La régression : `xion /un/dossier` ouvrait l'onglet actif de la session
    /// précédente et jetait l'argument.
    #[test]
    fn an_argument_opens_that_directory() {
        let (tabs, active) = initial_tabs(
            &persisted(&["/images"]),
            0,
            PathBuf::from("/repli"),
            Some(Path::new("/tmp/demande")),
        );
        assert_eq!(tabs[active].path, Path::new("/tmp/demande"));
    }

    #[test]
    fn an_argument_does_not_cost_the_saved_session() {
        let (tabs, _) = initial_tabs(
            &persisted(&["/images", "/musique"]),
            0,
            PathBuf::from("/repli"),
            Some(Path::new("/tmp/demande")),
        );
        assert_eq!(tabs.len(), 3, "les onglets enregistrés restent");
        assert_eq!(tabs[0].path, Path::new("/images"));
        assert_eq!(tabs[1].path, Path::new("/musique"));
    }

    /// Le même bug que `open_in_new_tab` avait, corrigé là et oublié ici : deux
    /// chemins vers la même action, un seul réparé.
    #[test]
    fn an_argument_for_an_already_open_folder_does_not_duplicate_it() {
        let (tabs, active) = initial_tabs(
            &persisted(&["/images", "/musique"]),
            0,
            PathBuf::from("/repli"),
            Some(Path::new("/images")),
        );
        assert_eq!(tabs.len(), 2, "aucun onglet ajouté");
        assert_eq!(tabs[active].path, Path::new("/images"), "on y est allé");
    }

    #[test]
    fn an_argument_works_from_an_empty_session_too() {
        let (tabs, active) = initial_tabs(
            &[],
            0,
            PathBuf::from("/repli"),
            Some(Path::new("/tmp/demande")),
        );
        assert_eq!(tabs[active].path, Path::new("/tmp/demande"));
    }

    /// `active_tab_index` vient d'un fichier que l'utilisateur peut éditer.
    #[test]
    fn an_out_of_range_active_index_does_not_panic() {
        let (tabs, active) = initial_tabs(&persisted(&["/a"]), 99, PathBuf::from("/repli"), None);
        assert!(active < tabs.len());
    }
}
