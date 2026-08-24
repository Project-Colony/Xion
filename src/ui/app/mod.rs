//! Main application module for Xion file explorer.
//!
//! This module contains the [`XionApp`] struct which implements the Iced
//! application trait and handles all UI state, messages, and rendering.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Holds the CLI-provided start path, consumed once during app initialization.
static CLI_START_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

use iced::alignment::Horizontal;
// Tracing is available for future use
use iced::{Font, Length, Point, Rectangle, Theme, keyboard, mouse};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::core::{ConfigManager, SortKeyConfig, ViewColumn};
use crate::services::{DirectoryLoader, FavoritesService, HistoryService, NetworkDiscoveryService};
use crate::ui::{AppState, ModifiersState, UiMessage};

mod archive;
mod construction;
mod helpers;
mod media;
mod navigation;
mod operations;
mod paging;
mod permissions;
mod shell;
mod state;
mod test_access;
mod types;
mod update;
mod view;
mod windowing;

use helpers::TreeNode;
use types::*;

use crate::ui::theme::{FONT_NAME, fonts};

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
    /// Messages held back while a mouse gesture is in progress.
    deferred_messages: Vec<UiMessage>,
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
    /// Pending irreversible action awaiting confirmation.
    confirm_dialog: Option<ConfirmDialog>,
    /// Last known window size, used to size the terminal pty.
    window_size: (f32, f32),
    // Feature Q: Undo
    undo_stack: UndoStack,
    /// Context for the in-flight file operation, used to build undo actions on completion.
    /// Undo context per in-flight operation, keyed by operation id.
    pending_undo_context: std::collections::HashMap<u64, PendingUndoContext>,
    /// Monotonic source of operation ids.
    next_operation_id: u64,
    // Feature R: Breadcrumb dropdown
    breadcrumb_dropdown: Option<PathBuf>,
    breadcrumb_dropdown_items: Vec<PathBuf>,
    breadcrumb_dropdown_has_more: bool,
    // #10: Search index LRU cache (max 8 entries)
    search_index_cache:
        std::collections::VecDeque<(PathBuf, std::sync::Arc<crate::services::SearchIndex>)>,
    // #18: Tab drag reorder
    tab_drag_source: Option<usize>,
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
    Ok(paths
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let new_name = if find.is_empty() {
                name.clone()
            } else {
                name.replace(find, replace)
            };
            (name, new_name)
        })
        .collect())
}

fn map_event_to_message(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<UiMessage> {
    match event {
        iced::Event::Window(iced::window::Event::CloseRequested) => Some(UiMessage::ExitRequested),
        iced::Event::Window(iced::window::Event::Resized(size)) => {
            Some(UiMessage::WindowResized(size.width, size.height))
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
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
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

    let result = iced::application(XionApp::new, XionApp::update, XionApp::view)
        .title(|state: &XionApp| format!("Xion — {}", state.state.route.display_label()))
        // A plain, resizable, centred window — the shape every desktop file
        // manager uses. Not maximised: the user's window manager decides that,
        // and a file manager that seizes the whole screen on launch is a
        // nuisance.
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 720.0),
            min_size: Some(iced::Size::new(720.0, 460.0)),
            position: iced::window::Position::Centered,
            resizable: true,
            decorations: true,
            maximized: false,
            fullscreen: false,
            ..iced::window::Settings::default()
        })
        // Follows the configured theme. This was hard-coded to `Light`, so
        // picking Dark left every default-styled widget bright.
        .theme(|state: &XionApp| {
            if state.state.config.theme.is_dark() {
                Theme::Dark
            } else {
                Theme::Light
            }
        })
        .font(fonts::REGULAR)
        .font(fonts::LIGHT_ITALIC)
        .font(fonts::SEMI_BOLD)
        .default_font(Font::with_name(FONT_NAME))
        .subscription(XionApp::subscription)
        .exit_on_close_request(false)
        .run();

    // Force a clean exit: background threads (the notify watcher, the tokio
    // workers, the pty readers) can keep the process alive after the event loop
    // ends. The exit code carries the outcome — the result used to be dropped
    // with `let _ =`, so a failed start still reported success to whatever
    // launched Xion.
    if let Err(error) = &result {
        eprintln!("Xion s'est arrêté sur une erreur : {error}");
        std::process::exit(1);
    }
    std::process::exit(0);
}
