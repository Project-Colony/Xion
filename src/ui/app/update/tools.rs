//! Dialogs and side panels: properties, diff, hex, permissions, dual pane.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;

use iced::Task;

use crate::i18n::{t, tf, tn};
use crate::ui::UiMessage;

use crate::ui::app::helpers::{self};
use crate::ui::app::permissions;
use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

impl XionApp {
    /// Last handler in the chain.
    ///
    /// A truly exhaustive match is impossible here: the compiler cannot know
    /// that the seven handlers before this one already consumed their
    /// variants, so it would demand an arm for all 156. The catch-all is
    /// therefore loud rather than silent, an unrouted message fails the test
    /// suite in debug and is logged in release, instead of quietly doing
    /// nothing forever.
    pub(super) fn update_tools(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Flow {
        match message {
            UiMessage::OpenProperties(path) => {
                tasks.push(self.open_properties(path));
            }
            UiMessage::PropertiesHashComputed { path, hash } => {
                if let Some(dialog) = &mut self.properties_dialog
                    && dialog.path == path
                {
                    dialog.sha256 = Some(hash);
                    dialog.computing_hash = false;
                }
            }
            UiMessage::CloseProperties => {
                self.properties_dialog = None;
            }
            UiMessage::TogglePreferences => {
                self.menus.preferences.open = !self.menus.preferences.open;
                // Every visit starts on General, as in every Colony program.
                self.menus.preferences.category = crate::ui::PreferencesCategory::default();
                // The `⋯` menu would cover the page that just opened.
                self.menus.overflow_open = false;
            }
            UiMessage::SelectPreferencesCategory(category) => {
                self.menus.preferences.category = category;
            }
            UiMessage::TogglePreferencesSection(key) => {
                let expanded = &mut self.menus.preferences.expanded;
                if !expanded.remove(key) {
                    expanded.insert(key.to_string());
                }
            }
            UiMessage::ToggleShowHidden => {
                self.state.config.list.show_hidden = !self.state.config.list.show_hidden;
                self.config_manager.save(&self.state.config);
                // Filtering happens at load, not at render: without a reload,
                // the list keeps the old content.
                tasks.push(self.refresh_entries());
            }
            UiMessage::ToggleDirectoriesFirst => {
                self.state.config.list.directories_first =
                    !self.state.config.list.directories_first;
                self.config_manager.save(&self.state.config);
                tasks.push(self.refresh_entries());
            }
            UiMessage::Preference(change) => {
                if let Some(task) = self.apply_preference(change) {
                    tasks.push(task);
                }
            }
            UiMessage::DismissToast(id) => {
                if self.menus.preferences.toast == Some(id) {
                    self.menus.preferences.toast = None;
                }
            }
            UiMessage::SetLanguage(language) => {
                self.state.config.language = language;
                crate::i18n::apply(language);
                self.config_manager.save(&self.state.config);
            }
            // Feature 5: Bulk rename
            UiMessage::OpenBulkRename => {
                let paths: Vec<PathBuf> = self
                    .state
                    .navigation
                    .selection
                    .selected
                    .iter()
                    .cloned()
                    .collect();
                if paths.len() > 1 {
                    let previews = paths
                        .iter()
                        .map(|p| {
                            let name = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            (name.clone(), name)
                        })
                        .collect();
                    self.bulk_rename = Some(BulkRenameState {
                        paths,
                        find: String::new(),
                        replace: String::new(),
                        use_regex: false,
                        previews,
                        error: None,
                    });
                } else {
                    self.last_action = Some(t("bulk_rename_select_several").to_string());
                }
            }
            UiMessage::BulkRenameFindChanged(val) => {
                if let Some(state) = &mut self.bulk_rename {
                    state.find = val;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameReplaceChanged(val) => {
                if let Some(state) = &mut self.bulk_rename {
                    state.replace = val;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameToggleRegex => {
                if let Some(state) = &mut self.bulk_rename {
                    state.use_regex = !state.use_regex;
                    self.recompute_bulk_rename_previews();
                }
            }
            UiMessage::BulkRenameApply => {
                tasks.push(self.apply_bulk_rename());
            }
            UiMessage::BulkRenameCancel => {
                self.bulk_rename = None;
            }
            UiMessage::BulkRenameCompleted(result) => {
                self.bulk_rename = None;
                match result {
                    Ok(n) => {
                        self.last_action = Some(tn("bulk_rename_done", n, &[]));
                        tasks.push(self.refresh_entries_in_place());
                    }
                    Err(e) => {
                        self.last_action = Some(tf("error_rename", &[("error", &e)]));
                    }
                }
            }
            // Feature 6: Syntax highlighting
            UiMessage::ToggleDualPane => {
                tasks.push(self.toggle_dual_pane());
            }
            UiMessage::PaneBNavigate(path) => {
                tasks.push(self.pane_b_navigate(path));
            }
            UiMessage::PaneBLoaded {
                path,
                entries,
                truncated,
            } => {
                if let Some(pane) = &mut self.dual_pane.pane_b
                    && pane.path == path
                {
                    pane.entries = entries;
                    pane.truncated = truncated;
                    pane.is_loading = false;
                }
            }
            UiMessage::PaneBActivate(path) => {
                if path.is_dir() {
                    tasks.push(self.pane_b_navigate(path));
                } else if let Err(error) = Self::shell_open(&path) {
                    self.last_action = Some(error);
                }
            }
            UiMessage::SwitchActivePane => {
                if self.dual_pane.enabled {
                    self.dual_pane.active = 1 - self.dual_pane.active;
                }
            }
            // ── Feature A: Compress to ZIP ──────────────────────────────────
            UiMessage::OpenWith(path) => {
                if let Err(error) = crate::platform::open_with(&path) {
                    self.last_action = Some(tf("error_open_with", &[("error", &error)]));
                }
            }
            // ── Feature C: File Diff ──────────────────────────────────────────
            UiMessage::OpenDiff => {
                let selected: Vec<PathBuf> = self
                    .state
                    .navigation
                    .selection
                    .selected
                    .iter()
                    .cloned()
                    .collect();
                if selected.len() == 2 {
                    let path_a = selected[0].clone();
                    let path_b = selected[1].clone();
                    self.diff_view = Some(DiffViewState {
                        path_a: path_a.clone(),
                        path_b: path_b.clone(),
                        lines: Vec::new(),
                        loading: true,
                    });
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                // Bounded: diffing two multi-gigabyte files used
                                // to load both of them whole before showing
                                // anything, and `unwrap_or_default` made an
                                // unreadable file look like an empty one.
                                let content_a = helpers::read_text_capped(
                                    &path_a,
                                    helpers::MAX_TEXT_PREVIEW_BYTES,
                                )
                                .unwrap_or_else(|error| format!("<{error}>"));
                                let content_b = helpers::read_text_capped(
                                    &path_b,
                                    helpers::MAX_TEXT_PREVIEW_BYTES,
                                )
                                .unwrap_or_else(|error| format!("<{error}>"));
                                let lines_a: Vec<&str> = content_a.lines().collect();
                                let lines_b: Vec<&str> = content_b.lines().collect();
                                let mut diff_lines = Vec::new();
                                diff_lines.push(crate::ui::DiffLine::Header(format!(
                                    "--- {}",
                                    path_a.display()
                                )));
                                diff_lines.push(crate::ui::DiffLine::Header(format!(
                                    "+++ {}",
                                    path_b.display()
                                )));
                                let max = lines_a.len().max(lines_b.len());
                                for i in 0..max {
                                    match (lines_a.get(i), lines_b.get(i)) {
                                        (Some(a), Some(b)) if a == b => {
                                            diff_lines
                                                .push(crate::ui::DiffLine::Same(a.to_string()));
                                        }
                                        (Some(a), Some(b)) => {
                                            diff_lines
                                                .push(crate::ui::DiffLine::Removed(a.to_string()));
                                            diff_lines
                                                .push(crate::ui::DiffLine::Added(b.to_string()));
                                        }
                                        (Some(a), None) => {
                                            diff_lines
                                                .push(crate::ui::DiffLine::Removed(a.to_string()));
                                        }
                                        (None, Some(b)) => {
                                            diff_lines
                                                .push(crate::ui::DiffLine::Added(b.to_string()));
                                        }
                                        (None, None) => {}
                                    }
                                }
                                (path_a, path_b, diff_lines)
                            })
                            .await
                            .ok()
                        },
                        |result| match result {
                            Some((path_a, path_b, lines)) => UiMessage::DiffLoaded {
                                path_a,
                                path_b,
                                lines,
                            },
                            None => UiMessage::Noop,
                        },
                    ));
                } else {
                    self.last_action = Some(t("compare_select_two").to_string());
                }
            }
            UiMessage::DiffLoaded {
                path_a,
                path_b,
                lines,
            } => {
                self.diff_view = Some(DiffViewState {
                    path_a,
                    path_b,
                    lines,
                    loading: false,
                });
            }
            UiMessage::CloseDiff => {
                self.diff_view = None;
            }
            // ── Feature D: Multi-selection properties ──────────────────────────
            UiMessage::OpenHexView(path) => {
                let hex_path = path.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            use std::io::Read;
                            let mut f = std::fs::File::open(&hex_path).ok()?;
                            let mut buf = vec![0u8; 4096];
                            let n = f.read(&mut buf).ok()?;
                            buf.truncate(n);
                            Some((hex_path, buf))
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |result| match result {
                        Some((path, data)) => UiMessage::HexViewLoaded { path, data },
                        None => UiMessage::Noop,
                    },
                ));
            }
            UiMessage::HexViewLoaded { path, data } => {
                self.hex_view = Some(HexViewState {
                    path,
                    data,
                    offset: 0,
                });
            }
            UiMessage::CloseHexView => {
                self.hex_view = None;
            }
            UiMessage::HexViewScroll(offset) => {
                if let Some(ref mut hv) = self.hex_view {
                    hv.offset = offset;
                }
            }
            // ── Network Discovery ─────────────────────────────────────────────
            UiMessage::OpenPermissions(path) => {
                self.permissions_view = Some(PermissionsViewState {
                    path: path.clone(),
                    entries: Vec::new(),
                    loading: true,
                    error: None,
                });
                tasks.push(Task::perform(
                    async move {
                        let path_fallback = path.clone();
                        tokio::task::spawn_blocking(move || {
                            match crate::platform::read_acl_text(&path) {
                                Ok(text) => {
                                    let entries = permissions::parse_icacls_output(&text);
                                    Ok((path, entries))
                                }
                                Err(error) => {
                                    Err((path, tf("permissions_read_failed", &[("error", &error)])))
                                }
                            }
                        })
                        .await
                        .unwrap_or_else(|_| {
                            Err((path_fallback, t("permissions_internal_error").to_string()))
                        })
                    },
                    |result| match result {
                        Ok((path, entries)) => UiMessage::PermissionsLoaded {
                            path,
                            entries,
                            error: None,
                        },
                        Err((path, error)) => UiMessage::PermissionsLoaded {
                            path,
                            entries: vec![],
                            error: Some(error),
                        },
                    },
                ));
            }
            UiMessage::PermissionsLoaded {
                path,
                entries,
                error,
            } => {
                self.permissions_view = Some(PermissionsViewState {
                    path,
                    entries,
                    loading: false,
                    error,
                });
            }
            UiMessage::ClosePermissions => {
                self.permissions_view = None;
            }
            UiMessage::FolderCompare => {
                if self.dual_pane.enabled {
                    let path_a = self.state.route.local_path().cloned();
                    let path_b = self.dual_pane.pane_b.as_ref().map(|p| p.path.clone());
                    if let (Some(a), Some(b)) = (path_a, path_b) {
                        tasks.push(Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    let entries_a: std::collections::HashSet<String> =
                                        std::fs::read_dir(&a)
                                            .into_iter()
                                            .flatten()
                                            .flatten()
                                            .filter_map(|e| {
                                                e.file_name().to_str().map(|s| s.to_string())
                                            })
                                            .collect();
                                    let entries_b: std::collections::HashSet<String> =
                                        std::fs::read_dir(&b)
                                            .into_iter()
                                            .flatten()
                                            .flatten()
                                            .filter_map(|e| {
                                                e.file_name().to_str().map(|s| s.to_string())
                                            })
                                            .collect();
                                    let only_a: Vec<String> =
                                        entries_a.difference(&entries_b).cloned().collect();
                                    let only_b: Vec<String> =
                                        entries_b.difference(&entries_a).cloned().collect();
                                    let common_names: Vec<&String> =
                                        entries_a.intersection(&entries_b).collect();
                                    let common = common_names.len();
                                    let mut different = Vec::new();
                                    for name in &common_names {
                                        let ma = std::fs::metadata(a.join(name));
                                        let mb = std::fs::metadata(b.join(name));
                                        if let (Ok(ma), Ok(mb)) = (ma, mb)
                                            && ma.len() != mb.len()
                                        {
                                            different.push((*name).clone());
                                        }
                                    }
                                    (only_a, only_b, different, common)
                                })
                                .await
                                .unwrap_or_default()
                            },
                            |(only_a, only_b, different, common)| UiMessage::FolderCompareResults {
                                only_a,
                                only_b,
                                different,
                                common,
                            },
                        ));
                    }
                }
            }
            UiMessage::FolderCompareResults {
                only_a,
                only_b,
                different,
                common,
            } => {
                self.last_action = Some(tf(
                    "compare_summary",
                    &[
                        ("common", &common),
                        ("different", &different.len()),
                        ("only_a", &only_a.len()),
                        ("only_b", &only_b.len()),
                    ],
                ));
                // Build diff lines for display in diff viewer
                let mut lines = Vec::new();
                lines.push(crate::ui::DiffLine::Header(tn(
                    "compare_common_files",
                    common,
                    &[],
                )));
                for name in &different {
                    lines.push(crate::ui::DiffLine::Header(tf(
                        "compare_size_differs",
                        &[("name", name)],
                    )));
                }
                for name in &only_a {
                    lines.push(crate::ui::DiffLine::Removed(tf(
                        "compare_only_a",
                        &[("name", name)],
                    )));
                }
                for name in &only_b {
                    lines.push(crate::ui::DiffLine::Added(tf(
                        "compare_only_b",
                        &[("name", name)],
                    )));
                }
                let path_a = self.state.route.local_path().cloned().unwrap_or_default();
                let path_b = self
                    .dual_pane
                    .pane_b
                    .as_ref()
                    .map(|p| p.path.clone())
                    .unwrap_or_default();
                self.diff_view = Some(DiffViewState {
                    path_a,
                    path_b,
                    lines,
                    loading: false,
                });
            }
            // ── #18: Tab drag reorder ────────────────────────────────────────
            UiMessage::ToggleDarkMode => {
                // Toggle light/dark within the current family, instead of
                // cycling five hard-coded themes. The catalogue has fifty-seven;
                // walking them one by one would make no sense any more, and the
                // precise choice belongs to the picker.
                //
                // A family whose variants are called neither `dark` nor `light`
                // (`catppuccin` and its four flavours) has no obvious opposite:
                // stay in place and say so rather than jump to another family.
                let standard = &self.state.config.standard;
                let opposite = match standard.selected_variant.as_str() {
                    "dark" => Some("light"),
                    "light" => Some("dark"),
                    _ => None,
                };
                let change =
                    colony_ui::theme::family(&standard.selected_theme).and_then(|family| {
                        let variant = family.variant(opposite?)?;
                        Some(colony_ui::preferences::Change::Theme {
                            family: family.key,
                            variant: variant.key,
                        })
                    });
                match change {
                    Some(change) => {
                        if let Some(task) = self.apply_preference(change) {
                            tasks.push(task);
                        }
                    }
                    None => {
                        self.last_action = Some(tf(
                            "theme_no_light_dark_variant",
                            &[("family", &standard.selected_theme)],
                        ));
                    }
                }
            }
            UiMessage::ToggleCompactMode => {
                self.state.config.compact_mode = !self.state.config.compact_mode;
                self.state.config.view.row_height = if self.state.config.compact_mode {
                    22.0
                } else {
                    32.0
                };
                self.config_manager.save(&self.state.config);
            }
            // ── Feature O: Grep ───────────────────────────────────────────────
            other => {
                debug_assert!(false, "UiMessage not routed to a domain: {other:?}");
                tracing::error!("UiMessage not routed, ignored: {other:?}");
            }
        }

        Flow::Continue
    }
}

impl XionApp {
    /// Records a shared Appearance or Accessibility change, puts it into
    /// effect and saves it: there is no Save button.
    ///
    /// A theme change also raises the "Theme applied." toast, since switching
    /// to a neighbouring variant can otherwise go unnoticed, and returns the
    /// timer that hides it again.
    pub(in crate::ui::app) fn apply_preference(
        &mut self,
        change: colony_ui::preferences::Change,
    ) -> Option<Task<UiMessage>> {
        use colony_ui::preferences::Change;

        let config = &mut self.state.config;
        config.standard.update(change);
        config.dark_mode = crate::ui::theme::resolves_dark(&config.standard);
        self.config_manager.save(&self.state.config);

        let Change::Theme { .. } = change else {
            return None;
        };
        // The highlighter keeps colours computed for the old theme until it is
        // cleared.
        self.cached_highlighted_preview = None;

        let preferences = &mut self.menus.preferences;
        preferences.toast_counter += 1;
        let id = preferences.toast_counter;
        preferences.toast = Some(id);
        // Inside `async`: the timer needs the runtime that runs the task, not
        // the one (if any) that builds it.
        Some(Task::perform(
            async { tokio::time::sleep(crate::ui::theme::timing::TOAST_DURATION).await },
            move |()| UiMessage::DismissToast(id),
        ))
    }
}

#[cfg(test)]
mod preference_tests {
    use colony_ui::preferences::Change;

    use crate::ui::UiMessage;
    use crate::ui::app::XionApp;

    fn apply_theme(app: &mut XionApp, family: &'static str, variant: &'static str) {
        let _ = app.update(UiMessage::Preference(Change::Theme { family, variant }));
    }

    /// Applying a theme is immediate, and the toast makes it noticeable.
    #[test]
    fn a_theme_change_saves_the_choice_and_raises_the_toast() {
        let mut app = XionApp::new_for_test();
        assert_eq!(app.menus.preferences.toast, None);

        apply_theme(&mut app, "nord", "dark");

        let standard = &app.state.config.standard;
        assert_eq!(
            (
                standard.selected_theme.as_str(),
                standard.selected_variant.as_str()
            ),
            ("nord", "dark")
        );
        assert!(app.menus.preferences.toast.is_some());
    }

    /// The timer of an older toast must not hide the one raised after it.
    #[test]
    fn only_the_latest_toast_is_dismissed_by_its_timer() {
        let mut app = XionApp::new_for_test();
        apply_theme(&mut app, "nord", "dark");
        let first = app.menus.preferences.toast.expect("first toast");
        apply_theme(&mut app, "gruvbox", "light");
        let second = app.menus.preferences.toast.expect("second toast");

        let _ = app.update(UiMessage::DismissToast(first));
        assert_eq!(app.menus.preferences.toast, Some(second));

        let _ = app.update(UiMessage::DismissToast(second));
        assert_eq!(app.menus.preferences.toast, None);
    }

    /// Only a theme change raises it: the other settings show their effect
    /// on the page itself.
    #[test]
    fn other_changes_raise_no_toast() {
        let mut app = XionApp::new_for_test();
        let _ = app.update(UiMessage::Preference(Change::AutoAccent(true)));
        assert!(app.state.config.standard.auto_accent);
        assert_eq!(app.menus.preferences.toast, None);
    }

    /// Every visit to the page starts on General.
    #[test]
    fn opening_the_page_starts_on_general() {
        let mut app = XionApp::new_for_test();
        let _ = app.update(UiMessage::TogglePreferences);
        let _ = app.update(UiMessage::SelectPreferencesCategory(
            crate::ui::PreferencesCategory::Files,
        ));
        let _ = app.update(UiMessage::TogglePreferences);
        let _ = app.update(UiMessage::TogglePreferences);
        assert!(app.menus.preferences.open);
        assert_eq!(
            app.menus.preferences.category,
            crate::ui::PreferencesCategory::General
        );
    }
}
