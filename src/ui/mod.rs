use std::path::PathBuf;

use crate::core::AppConfig;

pub mod app;

pub use app::{run, XionApp};

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
    pub selection: Option<PathBuf>,
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
                selection: None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiMessage {
    NavigateTo(PathBuf),
    Back,
    Forward,
    Refresh,
    FocusPane(PaneKind),
    SelectEntry(PathBuf),
}
