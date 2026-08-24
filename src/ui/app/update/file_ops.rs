//! Clipboard, rename, create, delete, undo and operation progress.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use iced::Task;

use crate::filesystem::FileOperationKind;
use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_file_ops(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::ClipboardCut => {
                self.capture_clipboard(ClipboardKind::Cut);
            }
            UiMessage::ClipboardCopy => {
                self.capture_clipboard(ClipboardKind::Copy);
            }
            UiMessage::ClipboardPaste => {
                tasks.push(self.paste_clipboard());
            }
            UiMessage::RenameInputChanged(value) => {
                if let Some(dialog) = &mut self.rename_dialog {
                    dialog.input = value;
                }
            }
            UiMessage::RenameSubmit => {
                tasks.push(self.submit_rename());
            }
            UiMessage::RenameCancel => {
                self.rename_dialog = None;
            }
            UiMessage::FileOperationFinished(operation_id, report) => {
                self.operation_progress = None;
                // Record undo action based on pending context
                if let Some(ctx) = self.pending_undo_context.remove(&operation_id) {
                    if !report.succeeded.is_empty() {
                        match ctx {
                            PendingUndoContext::Copy {
                                sources,
                                destination,
                            } => {
                                let created: Vec<PathBuf> = sources
                                    .iter()
                                    .filter_map(|s| s.file_name().map(|n| destination.join(n)))
                                    .filter(|p| report.succeeded.contains(p))
                                    .collect();
                                if !created.is_empty() {
                                    self.undo_stack.push(UndoAction::Copy { created });
                                }
                            }
                            PendingUndoContext::Move {
                                sources,
                                destination,
                            } => {
                                let pairs: Vec<(PathBuf, PathBuf)> = sources
                                    .iter()
                                    .filter_map(|s| {
                                        let dest = s.file_name().map(|n| destination.join(n))?;
                                        if report.succeeded.contains(&dest) {
                                            Some((s.clone(), dest))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                if !pairs.is_empty() {
                                    self.undo_stack.push(UndoAction::Move {
                                        original_paths: pairs,
                                    });
                                }
                            }
                            PendingUndoContext::Rename { old_path } => {
                                if let Some(new_path) = report.succeeded.first() {
                                    self.undo_stack.push(UndoAction::Renamed {
                                        old_path,
                                        new_path: new_path.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                self.handle_operation_report(&report);
                tasks.push(self.refresh_entries_in_place());
            }
            UiMessage::OperationProgressTick => {
                if let Some(op) = &self.operation_progress {
                    let done = op.counter.load(Ordering::Relaxed).min(op.total);
                    let label = match op.kind {
                        FileOperationKind::Copy => "Copie",
                        FileOperationKind::Move => "Déplacement",
                        FileOperationKind::Delete => "Suppression",
                        FileOperationKind::Rename => "Renommage",
                    };
                    self.last_action = Some(format!("{label} : {done}/{} éléments…", op.total));
                }
            }
            UiMessage::NewFolder => {
                tasks.push(self.create_new_folder());
            }
            UiMessage::NewFolderCreated(result) => match result {
                Ok(path) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.last_action = Some(format!("Dossier créé : {name}"));
                    self.undo_stack
                        .push(UndoAction::FolderCreated { path: path.clone() });
                    tasks.push(self.refresh_entries_in_place());
                    self.rename_dialog = Some(RenameDialog { path, input: name });
                }
                Err(error) => {
                    self.last_action = Some(format!("Erreur création dossier : {error}"));
                }
            },
            UiMessage::NewFile => {
                tasks.push(self.create_new_file());
            }
            UiMessage::NewFileCreated(result) => match result {
                Ok(path) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.last_action = Some(format!("Fichier créé : {name}"));
                    self.undo_stack
                        .push(UndoAction::FileCreated { path: path.clone() });
                    tasks.push(self.refresh_entries_in_place());
                    self.rename_dialog = Some(RenameDialog { path, input: name });
                }
                Err(error) => {
                    self.last_action = Some(format!("Erreur création fichier : {error}"));
                }
            },
            UiMessage::TrashCompleted(result) => {
                match result {
                    Ok(()) => {
                        self.last_action = Some("Déplacé vers la corbeille".to_string());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur corbeille : {}", e));
                    }
                }
                tasks.push(self.refresh_entries_in_place());
            }
            // Feature 3: Properties dialog
            UiMessage::Undo => {
                tasks.push(self.execute_undo());
            }
            UiMessage::UndoCompleted(result) => {
                match result {
                    Ok(msg) => {
                        self.last_action = Some(msg);
                    }
                    Err(msg) => {
                        self.last_action = Some(format!("Erreur annulation : {msg}"));
                    }
                }
                tasks.push(self.refresh_entries_in_place());
            }
            // Feature R: Breadcrumb dropdown
            UiMessage::RemoveFavorite(path) => {
                self.favorites.remove(&path);
                self.state.config.user_favorites.retain(|p| p != &path);
                self.config_manager.save(&self.state.config);
                self.last_action = Some("Retiré des favoris".to_string());
            }
            // ── #25: Terminal autocomplete ────────────────────────────────────
            UiMessage::SetLabel(path, label_opt) => {
                match label_opt {
                    Some(label) => {
                        self.state.config.labels.insert(path, label);
                    }
                    None => {
                        self.state.config.labels.remove(&path);
                    }
                }
                self.config_manager.save(&self.state.config);
            }
            // ── Feature G: Recent Files ──────────────────────────────────────
            UiMessage::ConfirmAccept => {
                tasks.push(self.accept_confirmation());
            }
            UiMessage::ConfirmCancel => {
                self.confirm_dialog = None;
            }
            UiMessage::SelectionSizeComputed(size) => {
                if let Some(ref mut dialog) = self.properties_dialog {
                    dialog.size_bytes = Some(size);
                }
            }
            // ── Feature E: Quick Filter ──────────────────────────────────────
            UiMessage::DirSizeLoaded {
                path,
                bytes,
                truncated,
            } => {
                self.dir_sizes_loading.remove(&path);
                self.remember_dir_size(path, DirSize { bytes, truncated });
            }
            // Feature 10: Archive browser
            UiMessage::GitStatusLoaded { statuses, .. } => {
                self.git_statuses = statuses;
            }
            // Feature 8: Disk usage
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
