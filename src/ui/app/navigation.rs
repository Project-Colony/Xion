//! Navigation, selection, focus, and search methods for XionApp.
//!
//! Extracted from the main `mod.rs` to keep method groups manageable.

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::{Arc, Mutex};

use iced::{Point, Rectangle, Task};

use crate::core::ViewMode;
use crate::filesystem::{FsEntry, WatchEvent};
use crate::services::{DirectoryLoader, PreviewImageService, ThumbnailService};
use crate::ui::{RouteKind, SelectionKind, UiMessage};
use crate::ui::theme::UiTokens;

use super::XionApp;
use super::helpers::rectangles_intersect;
use super::types::PagedEntries;

use std::time::Instant;

impl XionApp {
    pub(super) fn selection_kind_from_modifiers(&self) -> SelectionKind {
        if self.modifiers.shift {
            SelectionKind::Range
        } else if self.modifiers.control {
            SelectionKind::Toggle
        } else {
            SelectionKind::Single
        }
    }

    pub(super) fn base_display_entries(&self) -> &PagedEntries {
        if self.is_refreshing {
            self.stale_entries.as_ref().unwrap_or(&self.entries)
        } else {
            &self.entries
        }
    }

    pub(super) fn display_entries(&self) -> &PagedEntries {
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

    pub(super) fn begin_user_selection(&mut self) {
        self.is_user_selecting = true;
        if self.selection_snapshot.is_none() {
            self.selection_snapshot = Some(self.base_display_entries().clone());
        }
    }

    pub(super) fn list_content_point(&self, position: Point, clamp: bool) -> Option<Point> {
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

    pub(super) fn selection_box_rect(&self) -> Option<Rectangle> {
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

    pub(super) fn filtered_indices_for(&self, entries: &PagedEntries) -> Option<Vec<usize>> {
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

    pub(super) fn list_header_visible(
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

    pub(super) fn list_content_offset(&self, list_header_visible: bool) -> f32 {
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

    pub(super) fn entries_in_selection_box(&self) -> Vec<PathBuf> {
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

    pub(super) fn apply_box_selection(&mut self, paths: Vec<PathBuf>, kind: SelectionKind) {
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

    pub(super) fn reload_config(&mut self) -> Task<UiMessage> {
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

    pub(super) fn apply_selection(&mut self, path: PathBuf, kind: SelectionKind) {
        let previous_focus = self.state.navigation.selection.focused.clone();
        let anchor_path = self.state.navigation.selection.anchor.clone();
        let selection_kind = match kind {
            SelectionKind::Range if anchor_path.is_none() => SelectionKind::Single,
            SelectionKind::Range => SelectionKind::Range,
            other => other,
        };
        let range_paths = {
            let selection_entries = self.display_entries();
            let target_index = Self::index_for_path_in(selection_entries, &path);
            let anchor_index = anchor_path
                .as_ref()
                .and_then(|anchor_path| Self::index_for_path_in(selection_entries, anchor_path));
            if let (Some(anchor), Some(target)) = (anchor_index, target_index) {
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
            }
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
        self.menus.context_open = false;
        self.menus.context_position = None;
        if selection.focused != previous_focus {
            self.reset_preview_state();
        }
        // Animate preview panel open/close
        let has_selection = !self.state.navigation.selection.selected.is_empty();
        self.preview_anim_target = if has_selection { 1.0 } else { 0.0 };
    }

    pub(super) fn selected_entry<'a>(&'a self, entries: &'a PagedEntries) -> Option<&'a FsEntry> {
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

    // ── Search & filtering ──────────────────────────────────────────────────

    pub(super) fn index_for_path_in(entries: &PagedEntries, path: &Path) -> Option<usize> {
        entries.items.iter().position(|entry| {
            entry
                .as_ref()
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    pub(super) fn index_for_path(&self, path: &Path) -> Option<usize> {
        Self::index_for_path_in(self.display_entries(), path)
    }

    pub(super) fn normalized_search_query(&self) -> Option<String> {
        let trimmed = self.search.input.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }

    pub(super) fn matches_search(entry: &FsEntry, query: &str) -> bool {
        entry.name.to_lowercase().contains(query)
    }

    pub(super) fn filtered_indices(&self, query: &str) -> Vec<usize> {
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

    pub(super) fn filtered_position_for_path(&self, indices: &[usize], path: &Path) -> Option<usize> {
        indices.iter().position(|index| {
            self.entries
                .get(*index)
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    pub(super) fn first_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().position(|entry| entry.is_some())
    }

    pub(super) fn last_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().rposition(|entry| entry.is_some())
    }

    // ── Focus movement ──────────────────────────────────────────────────────

    pub(super) fn move_focus_by(&mut self, offset: isize, extend: bool) {
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

    pub(super) fn move_focus_to_start(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.first().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.first_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    pub(super) fn move_focus_to_end(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.last().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.last_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    pub(super) fn move_focus_to_actual_index(&mut self, index: usize, extend: bool) {
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

    pub(super) fn select_all_entries(&mut self) {
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

        // Use the first item in display order (deterministic) for focus
        let first = selected_paths.first().cloned();

        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        for path in selected_paths {
            selection.selected.insert(path);
        }
        selection.focused = first.clone();
        selection.anchor = first;
        self.reset_preview_state();
    }

    pub(super) fn clear_selection(&mut self) {
        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        selection.focused = None;
        selection.anchor = None;
        self.menus.context_open = false;
        self.menus.context_position = None;
        self.reset_preview_state();
        self.preview_anim_target = 0.0;
    }

    // ── Preview & misc ──────────────────────────────────────────────────────

    pub(super) fn reset_preview_state(&mut self) {
        self.media.preview_handles.clear();
        self.media.preview_misses.clear();
        self.media.previews_in_flight.clear();
        self.media.previews.clear();
        self.media.animated = None;
        self.cached_text_preview = None;
    }

    pub(super) fn advance_animated_preview(&mut self, now: Instant) {
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

    pub(super) fn is_event_relevant(event: &WatchEvent, watched_path: &Path) -> bool {
        event.path == watched_path
            || event
                .path
                .parent()
                .is_some_and(|parent| parent == watched_path)
    }

    pub(super) fn cycle_focus(&mut self) {
        self.state.navigation.focused_pane = match self.state.navigation.focused_pane {
            crate::ui::PaneKind::Tree => crate::ui::PaneKind::List,
            crate::ui::PaneKind::List => crate::ui::PaneKind::Preview,
            crate::ui::PaneKind::Preview => crate::ui::PaneKind::Tree,
        };
    }
}
