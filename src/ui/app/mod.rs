//! Main application module for Xion file explorer.
//!
//! This module contains the [`XionApp`] struct which implements the Iced
//! application trait and handles all UI state, messages, and rendering.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Holds the CLI-provided start path, consumed once during app initialization.
static CLI_START_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

use iced::alignment::Horizontal;
// Tracing is available for future use
#[allow(unused_imports)]
use tracing::{debug, info, warn};
use iced::widget::image;
use iced::{
    Font, Length, Point, Rectangle, Subscription,
    Task, Theme, keyboard, mouse, time,
};

use directories::UserDirs;

use crate::core::{
    ConfigManager, SortKeyConfig,
    ViewColumn, ViewMode,
};
use crate::filesystem::{
    FsEntryType,
    NativeFileWatcher, NoopFileWatcher,
};
use crate::services::{
    DirectoryLoader, FavoritesService, HistoryService, NetworkDiscoveryService, PreviewImageService, ThumbnailService, VirtualList,
    VirtualWindow, generate_preview, generate_thumbnail,
};
use crate::ui::{
    AppState,
    ModifiersState, UiMessage,
};

mod archive;
mod helpers;
mod navigation;
mod operations;
mod permissions;
mod shell;
mod types;
mod state;
mod update;
mod view;

use types::*;
use helpers::TreeNode;

use crate::ui::theme::{FONT_NAME, fonts};
use crate::ui::theme::layout::TREE_ROW_HEIGHT;
use crate::ui::theme::timing::WATCHER_POLL_INTERVAL;

/// Widget ID for the main list scrollable — used to trigger initial scroll_to
/// so that `on_scroll` fires and captures viewport bounds.
const LIST_SCROLLABLE_ID: &str = "main_list_scrollable";

#[derive(Debug)]
pub struct XionApp {
    state: AppState,
    history: HistoryService,
    directory_loader: Arc<Mutex<DirectoryLoader>>,
    network_discovery: NetworkDiscoveryService,
    media: MediaState,
    entries: PagedEntries,
    stale_entries: Option<PagedEntries>,
    selection_snapshot: Option<PagedEntries>,
    pending_pages: HashSet<usize>,
    is_loading: bool,
    is_refreshing: bool,
    is_user_selecting: bool,
    pending_refresh: bool,
    pending_refresh_reload_config: bool,
    show_loading_indicator: bool,
    loading_generation: u64,
    scroll: ScrollState,
    error: Option<String>,
    modifiers: ModifiersState,
    menus: MenuState,
    cursor_position: Option<Point>,
    last_action: Option<String>,
    address_input: String,
    search: SearchState,
    config_manager: ConfigManager,
    favorites: FavoritesService,
    tab_manager: TabManager,
    clipboard: ClipboardState,
    operation_progress: Option<FileOpProgress>,
    rename_dialog: Option<RenameDialog>,
    last_click_time: Option<Instant>,
    last_clicked_path: Option<PathBuf>,
    drag_state: Option<DragState>,
    drag_candidate: Option<PathBuf>,
    drag_start_position: Option<Point>,
    selection_box_start: Option<Point>,
    selection_box_current: Option<Point>,
    list_viewport_bounds: Option<Rectangle>,
    mouse_pressed: bool,
    ignore_next_navigation: Option<PathBuf>,
    pane_resize: PaneResizeState,
    file_watcher: FileWatcherHandle,
    watched_path: Option<PathBuf>,
    cached_tree_nodes: Vec<TreeNode>,
    cached_home_dir: Option<PathBuf>,
    address_validation_cache: AddressValidationCache,
    address_editing: bool,
    cached_text_preview: Option<(PathBuf, String)>,
    preview_anim_progress: f32,
    preview_anim_target: f32,
    terminal: TerminalState,
    terminal_anim_progress: f32,
    terminal_anim_target: f32,
    // Feature 3: Properties dialog
    properties_dialog: Option<PropertiesDialog>,
    // Feature 5: Bulk rename
    bulk_rename: Option<BulkRenameState>,
    // Feature 6: Syntax highlighting cache
    cached_highlighted_preview: Option<(PathBuf, Vec<crate::ui::HighlightedLine>)>,
    // Feature 7: Git status
    git_statuses: std::collections::HashMap<PathBuf, crate::ui::GitFileStatus>,
    // Feature 8: Disk usage
    dir_sizes: std::collections::HashMap<PathBuf, u64>,
    dir_sizes_loading: std::collections::HashSet<PathBuf>,
    // Feature 10: Archive browser
    archive_browser: Option<ArchiveBrowserState>,
    // Feature 11: Dual pane
    dual_pane: DualPaneState,
    // Feature A: Compress to ZIP
    // (no extra state needed)
    // Feature C: File Diff
    diff_view: Option<DiffViewState>,
    // Feature E: Quick Filter
    quick_filter: String,
    quick_filter_active: bool,
    // Feature G: Recent Files
    recents: RecentsService,
    // UX: Sidebar accordion (collapsed section names)
    sidebar_collapsed: HashSet<String>,
    // Feature H: Column Resizing
    column_resize_state: Option<ColumnResizeState>,
    // Feature K: Hex Viewer
    hex_view: Option<HexViewState>,
    // Feature L: Encoding detection
    preview_encoding: Option<String>,
    // Feature O: Grep
    grep_state: Option<GrepState>,
    // Feature P: NTFS Permissions
    permissions_view: Option<PermissionsViewState>,
    // Feature Q: Undo
    undo_stack: UndoStack,
    /// Context for the in-flight file operation, used to build undo actions on completion.
    pending_undo_context: Option<PendingUndoContext>,
    // Feature R: Breadcrumb dropdown
    breadcrumb_dropdown: Option<PathBuf>,
    breadcrumb_dropdown_items: Vec<PathBuf>,
    breadcrumb_dropdown_has_more: bool,
    // #10: Search index LRU cache (max 8 entries)
    search_index_cache: std::collections::VecDeque<(PathBuf, crate::services::SearchIndex)>,
    // #18: Tab drag reorder
    tab_drag_source: Option<usize>,
}


impl XionApp {
    fn new() -> (Self, Task<UiMessage>) {
        let config_manager = ConfigManager::new();
        let config_load = config_manager.load();
        let mut config = config_load.config;

        // Override start_path if a CLI path was provided
        if let Some(cli_path) = CLI_START_PATH.lock().ok().and_then(|mut guard| guard.take()) {
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
            let idx = state.config.active_tab_index.min(restored.len().saturating_sub(1));
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
        let cached_home_dir = UserDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf());
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
            undo_stack: UndoStack::default(),
            pending_undo_context: None,
            breadcrumb_dropdown: None,
            breadcrumb_dropdown_items: Vec::new(),
            breadcrumb_dropdown_has_more: false,
            search_index_cache: std::collections::VecDeque::new(),
            tab_drag_source: None,
        };
        // If we restored tabs, set the active route to the active tab's path.
        if !app.state.config.tabs.is_empty() {
            if let Some(path) = app.tab_manager.tabs.get(active_tab_init).map(|t| t.path.clone()) {
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
    #[doc(hidden)]
    pub fn new_for_test() -> Self {
        let config = crate::core::AppConfig::default();
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
        let tabs = vec![TabState { title: "Ce PC".to_string(), path: start_path }];
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
            scroll: ScrollState { offset: 0.0, height: 480.0, content_height: 0.0, tree_offset: 0.0, tree_height: 240.0 },
            error: None,
            modifiers: ModifiersState::default(),
            menus: MenuState::default(),
            cursor_position: None,
            last_action: None,
            address_input,
            search: SearchState::default(),
            config_manager: crate::core::ConfigManager::new(),
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
            undo_stack: UndoStack::default(),
            pending_undo_context: None,
            breadcrumb_dropdown: None,
            breadcrumb_dropdown_items: Vec::new(),
            breadcrumb_dropdown_has_more: false,
            search_index_cache: std::collections::VecDeque::new(),
            tab_drag_source: None,
        };
        app.rebuild_tree_cache();
        app
    }

    fn subscription(&self) -> Subscription<UiMessage> {
        let mut subscriptions = vec![
            iced::event::listen_with(map_event_to_message),
        ];

        if self.media.animated.is_some() {
            subscriptions
                .push(time::every(Duration::from_millis(30)).map(UiMessage::AnimatedPreviewTick));
        }

        subscriptions.push(time::every(WATCHER_POLL_INTERVAL).map(|_| UiMessage::FileWatchTick));

        if self.operation_progress.is_some() {
            subscriptions.push(
                time::every(Duration::from_millis(100)).map(|_| UiMessage::OperationProgressTick),
            );
        }

        // Preview panel slide animation — only ticks while animating
        if (self.preview_anim_progress - self.preview_anim_target).abs() > 0.001 {
            subscriptions.push(
                time::every(Duration::from_millis(16)).map(|_| UiMessage::PreviewAnimTick),
            );
        }

        // Terminal panel slide animation — only ticks while animating
        if (self.terminal_anim_progress - self.terminal_anim_target).abs() > 0.001 {
            subscriptions.push(
                time::every(Duration::from_millis(16)).map(|_| UiMessage::TerminalAnimTick),
            );
        }

        // Terminal output polling — only when a live process exists on the active tab
        if self.terminal.active_ref().process.is_some() {
            subscriptions.push(
                time::every(Duration::from_millis(50)).map(|_| UiMessage::TerminalPollOutput),
            );
        }

        Subscription::batch(subscriptions)
    }


    fn request_all_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let total_pages =
            self.entries.total.div_ceil(self.entries.page_size);
        let mut tasks = Vec::new();
        for page_index in 0..total_pages {
            if !self.entries.is_page_loaded(page_index) {
                tasks.push(self.request_page(page_index));
            }
        }
        Task::batch(tasks)
    }

    fn ensure_visible_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        if self.normalized_search_query().is_some() {
            return self.request_all_pages();
        }

        let window = self.entry_virtual_window();
        if window.is_empty() {
            return Task::none();
        }

        let start_page = window.start / self.entries.page_size;
        let end_page = (window.end.saturating_sub(1)) / self.entries.page_size;

        let mut tasks = Vec::new();
        for page_index in start_page..=end_page {
            if !self.entries.is_page_loaded(page_index) {
                tasks.push(self.request_page(page_index));
            }
        }

        Task::batch(tasks)
    }

    fn entry_virtual_window(&self) -> VirtualWindow {
        self.entry_virtual_window_for(self.entries.total)
    }

    fn entry_virtual_window_for(&self, total: usize) -> VirtualWindow {
        match self.state.config.view.mode {
            ViewMode::List => self.list_virtual_window_for(total),
            ViewMode::Grid => {
                let grid = self.grid_window_for(total);
                let start = grid.window.start * grid.columns;
                let end = (grid.window.end * grid.columns).min(total);
                VirtualWindow {
                    start,
                    end,
                    padding_top: grid.window.padding_top,
                    padding_bottom: grid.window.padding_bottom,
                }
            }
        }
    }

    fn list_virtual_window_for(&self, total: usize) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: self.state.config.view.row_height,
            viewport_height: self.scroll.height,
            overscan: self.state.config.view.overscan,
        };
        virtual_list.visible_range(self.scroll.offset, total)
    }

    fn grid_window_for(&self, total: usize) -> GridWindow {
        let columns = self.state.config.view.grid_columns.max(1);
        let rows = total.div_ceil(columns);
        let virtual_list = VirtualList {
            item_height: self.state.config.view.grid_row_height,
            viewport_height: self.scroll.height,
            overscan: self.state.config.view.overscan,
        };
        let window = virtual_list.visible_range(self.scroll.offset, rows);
        GridWindow {
            window,
            columns,
            total,
        }
    }

    fn tree_virtual_window(&self, total: usize) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: TREE_ROW_HEIGHT,
            viewport_height: self.scroll.tree_height,
            overscan: self.state.config.view.overscan,
        };
        virtual_list.visible_range(self.scroll.tree_offset, total)
    }

    fn request_visible_thumbnails(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let mut tasks = Vec::new();
        let thumbnail_size = self.state.config.view.thumbnail_size;
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if indices.is_empty() {
                return Task::none();
            }
            let window = self.entry_virtual_window_for(indices.len());
            if window.is_empty() {
                return Task::none();
            }
            for display_index in window.start..window.end {
                let Some(actual_index) = indices.get(display_index).copied() else {
                    continue;
                };
                let Some(entry) = self.entries.get(actual_index) else {
                    continue;
                };

                if entry.entry_type != FsEntryType::File {
                    continue;
                }

                if let Some(thumbnail) = self.media.thumbnails.get(&entry.path) {
                    if !self.media.thumbnail_handles.contains_key(&entry.path) {
                        self.media.thumbnail_handles.insert(
                            entry.path.clone(),
                            image::Handle::from_bytes(thumbnail.bytes.clone()),
                        );
                    }
                    continue;
                }

                self.media.thumbnail_handles.remove(&entry.path);

                if self.media.thumbnail_misses.contains(&entry.path)
                    || self.media.thumbnails_in_flight.contains(&entry.path)
                {
                    continue;
                }

                let path = entry.path.clone();
                self.media.thumbnails_in_flight.insert(path.clone());
                tasks.push(Task::perform(
                    async move {
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size)
                            .or_else(|| crate::services::generate_video_thumbnail(path.as_path(), thumbnail_size))
                            .or_else(|| crate::services::generate_pdf_thumbnail(path.as_path(), thumbnail_size));
                        (path, thumbnail)
                    },
                    |(path, thumbnail)| UiMessage::ThumbnailLoaded { path, thumbnail },
                ));
            }
        } else {
            let window = self.entry_virtual_window();
            if window.is_empty() {
                return Task::none();
            }

            for index in window.start..window.end {
                let Some(entry) = self.entries.get(index) else {
                    continue;
                };

                if entry.entry_type != FsEntryType::File {
                    continue;
                }

                if let Some(thumbnail) = self.media.thumbnails.get(&entry.path) {
                    if !self.media.thumbnail_handles.contains_key(&entry.path) {
                        self.media.thumbnail_handles.insert(
                            entry.path.clone(),
                            image::Handle::from_bytes(thumbnail.bytes.clone()),
                        );
                    }
                    continue;
                }

                self.media.thumbnail_handles.remove(&entry.path);

                if self.media.thumbnail_misses.contains(&entry.path)
                    || self.media.thumbnails_in_flight.contains(&entry.path)
                {
                    continue;
                }

                let path = entry.path.clone();
                self.media.thumbnails_in_flight.insert(path.clone());
                tasks.push(Task::perform(
                    async move {
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size)
                            .or_else(|| crate::services::generate_video_thumbnail(path.as_path(), thumbnail_size))
                            .or_else(|| crate::services::generate_pdf_thumbnail(path.as_path(), thumbnail_size));
                        (path, thumbnail)
                    },
                    |(path, thumbnail)| UiMessage::ThumbnailLoaded { path, thumbnail },
                ));
            }
        }

        Task::batch(tasks)
    }

    fn preview_image_size(&self) -> u32 {
        let base_size = self.state.config.view.thumbnail_size;
        base_size.saturating_mul(4).clamp(256, 512)
    }

    fn request_selected_preview(&mut self) -> Task<UiMessage> {
        let Some(entry) = self.selected_entry(&self.entries) else {
            return Task::none();
        };
        let entry_path = entry.path.clone();
        let entry_type = entry.entry_type;

        if entry_type != FsEntryType::File {
            return Task::none();
        }

        let mut tasks = Vec::new();

        // Request text preview for text-previewable files
        if Self::is_text_previewable(&entry_path) {
            let already_cached = self
                .cached_text_preview
                .as_ref()
                .is_some_and(|(p, _)| p == &entry_path);
            if !already_cached {
                let path = entry_path.clone();
                let is_pdf = entry_path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);
                if is_pdf {
                    // Feature 12: PDF text extraction
                    let err_path = path.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                pdf_extract::extract_text(&path)
                                    .ok()
                                    .map(|text| (path, text))
                            })
                            .await
                            .ok()
                            .flatten()
                        },
                        move |result| match result {
                            Some((path, content)) => UiMessage::TextPreviewLoaded { path, content, encoding: Some("PDF".to_string()) },
                            None => UiMessage::TextPreviewLoaded {
                                path: err_path,
                                content: "Impossible d'extraire le texte du PDF".to_string(),
                                encoding: None,
                            },
                        },
                    ));
                } else {
                    tasks.push(Task::perform(
                        async move {
                            let bytes = std::fs::read(&path).ok();
                            (path, bytes)
                        },
                        |(path, bytes): (PathBuf, Option<Vec<u8>>)| match bytes {
                            Some(bytes) => {
                                // Detect encoding
                                let (content, encoding) = if let Ok(s) = std::str::from_utf8(&bytes) {
                                    (s.to_string(), "UTF-8".to_string())
                                } else {
                                    let (cow, encoding, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
                                    (cow.into_owned(), encoding.name().to_string())
                                };
                                // Limit to first ~4000 chars, snapping to a char boundary
                                let truncated = if content.len() > 4000 {
                                    let end = content
                                        .char_indices()
                                        .map(|(i, _)| i)
                                        .take_while(|&i| i <= 4000)
                                        .last()
                                        .unwrap_or(0);
                                    format!("{}…", &content[..end])
                                } else {
                                    content
                                };
                                UiMessage::TextPreviewLoaded {
                                    path,
                                    content: truncated,
                                    encoding: Some(encoding),
                                }
                            }
                            None => {
                                tracing::warn!("Preview: lecture fichier {:?} échouée", path);
                                UiMessage::TextPreviewLoaded {
                                    path,
                                    content: "Impossible de lire le fichier".to_string(),
                                    encoding: None,
                                }
                            }
                        },
                    ));
                }
            }
        }

        // Request image preview
        if let Some(animated) = &self.media.animated {
            if animated.path == entry_path {
                return Task::batch(tasks);
            }
        }

        if let Some(preview) = self.media.previews.get(&entry_path) {
            if !self.media.preview_handles.contains_key(&entry_path) {
                self.media.preview_handles.insert(
                    entry_path.clone(),
                    image::Handle::from_bytes(preview.bytes.clone()),
                );
            }
            return Task::batch(tasks);
        }

        if self.media.preview_handles.contains_key(&entry_path)
            || self.media.preview_misses.contains(&entry_path)
            || self.media.previews_in_flight.contains(&entry_path)
        {
            return Task::batch(tasks);
        }

        let path = entry_path.clone();
        let preview_size = self.preview_image_size();
        self.media.previews_in_flight.insert(path.clone());
        tasks.push(Task::perform(
            async move {
                let preview = generate_preview(path.as_path(), preview_size);
                (path, preview)
            },
            |(path, preview)| UiMessage::PreviewLoaded { path, preview },
        ));
        Task::batch(tasks)
    }

    // ── Test-only state accessors ─────────────────────────────────────────────
    // These are compiled unconditionally so that integration tests in tests/
    // can call them. They are named *_for_test or have obvious test semantics.

    #[doc(hidden)]
    pub fn state_for_test(&self) -> &crate::ui::AppState { &self.state }
    #[doc(hidden)]
    pub fn dual_pane_for_test(&self) -> bool { self.dual_pane.enabled }
    #[doc(hidden)]
    pub fn quick_filter_for_test(&self) -> &str { &self.quick_filter }
    #[doc(hidden)]
    pub fn quick_filter_active_for_test(&self) -> bool { self.quick_filter_active }
    #[doc(hidden)]
    pub fn tab_count_for_test(&self) -> usize { self.tab_manager.count() }
    #[doc(hidden)]
    pub fn active_tab_for_test(&self) -> usize { self.tab_manager.active }
    #[doc(hidden)]
    pub fn context_menu_open_for_test(&self) -> bool { self.menus.context_open }
    #[doc(hidden)]
    pub fn rename_dialog_for_test(&self) -> bool { self.rename_dialog.is_some() }
    #[doc(hidden)]
    pub fn properties_dialog_for_test(&self) -> bool { self.properties_dialog.is_some() }
    #[doc(hidden)]
    pub fn hex_view_for_test(&self) -> bool { self.hex_view.is_some() }
    #[doc(hidden)]
    pub fn diff_view_for_test(&self) -> bool { self.diff_view.is_some() }
    #[doc(hidden)]
    pub fn grep_state_for_test(&self) -> bool { self.grep_state.is_some() }
    #[doc(hidden)]
    pub fn bulk_rename_for_test(&self) -> bool { self.bulk_rename.is_some() }
    #[doc(hidden)]
    pub fn address_input_for_test(&self) -> &str { &self.address_input }
    #[doc(hidden)]
    pub fn address_editing_for_test(&self) -> bool { self.address_editing }
    #[doc(hidden)]
    pub fn recents_is_empty_for_test(&self) -> bool { self.recents.list().is_empty() }
    #[doc(hidden)]
    pub fn network_needs_scan_for_test(&self) -> bool { self.network_discovery.needs_scan() }

}

struct ColumnSpec {
    column: ViewColumn,
    label: &'static str,
    width: Length,
    align: Horizontal,
    sort_key: Option<SortKeyConfig>,
}

impl ColumnSpec {
    fn from_column(column: &ViewColumn) -> Self {
        match column {
            ViewColumn::Name => Self {
                column: ViewColumn::Name,
                label: "Nom",
                width: Length::FillPortion(4),
                align: Horizontal::Left,
                sort_key: Some(SortKeyConfig::Name),
            },
            ViewColumn::Type => Self {
                column: ViewColumn::Type,
                label: "Type",
                width: Length::Fixed(120.0),
                align: Horizontal::Left,
                sort_key: None,
            },
            ViewColumn::Size => Self {
                column: ViewColumn::Size,
                label: "Taille",
                width: Length::Fixed(100.0),
                align: Horizontal::Right,
                sort_key: Some(SortKeyConfig::Size),
            },
            ViewColumn::Modified => Self {
                column: ViewColumn::Modified,
                label: "Modifié",
                width: Length::Fixed(160.0),
                align: Horizontal::Right,
                sort_key: Some(SortKeyConfig::Modified),
            },
        }
    }
}

fn regex_replace_preview(
    paths: &[PathBuf],
    find: &str,
    replace: &str,
) -> Result<Vec<(String, String)>, String> {
    // Simple literal replacement for now (regex support would need the regex crate)
    Ok(paths.iter().map(|p| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let new_name = if find.is_empty() {
            name.clone()
        } else {
            name.replace(find, replace)
        };
        (name, new_name)
    }).collect())
}

fn map_event_to_message(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<UiMessage> {
    match event {
        iced::Event::Window(iced::window::Event::CloseRequested) => {
            Some(UiMessage::ExitRequested)
        }
        iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(UiMessage::ModifiersChanged(ModifiersState {
                shift: modifiers.shift(),
                control: modifiers.control(),
                alt: modifiers.alt(),
            }))
        }
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            Some(UiMessage::RawKeyPressed { key, modifiers })
        }
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(UiMessage::CursorMoved(position))
        }
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(UiMessage::MouseReleased)
        }
        // #12: External drag & drop from Windows Explorer
        iced::Event::Window(iced::window::Event::FileHovered(path)) => {
            Some(UiMessage::ExternalFileHovered(path))
        }
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(UiMessage::ExternalFileDropped(path))
        }
        iced::Event::Window(iced::window::Event::FilesHoveredLeft) => {
            Some(UiMessage::ExternalFileCancelled)
        }
        _ => None,
    }
}

fn detect_archive_type(path: &std::path::Path) -> ArchiveType {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let path_str = path.to_string_lossy().to_lowercase();
    if ext == "7z" {
        ArchiveType::SevenZ
    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
        ArchiveType::TarGz
    } else {
        ArchiveType::Zip
    }
}

pub fn run(start_path: Option<PathBuf>) -> iced::Result {
    if let Some(path) = start_path {
        if let Ok(mut guard) = CLI_START_PATH.lock() {
            *guard = Some(path);
        }
    }

    let _ = iced::application(
        XionApp::new,
        XionApp::update,
        XionApp::view,
    )
    .title(|state: &XionApp| format!("Xion — {}", state.state.route.display_label()))
    .theme(|_: &XionApp| Theme::Light)
    .font(fonts::REGULAR)
    .font(fonts::ITALIC)
    .font(fonts::THIN)
    .font(fonts::THIN_ITALIC)
    .font(fonts::EXTRA_LIGHT)
    .font(fonts::EXTRA_LIGHT_ITALIC)
    .font(fonts::LIGHT)
    .font(fonts::LIGHT_ITALIC)
    .font(fonts::MEDIUM)
    .font(fonts::MEDIUM_ITALIC)
    .font(fonts::SEMI_BOLD)
    .font(fonts::SEMI_BOLD_ITALIC)
    .font(fonts::BOLD)
    .font(fonts::BOLD_ITALIC)
    .font(fonts::EXTRA_BOLD)
    .font(fonts::EXTRA_BOLD_ITALIC)
    .default_font(Font::with_name(FONT_NAME))
    .subscription(XionApp::subscription)
    .exit_on_close_request(false)
    .run();

    // Force a clean exit — background threads (notify watcher, tokio worker
    // threads) can keep the process alive after the event loop ends.
    std::process::exit(0);
}
