use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::AppConfig;
use crate::filesystem::{FsEntry, Page};
use crate::services::Thumbnail;

pub mod app;

pub use app::{XionApp, run};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Tree,
    List,
    Preview,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub pane: PaneKind,
    pub path: PathBuf,
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
                path: start_path,
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
    NavigateTo(PathBuf),
    AddTab,
    SwitchTab(usize),
    CloseTab(usize),
    Back,
    Forward,
    Refresh,
    FocusPane(PaneKind),
    SelectEntry {
        path: PathBuf,
        kind: SelectionKind,
    },
    ActivateEntry(PathBuf),
    KeyboardCommand(KeyboardCommand),
    ToggleContextMenu(bool),
    ContextAction(ContextAction),
    ModifiersChanged(ModifiersState),
    AddressInputChanged(String),
    AddressInputSubmitted,
    Scroll(ScrollViewport),
    PageLoaded {
        path: PathBuf,
        page_index: usize,
        result: Result<Page<FsEntry>, String>,
    },
    ThumbnailLoaded {
        path: PathBuf,
        thumbnail: Option<Thumbnail>,
    },
}

#[derive(Debug, Clone, Copy)]
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
}
