use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::core::{AppConfig, SortKeyConfig};
use crate::filesystem::{FsEntry, OperationReport, Page};
use crate::services::{SearchIndex, Thumbnail};
use iced::{Point, Rectangle, keyboard};

// ── Shared types used in UiMessage ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DiffLine {
    Same(String),
    Added(String),
    Removed(String),
    Header(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileLabel {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl FileLabel {
    pub fn color(&self) -> iced::Color {
        match self {
            FileLabel::Red => iced::Color::from_rgb(0.9, 0.2, 0.2),
            FileLabel::Orange => iced::Color::from_rgb(0.9, 0.5, 0.1),
            FileLabel::Yellow => iced::Color::from_rgb(0.9, 0.8, 0.1),
            FileLabel::Green => iced::Color::from_rgb(0.2, 0.7, 0.3),
            FileLabel::Blue => iced::Color::from_rgb(0.2, 0.5, 0.9),
            FileLabel::Purple => iced::Color::from_rgb(0.6, 0.2, 0.8),
            FileLabel::Gray => iced::Color::from_rgb(0.5, 0.5, 0.5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrepResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone)]
pub struct AclEntry {
    pub principal: String,
    pub allow: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitFileStatus {
    Modified,
    Untracked,
    Staged,
    Conflict,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub inner_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed_size: u64,
}

#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub spans: Vec<(u32, String)>,  // (RGBA color, text)
}

pub mod app;
pub mod theme;

pub use app::{run, XionApp};
pub use theme::{fonts, icons, layout, timing, UiColors, UiSpacing, UiTokens, UiTypography};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Tree,
    List,
    Preview,
}

pub const NETWORK_ROUTE: &str = "network://";

#[derive(Debug, Clone)]
pub enum RouteKind {
    Local(PathBuf),
    Network,
    Recent,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub pane: PaneKind,
    pub kind: RouteKind,
}

pub const RECENT_ROUTE: &str = "recent://";

impl Route {
    pub fn local_path(&self) -> Option<&PathBuf> {
        match &self.kind {
            RouteKind::Local(path) => Some(path),
            RouteKind::Network | RouteKind::Recent => None,
        }
    }

    pub fn key(&self) -> PathBuf {
        match &self.kind {
            RouteKind::Local(path) => path.clone(),
            RouteKind::Network => PathBuf::from(NETWORK_ROUTE),
            RouteKind::Recent => PathBuf::from(RECENT_ROUTE),
        }
    }

    pub fn address_label(&self) -> String {
        match &self.kind {
            RouteKind::Local(path) => path.display().to_string(),
            RouteKind::Network => NETWORK_ROUTE.to_string(),
            RouteKind::Recent => RECENT_ROUTE.to_string(),
        }
    }

    pub fn display_label(&self) -> String {
        match &self.kind {
            RouteKind::Local(path) => path.display().to_string(),
            RouteKind::Network => "Réseau".to_string(),
            RouteKind::Recent => "Récents".to_string(),
        }
    }

    pub fn is_network(&self) -> bool {
        matches!(self.kind, RouteKind::Network)
    }

    pub fn is_recent(&self) -> bool {
        matches!(self.kind, RouteKind::Recent)
    }
}

#[derive(Debug, Clone)]
pub struct NavigationState {
    pub focused_pane: PaneKind,
    pub selection: SelectionState,
}

#[derive(Debug, Clone)]
pub struct SelectionState {
    pub selected: HashSet<PathBuf>,
    pub focused: Option<PathBuf>,
    pub anchor: Option<PathBuf>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
            focused: None,
            anchor: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub route: Route,
    pub navigation: NavigationState,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let start_path = config.start_path.clone();
        Self {
            config,
            route: Route {
                pane: PaneKind::List,
                kind: RouteKind::Local(start_path),
            },
            navigation: NavigationState {
                focused_pane: PaneKind::List,
                selection: SelectionState::new(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiMessage {
    Noop,
    ExitRequested,
    CursorMoved(Point),
    NavigateTo(PathBuf),
    AddTab,
    SwitchTab(usize),
    CloseTab(usize),
    Back,
    Forward,
    Refresh,
    FocusPane(PaneKind),
    EntryPressed(PathBuf),
    ListBackgroundPressed,
    SelectEntry {
        path: PathBuf,
        kind: SelectionKind,
    },
    ActivateEntry(PathBuf),
    KeyboardCommand(KeyboardCommand),
    RawKeyPressed {
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    },
    ToggleContextMenu(bool),
    ToggleHistoryMenu(bool),
    OpenContextMenuForEntry(PathBuf),
    ContextAction(ContextAction),
    ModifiersChanged(ModifiersState),
    AddressInputChanged(String),
    AddressSuggestionSelected(PathBuf),
    AddressInputSubmitted,
    SearchInputChanged(String),
    SearchInputSubmitted,
    Scroll(ScrollViewport),
    TreeScroll(ScrollViewport),
    ChangeSort(SortKeyConfig),
    ToggleViewMode,
    LoadingDelayElapsed(u64),
    PageLoaded {
        path: PathBuf,
        page_index: usize,
        result: Result<Page<FsEntry>, String>,
    },
    ThumbnailLoaded {
        path: PathBuf,
        thumbnail: Option<Thumbnail>,
    },
    PreviewLoaded {
        path: PathBuf,
        preview: Option<Thumbnail>,
    },
    SearchIndexBuilt {
        path: PathBuf,
        result: Result<SearchIndex, String>,
    },
    AnimatedPreviewTick(Instant),
    FileWatchTick,
    ClipboardCut,
    ClipboardCopy,
    ClipboardPaste,
    RenameInputChanged(String),
    RenameSubmit,
    RenameCancel,
    MouseReleased,
    FinalizeDrag,
    TreeResizeStart,
    TreeResizeEnd,
    PreviewResizeStart,
    PreviewResizeEnd,
    DropOnPath(PathBuf),
    FileOperationFinished(OperationReport),
    OperationProgressTick,
    NewFolder,
    NewFolderCreated(Result<PathBuf, String>),
    AddressEditStart,
    AddressEditCancel,
    ToggleDarkMode,
    /// Fold/unfold a named sidebar section (accordion).
    ToggleSidebarSection(String),
    /// Switch between compact (22 px) and normal (32 px) list row height.
    ToggleCompactMode,
    TextPreviewLoaded {
        path: PathBuf,
        content: String,
    },
    PreviewAnimTick,
    ToggleTerminal,
    TerminalInputChanged(String),
    TerminalInputSubmitted,
    TerminalSpawned(Result<crate::terminal::TerminalProcess, String>),
    TerminalPollOutput,
    TerminalAnimTick,
    // Feature 1: Trash
    TrashCompleted(Result<(), String>),
    // Feature 3: Properties dialog
    OpenProperties(PathBuf),
    PropertiesHashComputed { path: PathBuf, hash: String },
    CloseProperties,
    // Feature 4: Color themes
    SetTheme(crate::core::ThemeConfig),
    // Feature 5: Bulk rename
    OpenBulkRename,
    BulkRenameFindChanged(String),
    BulkRenameReplaceChanged(String),
    BulkRenameToggleRegex,
    BulkRenameApply,
    BulkRenameCancel,
    BulkRenameCompleted(Result<usize, String>),
    // Feature 6: Syntax highlighting
    TextHighlightComplete { path: PathBuf, lines: Vec<HighlightedLine> },
    // Feature 7: Git status
    GitStatusLoaded { root: PathBuf, statuses: std::collections::HashMap<PathBuf, GitFileStatus> },
    // Feature 8: Disk usage
    DirSizeLoaded { path: PathBuf, bytes: u64 },
    // Feature 10: Archive browser
    ArchiveListLoaded { archive_path: PathBuf, inner_path: String, entries: Vec<ArchiveEntry> },
    ArchiveFolderOpen { inner_path: String },
    CloseArchiveBrowser,
    ExtractArchiveEntry { archive: PathBuf, inner_path: String, dest_dir: PathBuf },
    ExtractComplete(Result<PathBuf, String>),
    // Feature 11: Dual pane
    ToggleDualPane,
    PaneBNavigate(PathBuf),
    PaneBLoaded { path: PathBuf, entries: Vec<crate::filesystem::FsEntry> },
    PaneBActivate(PathBuf),
    SwitchActivePane,
    // Feature A: Compress to ZIP
    CompressToZip,
    CompressCompleted(Result<std::path::PathBuf, String>),
    // Feature B: Open With
    OpenWith(std::path::PathBuf),
    // Feature C: File Diff
    OpenDiff,
    DiffLoaded { path_a: std::path::PathBuf, path_b: std::path::PathBuf, lines: Vec<DiffLine> },
    CloseDiff,
    // Feature D: Multi-selection properties
    SelectionSizeComputed(u64),
    // Feature E: Quick Filter
    QuickFilterChanged(String),
    QuickFilterClear,
    // Feature F: File Labels
    SetLabel(std::path::PathBuf, Option<FileLabel>),
    // Feature G: Recent Files
    NavigateToRecent,
    ClearRecents,
    // Feature H: Column Resizing
    ColumnResizeStart(String),
    ColumnResizeEnd,
    ColumnResized(String, f32),
    // Feature I: Terminal Tabs
    TerminalAddTab,
    TerminalCloseTab(usize),
    TerminalSwitchTab(usize),
    // Feature J: Shell Switcher
    SetShell(crate::core::ShellConfig),
    // Feature K: Hex Viewer
    OpenHexView(std::path::PathBuf),
    HexViewLoaded { path: std::path::PathBuf, data: Vec<u8> },
    CloseHexView,
    HexViewScroll(usize),
    // Feature L: Encoding detection (handled in TextPreviewLoaded, no extra message)
    // Feature M: TAR.GZ / 7Z (reuses ArchiveListLoaded)
    // Network discovery
    NetworkScanCompleted(Vec<crate::services::NetworkResource>),
    // Feature N: Gitignore
    ToggleGitignore,
    // Feature O: Grep
    OpenGrep,
    GrepQueryChanged(String),
    GrepSearch,
    GrepResultsLoaded(Vec<GrepResult>),
    CloseGrep,
    // Feature P: NTFS Permissions
    OpenPermissions(std::path::PathBuf),
    PermissionsLoaded { path: std::path::PathBuf, entries: Vec<AclEntry> },
    ClosePermissions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Single,
    Toggle,
    Range,
}

#[derive(Debug, Clone)]
pub enum KeyboardCommand {
    MoveUp { extend: bool },
    MoveDown { extend: bool },
    MoveHome { extend: bool },
    MoveEnd { extend: bool },
    Activate,
    Back,
    Forward,
    Refresh,
    SelectAll,
    ClearSelection,
    ToggleContextMenu,
    CyclePaneFocus,
    Rename,
    Delete,
    NewFolder,
    FocusSearch,
    NewTab,
    CloseCurrentTab,
    NextTab,
    PrevTab,
    QuickLook,
    BulkRename,
    ToggleDualPane,
    SwitchActivePane,
    OpenDiff,
    QuickFilterChanged(String),
    QuickFilterClear,
    OpenGrep,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ModifiersState {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ContextAction {
    Open,
    Rename,
    MoveToTrash,
    Delete,
    CopyPath,
    OpenProperties,
    OpenBulkRename,
    CompressToZip,
    OpenWith,
    OpenDiff,
    OpenHexView,
    OpenPermissions,
    SetLabelRed,
    SetLabelOrange,
    SetLabelYellow,
    SetLabelGreen,
    SetLabelBlue,
    SetLabelPurple,
    RemoveLabel,
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollViewport {
    pub offset_y: f32,
    pub viewport_height: f32,
    pub content_height: f32,
    pub bounds: Rectangle,
}
