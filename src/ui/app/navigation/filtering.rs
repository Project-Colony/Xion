//! Which rows are visible, and how a path maps to an index.
//!
//! `visible_indices` is the single source of truth the view, the keyboard and
//! the mouse all go through.
//!
//! Moved verbatim out of the single `impl XionApp` block in `mod.rs`.

use crate::filesystem::FsEntry;
use crate::ui::app::XionApp;
use crate::ui::app::types::PagedEntries;
use std::path::Path;

use super::contains_lowercased;

impl XionApp {
    /// Indices of the entries the user can actually see, or `None` when no
    /// filter is active and every loaded entry is visible.
    ///
    /// This is the single source of truth for the view, the keyboard and the
    /// mouse. A second, search-only implementation used to exist alongside it;
    /// because that one ignored the quick filter, Ctrl+A selected the whole
    /// directory while three rows were on screen, and the next Delete removed
    /// files that had never been displayed.
    pub(in crate::ui::app) fn visible_indices(&self) -> Option<Vec<usize>> {
        self.filtered_indices_for(self.display_entries())
    }

    pub(in crate::ui::app) fn filtered_indices_for(
        &self,
        entries: &PagedEntries,
    ) -> Option<Vec<usize>> {
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
                // Both predicates lowercase the *query* once, above, and then
                // compare without allocating per entry. The previous code built
                // a fresh String for every name on every pass, and this runs on
                // each rebuild and each mouse move during a rubber-band drag.
                if let Some(ref query) = search_query
                    && !contains_lowercased(&entry.name, query)
                {
                    return None;
                }
                if let Some(ref filter) = quick_filter
                    && !contains_lowercased(&entry.name, filter)
                {
                    return None;
                }
                Some(index)
            })
            .collect::<Vec<_>>();
        Some(indices)
    }
}

/// Translates a display row into an index in the loaded entries.
///
/// `None` when a filter is active and the row is past its end. The view and the
/// rubber-band selection each carried their own copy of this closure, which is
/// how they could disagree about which row a coordinate belonged to.
pub(in crate::ui::app) fn entry_index_for(
    filtered_indices: Option<&[usize]>,
    display_index: usize,
) -> Option<usize> {
    match filtered_indices {
        Some(indices) => indices.get(display_index).copied(),
        None => Some(display_index),
    }
}

impl XionApp {
    pub(in crate::ui::app) fn normalized_search_query(&self) -> Option<String> {
        let trimmed = self.search.input.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }

    /// Whether a search query is active, without allocating the lowercased
    /// copy that `normalized_search_query` produces.
    pub(in crate::ui::app) fn has_search_query(&self) -> bool {
        !self.search.input.trim().is_empty()
    }

    pub(in crate::ui::app) fn filtered_position_for_path(
        &self,
        indices: &[usize],
        path: &Path,
    ) -> Option<usize> {
        let entries = self.display_entries();
        indices.iter().position(|index| {
            entries
                .get(*index)
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    pub(in crate::ui::app) fn index_for_path_in(
        entries: &PagedEntries,
        path: &Path,
    ) -> Option<usize> {
        entries.items.iter().position(|entry| {
            entry
                .as_ref()
                .map(|entry| entry.path == path)
                .unwrap_or(false)
        })
    }

    pub(in crate::ui::app) fn index_for_path(&self, path: &Path) -> Option<usize> {
        Self::index_for_path_in(self.display_entries(), path)
    }

    pub(in crate::ui::app) fn selected_entry<'a>(
        &'a self,
        entries: &'a PagedEntries,
    ) -> Option<&'a FsEntry> {
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

    pub(in crate::ui::app) fn first_entry_index(&self) -> Option<usize> {
        self.display_entries()
            .items
            .iter()
            .position(|entry| entry.is_some())
    }

    pub(in crate::ui::app) fn last_entry_index(&self) -> Option<usize> {
        self.display_entries()
            .items
            .iter()
            .rposition(|entry| entry.is_some())
    }

    // ── Focus movement ──────────────────────────────────────────────────────
}
