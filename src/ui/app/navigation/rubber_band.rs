//! Click-drag selection: the frozen buffer, the rectangle, and the rows it covers.
//!
//! Moved verbatim out of the single `impl XionApp` block in `mod.rs`.

use super::entry_index_for;
use crate::core::ViewMode;
use crate::ui::SelectionKind;
use crate::ui::app::XionApp;
use crate::ui::app::helpers::rectangles_intersect;
use crate::ui::app::types::PagedEntries;
use crate::ui::theme::UiTokens;
use iced::{Point, Rectangle};
use std::path::PathBuf;

impl XionApp {
    pub(in crate::ui::app) fn selection_kind_from_modifiers(&self) -> SelectionKind {
        if self.modifiers.shift {
            SelectionKind::Range
        } else if self.modifiers.control {
            SelectionKind::Toggle
        } else {
            SelectionKind::Single
        }
    }

    pub(in crate::ui::app) fn begin_user_selection(&mut self) {
        self.is_user_selecting = true;
        if self.selection_snapshot.is_none() {
            self.selection_snapshot = Some(self.base_display_entries().clone());
        }
    }

    pub(in crate::ui::app) fn list_content_point(
        &self,
        position: Point,
        clamp: bool,
    ) -> Option<Point> {
        let bounds = self.list_viewport_bounds?;
        let mut local_x = position.x - bounds.x;
        let mut local_y = position.y - bounds.y;

        if clamp {
            local_x = local_x.clamp(0.0, bounds.width.max(0.0));
            local_y = local_y.clamp(0.0, bounds.height.max(0.0));
        } else if local_x < 0.0
            || local_y < 0.0
            || local_x > bounds.width
            || local_y > bounds.height
        {
            return None;
        }

        Some(Point::new(local_x, local_y + self.scroll.offset))
    }

    pub(in crate::ui::app) fn selection_box_rect(&self) -> Option<Rectangle> {
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

    pub(in crate::ui::app) fn list_header_visible(
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

    pub(in crate::ui::app) fn list_content_offset(&self, list_header_visible: bool) -> f32 {
        let tokens = UiTokens::for_theme(&self.state.config.theme);
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

    pub(in crate::ui::app) fn entries_in_selection_box(&self) -> Vec<PathBuf> {
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
        let tokens = UiTokens::for_theme(&self.state.config.theme);
        let list_padding = tokens.spacing.md;
        let content_width = (bounds.width - list_padding * 2.0).max(1.0);
        let filter_slice = filtered_indices.as_deref();
        let mut selected = Vec::new();

        match view_mode {
            ViewMode::List => {
                let window = self.list_virtual_window_for(total_entries);
                let row_height = self.state.config.view.row_height;
                for display_index in window.start..window.end {
                    let Some(actual_index) = entry_index_for(filter_slice, display_index) else {
                        continue;
                    };
                    let Some(entry) = display_entries.get(actual_index) else {
                        continue;
                    };
                    let local_index = display_index.saturating_sub(window.start) as f32;
                    let item_y =
                        list_content_offset + window.padding_top + local_index * row_height;
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
                    let item_y =
                        list_content_offset + grid.window.padding_top + local_row * tile_height;
                    for column_index in 0..columns {
                        let display_index = row_index * columns + column_index;
                        if display_index >= total_entries {
                            continue;
                        }
                        let Some(actual_index) = entry_index_for(filter_slice, display_index)
                        else {
                            continue;
                        };
                        let Some(entry) = display_entries.get(actual_index) else {
                            continue;
                        };
                        let item_x =
                            list_padding + (tile_width + tokens.spacing.md) * column_index as f32;
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

    pub(in crate::ui::app) fn apply_box_selection(
        &mut self,
        paths: Vec<PathBuf>,
        kind: SelectionKind,
    ) {
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
        // Animate preview panel open/close based on resulting selection
        let has_selection = !self.state.navigation.selection.selected.is_empty();
        self.preview_anim_target = if has_selection { 1.0 } else { 0.0 };
        if !has_selection {
            self.reset_preview_state();
        }
    }
}
