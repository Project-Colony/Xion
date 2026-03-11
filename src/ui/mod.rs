use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::core::{AppConfig, SortKeyConfig};
use crate::filesystem::{FsEntry, OperationReport, Page};
use crate::services::{SearchIndex, Thumbnail};
use iced::{Point, Rectangle, keyboard};

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
}

#[derive(Debug, Clone)]
pub struct Route {
    pub pane: PaneKind,
    pub kind: RouteKind,
}

impl Route {
    pub fn local_path(&self) -> Option<&PathBuf> {
        match &self.kind {
            RouteKind::Local(path) => Some(path),
            RouteKind::Network => None,
        }
    }

    pub fn key(&self) -> PathBuf {
        match &self.kind {
            RouteKind::Local(path) => path.clone(),
            RouteKind::Network => PathBuf::from(NETWORK_ROUTE),
        }
    }

    pub fn address_label(&self) -> String {
        match &self.kind {
            RouteKind::Local(path) => path.display().to_string(),
            RouteKind::Network => NETWORK_ROUTE.to_string(),
        }
    }

    pub fn display_label(&self) -> String {
        match &self.kind {
            RouteKind::Local(path) => path.display().to_string(),
            RouteKind::Network => "Réseau".to_string(),
        }
    }

    pub fn is_network(&self) -> bool {
        matches!(self.kind, RouteKind::Network)
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
    NewFolder,
    NewFolderCreated(Result<PathBuf, String>),
    AddressEditStart,
    AddressEditCancel,
    ToggleDarkMode,
    TextPreviewLoaded {
        path: PathBuf,
        content: String,
    },
    PreviewAnimTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Single,
    Toggle,
    Range,
}

#[derive(Debug, Clone, Copy)]
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
    Delete,
    CopyPath,
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollViewport {
    pub offset_y: f32,
    pub viewport_height: f32,
    pub content_height: f32,
    pub bounds: Rectangle,
}
