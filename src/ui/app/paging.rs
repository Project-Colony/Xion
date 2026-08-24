//! Which pages to request, and which rows are actually on screen.
//!
//! Moved verbatim out of the single 809-line `impl XionApp` block in `mod.rs`.

use super::types::*;
use crate::core::ViewMode;
use crate::services::{VirtualList, VirtualWindow};
use crate::ui::UiMessage;
use crate::ui::theme::layout::TREE_ROW_HEIGHT;
use iced::Task;

use super::XionApp;

impl XionApp {
    /// Ceiling on how many entries a search will pull into memory.
    ///
    /// Filtering happens client-side, so an active query needs every page of
    /// the directory resident. Without a bound that is the whole directory:
    /// 100 000 entries is roughly 66 MB, allocated because someone typed one
    /// letter. Truncation is reported, never silent.
    const MAX_SEARCH_ENTRIES: usize = 20_000;

    pub(in crate::ui::app) fn request_all_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let total_pages = self.entries.total.div_ceil(self.entries.page_size);
        let allowed_pages = Self::MAX_SEARCH_ENTRIES
            .div_ceil(self.entries.page_size)
            .min(total_pages);

        if allowed_pages < total_pages {
            self.last_action = Some(format!(
                "Recherche limitée aux {} premiers éléments sur {}",
                allowed_pages * self.entries.page_size,
                self.entries.total
            ));
        }

        let mut tasks = Vec::new();
        for page_index in 0..allowed_pages {
            if !self.entries.is_page_loaded(page_index) {
                tasks.push(self.request_page(page_index));
            }
        }
        Task::batch(tasks)
    }

    pub(in crate::ui::app) fn ensure_visible_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        if self.has_search_query() {
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

    pub(in crate::ui::app) fn entry_virtual_window(&self) -> VirtualWindow {
        self.entry_virtual_window_for(self.entries.total)
    }

    pub(in crate::ui::app) fn entry_virtual_window_for(&self, total: usize) -> VirtualWindow {
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

    pub(in crate::ui::app) fn list_virtual_window_for(&self, total: usize) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: self.state.config.view.row_height,
            viewport_height: self.scroll.height,
            overscan: self.state.config.view.overscan,
        };
        virtual_list.visible_range(self.scroll.offset, total)
    }

    pub(in crate::ui::app) fn grid_window_for(&self, total: usize) -> GridWindow {
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

    pub(in crate::ui::app) fn tree_virtual_window(&self, total: usize) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: TREE_ROW_HEIGHT,
            viewport_height: self.scroll.tree_height,
            overscan: self.state.config.view.overscan,
        };
        virtual_list.visible_range(self.scroll.tree_offset, total)
    }
}
