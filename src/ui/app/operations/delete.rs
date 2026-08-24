//! Trash, permanent delete behind a confirmation, undo, and the report the UI shows afterwards.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;

use iced::Task;

use crate::filesystem::{FileOperationKind, LocalFileOperations, OperationReport};
use crate::ui::UiMessage;

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    /// Ask before deleting for good.
    ///
    /// Permanent deletion used to be one keystroke away with no prompt and no
    /// trash: pressing Delete ran `remove_dir_all`/`remove_file` straight away.
    /// Combined with a Ctrl+A that ignored the active filter, that erased files
    /// the user had never even seen.
    pub(in crate::ui::app) fn request_permanent_delete(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à supprimer".to_string());
            return Task::none();
        }

        let message = match items.as_slice() {
            [only] => format!(
                "Supprimer définitivement « {} » ?\nCet élément ne passera pas par la corbeille.",
                only.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| only.display().to_string())
            ),
            many => format!(
                "Supprimer définitivement {} éléments ?\nIls ne passeront pas par la corbeille.",
                many.len()
            ),
        };

        self.confirm_dialog = Some(ConfirmDialog {
            title: "Suppression définitive".to_string(),
            message,
            confirm_label: "Supprimer".to_string(),
            action: ConfirmedAction::DeletePermanently(items),
        });
        Task::none()
    }

    /// Carry out the action the user just confirmed.
    pub(in crate::ui::app) fn accept_confirmation(&mut self) -> Task<UiMessage> {
        let Some(dialog) = self.confirm_dialog.take() else {
            return Task::none();
        };
        match dialog.action {
            ConfirmedAction::DeletePermanently(items) => self.delete_permanently(items),
        }
    }

    fn delete_permanently(&mut self, items: Vec<PathBuf>) -> Task<UiMessage> {
        // No undo context: a permanent delete cannot be undone. The id is still
        // allocated so the result message has the same shape as every other.
        let operation_id = self.next_operation_id();
        self.clear_selection();
        Task::perform(
            async move {
                // Deleting a tree is blocking work; running it inline held a
                // tokio worker for the whole traversal.
                tokio::task::spawn_blocking(move || LocalFileOperations::new().delete_items(&items))
                    .await
                    .unwrap_or_else(|error| {
                        tracing::error!("Tâche de suppression interrompue : {error}");
                        OperationReport::empty(FileOperationKind::Delete)
                    })
            },
            move |report| UiMessage::FileOperationFinished(operation_id, report),
        )
    }

    pub(in crate::ui::app) fn trash_selection(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à déplacer".to_string());
            return Task::none();
        }
        self.clear_selection();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    for path in &items {
                        trash::delete(path).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::TrashCompleted,
        )
    }

    // ── Feature Q: Undo ────────────────────────────────────────────────────

    pub(in crate::ui::app) fn execute_undo(&mut self) -> Task<UiMessage> {
        let Some(action) = self.undo_stack.pop() else {
            self.last_action = Some("Rien à annuler".to_string());
            return Task::none();
        };
        match action {
            UndoAction::Copy { created, .. } => {
                // Undo copy = delete the created copies
                let paths = created;
                Task::perform(
                    async move {
                        let ops = LocalFileOperations::new();
                        let report = ops.delete_items(&paths);
                        if report.failed.is_empty() {
                            Ok(format!(
                                "Annulé : {} copie(s) supprimée(s)",
                                report.succeeded.len()
                            ))
                        } else {
                            Err(format!(
                                "{} erreur(s) lors de l'annulation",
                                report.failed.len()
                            ))
                        }
                    },
                    UiMessage::UndoCompleted,
                )
            }
            UndoAction::Move { original_paths } => {
                // Undo move = move files back to original locations
                Task::perform(
                    async move {
                        let ops = LocalFileOperations::new();
                        let mut ok = 0usize;
                        let mut err = 0usize;
                        for (source, dest) in &original_paths {
                            let report = ops.rename_item(dest, source);
                            if report.failed.is_empty() {
                                ok += 1;
                            } else {
                                err += 1;
                            }
                        }
                        if err == 0 {
                            Ok(format!("Annulé : {} déplacement(s)", ok))
                        } else {
                            Err(format!("{err} erreur(s) lors de l'annulation ({ok} ok)"))
                        }
                    },
                    UiMessage::UndoCompleted,
                )
            }
            UndoAction::FileCreated { path } => Task::perform(
                async move {
                    let ops = LocalFileOperations::new();
                    let report = ops.delete_items(&[path]);
                    if report.failed.is_empty() {
                        Ok("Annulé : fichier supprimé".to_string())
                    } else {
                        Err("Erreur suppression fichier".to_string())
                    }
                },
                UiMessage::UndoCompleted,
            ),
            UndoAction::FolderCreated { path } => Task::perform(
                async move {
                    let ops = LocalFileOperations::new();
                    let report = ops.delete_items(&[path]);
                    if report.failed.is_empty() {
                        Ok("Annulé : dossier supprimé".to_string())
                    } else {
                        Err("Erreur suppression dossier".to_string())
                    }
                },
                UiMessage::UndoCompleted,
            ),
            UndoAction::Renamed { old_path, new_path } => Task::perform(
                async move {
                    let ops = LocalFileOperations::new();
                    let report = ops.rename_item(&new_path, &old_path);
                    if report.failed.is_empty() {
                        Ok(format!("Annulé : renommé en {}", old_path.display()))
                    } else {
                        Err("Erreur annulation renommage".to_string())
                    }
                },
                UiMessage::UndoCompleted,
            ),
        }
    }

    // ── Feature 3: Properties dialog ─────────────────────────────────────────

    pub(in crate::ui::app) fn handle_operation_report(&mut self, report: &OperationReport) {
        let success = report.succeeded.len();
        let failure = report.failed.len();
        let base_message = match report.action {
            FileOperationKind::Copy => format!("Copie : {} ok", success),
            FileOperationKind::Move => format!("Déplacement : {} ok", success),
            FileOperationKind::Rename => {
                if success == 1 {
                    report
                        .succeeded
                        .first()
                        .map(|path| format!("Renommé : {}", path.display()))
                        .unwrap_or_else(|| "Renommage terminé".to_string())
                } else {
                    format!("Renommage : {} ok", success)
                }
            }
            FileOperationKind::Delete => format!("Suppression : {} ok", success),
        };
        let full_message = if failure > 0 {
            format!("{base_message} / {failure} erreur(s)")
        } else {
            base_message
        };
        self.last_action = Some(full_message);
        // Always clear clipboard after a move (cut-paste), even on partial failure,
        // to prevent retrying the same cut on stale/moved source paths.
        if report.action == FileOperationKind::Move {
            self.clipboard = ClipboardState::default();
        }
        if matches!(
            report.action,
            FileOperationKind::Rename | FileOperationKind::Delete
        ) {
            self.rename_dialog = None;
        }
    }
}
