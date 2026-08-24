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

impl XionApp {
    pub(in crate::ui::app) fn new() -> (Self, Task<UiMessage>) {
        let config_manager = ConfigManager::new();
        let config_load = config_manager.load();
        let mut config = config_load.config;

        // Override start_path if a CLI path was provided
        if let Some(cli_path) = super::CLI_START_PATH
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
        {
            config.start_path = cli_path;
        }

        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.key());

        // Restore tabs from persisted config; fall back to a single default tab.
        let persisted_tabs = &state.config.tabs;
        let (tabs, active_tab_init) = if persisted_tabs.is_empty() {
            (
                vec![TabState {
                    title: "Ce PC".to_string(),
                    path: state.route.key(),
                }],
                0usize,
            )
        } else {
            let restored: Vec<TabState> = persisted_tabs
                .iter()
                .enumerate()
                .map(|(i, t)| TabState {
                    title: if i == 0 {
                        "Ce PC".to_string()
                    } else {
                        format!("Ce PC {}", i + 1)
                    },
                    path: t.path.clone(),
                })
                .collect();
            let idx = state
                .config
                .active_tab_index
                .min(restored.len().saturating_sub(1));
            (restored, idx)
        };

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
            entries,
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
            search_index_cache: std::collections::VecDeque::new(),
            tab_drag_source: None,
        };
        // If we restored tabs, set the active route to the active tab's path.
        if !app.state.config.tabs.is_empty() {
            if let Some(path) = app
                .tab_manager
                .tabs
                .get(active_tab_init)
                .map(|t| t.path.clone())
            {
                app.update_active_tab_path(path.clone());
                app.history.record(path);
            }
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
            entries: PagedEntries::new(0, page_size),
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
            search_index_cache: std::collections::VecDeque::new(),
            tab_drag_source: None,
        };
        app.rebuild_tree_cache();
        app
    }
}
