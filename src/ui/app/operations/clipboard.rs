//! Cut, copy, paste and the selection snapshot they operate on.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use iced::Task;

use crate::filesystem::{FileOperationKind, LocalFileOperations, OperationReport};
use crate::ui::UiMessage;

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    pub(in crate::ui::app) fn selected_paths(&self) -> Vec<PathBuf> {
        self.state
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect()
    }

    pub(in crate::ui::app) fn capture_clipboard(&mut self, kind: ClipboardKind) {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à mettre en presse-papiers".to_string());
            return;
        }
        let label = match kind {
            ClipboardKind::Copy => "Copie",
            ClipboardKind::Cut => "Déplacement",
        };
        self.clipboard.set(kind, items);
        self.last_action = Some(format!(
            "{} : {} élément(s)",
            label,
            self.clipboard.items.len()
        ));
    }

    pub(in crate::ui::app) fn paste_clipboard(&mut self) -> Task<UiMessage> {
        let Some(kind) = self.clipboard.kind else {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        };
        if self.clipboard.items.is_empty() {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        }

        let items = self.clipboard.items.clone();
        let Some(destination) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Opération indisponible en vue réseau".to_string());
            return Task::none();
        };
        let total = items.len();
        let counter = Arc::new(AtomicUsize::new(0));
        self.operation_progress = Some(FileOpProgress {
            counter: counter.clone(),
            total,
            kind: match kind {
                ClipboardKind::Copy => FileOperationKind::Copy,
                ClipboardKind::Cut => FileOperationKind::Move,
            },
        });
        // Stash undo context
        let operation_id = self.next_operation_id();
        self.pending_undo_context.insert(
            operation_id,
            match kind {
                ClipboardKind::Copy => PendingUndoContext::Copy {
                    sources: items.clone(),
                    destination: destination.clone(),
                },
                ClipboardKind::Cut => PendingUndoContext::Move {
                    sources: items.clone(),
                    destination: destination.clone(),
                },
            },
        );
        let report_kind = match kind {
            ClipboardKind::Copy => FileOperationKind::Copy,
            ClipboardKind::Cut => FileOperationKind::Move,
        };
        Task::perform(
            async move {
                // Copying a tree is blocking work. Running it directly in the
                // future pinned a tokio worker for the whole operation, which
                // is why thumbnails and listings froze during a large paste.
                tokio::task::spawn_blocking(move || {
                    let operations = LocalFileOperations::new();
                    match kind {
                        ClipboardKind::Copy => {
                            operations.copy_items(&items, &destination, Some(counter))
                        }
                        ClipboardKind::Cut => {
                            operations.move_items(&items, &destination, Some(counter))
                        }
                    }
                })
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Tâche de copie interrompue : {error}");
                    OperationReport::empty(report_kind)
                })
            },
            move |report| UiMessage::FileOperationFinished(operation_id, report),
        )
    }

    pub(in crate::ui::app) fn copy_selection_path(&mut self) {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() == 1
            && let Some(path) = selection.selected.iter().next()
        {
            self.last_action = Some(format!("Chemin copié : {}", path.display()));
            return;
        }
        self.last_action = Some("Sélectionnez un élément pour copier le chemin".to_string());
    }
}
