//! Navigation, selection, focus, and search methods for XionApp.
//!
//! Extracted from the main `mod.rs` to keep method groups manageable.

use std::path::PathBuf;

use crate::ui::SelectionKind;

use super::XionApp;

mod buffers;
mod filtering;
mod focus;
mod rubber_band;

impl XionApp {
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
            let visible = self.filtered_indices_for(selection_entries);
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
                match visible {
                    // With a filter active, Shift+click must join the two
                    // endpoints through the visible rows only. Walking the raw
                    // index range used to swallow every hidden entry in
                    // between — 42 files selected where 3 were on screen.
                    Some(indices) => indices
                        .into_iter()
                        .filter(|index| (start..=end).contains(index))
                        .filter_map(|index| selection_entries.get(index))
                        .map(|entry| entry.path.clone())
                        .collect::<Vec<_>>(),
                    None => (start..=end)
                        .filter_map(|index| selection_entries.get(index))
                        .map(|entry| entry.path.clone())
                        .collect::<Vec<_>>(),
                }
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
}

/// Case-insensitive `contains`, allocating nothing.
///
/// `needle` must already be lowercase — callers lowercase the query once, not
/// once per entry. Comparison goes through `char::to_lowercase` rather than
/// `eq_ignore_ascii_case`, so `Éclair` still matches `éclair`; an ASCII-only
/// shortcut would quietly break every accented file name.
pub(super) fn contains_lowercased(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_empty() {
        return false;
    }
    haystack
        .char_indices()
        .any(|(offset, _)| starts_with_lowercased(&haystack[offset..], needle))
}

/// Whether `haystack`, lowercased on the fly, starts with the already-lowercase
/// `needle`. `char::to_lowercase` can yield several chars for one input char,
/// hence the flattened iterator.
fn starts_with_lowercased(haystack: &str, needle: &str) -> bool {
    let mut folded = haystack.chars().flat_map(char::to_lowercase);
    let mut wanted = needle.chars();
    loop {
        match wanted.next() {
            None => return true,
            Some(expected) => match folded.next() {
                None => return false,
                Some(actual) if actual != expected => return false,
                Some(_) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::metadata::FsMetadata;
    use crate::filesystem::{FsEntry, FsEntryType};

    fn entry(name: &str) -> FsEntry {
        FsEntry {
            path: PathBuf::from("/tmp").join(name),
            name: name.to_string(),
            entry_type: FsEntryType::File,
            metadata: FsMetadata::default(),
        }
    }

    /// A `XionApp` whose entry buffer is fully loaded with `names`.
    fn app_with(names: &[&str]) -> XionApp {
        let mut app = XionApp::new_for_test();
        let items: Vec<Option<FsEntry>> = names.iter().map(|name| Some(entry(name))).collect();
        app.entries.total = items.len();
        app.entries.items = items;
        app
    }

    fn selected_names(app: &XionApp) -> Vec<String> {
        let mut names: Vec<String> = app
            .state
            .navigation
            .selection
            .selected
            .iter()
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        names.sort();
        names
    }

    /// Regression: with only the quick filter active, `select_all_entries` read
    /// the raw buffer and selected the whole directory — 200 files where 3 were
    /// on screen — and the next Delete removed them without a trash bin.
    #[test]
    fn quick_filter_select_all_ignores_hidden_entries() {
        let mut app = app_with(&["facture-01.pdf", "notes.txt", "facture-02.pdf", "photo.png"]);
        app.quick_filter = "fact".to_string();
        app.quick_filter_active = true;

        app.select_all_entries();

        assert_eq!(
            selected_names(&app),
            vec!["facture-01.pdf".to_string(), "facture-02.pdf".to_string()]
        );
    }

    #[test]
    fn select_all_without_a_filter_takes_everything() {
        let mut app = app_with(&["a.txt", "b.txt", "c.txt"]);

        app.select_all_entries();

        assert_eq!(app.state.navigation.selection.selected.len(), 3);
    }

    #[test]
    fn search_query_select_all_ignores_hidden_entries() {
        let mut app = app_with(&["alpha.log", "beta.txt", "gamma.log"]);
        app.search.input = "log".to_string();

        app.select_all_entries();

        assert_eq!(
            selected_names(&app),
            vec!["alpha.log".to_string(), "gamma.log".to_string()]
        );
    }

    /// Regression: Shift+click walked the raw index range and swallowed every
    /// hidden entry between the two endpoints.
    #[test]
    fn shift_click_range_respects_filter() {
        let mut app = app_with(&[
            "a.log",
            "hidden1.txt",
            "hidden2.txt",
            "b.log",
            "hidden3.txt",
            "c.log",
        ]);
        app.quick_filter = ".log".to_string();
        app.quick_filter_active = true;

        app.apply_selection(PathBuf::from("/tmp/a.log"), SelectionKind::Single);
        app.apply_selection(PathBuf::from("/tmp/c.log"), SelectionKind::Range);

        assert_eq!(
            selected_names(&app),
            vec![
                "a.log".to_string(),
                "b.log".to_string(),
                "c.log".to_string()
            ],
            "la plage ne doit contenir que les lignes visibles"
        );
    }

    #[test]
    fn shift_click_range_without_filter_takes_the_whole_span() {
        let mut app = app_with(&["a", "b", "c", "d"]);

        app.apply_selection(PathBuf::from("/tmp/a"), SelectionKind::Single);
        app.apply_selection(PathBuf::from("/tmp/c"), SelectionKind::Range);

        assert_eq!(
            selected_names(&app),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    /// Regression: arrow keys navigated the raw buffer, so the focus vanished
    /// onto rows the filter had removed.
    #[test]
    fn arrow_keys_walk_visible_rows_only() {
        let mut app = app_with(&["a.log", "hidden.txt", "b.log"]);
        app.quick_filter = ".log".to_string();
        app.quick_filter_active = true;

        app.move_focus_to_start(false);
        assert_eq!(
            app.state.navigation.selection.focused,
            Some(PathBuf::from("/tmp/a.log"))
        );

        app.move_focus_by(1, false);
        assert_eq!(
            app.state.navigation.selection.focused,
            Some(PathBuf::from("/tmp/b.log")),
            "la flèche bas doit sauter la ligne masquée"
        );
    }

    #[test]
    fn move_focus_to_end_lands_on_the_last_visible_row() {
        let mut app = app_with(&["a.log", "b.log", "zzz.txt"]);
        app.quick_filter = ".log".to_string();
        app.quick_filter_active = true;

        app.move_focus_to_end(false);

        assert_eq!(
            app.state.navigation.selection.focused,
            Some(PathBuf::from("/tmp/b.log"))
        );
    }

    // ── contains_lowercased ────────────────────────────────────────────────

    #[test]
    fn matching_is_case_insensitive() {
        assert!(contains_lowercased("Rapport Final.PDF", "final"));
        assert!(contains_lowercased("Rapport Final.PDF", "pdf"));
        assert!(!contains_lowercased("Rapport Final.PDF", "docx"));
    }

    /// An ASCII-only shortcut would break here; accented names are the norm in
    /// a French file tree.
    #[test]
    fn matching_folds_non_ascii_case() {
        assert!(contains_lowercased("ÉCLAIR.txt", "éclair"));
        assert!(contains_lowercased("Déjà-Vu", "jà-v"));
        assert!(contains_lowercased("STRASSE", "strasse"));
    }

    #[test]
    fn matching_handles_edges() {
        assert!(contains_lowercased("anything", ""));
        assert!(!contains_lowercased("", "x"));
        assert!(contains_lowercased("abc", "abc"));
        assert!(!contains_lowercased("ab", "abc"));
    }
}
