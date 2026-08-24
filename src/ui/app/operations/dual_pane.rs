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
                truncated: 0,
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
        // The same options as the first pane, rather than a second set written
        // out by hand: this used to force sort-by-name-ascending and ignore
        // `respect_gitignore`, so the two panes of a dual-pane view could show
        // the same directory in two different orders.
        let options = crate::ui::app::helpers::list_options_from_config(
            self.state.config.list.clone(),
            self.state.config.respect_gitignore,
        );
        let filesystem_config = self.state.config.filesystem.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use crate::filesystem::{FileSystem, LocalFileSystem, PageRequest};
                    let fs = LocalFileSystem::from_config(filesystem_config);
                    // Only what the pane can draw. It read the whole directory
                    // before, however large, to render the first 200 rows.
                    match fs.list_dir_paged(&path, options, PageRequest::new(0, PANE_B_LIMIT)) {
                        Ok(page) => {
                            let truncated = page.total.saturating_sub(page.items.len());
                            (path, page.items, truncated)
                        }
                        Err(_) => (path, Vec::new(), 0),
                    }
                })
                .await
                .ok()
                .unwrap_or_else(|| (PathBuf::new(), Vec::new(), 0))
            },
            |(path, entries, truncated)| UiMessage::PaneBLoaded {
                path,
                entries,
                truncated,
            },
        )
    }
}
