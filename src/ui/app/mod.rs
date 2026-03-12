//! Main application module for Xion file explorer.
//!
//! This module contains the [`XionApp`] struct which implements the Iced
//! application trait and handles all UI state, messages, and rendering.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
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
    SortOrderConfig, ViewColumn, ViewMode,
};
use crate::filesystem::{
    FileOperationKind, FsEntry, FsEntryType,
    LocalFileOperations, NativeFileWatcher, NoopFileWatcher, OperationReport, WatchEvent,
};
use crate::services::{
    DirectoryLoader, FavoritesService, HistoryService, NetworkDiscoveryService, PreviewImageService, ThumbnailService, VirtualList,
    VirtualWindow, generate_preview, generate_thumbnail,
};
use crate::core::ThemeConfig;
use crate::ui::{
    AppState, ArchiveEntry, ContextAction, KeyboardCommand,
    ModifiersState, RouteKind, SelectionKind, UiMessage,
};

mod archive;
mod helpers;
mod permissions;
mod shell;
mod types;
mod state;
mod view;

use types::*;
use helpers::{
    TreeNode, build_animated_preview,
    command_from_key_press_with_shortcuts, is_gif_preview, rectangles_intersect,
};

use crate::ui::theme::{FONT_NAME, UiTokens, fonts};
use crate::ui::theme::layout::{
    DRAG_START_THRESHOLD, PREVIEW_MAX_WIDTH, PREVIEW_MIN_WIDTH, TREE_MAX_HEIGHT, TREE_MIN_HEIGHT,
    TREE_ROW_HEIGHT,
};
use crate::ui::theme::timing::{
    DOUBLE_CLICK_THRESHOLD, WATCHER_POLL_INTERVAL,
};

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
    context_menu_open: bool,
    context_menu_position: Option<Point>,
    history_menu_open: bool,
    history_menu_position: Option<Point>,
    cursor_position: Option<Point>,
    last_action: Option<String>,
    address_input: String,
    search: SearchState,
    config_manager: ConfigManager,
    favorites: FavoritesService,
    tabs: Vec<TabState>,
    active_tab: usize,
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
    git_root: Option<PathBuf>,
    // Feature 8: Disk usage
    dir_sizes: std::collections::HashMap<PathBuf, u64>,
    dir_sizes_loading: std::collections::HashSet<PathBuf>,
    // Feature 10: Archive browser
    archive_browser: Option<ArchiveBrowserState>,
    // Feature 11: Dual pane
    dual_pane: bool,
    pane_b: Option<PaneB>,
    active_pane: usize,
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
        let favorites = build_default_favorites();
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
            context_menu_open: false,
            context_menu_position: None,
            history_menu_open: false,
            history_menu_position: None,
            cursor_position: None,
            last_action: None,
            address_input,
            search: SearchState::default(),
            config_manager,
            favorites,
            tabs,
            active_tab: active_tab_init,
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
            git_root: None,
            dir_sizes: std::collections::HashMap::new(),
            dir_sizes_loading: std::collections::HashSet::new(),
            archive_browser: None,
            dual_pane: false,
            pane_b: None,
            active_pane: 0,
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
        };
        // If we restored tabs, set the active route to the active tab's path.
        if !app.state.config.tabs.is_empty() {
            if let Some(path) = app.tabs.get(active_tab_init).map(|t| t.path.clone()) {
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
        let favorites = build_default_favorites();
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
            context_menu_open: false,
            context_menu_position: None,
            history_menu_open: false,
            history_menu_position: None,
            cursor_position: None,
            last_action: None,
            address_input,
            search: SearchState::default(),
            config_manager: crate::core::ConfigManager::new(),
            favorites,
            tabs,
            active_tab: 0,
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
            git_root: None,
            dir_sizes: std::collections::HashMap::new(),
            dir_sizes_loading: std::collections::HashSet::new(),
            archive_browser: None,
            dual_pane: false,
            pane_b: None,
            active_pane: 0,
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
        };
        app.rebuild_tree_cache();
        app
    }

    /// Visible to integration tests; the Iced framework calls this without `pub`.
    #[doc(hidden)]
    pub fn update_for_test(&mut self, message: UiMessage) -> Task<UiMessage> {
        self.update(message)
    }

    fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        if self.is_user_selecting {
            match message {
                UiMessage::Refresh => {
                    self.pending_refresh = true;
                    self.pending_refresh_reload_config = true;
                    return Task::none();
                }
                UiMessage::FileWatchTick => {
                    if let (Some(watched_path), Some(events)) =
                        (self.watched_path.clone(), self.poll_watcher())
                    {
                        let should_refresh = events
                            .iter()
                            .any(|event| Self::is_event_relevant(event, &watched_path));
                        if should_refresh && !self.is_refreshing {
                            self.pending_refresh = true;
                        }
                    }
                    return Task::none();
                }
                _ => {}
            }
        }
        match message {
            UiMessage::Noop => {}
            UiMessage::ExitRequested => {
                // Kill cmd.exe before exiting so no orphan processes remain.
                for tab in &mut self.terminal.tabs {
                    tab.process = None;
                }
                std::process::exit(0);
            }
            UiMessage::CursorMoved(position) => {
                self.cursor_position = Some(position);
                // Feature H: column resize tracking
                if let Some(ref resize) = self.column_resize_state {
                    let delta = position.x - resize.start_x;
                    let new_width = (resize.start_width + delta).max(30.0);
                    let column = resize.column.clone();
                    self.state.config.column_widths.insert(column, new_width);
                }
                if self.pane_resize.tree_resizing {
                    if let Some((start_y, start_height)) = self.pane_resize.tree_resize_anchor {
                        let next_height = start_height + (position.y - start_y);
                        self.pane_resize.tree_height = next_height.clamp(TREE_MIN_HEIGHT, TREE_MAX_HEIGHT);
                        self.scroll.tree_height = self.pane_resize.tree_height.max(1.0);
                    }
                }
                if self.pane_resize.preview_resizing {
                    if let Some((start_x, start_width)) = self.pane_resize.preview_resize_anchor {
                        let delta = position.x - start_x;
                        let next_width =
                            (start_width - delta).clamp(PREVIEW_MIN_WIDTH, PREVIEW_MAX_WIDTH);
                        self.pane_resize.preview_width = next_width;
                    }
                }
                if self.selection_box_start.is_some() && self.drag_candidate.is_none() {
                    self.begin_user_selection();
                    if let Some(content_point) = self.list_content_point(position, true) {
                        self.selection_box_current = Some(content_point);
                        let selected = self.entries_in_selection_box();
                        self.apply_box_selection(selected, self.selection_kind_from_modifiers());
                    }
                }
                if self.mouse_pressed && self.drag_state.is_none() {
                    if self.drag_start_position.is_none() && self.drag_candidate.is_some() {
                        self.drag_start_position = Some(position);
                    }
                    if let (Some(candidate), Some(start_pos)) =
                        (self.drag_candidate.clone(), self.drag_start_position)
                    {
                        let dx = position.x - start_pos.x;
                        let dy = position.y - start_pos.y;
                        if (dx * dx + dy * dy).sqrt() >= DRAG_START_THRESHOLD {
                            self.begin_user_selection();
                            if !self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&candidate)
                            {
                                self.last_click_time = None;
                                self.last_clicked_path = None;
                                self.apply_selection(candidate.clone(), SelectionKind::Single);
                            }
                            let items = if self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&candidate)
                            {
                                self.state
                                    .navigation
                                    .selection
                                    .selected
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                            } else {
                                vec![candidate.clone()]
                            };
                            self.drag_state = Some(DragState { items });
                        }
                    }
                }
            }
            UiMessage::NavigateTo(path) => {
                if self
                    .ignore_next_navigation
                    .as_ref()
                    .is_some_and(|ignore| ignore == &path)
                {
                    self.ignore_next_navigation = None;
                    return Task::none();
                }
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddTab => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                tasks.push(self.add_tab());
            }
            UiMessage::SwitchTab(index) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                tasks.push(self.switch_tab(index));
            }
            UiMessage::CloseTab(index) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                tasks.push(self.close_tab(index));
            }
            UiMessage::Back => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                if let Some(path) = self.history.back() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Forward => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_editing = false;
                if let Some(path) = self.history.forward() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Refresh => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.reload_config());
                tasks.push(self.refresh_entries());
            }
            UiMessage::FocusPane(pane) => {
                self.state.navigation.focused_pane = pane;
            }
            UiMessage::EntryPressed(path) => {
                self.begin_user_selection();
                self.mouse_pressed = true;
                self.drag_candidate = Some(path);
                self.drag_start_position = self.cursor_position;
                self.selection_box_start = None;
                self.selection_box_current = None;
            }
            UiMessage::ListBackgroundPressed => {
                if self.drag_state.is_some() || self.drag_candidate.is_some() {
                    return Task::none();
                }
                self.begin_user_selection();
                self.mouse_pressed = true;
                self.drag_candidate = None;
                self.drag_start_position = None;
                if let Some(position) = self.cursor_position {
                    if let Some(content_point) = self.list_content_point(position, false) {
                        self.selection_box_start = Some(content_point);
                        self.selection_box_current = Some(content_point);
                    }
                }
            }
            UiMessage::SelectEntry { path, kind } => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                let now = Instant::now();
                if kind != SelectionKind::Single {
                    self.last_click_time = None;
                    self.last_clicked_path = None;
                    self.apply_selection(path, kind);
                } else {
                    let is_double_click = self
                        .last_clicked_path
                        .as_ref()
                        .is_some_and(|last_path| last_path == &path)
                        && self.last_click_time.is_some_and(|last_click| {
                            now.duration_since(last_click) <= DOUBLE_CLICK_THRESHOLD
                        });
                    self.apply_selection(path.clone(), kind);
                    if is_double_click {
                        self.last_click_time = None;
                        self.last_clicked_path = None;
                        tasks.push(self.activate_entry(path));
                    } else {
                        self.last_click_time = Some(now);
                        self.last_clicked_path = Some(path);
                    }
                }
            }
            UiMessage::ActivateEntry(path) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.activate_entry(path));
            }
            UiMessage::RawKeyPressed { key, modifiers } => {
                if let Some(command) = command_from_key_press_with_shortcuts(
                    &self.state.config.shortcuts,
                    key,
                    modifiers,
                ) {
                    if self.address_editing && matches!(command, KeyboardCommand::ClearSelection) {
                        self.address_editing = false;
                        self.address_input = self.state.route.address_label();
                    } else if self.quick_filter_active && matches!(command, KeyboardCommand::ClearSelection) {
                        self.quick_filter.clear();
                        self.quick_filter_active = false;
                    } else {
                        tasks.push(self.handle_keyboard_command(command));
                    }
                }
            }
            UiMessage::KeyboardCommand(command) => {
                // Escape while editing the address bar cancels editing
                if self.address_editing && matches!(command, KeyboardCommand::ClearSelection) {
                    self.address_editing = false;
                    self.address_input = self.state.route.address_label();
                } else if self.quick_filter_active && matches!(command, KeyboardCommand::ClearSelection) {
                    self.quick_filter.clear();
                    self.quick_filter_active = false;
                } else {
                    tasks.push(self.handle_keyboard_command(command));
                }
            }
            UiMessage::ToggleContextMenu(force_open) => {
                self.context_menu_open = force_open;
                if force_open {
                    self.context_menu_position = self.cursor_position;
                } else {
                    self.context_menu_position = None;
                }
                self.history_menu_open = false;
                self.history_menu_position = None;
            }
            UiMessage::ToggleHistoryMenu(force_open) => {
                self.history_menu_open = force_open;
                if force_open {
                    self.history_menu_position = self.cursor_position;
                } else {
                    self.history_menu_position = None;
                }
            }
            UiMessage::OpenContextMenuForEntry(path) => {
                let is_selected = self.state.navigation.selection.selected.contains(&path);
                if !is_selected {
                    self.apply_selection(path, SelectionKind::Single);
                }
                self.context_menu_open = true;
                self.context_menu_position = self.cursor_position;
                self.history_menu_open = false;
                self.history_menu_position = None;
            }
            UiMessage::ContextAction(action) => {
                tasks.push(self.apply_context_action(action));
            }
            UiMessage::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            UiMessage::AddressInputChanged(value) => {
                self.address_input = value;
            }
            UiMessage::AddressSuggestionSelected(path) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_input = path.display().to_string();
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddressInputSubmitted => {
                self.address_editing = false;
                self.history_menu_open = false;
                self.history_menu_position = None;
                if let Some(target) = self.address_target_from_input() {
                    if is_network_path(&target) || target.is_dir() {
                        tasks.push(self.navigate_to(target));
                    } else if target.exists() {
                        self.last_action = Some(format!(
                            "Le chemin pointe vers un fichier : {}",
                            target.display()
                        ));
                    } else {
                        self.last_action =
                            Some(format!("Chemin introuvable : {}", target.display()));
                    }
                }
            }
            UiMessage::SearchInputChanged(value) => {
                self.search.input = value;
                self.scroll.offset = 0.0;
                self.clear_selection();
                self.update_search_index_matches();
                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                } else {
                    tasks.push(self.ensure_visible_pages());
                }
            }
            UiMessage::SearchInputSubmitted => {
                self.update_search_index_matches();
                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                }
            }
            UiMessage::Scroll(viewport) => {
                self.scroll.offset = viewport.offset_y;
                self.scroll.height = viewport.viewport_height.max(1.0);
                self.scroll.content_height = viewport.content_height;
                self.list_viewport_bounds = Some(viewport.bounds);
                tasks.push(self.ensure_visible_pages());
            }
            UiMessage::TreeScroll(viewport) => {
                self.scroll.tree_offset = viewport.offset_y;
                self.scroll.tree_height = viewport.viewport_height.max(1.0);
            }
            UiMessage::ChangeSort(sort_key) => {
                if self.state.config.list.sort_key == sort_key {
                    self.state.config.list.sort_order = match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => SortOrderConfig::Desc,
                        SortOrderConfig::Desc => SortOrderConfig::Asc,
                    };
                } else {
                    self.state.config.list.sort_key = sort_key;
                    self.state.config.list.sort_order = SortOrderConfig::Asc;
                }
                self.last_action = Some(format!(
                    "Tri : {:?} ({:?})",
                    self.state.config.list.sort_key, self.state.config.list.sort_order
                ));
                tasks.push(self.refresh_entries());
            }
            UiMessage::ToggleViewMode => {
                self.state.config.view.mode = self.state.config.view.mode.toggle();
                self.scroll.offset = 0.0;
                self.clear_selection();
                tasks.push(self.ensure_visible_pages());
                tasks.push(self.request_visible_thumbnails());
            }
            UiMessage::LoadingDelayElapsed(generation) => {
                if self.is_refreshing
                    && self.is_loading
                    && generation == self.loading_generation
                    && self.entries.total == 0
                {
                    self.show_loading_indicator = true;
                }
            }
            UiMessage::PageLoaded {
                path,
                page_index,
                result,
            } => {
                if path != self.state.route.key() {
                    return Task::batch(tasks);
                }

                self.pending_pages.remove(&page_index);
                match result {
                    Ok(page) => {
                        self.entries.apply_page(page_index, page);
                        self.error = None;
                        if self.is_refreshing {
                            self.stale_entries = None;
                        }
                        tasks.push(self.ensure_visible_pages());
                    }
                    Err(message) => {
                        self.error = Some(message);
                    }
                }

                if self.pending_pages.is_empty() {
                    self.is_loading = false;
                    self.is_refreshing = false;
                    self.show_loading_indicator = false;
                }

                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                }
                // Feature 8: Load dir sizes after page load
                tasks.push(self.request_dir_sizes());
            }
            UiMessage::ThumbnailLoaded { path, thumbnail } => {
                self.media.thumbnails_in_flight.remove(&path);
                match thumbnail {
                    Some(thumbnail) => {
                        // Create handle from bytes before moving into cache
                        let handle = image::Handle::from_bytes(thumbnail.bytes.clone());
                        self.media.thumbnail_handles.insert(path.clone(), handle);
                        self.media.thumbnails.insert(path.clone(), thumbnail);
                        self.media.thumbnail_misses.remove(&path);
                    }
                    None => {
                        self.media.thumbnail_misses.insert(path);
                    }
                }
            }
            UiMessage::PreviewLoaded { path, preview } => {
                self.media.previews_in_flight.remove(&path);
                let is_selected = self
                    .state
                    .navigation
                    .selection
                    .focused
                    .as_ref()
                    .is_some_and(|focused| focused == &path)
                    || self.state.navigation.selection.selected.contains(&path);
                if is_selected {
                    match preview {
                        Some(preview) => {
                            if is_gif_preview(&preview) {
                                self.media.preview_handles.remove(&path);
                                self.media.previews.remove(&path);
                                if let Some(animated) =
                                    build_animated_preview(path.clone(), &preview)
                                {
                                    self.media.animated = Some(animated);
                                    self.media.preview_misses.remove(&path);
                                } else {
                                    self.media.preview_misses.insert(path);
                                }
                            } else {
                                let handle = image::Handle::from_bytes(preview.bytes.clone());
                                self.media.preview_handles.insert(path.clone(), handle);
                                self.media.previews.insert(path.clone(), preview);
                                self.media.preview_misses.remove(&path);
                            }
                        }
                        None => {
                            self.media.preview_misses.insert(path);
                        }
                    }
                }
            }
            UiMessage::SearchIndexBuilt { path, result } => {
                if self.search.index_path.as_ref() == Some(&path) {
                    match result {
                        Ok(index) => {
                            self.search.index = Some(index);
                            self.search.indexing = false;
                            self.update_search_index_matches();
                            self.last_action = Some("Indexation terminée".to_string());
                        }
                        Err(error) => {
                            self.search.index = None;
                            self.search.indexing = false;
                            self.search.matches = None;
                            self.last_action = Some(format!(
                                "Indexation impossible pour {} ({})",
                                path.display(),
                                error
                            ));
                        }
                    }
                }
            }
            UiMessage::AnimatedPreviewTick(now) => {
                self.advance_animated_preview(now);
            }
            UiMessage::FileWatchTick => {
                if let (Some(watched_path), Some(events)) =
                    (self.watched_path.clone(), self.poll_watcher())
                {
                    let should_refresh = events
                        .iter()
                        .any(|event| Self::is_event_relevant(event, &watched_path));
                    if should_refresh && !self.is_refreshing {
                        tasks.push(self.refresh_entries());
                    }
                }
            }
            UiMessage::ClipboardCut => {
                self.capture_clipboard(ClipboardKind::Cut);
            }
            UiMessage::ClipboardCopy => {
                self.capture_clipboard(ClipboardKind::Copy);
            }
            UiMessage::ClipboardPaste => {
                tasks.push(self.paste_clipboard());
            }
            UiMessage::RenameInputChanged(value) => {
                if let Some(dialog) = &mut self.rename_dialog {
                    dialog.input = value;
                }
            }
            UiMessage::RenameSubmit => {
                tasks.push(self.submit_rename());
            }
            UiMessage::RenameCancel => {
                self.rename_dialog = None;
            }
            UiMessage::TreeResizeStart => {
                self.pane_resize.tree_resizing = true;
                self.pane_resize.tree_resize_anchor = self
                    .cursor_position
                    .map(|position| (position.y, self.pane_resize.tree_height));
            }
            UiMessage::TreeResizeEnd => {
                self.pane_resize.tree_resizing = false;
                self.pane_resize.tree_resize_anchor = None;
            }
            UiMessage::PreviewResizeStart => {
                self.pane_resize.preview_resizing = true;
                self.pane_resize.preview_resize_anchor = self
                    .cursor_position
                    .map(|position| (position.x, self.pane_resize.preview_width));
            }
            UiMessage::PreviewResizeEnd => {
                self.pane_resize.preview_resizing = false;
                self.pane_resize.preview_resize_anchor = None;
            }
            UiMessage::MouseReleased => {
                // Feature H: release column resize
                if self.column_resize_state.is_some() {
                    self.column_resize_state = None;
                    self.config_manager.save(&self.state.config);
                }
                if self.pane_resize.tree_resizing {
                    self.pane_resize.tree_resizing = false;
                    self.pane_resize.tree_resize_anchor = None;
                }
                if self.pane_resize.preview_resizing {
                    self.pane_resize.preview_resizing = false;
                    self.pane_resize.preview_resize_anchor = None;
                }
                self.mouse_pressed = false;
                self.drag_candidate = None;
                self.drag_start_position = None;
                self.is_user_selecting = false;
                self.selection_snapshot = None;
                if self.selection_box_start.is_some() {
                    let kind = self.selection_kind_from_modifiers();
                    let selected = self.entries_in_selection_box();
                    self.apply_box_selection(selected, kind);
                    self.selection_box_start = None;
                    self.selection_box_current = None;
                }
                if self.drag_state.is_some() {
                    tasks.push(Task::perform(async {}, |_| UiMessage::FinalizeDrag));
                }
                if self.pending_refresh {
                    let reload_config = self.pending_refresh_reload_config;
                    self.pending_refresh = false;
                    self.pending_refresh_reload_config = false;
                    if reload_config {
                        tasks.push(self.reload_config());
                    }
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::FinalizeDrag => {
                self.drag_state = None;
            }
            UiMessage::FileOperationFinished(report) => {
                self.operation_progress = None;
                self.handle_operation_report(&report);
                tasks.push(self.refresh_entries());
            }
            UiMessage::OperationProgressTick => {
                if let Some(op) = &self.operation_progress {
                    let done = op.counter.load(Ordering::Relaxed).min(op.total);
                    let label = match op.kind {
                        FileOperationKind::Copy => "Copie",
                        FileOperationKind::Move => "Déplacement",
                        FileOperationKind::Delete => "Suppression",
                        FileOperationKind::Rename => "Renommage",
                    };
                    self.last_action = Some(format!("{label} : {done}/{} éléments…", op.total));
                }
            }
            UiMessage::NewFolder => {
                tasks.push(self.create_new_folder());
            }
            UiMessage::NewFolderCreated(result) => {
                match result {
                    Ok(path) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        self.last_action = Some(format!("Dossier créé : {name}"));
                        tasks.push(self.refresh_entries());
                        self.rename_dialog = Some(RenameDialog {
                            path,
                            input: name,
                        });
                    }
                    Err(error) => {
                        self.last_action = Some(format!("Erreur création dossier : {error}"));
                    }
                }
            }
            UiMessage::AddressEditStart => {
                self.address_editing = true;
            }
            UiMessage::AddressEditCancel => {
                self.address_editing = false;
                self.address_input = self.state.route.address_label();
            }
            UiMessage::ToggleDarkMode => {
                // Cycle through themes
                let next_theme = match self.state.config.theme {
                    ThemeConfig::Light => ThemeConfig::Dark,
                    ThemeConfig::Dark => ThemeConfig::Nord,
                    ThemeConfig::Nord => ThemeConfig::Solarized,
                    ThemeConfig::Solarized => ThemeConfig::HighContrast,
                    ThemeConfig::HighContrast => ThemeConfig::Light,
                };
                self.state.config.dark_mode = matches!(next_theme, ThemeConfig::Dark | ThemeConfig::Nord | ThemeConfig::Solarized | ThemeConfig::HighContrast);
                self.state.config.theme = next_theme;
                self.config_manager.save(&self.state.config);
            }
            UiMessage::TextPreviewLoaded { path, content } => {
                // Feature 6: spawn syntax highlighting
                let dark = self.state.config.dark_mode;
                let hl_path = path.clone();
                let hl_content = content.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::services::highlight::highlight_text(&hl_path, &hl_content, dark)
                                .map(|lines| (hl_path, lines))
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |result| match result {
                        Some((path, lines)) => UiMessage::TextHighlightComplete { path, lines },
                        None => UiMessage::Noop,
                    },
                ));
                self.cached_text_preview = Some((path, content));
            }
            UiMessage::PreviewAnimTick => {
                // Ease-out interpolation: fast start, smooth stop
                let speed = 0.15;
                let diff = self.preview_anim_target - self.preview_anim_progress;
                if diff.abs() < 0.005 {
                    self.preview_anim_progress = self.preview_anim_target;
                } else {
                    self.preview_anim_progress += diff * speed;
                }
            }
            UiMessage::ToggleTerminal => {
                if self.terminal_anim_target > 0.5 {
                    // Close: drop process on active tab
                    self.terminal.active().process = None;
                    self.terminal_anim_target = 0.0;
                } else {
                    // Open: spawn a persistent cmd.exe session
                    self.terminal_anim_target = 1.0;
                    let cwd = self
                        .state
                        .route
                        .local_path()
                        .cloned()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    self.terminal.active().cwd = Some(cwd.clone());
                    let shell = self.state.config.terminal_shell.clone();
                    tasks.push(Task::perform(
                        async move {
                            shell::spawn_shell_process(&shell, &cwd)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        UiMessage::TerminalSpawned,
                    ));
                }
            }
            UiMessage::TerminalSpawned(result) => match result {
                Ok(process) => {
                    self.terminal.active().process = Some(process);
                    self.terminal.active().lines.push("Terminal prêt.".to_string());
                }
                Err(e) => {
                    self.terminal.active().lines.push(format!("Erreur démarrage terminal : {e}"));
                    self.terminal_anim_target = 0.0;
                }
            },
            UiMessage::TerminalInputChanged(input) => {
                self.terminal.active().input = input;
            }
            UiMessage::TerminalInputSubmitted => {
                let cmd = self.terminal.active_ref().input.trim().to_string();
                if cmd.is_empty() {
                    return Task::batch(tasks);
                }
                let fallback = self
                    .state
                    .route
                    .local_path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let cwd = self.terminal.effective_cwd(&fallback).to_path_buf();
                self.terminal.push_prompt(&cwd, &cmd);
                self.terminal.apply_cd(&cmd, &cwd);
                self.terminal.active().input.clear();
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    tasks.push(Task::perform(
                        async move { process.write_line(&cmd).await },
                        |_| UiMessage::Noop,
                    ));
                } else {
                    self.terminal.active().lines.push("Erreur : terminal non démarré.".to_string());
                }
            }
            UiMessage::TerminalPollOutput => {
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    let lines = process.poll();
                    if !lines.is_empty() {
                        self.terminal.push_lines(lines);
                    }
                }
            }
            UiMessage::TerminalAddTab => {
                let count = self.terminal.tabs.len() + 1;
                let mut tab = TerminalTab::default();
                tab.title = format!("Terminal {}", count);
                self.terminal.tabs.push(tab);
                self.terminal.active_tab = self.terminal.tabs.len() - 1;
                let cwd = self
                    .state
                    .route
                    .local_path()
                    .cloned()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                self.terminal.active().cwd = Some(cwd.clone());
                let shell = self.state.config.terminal_shell.clone();
                tasks.push(Task::perform(
                    async move {
                        shell::spawn_shell_process(&shell, &cwd)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    UiMessage::TerminalSpawned,
                ));
            }
            UiMessage::TerminalCloseTab(index) => {
                if self.terminal.tabs.len() > 1 {
                    self.terminal.tabs[index].process = None;
                    self.terminal.tabs.remove(index);
                    if self.terminal.active_tab >= self.terminal.tabs.len() {
                        self.terminal.active_tab = self.terminal.tabs.len() - 1;
                    }
                }
            }
            UiMessage::TerminalSwitchTab(index) => {
                if index < self.terminal.tabs.len() {
                    self.terminal.active_tab = index;
                }
            }
            UiMessage::TerminalAnimTick => {
                let speed = 0.15;
                let diff = self.terminal_anim_target - self.terminal_anim_progress;
                if diff.abs() < 0.005 {
                    self.terminal_anim_progress = self.terminal_anim_target;
                } else {
                    self.terminal_anim_progress += diff * speed;
                }
            }
            // Feature 1: Trash
            UiMessage::TrashCompleted(result) => {
                match result {
                    Ok(()) => {
                        self.last_action = Some("Déplacé vers la corbeille".to_string());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur corbeille : {}", e));
                    }
                }
                tasks.push(self.refresh_entries());
            }
            // Feature 3: Properties dialog
            UiMessage::OpenProperties(path) => {
                tasks.push(self.open_properties(path));
            }
            UiMessage::PropertiesHashComputed { path, hash } => {
                if let Some(dialog) = &mut self.properties_dialog {
                    if dialog.path == path {
                        dialog.sha256 = Some(hash);
                        dialog.computing_hash = false;
                    }
                }
            }
            UiMessage::CloseProperties => {
                self.properties_dialog = None;
            }
            // Feature 4: Color themes
            UiMessage::SetTheme(theme) => {
                self.state.config.dark_mode = matches!(theme, ThemeConfig::Dark | ThemeConfig::Nord | ThemeConfig::Solarized | ThemeConfig::HighContrast);
                self.state.config.theme = theme;
                self.config_manager.save(&self.state.config);
            }
            // Feature 5: Bulk rename
            UiMessage::OpenBulkRename => {
                let paths: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if paths.len() > 1 {
                    let previews = paths.iter().map(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        (name.clone(), name)
                    }).collect();
                    self.bulk_rename = Some(BulkRenameState {
                        paths,
                        find: String::new(),
                        replace: String::new(),
                        use_regex: false,
                        previews,
                        error: None,
                    });
                } else {
                    self.last_action = Some("Sélectionnez plusieurs fichiers pour renommer".to_string());
                }
            }
            UiMessage::BulkRenameFindChanged(val) => {
                if let Some(state) = &mut self.bulk_rename {
                    state.find = val;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameReplaceChanged(val) => {
                if let Some(state) = &mut self.bulk_rename {
                    state.replace = val;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameToggleRegex => {
                if let Some(state) = &mut self.bulk_rename {
                    state.use_regex = !state.use_regex;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameApply => {
                tasks.push(self.apply_bulk_rename());
            }
            UiMessage::BulkRenameCancel => {
                self.bulk_rename = None;
            }
            UiMessage::BulkRenameCompleted(result) => {
                self.bulk_rename = None;
                match result {
                    Ok(n) => {
                        self.last_action = Some(format!("{} fichiers renommés", n));
                        tasks.push(self.refresh_entries());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur renommage : {}", e));
                    }
                }
            }
            // Feature 6: Syntax highlighting
            UiMessage::TextHighlightComplete { path, lines } => {
                self.cached_highlighted_preview = Some((path, lines));
            }
            // Feature 7: Git status
            UiMessage::GitStatusLoaded { root, statuses } => {
                self.git_root = Some(root);
                self.git_statuses = statuses;
            }
            // Feature 8: Disk usage
            UiMessage::DirSizeLoaded { path, bytes } => {
                self.dir_sizes_loading.remove(&path);
                self.dir_sizes.insert(path, bytes);
            }
            // Feature 10: Archive browser
            UiMessage::ArchiveListLoaded { archive_path, inner_path, entries } => {
                let archive_type = detect_archive_type(&archive_path);
                self.archive_browser = Some(ArchiveBrowserState {
                    archive_path,
                    inner_path,
                    entries,
                    archive_type,
                });
            }
            UiMessage::ArchiveFolderOpen { inner_path } => {
                if let Some(browser) = &mut self.archive_browser {
                    browser.inner_path = inner_path;
                }
            }
            UiMessage::ExtractArchiveEntry { archive, inner_path, dest_dir } => {
                tasks.push(self.extract_archive_entry(archive, inner_path, dest_dir));
            }
            UiMessage::ExtractComplete(result) => {
                match result {
                    Ok(path) => {
                        self.last_action = Some(format!("Extrait : {}", path.display()));
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", "", &path.display().to_string()])
                                .spawn();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                        }
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur extraction : {}", e));
                    }
                }
            }
            // Feature 11: Dual pane
            UiMessage::ToggleDualPane => {
                tasks.push(self.toggle_dual_pane());
            }
            UiMessage::PaneBNavigate(path) => {
                tasks.push(self.pane_b_navigate(path));
            }
            UiMessage::PaneBLoaded { path, entries } => {
                if let Some(pane) = &mut self.pane_b {
                    if pane.path == path {
                        pane.entries = entries;
                        pane.is_loading = false;
                    }
                }
            }
            UiMessage::PaneBActivate(path) => {
                if path.is_dir() {
                    tasks.push(self.pane_b_navigate(path));
                } else {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &path.display().to_string()])
                            .spawn();
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                    }
                }
            }
            UiMessage::SwitchActivePane => {
                if self.dual_pane {
                    self.active_pane = 1 - self.active_pane;
                }
            }
            // ── Feature A: Compress to ZIP ──────────────────────────────────
            UiMessage::CompressToZip => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if selected.is_empty() {
                    self.last_action = Some("Aucune sélection à compresser".to_string());
                } else {
                    let first = selected[0].clone();
                    let output_dir = self.state.route.local_path().cloned()
                        .unwrap_or_else(|| first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
                    let archive_name = first.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("archive")
                        .to_string();
                    let out_path = output_dir.join(format!("{}.zip", archive_name));
                    self.last_action = Some("Compression en cours...".to_string());
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                                fn zip_add(
                                    zip: &mut zip::ZipWriter<std::fs::File>,
                                    src: &std::path::Path,
                                    name_in_zip: &str,
                                    options: zip::write::FileOptions<()>,
                                ) -> Result<(), String> {
                                    if src.is_file() {
                                        zip.start_file(name_in_zip, options).map_err(|e| e.to_string())?;
                                        let mut f = std::fs::File::open(src).map_err(|e| e.to_string())?;
                                        let mut buf = Vec::new();
                                        use std::io::Read;
                                        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                                        use std::io::Write;
                                        zip.write_all(&buf).map_err(|e| e.to_string())?;
                                    } else if src.is_dir() {
                                        zip.add_directory(format!("{}/", name_in_zip), options).map_err(|e| e.to_string())?;
                                        for child in std::fs::read_dir(src).map_err(|e| e.to_string())? {
                                            let child = child.map_err(|e| e.to_string())?;
                                            let child_name = child.file_name();
                                            let child_zip_name = format!("{}/{}", name_in_zip, child_name.to_string_lossy());
                                            zip_add(zip, &child.path(), &child_zip_name, options)?;
                                        }
                                    }
                                    Ok(())
                                }
                                let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                                let mut zip = zip::ZipWriter::new(file);
                                let options = zip::write::FileOptions::<()>::default()
                                    .compression_method(zip::CompressionMethod::Deflated);
                                for src in &selected {
                                    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
                                    zip_add(&mut zip, src, &name, options)?;
                                }
                                zip.finish().map_err(|e| e.to_string())?;
                                Ok(out_path)
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        UiMessage::CompressCompleted,
                    ));
                }
            }
            UiMessage::CompressCompleted(result) => {
                match result {
                    Ok(path) => {
                        self.last_action = Some(format!("{} créé", path.file_name().and_then(|n| n.to_str()).unwrap_or("archive.zip")));
                        tasks.push(self.refresh_entries());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur compression : {}", e));
                    }
                }
            }
            // ── Feature B: Open With ─────────────────────────────────────────
            UiMessage::OpenWith(path) => {
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,OpenAs_RunDLL", path.to_str().unwrap_or("")])
                                .spawn();
                        })
                        .await
                        .ok();
                    },
                    |_| UiMessage::Noop,
                ));
            }
            // ── Feature C: File Diff ──────────────────────────────────────────
            UiMessage::OpenDiff => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if selected.len() == 2 {
                    let path_a = selected[0].clone();
                    let path_b = selected[1].clone();
                    self.diff_view = Some(DiffViewState {
                        path_a: path_a.clone(),
                        path_b: path_b.clone(),
                        lines: Vec::new(),
                        loading: true,
                    });
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let content_a = std::fs::read_to_string(&path_a).unwrap_or_default();
                                let content_b = std::fs::read_to_string(&path_b).unwrap_or_default();
                                let lines_a: Vec<&str> = content_a.lines().collect();
                                let lines_b: Vec<&str> = content_b.lines().collect();
                                let mut diff_lines = Vec::new();
                                diff_lines.push(crate::ui::DiffLine::Header(format!("--- {}", path_a.display())));
                                diff_lines.push(crate::ui::DiffLine::Header(format!("+++ {}", path_b.display())));
                                let max = lines_a.len().max(lines_b.len());
                                for i in 0..max {
                                    match (lines_a.get(i), lines_b.get(i)) {
                                        (Some(a), Some(b)) if a == b => {
                                            diff_lines.push(crate::ui::DiffLine::Same(a.to_string()));
                                        }
                                        (Some(a), Some(b)) => {
                                            diff_lines.push(crate::ui::DiffLine::Removed(a.to_string()));
                                            diff_lines.push(crate::ui::DiffLine::Added(b.to_string()));
                                        }
                                        (Some(a), None) => {
                                            diff_lines.push(crate::ui::DiffLine::Removed(a.to_string()));
                                        }
                                        (None, Some(b)) => {
                                            diff_lines.push(crate::ui::DiffLine::Added(b.to_string()));
                                        }
                                        (None, None) => {}
                                    }
                                }
                                (path_a, path_b, diff_lines)
                            })
                            .await
                            .ok()
                        },
                        |result| match result {
                            Some((path_a, path_b, lines)) => UiMessage::DiffLoaded { path_a, path_b, lines },
                            None => UiMessage::Noop,
                        },
                    ));
                } else {
                    self.last_action = Some("Sélectionnez exactement 2 fichiers pour comparer".to_string());
                }
            }
            UiMessage::DiffLoaded { path_a, path_b, lines } => {
                self.diff_view = Some(DiffViewState {
                    path_a,
                    path_b,
                    lines,
                    loading: false,
                });
            }
            UiMessage::CloseDiff => {
                self.diff_view = None;
            }
            // ── Feature D: Multi-selection properties ──────────────────────────
            UiMessage::SelectionSizeComputed(size) => {
                if let Some(ref mut dialog) = self.properties_dialog {
                    dialog.size_bytes = Some(size);
                }
            }
            // ── Feature E: Quick Filter ──────────────────────────────────────
            UiMessage::QuickFilterChanged(c) => {
                if !self.quick_filter_active {
                    self.quick_filter_active = true;
                    self.quick_filter = c;
                } else {
                    self.quick_filter.push_str(&c);
                }
            }
            UiMessage::QuickFilterClear => {
                self.quick_filter.clear();
                self.quick_filter_active = false;
            }
            // ── Feature F: File Labels ───────────────────────────────────────
            UiMessage::SetLabel(path, label_opt) => {
                match label_opt {
                    Some(label) => { self.state.config.labels.insert(path, label); }
                    None => { self.state.config.labels.remove(&path); }
                }
                self.config_manager.save(&self.state.config);
            }
            // ── Feature G: Recent Files ──────────────────────────────────────
            UiMessage::NavigateToRecent => {
                self.state.route.kind = crate::ui::RouteKind::Recent;
                self.address_input = crate::ui::RECENT_ROUTE.to_string();
                self.clear_selection();
            }
            UiMessage::ClearRecents => {
                self.recents.entries.clear();
                self.last_action = Some("Historique récents effacé".to_string());
            }
            // ── Feature H: Column Resizing ────────────────────────────────────
            UiMessage::ColumnResizeStart(column) => {
                let current_width = self.state.config.column_widths
                    .get(&column).copied().unwrap_or(150.0);
                let start_x = self.cursor_position.map(|p| p.x).unwrap_or(0.0);
                self.column_resize_state = Some(ColumnResizeState {
                    column,
                    start_x,
                    start_width: current_width,
                });
            }
            UiMessage::ColumnResizeEnd => {
                self.column_resize_state = None;
            }
            UiMessage::ColumnResized(column, width) => {
                self.state.config.column_widths.insert(column, width.max(30.0));
            }
            // ── Feature J: Shell Switcher ─────────────────────────────────────
            UiMessage::SetShell(shell) => {
                self.state.config.terminal_shell = shell;
                // Kill current process so next open uses new shell
                self.terminal.active().process = None;
                self.config_manager.save(&self.state.config);
            }
            // ── Feature K: Hex Viewer ─────────────────────────────────────────
            UiMessage::OpenHexView(path) => {
                let hex_path = path.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            use std::io::Read;
                            let mut f = std::fs::File::open(&hex_path).ok()?;
                            let mut buf = vec![0u8; 4096];
                            let n = f.read(&mut buf).ok()?;
                            buf.truncate(n);
                            Some((hex_path, buf))
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |result| match result {
                        Some((path, data)) => UiMessage::HexViewLoaded { path, data },
                        None => UiMessage::Noop,
                    },
                ));
            }
            UiMessage::HexViewLoaded { path, data } => {
                self.hex_view = Some(HexViewState { path, data, offset: 0 });
            }
            UiMessage::CloseHexView => {
                self.hex_view = None;
            }
            UiMessage::HexViewScroll(offset) => {
                if let Some(ref mut hv) = self.hex_view {
                    hv.offset = offset;
                }
            }
            // ── Network Discovery ─────────────────────────────────────────────
            UiMessage::NetworkScanCompleted(resources) => {
                self.network_discovery.update(resources);
                if self.state.route.is_network() {
                    tasks.push(self.request_page(0));
                }
            }
            // ── Feature N: Gitignore ──────────────────────────────────────────
            UiMessage::ToggleGitignore => {
                self.state.config.respect_gitignore = !self.state.config.respect_gitignore;
                self.last_action = Some(if self.state.config.respect_gitignore {
                    ".gitignore activé".to_string()
                } else {
                    ".gitignore désactivé".to_string()
                });
                tasks.push(self.refresh_entries());
            }
            // ── UX: Sidebar accordion ─────────────────────────────────────────
            UiMessage::ToggleSidebarSection(section) => {
                if !self.sidebar_collapsed.remove(&section) {
                    self.sidebar_collapsed.insert(section);
                }
            }
            // ── UX: Compact mode ──────────────────────────────────────────────
            UiMessage::ToggleCompactMode => {
                self.state.config.compact_mode = !self.state.config.compact_mode;
                self.state.config.view.row_height = if self.state.config.compact_mode { 22.0 } else { 32.0 };
                self.config_manager.save(&self.state.config);
            }
            // ── Feature O: Grep ───────────────────────────────────────────────
            UiMessage::OpenGrep => {
                let root = self.state.route.local_path().cloned()
                    .unwrap_or_else(|| PathBuf::from("."));
                self.grep_state = Some(GrepState {
                    query: String::new(),
                    root,
                    results: Vec::new(),
                    searching: false,
                });
            }
            UiMessage::GrepQueryChanged(q) => {
                if let Some(ref mut gs) = self.grep_state {
                    gs.query = q;
                }
            }
            UiMessage::GrepSearch => {
                if let Some(ref mut gs) = self.grep_state {
                    if gs.query.is_empty() {
                        return Task::batch(tasks);
                    }
                    gs.searching = true;
                    let root = gs.root.clone();
                    let query = gs.query.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut results = Vec::new();
                                let walker = walkdir::WalkDir::new(&root).into_iter();
                                let mut file_count = 0usize;
                                for entry in walker.filter_map(|e| e.ok()) {
                                    if file_count >= 10_000 || results.len() >= 100 {
                                        break;
                                    }
                                    if entry.file_type().is_file() {
                                        file_count += 1;
                                        let path = entry.path().to_path_buf();
                                        if let Ok(content) = std::fs::read(&path) {
                                            // Skip binary files
                                            if content[..content.len().min(512)].contains(&0u8) {
                                                continue;
                                            }
                                            if let Ok(text) = String::from_utf8(content) {
                                                for (ln, line) in text.lines().enumerate() {
                                                    if line.to_lowercase().contains(&query.to_lowercase()) {
                                                        results.push(crate::ui::GrepResult {
                                                            path: path.clone(),
                                                            line_number: ln + 1,
                                                            line: line.to_string(),
                                                        });
                                                        if results.len() >= 100 {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                results
                            })
                            .await
                            .unwrap_or_default()
                        },
                        UiMessage::GrepResultsLoaded,
                    ));
                }
            }
            UiMessage::GrepResultsLoaded(results) => {
                if let Some(ref mut gs) = self.grep_state {
                    gs.results = results;
                    gs.searching = false;
                }
            }
            UiMessage::CloseGrep => {
                self.grep_state = None;
            }
            // ── Feature P: NTFS Permissions ───────────────────────────────────
            UiMessage::OpenPermissions(path) => {
                self.permissions_view = Some(PermissionsViewState {
                    path: path.clone(),
                    entries: Vec::new(),
                    loading: true,
                });
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let output = std::process::Command::new("icacls")
                                .arg(&path)
                                .output()
                                .ok()?;
                            let text = String::from_utf8_lossy(&output.stdout).to_string();
                            let entries = permissions::parse_icacls_output(&text);
                            Some((path, entries))
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |result| match result {
                        Some((path, entries)) => UiMessage::PermissionsLoaded { path, entries },
                        None => UiMessage::Noop,
                    },
                ));
            }
            UiMessage::PermissionsLoaded { path, entries } => {
                self.permissions_view = Some(PermissionsViewState {
                    path,
                    entries,
                    loading: false,
                });
            }
            UiMessage::ClosePermissions => {
                self.permissions_view = None;
            }
            UiMessage::DropOnPath(path) => {
                if let Some(drag_state) = self.drag_state.take() {
                    let destination = path.clone();
                    let items = drag_state.items;
                    let is_copy = self.modifiers.control;
                    let action_label = if is_copy { "Copie" } else { "Déplacement" };
                    self.last_action = Some(format!(
                        "{} (glisser-déposer) vers {}",
                        action_label,
                        destination.display()
                    ));
                    self.ignore_next_navigation = Some(destination.clone());
                    let operation = if is_copy {
                        FileOperationKind::Copy
                    } else {
                        FileOperationKind::Move
                    };
                    let total = items.len();
                    let counter = Arc::new(AtomicUsize::new(0));
                    self.operation_progress = Some(FileOpProgress {
                        counter: counter.clone(),
                        total,
                        kind: operation,
                    });
                    tasks.push(Task::perform(
                        async move {
                            let operations = LocalFileOperations::new();
                            if matches!(operation, FileOperationKind::Copy) {
                                operations.copy_items(&items, &destination, Some(counter))
                            } else {
                                operations.move_items(&items, &destination, Some(counter))
                            }
                        },
                        UiMessage::FileOperationFinished,
                    ));
                }
            }
        }

        tasks.push(self.request_visible_thumbnails());
        tasks.push(self.request_selected_preview());
        Task::batch(tasks)
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

    fn selection_kind_from_modifiers(&self) -> SelectionKind {
        if self.modifiers.shift {
            SelectionKind::Range
        } else if self.modifiers.control {
            SelectionKind::Toggle
        } else {
            SelectionKind::Single
        }
    }

    fn base_display_entries(&self) -> &PagedEntries {
        if self.is_refreshing {
            self.stale_entries.as_ref().unwrap_or(&self.entries)
        } else {
            &self.entries
        }
    }

    fn display_entries(&self) -> &PagedEntries {
        if self.is_user_selecting {
            if let Some(snapshot) = &self.selection_snapshot {
                snapshot
            } else {
                self.base_display_entries()
            }
        } else {
            self.base_display_entries()
        }
    }

    fn begin_user_selection(&mut self) {
        self.is_user_selecting = true;
        if self.selection_snapshot.is_none() {
            self.selection_snapshot = Some(self.base_display_entries().clone());
        }
    }

    fn list_content_point(&self, position: Point, clamp: bool) -> Option<Point> {
        let bounds = self.list_viewport_bounds?;
        let mut local_x = position.x - bounds.x;
        let mut local_y = position.y - bounds.y;

        if clamp {
            local_x = local_x.clamp(0.0, bounds.width.max(0.0));
            local_y = local_y.clamp(0.0, bounds.height.max(0.0));
        } else if local_x < 0.0 || local_y < 0.0 || local_x > bounds.width || local_y > bounds.height
        {
            return None;
        }

        Some(Point::new(local_x, local_y + self.scroll.offset))
    }

    fn selection_box_rect(&self) -> Option<Rectangle> {
        let start = self.selection_box_start?;
        let current = self.selection_box_current?;
        let min_x = start.x.min(current.x);
        let min_y = start.y.min(current.y);
        let max_x = start.x.max(current.x);
        let max_y = start.y.max(current.y);
        Some(Rectangle {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    fn filtered_indices_for(&self, entries: &PagedEntries) -> Option<Vec<usize>> {
        let search_query = self.normalized_search_query();
        let quick_filter = if self.quick_filter_active && !self.quick_filter.is_empty() {
            Some(self.quick_filter.to_lowercase())
        } else {
            None
        };

        if search_query.is_none() && quick_filter.is_none() {
            return None;
        }

        let indices = entries
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let entry = entry.as_ref()?;
                // Apply search query filter
                if let Some(ref q) = search_query {
                    if !Self::matches_search(entry, q) {
                        return None;
                    }
                }
                // Apply quick filter
                if let Some(ref qf) = quick_filter {
                    let name = entry.name.to_lowercase();
                    if !name.contains(qf.as_str()) {
                        return None;
                    }
                }
                Some(index)
            })
            .collect::<Vec<_>>();
        Some(indices)
    }

    fn list_header_visible(
        &self,
        view_mode: ViewMode,
        display_entries: &PagedEntries,
        filtered_indices: Option<&Vec<usize>>,
    ) -> bool {
        if self.error.is_some() || !matches!(view_mode, ViewMode::List) {
            return false;
        }
        if let Some(indices) = filtered_indices {
            !indices.is_empty()
        } else {
            display_entries.total > 0 || self.is_loading
        }
    }

    fn list_content_offset(&self, list_header_visible: bool) -> f32 {
        let tokens = UiTokens::for_mode(self.state.config.dark_mode);
        let mut offset = tokens.spacing.md; // container top padding
        if self.rename_dialog.is_some() {
            offset += self.state.config.view.row_height + tokens.spacing.xl;
        }
        if list_header_visible {
            // Header has same structure as a data row — use row_height as height
            offset += self.state.config.view.row_height;
            offset += tokens.spacing.xl; // column spacing
        }
        offset
    }

    fn entries_in_selection_box(&self) -> Vec<PathBuf> {
        let rect = match self.selection_box_rect() {
            Some(rect) => rect,
            None => return Vec::new(),
        };
        let bounds = match self.list_viewport_bounds {
            Some(bounds) => bounds,
            None => return Vec::new(),
        };
        let display_entries = self.display_entries();
        let filtered_indices = self.filtered_indices_for(display_entries);
        let view_mode = self.state.config.view.mode;
        let total_entries = filtered_indices
            .as_ref()
            .map_or(display_entries.total, |indices| indices.len());
        if total_entries == 0 {
            return Vec::new();
        }
        let list_header_visible =
            self.list_header_visible(view_mode, display_entries, filtered_indices.as_ref());
        let list_content_offset = self.list_content_offset(list_header_visible);
        let tokens = UiTokens::for_mode(self.state.config.dark_mode);
        let list_padding = tokens.spacing.md;
        let content_width = (bounds.width - list_padding * 2.0).max(1.0);
        let entry_index_for = |display_index: usize| -> Option<usize> {
            if let Some(indices) = &filtered_indices {
                indices.get(display_index).copied()
            } else {
                Some(display_index)
            }
        };
        let mut selected = Vec::new();

        match view_mode {
            ViewMode::List => {
                let window = self.list_virtual_window_for(total_entries);
                let row_height = self.state.config.view.row_height;
                for display_index in window.start..window.end {
                    let Some(actual_index) = entry_index_for(display_index) else {
                        continue;
                    };
                    let Some(entry) = display_entries.get(actual_index) else {
                        continue;
                    };
                    let local_index = display_index.saturating_sub(window.start) as f32;
                    let item_y = list_content_offset + window.padding_top + local_index * row_height;
                    let entry_rect = Rectangle {
                        x: list_padding,
                        y: item_y,
                        width: content_width,
                        height: row_height,
                    };
                    if rectangles_intersect(rect, entry_rect) {
                        selected.push(entry.path.clone());
                    }
                }
            }
            ViewMode::Grid => {
                let grid = self.grid_window_for(total_entries);
                let tile_height = self.state.config.view.grid_row_height;
                let columns = grid.columns.max(1);
                let total_spacing = tokens.spacing.md * (columns.saturating_sub(1) as f32);
                let tile_width = ((content_width - total_spacing) / columns as f32).max(1.0);
                for row_index in grid.window.start..grid.window.end {
                    let local_row = row_index.saturating_sub(grid.window.start) as f32;
                    let item_y = list_content_offset + grid.window.padding_top + local_row * tile_height;
                    for column_index in 0..columns {
                        let display_index = row_index * columns + column_index;
                        if display_index >= total_entries {
                            continue;
                        }
                        let Some(actual_index) = entry_index_for(display_index) else {
                            continue;
                        };
                        let Some(entry) = display_entries.get(actual_index) else {
                            continue;
                        };
                        let item_x = list_padding
                            + (tile_width + tokens.spacing.md) * column_index as f32;
                        let entry_rect = Rectangle {
                            x: item_x,
                            y: item_y,
                            width: tile_width,
                            height: tile_height,
                        };
                        if rectangles_intersect(rect, entry_rect) {
                            selected.push(entry.path.clone());
                        }
                    }
                }
            }
        }

        selected
    }

    fn apply_box_selection(&mut self, paths: Vec<PathBuf>, kind: SelectionKind) {
        self.last_click_time = None;
        self.last_clicked_path = None;
        let selection = &mut self.state.navigation.selection;
        match kind {
            SelectionKind::Single => {
                selection.selected = paths.iter().cloned().collect();
                selection.focused = paths.first().cloned();
                selection.anchor = paths.first().cloned();
            }
            SelectionKind::Toggle => {
                for path in &paths {
                    if selection.selected.contains(path) {
                        selection.selected.remove(path);
                    } else {
                        selection.selected.insert(path.clone());
                    }
                }
                if selection.focused.is_none() {
                    selection.focused = paths.first().cloned();
                }
                if selection.anchor.is_none() {
                    selection.anchor = paths.first().cloned();
                }
            }
            SelectionKind::Range => {
                for path in &paths {
                    selection.selected.insert(path.clone());
                }
                if selection.focused.is_none() {
                    selection.focused = paths.first().cloned();
                }
                if selection.anchor.is_none() {
                    selection.anchor = paths.first().cloned();
                }
            }
        }
    }

    fn reload_config(&mut self) -> Task<UiMessage> {
        let load = self.config_manager.load();
        let new_config = load.config;
        if new_config == self.state.config {
            return Task::none();
        }

        let should_reset_loader = new_config.cache.directory_entries
            != self.state.config.cache.directory_entries
            || new_config.cache.directory_ttl_seconds
                != self.state.config.cache.directory_ttl_seconds
            || new_config.paging.page_size != self.state.config.paging.page_size;

        if should_reset_loader {
            self.directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
                new_config.cache.directory_entries,
                Duration::from_secs(new_config.cache.directory_ttl_seconds),
                new_config.paging.page_size,
            )));
            self.entries = PagedEntries::new(0, new_config.paging.page_size);
            self.pending_pages.clear();
        }

        if new_config.cache.thumbnail_entries != self.state.config.cache.thumbnail_entries
            || new_config.cache.thumbnail_ttl_seconds
                != self.state.config.cache.thumbnail_ttl_seconds
        {
            self.media.thumbnails = ThumbnailService::new(
                new_config.cache.thumbnail_entries,
                Duration::from_secs(new_config.cache.thumbnail_ttl_seconds),
            );
            self.media.thumbnail_handles.clear();
            self.media.thumbnail_misses.clear();
            self.media.thumbnails_in_flight.clear();
            let preview_cache_entries = new_config.cache.thumbnail_entries.clamp(1, 8);
            self.media.previews = PreviewImageService::new(
                preview_cache_entries,
                Duration::from_secs(new_config.cache.thumbnail_ttl_seconds),
            );
            self.media.preview_handles.clear();
            self.media.preview_misses.clear();
            self.media.previews_in_flight.clear();
        }

        if self
            .state
            .route
            .local_path()
            .is_some_and(|path| path == &self.state.config.start_path)
        {
            self.state.route.kind = RouteKind::Local(new_config.start_path.clone());
        }

        if !load.warnings.is_empty() {
            self.last_action = Some(format!(
                "Config: {}",
                load.warnings
                    .iter()
                    .map(|warning| warning.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        } else {
            self.last_action = Some("Config rechargée".to_string());
        }

        self.state.config = new_config;
        Task::none()
    }

    fn apply_selection(&mut self, path: PathBuf, kind: SelectionKind) {
        let previous_focus = self.state.navigation.selection.focused.clone();
        let anchor_path = self.state.navigation.selection.anchor.clone();
        let selection_kind = match kind {
            SelectionKind::Range if anchor_path.is_none() => SelectionKind::Single,
            SelectionKind::Range => SelectionKind::Range,
            other => other,
        };
        let (_target_index, _anchor_index, range_paths) = {
            let selection_entries = self.display_entries();
            let target_index = Self::index_for_path_in(selection_entries, &path);
            let anchor_index = anchor_path
                .as_ref()
                .and_then(|anchor_path| Self::index_for_path_in(selection_entries, anchor_path));
            let range_paths = if let (Some(anchor), Some(target)) = (anchor_index, target_index) {
                let (start, end) = if anchor <= target {
                    (anchor, target)
                } else {
                    (target, anchor)
                };
                (start..=end)
                    .filter_map(|index| selection_entries.get(index))
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            (target_index, anchor_index, range_paths)
        };

        let selection = &mut self.state.navigation.selection;
        match selection_kind {
            SelectionKind::Single => {
                selection.selected.clear();
                selection.selected.insert(path.clone());
                selection.focused = Some(path.clone());
                selection.anchor = Some(path);
            }
            SelectionKind::Toggle => {
                if selection.selected.contains(&path) {
                    selection.selected.remove(&path);
                } else {
                    selection.selected.insert(path.clone());
                }
                selection.focused = Some(path.clone());
                selection.anchor.get_or_insert(path);
            }
            SelectionKind::Range => {
                let anchor_path = anchor_path.unwrap_or_else(|| path.clone());
                if !range_paths.is_empty() {
                    selection.selected.clear();
                    for range_path in range_paths {
                        selection.selected.insert(range_path);
                    }
                    selection.focused = Some(path.clone());
                    selection.anchor = Some(anchor_path);
                } else {
                    selection.selected.clear();
                    selection.selected.insert(path.clone());
                    selection.focused = Some(path.clone());
                    selection.anchor = Some(path);
                }
            }
        }

        if selection.selected.is_empty() {
            selection.focused = None;
            selection.anchor = None;
        }
        self.context_menu_open = false;
        self.context_menu_position = None;
        if selection.focused != previous_focus {
            self.reset_preview_state();
        }
        // Animate preview panel open/close
        let has_selection = !self.state.navigation.selection.selected.is_empty();
        self.preview_anim_target = if has_selection { 1.0 } else { 0.0 };
    }

    fn selected_entry<'a>(&'a self, entries: &'a PagedEntries) -> Option<&'a FsEntry> {
        let selection = &self.state.navigation.selection;
        let selected_path = selection
            .focused
            .as_ref()
            .or_else(|| selection.selected.iter().next());
        let selected_path = selected_path?;
        entries
            .items
            .iter()
            .filter_map(|entry| entry.as_ref())
            .find(|entry| &entry.path == selected_path)
    }

    fn handle_keyboard_command(&mut self, command: KeyboardCommand) -> Task<UiMessage> {
        match command {
            KeyboardCommand::MoveUp { extend } => {
                self.move_focus_by(-1, extend);
                Task::none()
            }
            KeyboardCommand::MoveDown { extend } => {
                self.move_focus_by(1, extend);
                Task::none()
            }
            KeyboardCommand::MoveHome { extend } => {
                self.move_focus_to_start(extend);
                Task::none()
            }
            KeyboardCommand::MoveEnd { extend } => {
                self.move_focus_to_end(extend);
                Task::none()
            }
            KeyboardCommand::Activate => self.activate_focused_entry(),
            KeyboardCommand::Back => {
                if self.history.can_back() {
                    if let Some(path) = self.history.back() {
                        self.update_active_tab_path(path);
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Forward => {
                if self.history.can_forward() {
                    if let Some(path) = self.history.forward() {
                        self.update_active_tab_path(path);
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Refresh => self.refresh_entries(),
            KeyboardCommand::SelectAll => {
                self.select_all_entries();
                Task::none()
            }
            KeyboardCommand::ClearSelection => {
                self.clear_selection();
                Task::none()
            }
            KeyboardCommand::ToggleContextMenu => {
                self.context_menu_open = !self.context_menu_open;
                if self.context_menu_open {
                    self.context_menu_position = self.cursor_position;
                } else {
                    self.context_menu_position = None;
                }
                Task::none()
            }
            KeyboardCommand::CyclePaneFocus => {
                self.cycle_focus();
                Task::none()
            }
            KeyboardCommand::Rename => self.open_rename_dialog(),
            KeyboardCommand::Delete => self.delete_selection(),
            KeyboardCommand::NewFolder => self.create_new_folder(),
            KeyboardCommand::FocusSearch => {
                iced::widget::operation::focus(iced::widget::Id::new("search_input"))
            }
            KeyboardCommand::NewTab => Task::done(UiMessage::AddTab),
            KeyboardCommand::CloseCurrentTab => {
                let idx = self.active_tab;
                if idx > 0 {
                    Task::done(UiMessage::CloseTab(idx))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::NextTab => {
                if self.tabs.len() > 1 {
                    let next = (self.active_tab + 1) % self.tabs.len();
                    Task::done(UiMessage::SwitchTab(next))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::PrevTab => {
                if self.tabs.len() > 1 {
                    let prev = if self.active_tab == 0 {
                        self.tabs.len() - 1
                    } else {
                        self.active_tab - 1
                    };
                    Task::done(UiMessage::SwitchTab(prev))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::QuickLook => {
                let focused = self.state.navigation.selection.focused.clone();
                if let Some(path) = focused {
                    let already_previewing = self
                        .cached_text_preview
                        .as_ref()
                        .is_some_and(|(p, _)| p == &path)
                        || self.media.preview_handles.contains_key(&path);
                    if already_previewing && self.preview_anim_target > 0.5 {
                        self.preview_anim_target = 0.0;
                    } else {
                        self.apply_selection(path, SelectionKind::Single);
                    }
                }
                Task::none()
            }
            KeyboardCommand::BulkRename => {
                Task::done(UiMessage::OpenBulkRename)
            }
            KeyboardCommand::ToggleDualPane => {
                self.toggle_dual_pane()
            }
            KeyboardCommand::SwitchActivePane => {
                if self.dual_pane {
                    self.active_pane = 1 - self.active_pane;
                }
                Task::none()
            }
            KeyboardCommand::OpenDiff => {
                Task::done(UiMessage::OpenDiff)
            }
            KeyboardCommand::QuickFilterChanged(c) => {
                Task::done(UiMessage::QuickFilterChanged(c))
            }
            KeyboardCommand::QuickFilterClear => {
                Task::done(UiMessage::QuickFilterClear)
            }
            KeyboardCommand::OpenGrep => {
                Task::done(UiMessage::OpenGrep)
            }
        }
    }

    fn apply_context_action(&mut self, action: ContextAction) -> Task<UiMessage> {
        self.context_menu_open = false;
        self.context_menu_position = None;
        let selection = &self.state.navigation.selection;
        let selected_label = if selection.selected.len() == 1 {
            selection
                .selected
                .iter()
                .next()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".to_string())
        } else if selection.selected.is_empty() {
            "—".to_string()
        } else {
            format!("{} éléments", selection.selected.len())
        };

        self.last_action = Some(match action {
            ContextAction::Open => format!("Ouverture : {}", selected_label),
            ContextAction::Rename => format!("Renommer : {}", selected_label),
            ContextAction::MoveToTrash => format!("Corbeille : {}", selected_label),
            ContextAction::Delete => format!("Supprimer : {}", selected_label),
            ContextAction::CopyPath => format!("Copier le chemin : {}", selected_label),
            ContextAction::OpenProperties => format!("Propriétés : {}", selected_label),
            ContextAction::OpenBulkRename => format!("Renommage multiple : {}", selected_label),
            ContextAction::CompressToZip => format!("Compression : {}", selected_label),
            ContextAction::OpenWith => format!("Ouvrir avec : {}", selected_label),
            ContextAction::OpenDiff => format!("Comparaison : {}", selected_label),
            ContextAction::OpenHexView => format!("Hex : {}", selected_label),
            ContextAction::OpenPermissions => format!("Permissions : {}", selected_label),
            ContextAction::SetLabelRed | ContextAction::SetLabelOrange | ContextAction::SetLabelYellow
            | ContextAction::SetLabelGreen | ContextAction::SetLabelBlue | ContextAction::SetLabelPurple
            | ContextAction::RemoveLabel => format!("Étiquette : {}", selected_label),
        });

        match action {
            ContextAction::Open => self.activate_focused_entry(),
            ContextAction::Rename => self.open_rename_dialog(),
            ContextAction::MoveToTrash => self.trash_selection(),
            ContextAction::Delete => self.delete_selection(),
            ContextAction::CopyPath => {
                self.copy_selection_path();
                Task::none()
            }
            ContextAction::OpenProperties => {
                let selection_count = self.state.navigation.selection.selected.len();
                if selection_count > 1 {
                    // Multi-selection properties
                    let paths: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                    let count = paths.len();
                    // Show a simple dialog and compute total size async
                    self.properties_dialog = Some(PropertiesDialog {
                        path: paths[0].clone(),
                        size_bytes: None,
                        created: None,
                        modified: None,
                        readonly: false,
                        sha256: None,
                        computing_hash: false,
                        selection_count: Some(count),
                    });
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                paths.iter().filter_map(|p| std::fs::metadata(p).ok())
                                    .map(|m| m.len()).sum::<u64>()
                            })
                            .await
                            .unwrap_or(0)
                        },
                        UiMessage::SelectionSizeComputed,
                    );
                }
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenProperties(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenBulkRename => {
                Task::done(UiMessage::OpenBulkRename)
            }
            ContextAction::CompressToZip => {
                Task::done(UiMessage::CompressToZip)
            }
            ContextAction::OpenWith => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenWith(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenDiff => {
                Task::done(UiMessage::OpenDiff)
            }
            ContextAction::OpenHexView => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenHexView(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenPermissions => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenPermissions(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::SetLabelRed => self.set_label_for_focused(Some(crate::ui::FileLabel::Red)),
            ContextAction::SetLabelOrange => self.set_label_for_focused(Some(crate::ui::FileLabel::Orange)),
            ContextAction::SetLabelYellow => self.set_label_for_focused(Some(crate::ui::FileLabel::Yellow)),
            ContextAction::SetLabelGreen => self.set_label_for_focused(Some(crate::ui::FileLabel::Green)),
            ContextAction::SetLabelBlue => self.set_label_for_focused(Some(crate::ui::FileLabel::Blue)),
            ContextAction::SetLabelPurple => self.set_label_for_focused(Some(crate::ui::FileLabel::Purple)),
            ContextAction::RemoveLabel => self.set_label_for_focused(None),
        }
    }

    fn set_label_for_focused(&mut self, label: Option<crate::ui::FileLabel>) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone()
            .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
        if let Some(path) = focused {
            Task::done(UiMessage::SetLabel(path, label))
        } else {
            Task::none()
        }
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        self.state
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect()
    }

    fn capture_clipboard(&mut self, kind: ClipboardKind) {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à mettre en presse-papiers".to_string());
            return;
        }
        let label = match kind {
            ClipboardKind::Copy => "Copie",
            ClipboardKind::Cut => "Déplacement",
        };
        self.clipboard.kind = Some(kind);
        self.clipboard.items = items;
        self.last_action = Some(format!(
            "{} : {} élément(s)",
            label,
            self.clipboard.items.len()
        ));
    }

    fn paste_clipboard(&mut self) -> Task<UiMessage> {
        let Some(kind) = self.clipboard.kind else {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        };
        if self.clipboard.items.is_empty() {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        }

        let items = self.clipboard.items.clone();
        let Some(destination) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Opération indisponible en vue réseau".to_string());
            return Task::none();
        };
        let total = items.len();
        let counter = Arc::new(AtomicUsize::new(0));
        self.operation_progress = Some(FileOpProgress {
            counter: counter.clone(),
            total,
            kind: match kind {
                ClipboardKind::Copy => FileOperationKind::Copy,
                ClipboardKind::Cut => FileOperationKind::Move,
            },
        });
        Task::perform(
            async move {
                let operations = LocalFileOperations::new();
                match kind {
                    ClipboardKind::Copy => operations.copy_items(&items, &destination, Some(counter)),
                    ClipboardKind::Cut => operations.move_items(&items, &destination, Some(counter)),
                }
            },
            UiMessage::FileOperationFinished,
        )
    }

    fn open_rename_dialog(&mut self) -> Task<UiMessage> {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() != 1 {
            self.last_action = Some("Renommage : sélectionnez un seul élément".to_string());
            return Task::none();
        }
        let path = selection
            .selected
            .iter()
            .next()
            .cloned()
            .or_else(|| self.state.route.local_path().cloned());
        let Some(path) = path else {
            self.last_action = Some("Renommage indisponible en vue réseau".to_string());
            return Task::none();
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        }
        self.rename_dialog = Some(RenameDialog { path, input: name });
        Task::none()
    }

    fn submit_rename(&mut self) -> Task<UiMessage> {
        let Some(dialog) = self.rename_dialog.take() else {
            return Task::none();
        };
        let trimmed = dialog.input.trim();
        if trimmed.is_empty() {
            self.last_action = Some("Renommage : nom invalide".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        if Path::new(trimmed).components().count() > 1 {
            self.last_action = Some("Renommage : le nom doit être simple".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        let Some(parent) = dialog.path.parent() else {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        };
        let target = parent.join(trimmed);
        if target == dialog.path {
            self.last_action = Some("Renommage : nom identique".to_string());
            return Task::none();
        }
        let source = dialog.path.clone();
        Task::perform(
            async move { LocalFileOperations::new().rename_item(&source, &target) },
            UiMessage::FileOperationFinished,
        )
    }

    fn delete_selection(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à supprimer".to_string());
            return Task::none();
        }
        self.clear_selection();
        Task::perform(
            async move { LocalFileOperations::new().delete_items(&items) },
            UiMessage::FileOperationFinished,
        )
    }

    fn copy_selection_path(&mut self) {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() == 1 {
            if let Some(path) = selection.selected.iter().next() {
                self.last_action = Some(format!("Chemin copié : {}", path.display()));
                return;
            }
        }
        self.last_action = Some("Sélectionnez un élément pour copier le chemin".to_string());
    }

    fn handle_operation_report(&mut self, report: &OperationReport) {
        let success = report.succeeded.len();
        let failure = report.failed.len();
        let base_message = match report.action {
            FileOperationKind::Copy => format!("Copie : {} ok", success),
            FileOperationKind::Move => format!("Déplacement : {} ok", success),
            FileOperationKind::Rename => {
                if success == 1 {
                    report
                        .succeeded
                        .first()
                        .map(|path| format!("Renommé : {}", path.display()))
                        .unwrap_or_else(|| "Renommage terminé".to_string())
                } else {
                    format!("Renommage : {} ok", success)
                }
            }
            FileOperationKind::Delete => format!("Suppression : {} ok", success),
        };
        let full_message = if failure > 0 {
            format!("{base_message} / {failure} erreur(s)")
        } else {
            base_message
        };
        self.last_action = Some(full_message);
        if report.action == FileOperationKind::Move && failure == 0 {
            self.clipboard = ClipboardState::default();
        }
        if matches!(
            report.action,
            FileOperationKind::Rename | FileOperationKind::Delete
        ) {
            self.rename_dialog = None;
        }
    }

    fn activate_focused_entry(&mut self) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone();
        if let Some(path) = focused {
            return self.activate_entry(path);
        }
        Task::none()
    }

    fn activate_entry(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(entry) = self
            .entries
            .items
            .iter()
            .flatten()
            .find(|entry| entry.path == path)
        {
            if entry.entry_type == FsEntryType::Directory {
                if self.state.route.is_network() {
                    self.last_action =
                        Some(format!("Connexion au partage : {}", entry.path.display()));
                    return Task::none();
                }
                return self.navigate_to(entry.path.clone());
            }
            // Feature 10 / M: Open archives in archive browser
            let ext = entry.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let path_str = entry.path.to_string_lossy().to_lowercase();
            if ext == "zip" || ext == "7z" || ext == "tgz" || path_str.ends_with(".tar.gz") {
                // Record in recents
                self.recents.record(entry.path.clone());
                return self.open_archive(entry.path.clone());
            }
            // Record opened file in recents
            self.recents.record(entry.path.clone());
            // Open file with system default application
            let file_path = entry.path.clone();
            self.last_action = Some(format!(
                "Ouverture : {}",
                file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &file_path.display().to_string()])
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(&file_path).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&file_path)
                    .spawn();
            }
        }
        Task::none()
    }

    fn create_new_folder(&mut self) -> Task<UiMessage> {
        let Some(current_dir) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Création impossible en vue réseau".to_string());
            return Task::none();
        };
        Task::perform(
            async move {
                let base_name = "Nouveau dossier";
                let mut target = current_dir.join(base_name);
                let mut counter = 1u32;
                while target.exists() {
                    counter += 1;
                    target = current_dir.join(format!("{base_name} ({counter})"));
                }
                match std::fs::create_dir(&target) {
                    Ok(()) => Ok(target),
                    Err(error) => Err(error.to_string()),
                }
            },
            UiMessage::NewFolderCreated,
        )
    }

    fn index_for_path_in(entries: &PagedEntries, path: &Path) -> Option<usize> {
        entries.items.iter().position(|entry| {
            entry
                .as_ref()
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    fn index_for_path(&self, path: &Path) -> Option<usize> {
        Self::index_for_path_in(self.display_entries(), path)
    }

    fn normalized_search_query(&self) -> Option<String> {
        let trimmed = self.search.input.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }

    fn matches_search(entry: &FsEntry, query: &str) -> bool {
        entry.name.to_lowercase().contains(query)
    }

    fn filtered_indices(&self, query: &str) -> Vec<usize> {
        self.entries
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let entry = entry.as_ref()?;
                if Self::matches_search(entry, query) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn filtered_position_for_path(&self, indices: &[usize], path: &Path) -> Option<usize> {
        indices.iter().position(|index| {
            self.entries
                .get(*index)
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    fn first_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().position(|entry| entry.is_some())
    }

    fn last_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().rposition(|entry| entry.is_some())
    }

    fn move_focus_by(&mut self, offset: isize, extend: bool) {
        let selection = &self.state.navigation.selection;
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            let start_index = selection
                .focused
                .as_ref()
                .and_then(|path| self.filtered_position_for_path(&indices, path))
                .or(if indices.is_empty() { None } else { Some(0) });

            let Some(start_index) = start_index else {
                return;
            };

            let target_index = if offset.is_negative() {
                start_index.saturating_sub(offset.unsigned_abs())
            } else {
                (start_index + offset as usize).min(indices.len().saturating_sub(1))
            };

            if let Some(actual_index) = indices.get(target_index).copied() {
                self.move_focus_to_actual_index(actual_index, extend);
            }
        } else {
            let start_index = selection
                .focused
                .as_ref()
                .and_then(|path| self.index_for_path(path))
                .or_else(|| self.first_entry_index());

            let Some(start_index) = start_index else {
                return;
            };

            let target_index = if offset.is_negative() {
                start_index.saturating_sub(offset.unsigned_abs())
            } else {
                (start_index + offset as usize).min(self.entries.total.saturating_sub(1))
            };

            self.move_focus_to_actual_index(target_index, extend);
        }
    }

    fn move_focus_to_start(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.first().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.first_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    fn move_focus_to_end(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.last().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.last_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    fn move_focus_to_actual_index(&mut self, index: usize, extend: bool) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let kind = if extend {
            SelectionKind::Range
        } else {
            SelectionKind::Single
        };
        self.apply_selection(entry.path.clone(), kind);
    }

    fn select_all_entries(&mut self) {
        let selected_paths: Vec<PathBuf> = if let Some(query) = self.normalized_search_query() {
            self.filtered_indices(&query)
                .into_iter()
                .filter_map(|index| self.entries.get(index).map(|entry| entry.path.clone()))
                .collect()
        } else {
            self.entries
                .items
                .iter()
                .flatten()
                .map(|entry| entry.path.clone())
                .collect()
        };

        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        for path in selected_paths {
            selection.selected.insert(path);
        }
        selection.focused = selection.selected.iter().next().cloned();
        selection.anchor = selection.focused.clone();
        self.reset_preview_state();
    }

    fn clear_selection(&mut self) {
        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        selection.focused = None;
        selection.anchor = None;
        self.context_menu_open = false;
        self.context_menu_position = None;
        self.reset_preview_state();
        self.preview_anim_target = 0.0;
    }

    fn reset_preview_state(&mut self) {
        self.media.preview_handles.clear();
        self.media.preview_misses.clear();
        self.media.previews_in_flight.clear();
        self.media.previews.clear();
        self.media.animated = None;
        self.cached_text_preview = None;
    }

    fn advance_animated_preview(&mut self, now: Instant) {
        let Some(animated) = &mut self.media.animated else {
            return;
        };

        if animated.frames.is_empty() || now < animated.next_frame_at {
            return;
        }

        animated.current = (animated.current + 1) % animated.frames.len();
        let frame = &animated.frames[animated.current];
        animated.handle = frame.handle.clone(); // cheap Arc clone, no pixel copy
        animated.next_frame_at = now + frame.delay;
    }

    fn is_event_relevant(event: &WatchEvent, watched_path: &Path) -> bool {
        event.path == watched_path
            || event
                .path
                .parent()
                .is_some_and(|parent| parent == watched_path)
    }

    fn cycle_focus(&mut self) {
        self.state.navigation.focused_pane = match self.state.navigation.focused_pane {
            crate::ui::PaneKind::Tree => crate::ui::PaneKind::List,
            crate::ui::PaneKind::List => crate::ui::PaneKind::Preview,
            crate::ui::PaneKind::Preview => crate::ui::PaneKind::Tree,
        };
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
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size);
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
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size);
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
                        |result| match result {
                            Some((path, content)) => UiMessage::TextPreviewLoaded { path, content },
                            None => UiMessage::TextPreviewLoaded {
                                path: PathBuf::new(),
                                content: "Impossible d'extraire le texte du PDF".to_string(),
                            },
                        },
                    ));
                } else {
                    tasks.push(Task::perform(
                        async move {
                            let content = std::fs::read_to_string(&path).ok();
                            (path, content)
                        },
                        |(path, content): (PathBuf, Option<String>)| match content {
                            Some(content) => {
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
                                }
                            }
                            None => UiMessage::Noop,
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

    // ── Feature 1: Trash ──────────────────────────────────────────────────────

    fn trash_selection(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à déplacer".to_string());
            return Task::none();
        }
        self.clear_selection();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    for path in &items {
                        trash::delete(path).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::TrashCompleted,
        )
    }

    // ── Feature 3: Properties dialog ─────────────────────────────────────────

    fn open_properties(&mut self, path: PathBuf) -> Task<UiMessage> {
        use std::fs;
        use chrono::{DateTime, Local};

        let meta = fs::metadata(&path).ok();
        let size_bytes = meta.as_ref().map(|m| m.len());
        let readonly = meta.as_ref().map(|m| m.permissions().readonly()).unwrap_or(false);
        let created = meta.as_ref().and_then(|m| m.created().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });

        self.properties_dialog = Some(PropertiesDialog {
            path: path.clone(),
            size_bytes,
            created,
            modified,
            readonly,
            sha256: None,
            computing_hash: true,
            selection_count: None,
        });

        // Spawn async SHA-256 computation (only for files)
        if path.is_file() {
            let hash_path = path.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        use sha2::{Digest, Sha256};
                        use std::io::Read;
                        let mut file = std::fs::File::open(&hash_path).ok()?;
                        let mut hasher = Sha256::new();
                        let mut buf = vec![0u8; 65536];
                        loop {
                            let n = file.read(&mut buf).ok()?;
                            if n == 0 { break; }
                            hasher.update(&buf[..n]);
                        }
                        let result = hasher.finalize();
                        Some((hash_path, format!("{:x}", result)))
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |result| match result {
                    Some((path, hash)) => UiMessage::PropertiesHashComputed { path, hash },
                    None => UiMessage::Noop,
                },
            )
        } else {
            // For directories, mark hash as not applicable
            if let Some(dialog) = &mut self.properties_dialog {
                dialog.computing_hash = false;
                dialog.sha256 = Some("N/A (dossier)".to_string());
            }
            Task::none()
        }
    }

    // ── Feature 5: Bulk rename ────────────────────────────────────────────────

    fn recompute_bulk_rename_previews(&mut self) {
        let Some(state) = &mut self.bulk_rename else { return; };
        state.error = None;
        if state.use_regex {
            match regex_replace_preview(&state.paths, &state.find, &state.replace) {
                Ok(previews) => state.previews = previews,
                Err(e) => {
                    state.error = Some(e);
                    state.previews = state.paths.iter().map(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        (name.clone(), name)
                    }).collect();
                }
            }
        } else {
            state.previews = state.paths.iter().map(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let new_name = if state.find.is_empty() {
                    name.clone()
                } else {
                    name.replace(&state.find, &state.replace)
                };
                (name, new_name)
            }).collect();
        }
    }

    fn apply_bulk_rename(&mut self) -> Task<UiMessage> {
        let Some(state) = &self.bulk_rename else {
            return Task::none();
        };
        if state.error.is_some() {
            return Task::none();
        }
        let renames: Vec<(PathBuf, String)> = state.paths.iter().zip(state.previews.iter())
            .filter(|(_, (old, new))| old != new)
            .map(|(path, (_, new_name))| (path.clone(), new_name.clone()))
            .collect();
        if renames.is_empty() {
            return Task::none();
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut count = 0usize;
                    for (path, new_name) in &renames {
                        if let Some(parent) = path.parent() {
                            let target = parent.join(new_name);
                            if std::fs::rename(path, &target).is_ok() {
                                count += 1;
                            }
                        }
                    }
                    Ok(count)
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::BulkRenameCompleted,
        )
    }

    // ── Feature 7: Git status (loaded from state.rs) ──────────────────────────

    // ── Feature 10: Archive browser ───────────────────────────────────────────

    pub(super) fn open_archive(&mut self, path: PathBuf) -> Task<UiMessage> {
        let archive_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let ext = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let path_str = archive_path.to_string_lossy().to_lowercase();
                    let entries = if ext == "7z" {
                        archive::list_7z(&archive_path)
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        archive::list_tar_gz(&archive_path)
                    } else {
                        // ZIP (default)
                        let file = std::fs::File::open(&archive_path).ok()?;
                        let mut zip = zip::ZipArchive::new(file).ok()?;
                        let mut entries = Vec::new();
                        for i in 0..zip.len() {
                            if let Ok(entry) = zip.by_index(i) {
                                let inner_path = entry.name().to_string();
                                let name = inner_path.split('/').filter(|s| !s.is_empty()).last().unwrap_or(&inner_path).to_string();
                                let is_dir = entry.is_dir();
                                let size = entry.size();
                                let compressed_size = entry.compressed_size();
                                entries.push(ArchiveEntry { name, inner_path, is_dir, size, compressed_size });
                            }
                        }
                        return Some((archive_path, entries));
                    };
                    let entries = entries?;
                    Some((archive_path, entries))
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((archive_path, entries)) => UiMessage::ArchiveListLoaded {
                    archive_path,
                    inner_path: String::new(),
                    entries,
                },
                None => UiMessage::Noop,
            },
        )
    }

    fn extract_archive_entry(&self, archive: PathBuf, inner_path: String, dest_dir: PathBuf) -> Task<UiMessage> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                    let file_name = inner_path.split('/').filter(|s| !s.is_empty()).last().unwrap_or("file");
                    let out_path = dest_dir.join(file_name);
                    let ext = archive.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let path_str = archive.to_string_lossy().to_lowercase();

                    if ext == "7z" {
                        // 7Z extraction
                        let mut found = false;
                        let mut arch = sevenz_rust::SevenZReader::open(&archive, sevenz_rust::Password::empty())
                            .map_err(|e| e.to_string())?;
                        arch.for_each_entries(|entry, reader| {
                            if entry.name() == inner_path {
                                let mut out_file = std::fs::File::create(&out_path)
                                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                                std::io::copy(reader, &mut out_file)?;
                                found = true;
                            }
                            Ok(true)
                        }).map_err(|e| e.to_string())?;
                        if found { Ok(out_path) } else { Err("Fichier non trouvé dans l'archive".to_string()) }
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        // TAR.GZ extraction
                        use std::io::BufReader;
                        let file = std::fs::File::open(&archive).map_err(|e| e.to_string())?;
                        let gz = flate2::read::GzDecoder::new(BufReader::new(file));
                        let mut tar = tar::Archive::new(gz);
                        for entry in tar.entries().map_err(|e| e.to_string())? {
                            let mut e = entry.map_err(|e| e.to_string())?;
                            let path_in_tar = e.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
                            if path_in_tar == inner_path {
                                e.unpack(&out_path).map_err(|e| e.to_string())?;
                                return Ok(out_path);
                            }
                        }
                        Err("Fichier non trouvé dans l'archive".to_string())
                    } else {
                        // ZIP (default)
                        let file = std::fs::File::open(&archive).map_err(|e| e.to_string())?;
                        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                        let mut entry = zip.by_name(&inner_path).map_err(|e| e.to_string())?;
                        let mut out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
                        Ok(out_path)
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::ExtractComplete,
        )
    }

    // ── Feature 11: Dual pane ─────────────────────────────────────────────────

    fn toggle_dual_pane(&mut self) -> Task<UiMessage> {
        self.dual_pane = !self.dual_pane;
        if self.dual_pane {
            let path = self.state.route.local_path().cloned()
                .unwrap_or_else(|| self.state.config.start_path.clone());
            self.pane_b = Some(PaneB {
                path: path.clone(),
                entries: Vec::new(),
                selection: std::collections::HashSet::new(),
                is_loading: true,
            });
            self.pane_b_navigate(path)
        } else {
            self.pane_b = None;
            self.active_pane = 0;
            Task::none()
        }
    }

    fn pane_b_navigate(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(pane) = &mut self.pane_b {
            pane.path = path.clone();
            pane.is_loading = true;
            pane.entries.clear();
        }
        let list_config = self.state.config.list.clone();
        let filesystem_config = self.state.config.filesystem.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use crate::filesystem::{LocalFileSystem, ListOptions, EntryFilter, FileSystem, SortKey, SortOrder};
                    let fs = LocalFileSystem::from_config(filesystem_config);
                    let options = ListOptions {
                        show_hidden: list_config.show_hidden,
                        sort_by: SortKey::Name,
                        sort_order: SortOrder::Asc,
                        directories_first: true,
                        filter: EntryFilter::All,
                        name_query: None,
                    };
                    let entries = fs.list_dir(&path, options).unwrap_or_default();
                    (path, entries)
                })
                .await
                .ok()
                .unwrap_or_else(|| (PathBuf::new(), Vec::new()))
            },
            |(path, entries)| UiMessage::PaneBLoaded { path, entries },
        )
    }

    /// Returns true if the file extension suggests a text/code file suitable for preview.
    fn is_text_previewable(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "txt" | "md" | "log" | "nfo" | "readme"
                | "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp"
                | "cs" | "java" | "go" | "rb" | "php" | "swift" | "kt" | "lua" | "zig"
                | "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd"
                | "html" | "htm" | "css" | "scss" | "sass" | "less"
                | "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "cfg" | "conf"
                | "env" | "properties" | "csv" | "sql"
                | "gitignore" | "gitmodules" | "gitattributes"
                | "dockerfile" | "makefile" | "cmake"
                | "r" | "dart" | "scala" | "vue" | "svelte"
                | "pdf"
        )
    }

    // ── Test-only state accessors ─────────────────────────────────────────────
    // These are compiled unconditionally so that integration tests in tests/
    // can call them. They are named *_for_test or have obvious test semantics.

    #[doc(hidden)]
    pub fn state_for_test(&self) -> &crate::ui::AppState { &self.state }
    #[doc(hidden)]
    pub fn dual_pane_for_test(&self) -> bool { self.dual_pane }
    #[doc(hidden)]
    pub fn quick_filter_for_test(&self) -> &str { &self.quick_filter }
    #[doc(hidden)]
    pub fn quick_filter_active_for_test(&self) -> bool { self.quick_filter_active }
    #[doc(hidden)]
    pub fn tab_count_for_test(&self) -> usize { self.tabs.len() }
    #[doc(hidden)]
    pub fn active_tab_for_test(&self) -> usize { self.active_tab }
    #[doc(hidden)]
    pub fn context_menu_open_for_test(&self) -> bool { self.context_menu_open }
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
