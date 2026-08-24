//! Single rename and bulk rename.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;

use iced::Task;

use crate::filesystem::{FileOperationKind, LocalFileOperations, OperationReport};
use crate::ui::UiMessage;

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    pub(in crate::ui::app) fn open_rename_dialog(&mut self) -> Task<UiMessage> {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() != 1 {
            self.last_action = Some("Renommage : sélectionnez un seul élément".to_string());
            return Task::none();
        }
        let path = selection
            .selected
            .iter()
            .next()
            .cloned()
            .or_else(|| self.state.route.local_path().cloned());
        let Some(path) = path else {
            self.last_action = Some("Renommage indisponible en vue réseau".to_string());
            return Task::none();
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        }
        self.rename_dialog = Some(RenameDialog { path, input: name });
        Task::none()
    }

    pub(in crate::ui::app) fn submit_rename(&mut self) -> Task<UiMessage> {
        let Some(dialog) = self.rename_dialog.take() else {
            return Task::none();
        };
        let trimmed = dialog.input.trim();
        if trimmed.is_empty() {
            self.last_action = Some("Renommage : nom invalide".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        if let Err(reason) = super::validate_file_name(trimmed) {
            self.last_action = Some(format!("Renommage : {reason}"));
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        let Some(parent) = dialog.path.parent() else {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        };
        let target = parent.join(trimmed);
        if target == dialog.path {
            self.last_action = Some("Renommage : nom identique".to_string());
            return Task::none();
        }
        let source = dialog.path.clone();
        // Stash undo context for rename
        let operation_id = self.next_operation_id();
        self.pending_undo_context.insert(
            operation_id,
            PendingUndoContext::Rename {
                old_path: source.clone(),
            },
        );
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    LocalFileOperations::new().rename_item(&source, &target)
                })
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Tâche de renommage interrompue : {error}");
                    OperationReport::empty(FileOperationKind::Rename)
                })
            },
            move |report| UiMessage::FileOperationFinished(operation_id, report),
        )
    }

    pub(in crate::ui::app) fn recompute_bulk_rename_previews(&mut self) {
        let Some(state) = &mut self.bulk_rename else {
            return;
        };
        state.error = None;
        if state.use_regex {
            match crate::ui::app::regex_replace_preview(&state.paths, &state.find, &state.replace) {
                Ok(previews) => state.previews = previews,
                Err(e) => {
                    state.error = Some(e);
                    state.previews = state
                        .paths
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
                }
            }
        } else {
            state.previews = state
                .paths
                .iter()
                .map(|p| {
                    let name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let new_name = if state.find.is_empty() {
                        name.clone()
                    } else {
                        name.replace(&state.find, &state.replace)
                    };
                    (name, new_name)
                })
                .collect();
        }
    }

    pub(in crate::ui::app) fn apply_bulk_rename(&mut self) -> Task<UiMessage> {
        let Some(state) = &self.bulk_rename else {
            return Task::none();
        };
        if state.error.is_some() {
            return Task::none();
        }
        let renames: Vec<(PathBuf, String)> = state
            .paths
            .iter()
            .zip(state.previews.iter())
            .filter(|(_, (old, new))| old != new)
            .map(|(path, (_, new_name))| (path.clone(), new_name.clone()))
            .collect();
        if renames.is_empty() {
            return Task::none();
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut count = 0usize;
                    for (path, new_name) in &renames {
                        if let Some(parent) = path.parent() {
                            let target = parent.join(new_name);
                            if std::fs::rename(path, &target).is_ok() {
                                count += 1;
                            }
                        }
                    }
                    Ok(count)
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::BulkRenameCompleted,
        )
    }

    // ── Feature 10: Archive browser ───────────────────────────────────────────
}
