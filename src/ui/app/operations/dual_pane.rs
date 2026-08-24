//! The second pane.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    pub(in crate::ui::app) fn toggle_dual_pane(&mut self) -> Task<UiMessage> {
        self.dual_pane.enabled = !self.dual_pane.enabled;
        if self.dual_pane.enabled {
            let path = self
                .state
                .route
                .local_path()
                .cloned()
                .unwrap_or_else(|| self.state.config.start_path.clone());
            self.dual_pane.pane_b = Some(PaneB {
                path: path.clone(),
                entries: Vec::new(),
                is_loading: true,
            });
            self.pane_b_navigate(path)
        } else {
            self.dual_pane.pane_b = None;
            self.dual_pane.active = 0;
            Task::none()
        }
    }

    pub(in crate::ui::app) fn pane_b_navigate(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(pane) = &mut self.dual_pane.pane_b {
            pane.path = path.clone();
            pane.is_loading = true;
            pane.entries.clear();
        }
        let list_config = self.state.config.list.clone();
        let filesystem_config = self.state.config.filesystem.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use crate::filesystem::{
                        EntryFilter, FileSystem, ListOptions, LocalFileSystem, SortKey, SortOrder,
                    };
                    let fs = LocalFileSystem::from_config(filesystem_config);
                    let options = ListOptions {
                        show_hidden: list_config.show_hidden,
                        sort_by: SortKey::Name,
                        sort_order: SortOrder::Asc,
                        directories_first: true,
                        filter: EntryFilter::All,
                        name_query: None,
                        respect_gitignore: false,
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
}
