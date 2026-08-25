//! Dialogs and side panels: properties, diff, hex, permissions, dual pane.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;

use iced::Task;

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
    /// therefore loud rather than silent — an unrouted message fails the test
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
                if let Some(dialog) = &mut self.properties_dialog {
                    if dialog.path == path {
                        dialog.sha256 = Some(hash);
                        dialog.computing_hash = false;
                    }
                }
            }
            UiMessage::CloseProperties => {
                self.properties_dialog = None;
            }
            // Feature 4: Color themes
            UiMessage::SetTheme(theme) => {
                self.state.config.dark_mode = crate::ui::theme::resolves_dark(&theme);
                self.state.config.theme = theme;
                // Invalidate syntax highlight cache so it's regenerated with the new theme colors
                self.cached_highlighted_preview = None;
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
                    self.last_action =
                        Some("Sélectionnez plusieurs fichiers pour renommer".to_string());
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
                        self.last_action = Some(format!("{} fichiers renommés", n));
                        tasks.push(self.refresh_entries_in_place());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur renommage : {}", e));
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
                if let Some(pane) = &mut self.dual_pane.pane_b {
                    if pane.path == path {
                        pane.entries = entries;
                        pane.truncated = truncated;
                        pane.is_loading = false;
                    }
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
                    self.last_action = Some(format!("« Ouvrir avec » : {error}"));
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
                    self.last_action =
                        Some("Sélectionnez exactement 2 fichiers pour comparer".to_string());
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
                                Err(error) => Err((
                                    path,
                                    format!("Impossible de lire les permissions : {error}"),
                                )),
                            }
                        })
                        .await
                        .unwrap_or_else(|_| {
                            Err((
                                path_fallback,
                                "Erreur interne lors de la lecture des permissions".to_string(),
                            ))
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
                                        if let (Ok(ma), Ok(mb)) = (ma, mb) {
                                            if ma.len() != mb.len() {
                                                different.push((*name).clone());
                                            }
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
                self.last_action = Some(format!(
                    "Comparaison : {} communs, {} différents, {} unique A, {} unique B",
                    common,
                    different.len(),
                    only_a.len(),
                    only_b.len()
                ));
                // Build diff lines for display in diff viewer
                let mut lines = Vec::new();
                lines.push(crate::ui::DiffLine::Header(format!(
                    "--- {} fichiers en commun ---",
                    common
                )));
                for name in &different {
                    lines.push(crate::ui::DiffLine::Header(format!(
                        "≠ {name} (taille différente)"
                    )));
                }
                for name in &only_a {
                    lines.push(crate::ui::DiffLine::Removed(format!(
                        "Seulement dans A: {name}"
                    )));
                }
                for name in &only_b {
                    lines.push(crate::ui::DiffLine::Added(format!(
                        "Seulement dans B: {name}"
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
                // Bascule clair/sombre dans la famille courante, au lieu de
                // faire tourner cinq thèmes en dur. Le catalogue en compte
                // cinquante-sept ; les parcourir un par un n'aurait plus de
                // sens, et le choix précis appartient au sélecteur.
                //
                // Une famille dont la variante ne s'appelle ni `dark` ni
                // `light` — `catppuccin` et ses quatre saveurs — n'a pas de
                // contraire évident : `resolve` retombe alors sur le repli, ce
                // qui serait un saut brutal. On reste sur place dans ce cas.
                let choice = &self.state.config.theme;
                let opposite = match choice.variant.as_str() {
                    "dark" => Some("light"),
                    "light" => Some("dark"),
                    _ => None,
                };
                if let Some(variant) = opposite {
                    let mut next = choice.clone();
                    next.variant = variant.to_string();
                    self.state.config.dark_mode = crate::ui::theme::resolves_dark(&next);
                    self.state.config.theme = next;
                    self.cached_highlighted_preview = None;
                    self.config_manager.save(&self.state.config);
                } else {
                    self.last_action = Some(format!(
                        "« {} » n'a pas de variante claire ou sombre — choisissez dans le menu",
                        choice.family
                    ));
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
                debug_assert!(false, "UiMessage non routé vers un domaine : {other:?}");
                tracing::error!("UiMessage non routé, ignoré : {other:?}");
            }
        }

        Flow::Continue
    }
}
