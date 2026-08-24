//! Moving the focus and selecting, always over the visible rows only.
//!
//! Moved verbatim out of the single `impl XionApp` block in `mod.rs`.

use crate::ui::SelectionKind;
use crate::ui::app::XionApp;
use std::path::PathBuf;

impl XionApp {
    pub(in crate::ui::app) fn move_focus_by(&mut self, offset: isize, extend: bool) {
        let selection = &self.state.navigation.selection;
        if let Some(indices) = self.visible_indices() {
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
                (start_index + offset as usize).min(self.display_entries().total.saturating_sub(1))
            };

            self.move_focus_to_actual_index(target_index, extend);
        }
    }

    pub(in crate::ui::app) fn move_focus_to_start(&mut self, extend: bool) {
        if let Some(indices) = self.visible_indices() {
            if let Some(index) = indices.first().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.first_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    pub(in crate::ui::app) fn move_focus_to_end(&mut self, extend: bool) {
        if let Some(indices) = self.visible_indices() {
            if let Some(index) = indices.last().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.last_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    pub(in crate::ui::app) fn move_focus_to_actual_index(&mut self, index: usize, extend: bool) {
        let Some(path) = self
            .display_entries()
            .get(index)
            .map(|entry| entry.path.clone())
        else {
            return;
        };
        let kind = if extend {
            SelectionKind::Range
        } else {
            SelectionKind::Single
        };
        self.apply_selection(path, kind);
    }

    pub(in crate::ui::app) fn select_all_entries(&mut self) {
        // Ctrl+A means "everything I can see". The `else` branch used to read
        // `self.entries` directly, so with only the quick filter active it
        // selected the entire directory behind the user's back.
        let entries = self.display_entries();
        let selected_paths: Vec<PathBuf> = match self.filtered_indices_for(entries) {
            Some(indices) => indices
                .into_iter()
                .filter_map(|index| entries.get(index).map(|entry| entry.path.clone()))
                .collect(),
            None => entries
                .items
                .iter()
                .flatten()
                .map(|entry| entry.path.clone())
                .collect(),
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

    pub(in crate::ui::app) fn clear_selection(&mut self) {
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

    pub(in crate::ui::app) fn cycle_focus(&mut self) {
        self.state.navigation.focused_pane = match self.state.navigation.focused_pane {
            crate::ui::PaneKind::Tree => crate::ui::PaneKind::List,
            crate::ui::PaneKind::List => crate::ui::PaneKind::Preview,
            crate::ui::PaneKind::Preview => crate::ui::PaneKind::Tree,
        };
    }
}
