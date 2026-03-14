//! File operation methods extracted from the main `XionApp` impl.
//!
//! Covers keyboard commands, context actions, clipboard, rename, delete,
//! trash, undo, properties, bulk rename, archive extraction, dual pane,
//! and text-preview detection.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use iced::Task;

use crate::filesystem::{FileOperationKind, FsEntryType, LocalFileOperations, OperationReport};
use crate::ui::{ArchiveEntry, ContextAction, KeyboardCommand, SelectionKind, UiMessage};

use super::archive;
use super::types::*;
use super::XionApp;

impl XionApp {
    pub(super) fn handle_keyboard_command(&mut self, command: KeyboardCommand) -> Task<UiMessage> {
        match command {
            KeyboardCommand::MoveUp { extend } => {
                self.move_focus_by(-1, extend);
                Task::none()
            }
            KeyboardCommand::MoveDown { extend } => {
                self.move_focus_by(1, extend);
                Task::none()
            }
            KeyboardCommand::MoveHome { extend } => {
                self.move_focus_to_start(extend);
                Task::none()
            }
            KeyboardCommand::MoveEnd { extend } => {
                self.move_focus_to_end(extend);
                Task::none()
            }
            KeyboardCommand::Activate => self.activate_focused_entry(),
            KeyboardCommand::Back => {
                if self.history.can_back() {
                    if let Some(path) = self.history.back() {
                        self.update_active_tab_path(path);
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Forward => {
                if self.history.can_forward() {
                    if let Some(path) = self.history.forward() {
                        self.update_active_tab_path(path);
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Refresh => self.refresh_entries(),
            KeyboardCommand::SelectAll => {
                self.select_all_entries();
                Task::none()
            }
            KeyboardCommand::ClearSelection => {
                self.clear_selection();
                Task::none()
            }
            KeyboardCommand::ToggleContextMenu => {
                self.menus.context_open = !self.menus.context_open;
                if self.menus.context_open {
                    self.menus.context_position = self.cursor_position;
                } else {
                    self.menus.context_position = None;
                }
                Task::none()
            }
            KeyboardCommand::CyclePaneFocus => {
                self.cycle_focus();
                Task::none()
            }
            KeyboardCommand::Rename => self.open_rename_dialog(),
            KeyboardCommand::Delete => self.delete_selection(),
            KeyboardCommand::NewFolder => self.create_new_folder(),
            KeyboardCommand::FocusSearch => {
                iced::widget::operation::focus(iced::widget::Id::new("search_input"))
            }
            KeyboardCommand::NewTab => Task::done(UiMessage::AddTab),
            KeyboardCommand::CloseCurrentTab => {
                let idx = self.tab_manager.active;
                if idx > 0 {
                    Task::done(UiMessage::CloseTab(idx))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::NextTab => {
                if self.tab_manager.count() > 1 {
                    let next = (self.tab_manager.active + 1) % self.tab_manager.count();
                    Task::done(UiMessage::SwitchTab(next))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::PrevTab => {
                if self.tab_manager.count() > 1 {
                    let prev = if self.tab_manager.active == 0 {
                        self.tab_manager.count() - 1
                    } else {
                        self.tab_manager.active - 1
                    };
                    Task::done(UiMessage::SwitchTab(prev))
                } else {
                    Task::none()
                }
            }
            KeyboardCommand::QuickLook => {
                let focused = self.state.navigation.selection.focused.clone();
                if let Some(path) = focused {
                    let already_previewing = self
                        .cached_text_preview
                        .as_ref()
                        .is_some_and(|(p, _)| p == &path)
                        || self.media.preview_handles.contains_key(&path);
                    if already_previewing && self.preview_anim_target > 0.5 {
                        self.preview_anim_target = 0.0;
                    } else {
                        self.apply_selection(path, SelectionKind::Single);
                    }
                }
                Task::none()
            }
            KeyboardCommand::BulkRename => {
                Task::done(UiMessage::OpenBulkRename)
            }
            KeyboardCommand::ToggleDualPane => {
                self.toggle_dual_pane()
            }
            KeyboardCommand::SwitchActivePane => {
                if self.dual_pane.enabled {
                    self.dual_pane.active = 1 - self.dual_pane.active;
                }
                Task::none()
            }
            KeyboardCommand::OpenDiff => {
                Task::done(UiMessage::OpenDiff)
            }
            KeyboardCommand::QuickFilterChanged(c) => {
                Task::done(UiMessage::QuickFilterChanged(c))
            }
            KeyboardCommand::QuickFilterClear => {
                Task::done(UiMessage::QuickFilterClear)
            }
            KeyboardCommand::OpenGrep => {
                Task::done(UiMessage::OpenGrep)
            }
            KeyboardCommand::Undo => {
                Task::done(UiMessage::Undo)
            }
            KeyboardCommand::FocusAddress => {
                self.address_editing = true;
                iced::widget::operation::focus(iced::widget::Id::new("address_input"))
            }
            KeyboardCommand::GoToParent => {
                if let Some(path) = self.state.route.local_path().cloned() {
                    if let Some(parent) = path.parent() {
                        return self.navigate_to(parent.to_path_buf());
                    }
                }
                Task::none()
            }
            KeyboardCommand::CopyPath => {
                self.copy_selection_path();
                Task::none()
            }
            KeyboardCommand::GoToBookmark(index) => {
                if let Some(path) = self.state.config.user_favorites.get(index).cloned() {
                    self.navigate_to(path)
                } else {
                    self.last_action = Some(format!("Aucun favori #{}", index + 1));
                    Task::none()
                }
            }
        }
    }

    pub(super) fn apply_context_action(&mut self, action: ContextAction) -> Task<UiMessage> {
        self.menus.context_open = false;
        self.menus.background_context_open = false;
        self.menus.context_position = None;
        let selection = &self.state.navigation.selection;
        let selected_label = if selection.selected.len() == 1 {
            selection
                .selected
                .iter()
                .next()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".to_string())
        } else if selection.selected.is_empty() {
            "—".to_string()
        } else {
            format!("{} éléments", selection.selected.len())
        };

        self.last_action = Some(match action {
            ContextAction::Open => format!("Ouverture : {}", selected_label),
            ContextAction::Rename => format!("Renommer : {}", selected_label),
            ContextAction::MoveToTrash => format!("Corbeille : {}", selected_label),
            ContextAction::Delete => format!("Supprimer : {}", selected_label),
            ContextAction::CopyPath => format!("Copier le chemin : {}", selected_label),
            ContextAction::OpenProperties => format!("Propriétés : {}", selected_label),
            ContextAction::OpenBulkRename => format!("Renommage multiple : {}", selected_label),
            ContextAction::CompressToZip => format!("Compression ZIP : {}", selected_label),
            ContextAction::CompressToTarGz => format!("Compression TAR.GZ : {}", selected_label),
            ContextAction::CompressTo7z => format!("Compression 7Z : {}", selected_label),
            ContextAction::OpenWith => format!("Ouvrir avec : {}", selected_label),
            ContextAction::OpenDiff => format!("Comparaison : {}", selected_label),
            ContextAction::OpenHexView => format!("Hex : {}", selected_label),
            ContextAction::OpenPermissions => format!("Permissions : {}", selected_label),
            ContextAction::SetLabelRed | ContextAction::SetLabelOrange | ContextAction::SetLabelYellow
            | ContextAction::SetLabelGreen | ContextAction::SetLabelBlue | ContextAction::SetLabelPurple
            | ContextAction::RemoveLabel => format!("Étiquette : {}", selected_label),
            ContextAction::NewFile => format!("Nouveau fichier : {}", selected_label),
            ContextAction::NewFolder => "Nouveau dossier".to_string(),
            ContextAction::AddToFavorites => format!("Ajouter aux favoris : {}", selected_label),
        });

        // Prevent opening a new modal if one is already open
        if self.has_open_modal() {
            match action {
                ContextAction::Rename
                | ContextAction::OpenProperties
                | ContextAction::OpenBulkRename
                | ContextAction::OpenDiff
                | ContextAction::OpenHexView
                | ContextAction::OpenPermissions => {
                    self.last_action = Some("Fermez le dialogue actuel d'abord".to_string());
                    return Task::none();
                }
                _ => {}
            }
        }

        match action {
            ContextAction::Open => self.activate_focused_entry(),
            ContextAction::Rename => self.open_rename_dialog(),
            ContextAction::MoveToTrash => self.trash_selection(),
            ContextAction::Delete => self.delete_selection(),
            ContextAction::CopyPath => {
                self.copy_selection_path();
                Task::none()
            }
            ContextAction::OpenProperties => {
                let selection_count = self.state.navigation.selection.selected.len();
                if selection_count > 1 {
                    // Multi-selection properties
                    let paths: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                    let count = paths.len();
                    // Show a simple dialog and compute total size async
                    self.properties_dialog = Some(PropertiesDialog {
                        path: paths[0].clone(),
                        size_bytes: None,
                        created: None,
                        modified: None,
                        readonly: false,
                        sha256: None,
                        computing_hash: false,
                        selection_count: Some(count),
                    });
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                paths.iter().filter_map(|p| std::fs::metadata(p).ok())
                                    .map(|m| m.len()).sum::<u64>()
                            })
                            .await
                            .unwrap_or(0)
                        },
                        UiMessage::SelectionSizeComputed,
                    );
                }
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenProperties(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenBulkRename => {
                Task::done(UiMessage::OpenBulkRename)
            }
            ContextAction::CompressToZip => {
                Task::done(UiMessage::CompressToZip)
            }
            ContextAction::CompressToTarGz => {
                Task::done(UiMessage::CompressToTarGz)
            }
            ContextAction::CompressTo7z => {
                Task::done(UiMessage::CompressTo7z)
            }
            ContextAction::OpenWith => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenWith(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenDiff => {
                Task::done(UiMessage::OpenDiff)
            }
            ContextAction::OpenHexView => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenHexView(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenPermissions => {
                let focused = self.state.navigation.selection.focused.clone()
                    .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenPermissions(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::SetLabelRed => self.set_label_for_focused(Some(crate::ui::FileLabel::Red)),
            ContextAction::SetLabelOrange => self.set_label_for_focused(Some(crate::ui::FileLabel::Orange)),
            ContextAction::SetLabelYellow => self.set_label_for_focused(Some(crate::ui::FileLabel::Yellow)),
            ContextAction::SetLabelGreen => self.set_label_for_focused(Some(crate::ui::FileLabel::Green)),
            ContextAction::SetLabelBlue => self.set_label_for_focused(Some(crate::ui::FileLabel::Blue)),
            ContextAction::SetLabelPurple => self.set_label_for_focused(Some(crate::ui::FileLabel::Purple)),
            ContextAction::RemoveLabel => self.set_label_for_focused(None),
            ContextAction::NewFile => self.create_new_file(),
            ContextAction::NewFolder => {
                Task::done(UiMessage::KeyboardCommand(KeyboardCommand::NewFolder))
            }
            ContextAction::AddToFavorites => {
                if let Some(path) = self.state.navigation.selection.focused.clone() {
                    if self.favorites.add(path.clone()) {
                        self.state.config.user_favorites.push(path);
                        self.config_manager.save(&self.state.config);
                        self.last_action = Some("Ajouté aux favoris".to_string());
                    } else {
                        self.last_action = Some("Déjà dans les favoris".to_string());
                    }
                }
                Task::none()
            }
        }
    }

    pub(super) fn set_label_for_focused(&mut self, label: Option<crate::ui::FileLabel>) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone()
            .or_else(|| self.state.navigation.selection.selected.iter().next().cloned());
        if let Some(path) = focused {
            Task::done(UiMessage::SetLabel(path, label))
        } else {
            Task::none()
        }
    }

    pub(super) fn selected_paths(&self) -> Vec<PathBuf> {
        self.state
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect()
    }

    pub(super) fn capture_clipboard(&mut self, kind: ClipboardKind) {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à mettre en presse-papiers".to_string());
            return;
        }
        let label = match kind {
            ClipboardKind::Copy => "Copie",
            ClipboardKind::Cut => "Déplacement",
        };
        self.clipboard.kind = Some(kind);
        self.clipboard.items = items;
        self.last_action = Some(format!(
            "{} : {} élément(s)",
            label,
            self.clipboard.items.len()
        ));
    }

    pub(super) fn paste_clipboard(&mut self) -> Task<UiMessage> {
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
        self.pending_undo_context = Some(match kind {
            ClipboardKind::Copy => PendingUndoContext::Copy {
                sources: items.clone(),
                destination: destination.clone(),
            },
            ClipboardKind::Cut => PendingUndoContext::Move {
                sources: items.clone(),
                destination: destination.clone(),
            },
        });
        Task::perform(
            async move {
                let operations = LocalFileOperations::new();
                match kind {
                    ClipboardKind::Copy => operations.copy_items(&items, &destination, Some(counter)),
                    ClipboardKind::Cut => operations.move_items(&items, &destination, Some(counter)),
                }
            },
            UiMessage::FileOperationFinished,
        )
    }

    pub(super) fn open_rename_dialog(&mut self) -> Task<UiMessage> {
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

    pub(super) fn submit_rename(&mut self) -> Task<UiMessage> {
        let Some(dialog) = self.rename_dialog.take() else {
            return Task::none();
        };
        let trimmed = dialog.input.trim();
        if trimmed.is_empty() {
            self.last_action = Some("Renommage : nom invalide".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        if Path::new(trimmed).components().count() > 1 {
            self.last_action = Some("Renommage : le nom doit être simple".to_string());
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
        self.pending_undo_context = Some(PendingUndoContext::Rename {
            old_path: source.clone(),
        });
        Task::perform(
            async move { LocalFileOperations::new().rename_item(&source, &target) },
            UiMessage::FileOperationFinished,
        )
    }

    pub(super) fn delete_selection(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à supprimer".to_string());
            return Task::none();
        }
        self.clear_selection();
        Task::perform(
            async move { LocalFileOperations::new().delete_items(&items) },
            UiMessage::FileOperationFinished,
        )
    }

    pub(super) fn copy_selection_path(&mut self) {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() == 1 {
            if let Some(path) = selection.selected.iter().next() {
                self.last_action = Some(format!("Chemin copié : {}", path.display()));
                return;
            }
        }
        self.last_action = Some("Sélectionnez un élément pour copier le chemin".to_string());
    }

    pub(super) fn handle_operation_report(&mut self, report: &OperationReport) {
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

    pub(super) fn activate_focused_entry(&mut self) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone();
        if let Some(path) = focused {
            return self.activate_entry(path);
        }
        Task::none()
    }

    pub(super) fn activate_entry(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(entry) = self
            .entries
            .items
            .iter()
            .flatten()
            .find(|entry| entry.path == path)
        {
            if entry.entry_type == FsEntryType::Directory {
                if self.state.route.is_network() {
                    self.last_action =
                        Some(format!("Connexion au partage : {}", entry.path.display()));
                    return Task::none();
                }
                return self.navigate_to(entry.path.clone());
            }
            // Feature 10 / M: Open archives in archive browser
            let ext = entry.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let path_str = entry.path.to_string_lossy().to_lowercase();
            if ext == "zip" || ext == "7z" || ext == "tgz" || path_str.ends_with(".tar.gz") {
                // Record in recents
                self.recents.record(entry.path.clone());
                return self.open_archive(entry.path.clone());
            }
            // Record opened file in recents
            self.recents.record(entry.path.clone());
            // Open file with system default application
            let file_path = entry.path.clone();
            self.last_action = Some(format!(
                "Ouverture : {}",
                file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
            Self::shell_open(&file_path);
        }
        Task::none()
    }

    pub(super) fn create_new_folder(&mut self) -> Task<UiMessage> {
        let Some(current_dir) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Création impossible en vue réseau".to_string());
            return Task::none();
        };
        Task::perform(
            async move {
                let base_name = "Nouveau dossier";
                let mut target = current_dir.join(base_name);
                let mut counter = 1u32;
                while target.exists() && counter < 1000 {
                    counter += 1;
                    target = current_dir.join(format!("{base_name} ({counter})"));
                }
                if target.exists() {
                    return Err("Impossible de trouver un nom disponible".to_string());
                }
                match std::fs::create_dir(&target) {
                    Ok(()) => Ok(target),
                    Err(error) => Err(error.to_string()),
                }
            },
            UiMessage::NewFolderCreated,
        )
    }

    /// Returns true if any modal dialog is currently open.
    pub(super) fn has_open_modal(&self) -> bool {
        self.rename_dialog.is_some()
            || self.properties_dialog.is_some()
            || self.bulk_rename.is_some()
            || self.diff_view.is_some()
            || self.hex_view.is_some()
            || self.permissions_view.is_some()
            || self.archive_browser.is_some()
            || self.grep_state.is_some()
    }

    /// Open a file or folder with the OS default handler.
    pub(super) fn shell_open(path: &std::path::Path) {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", &format!("\"{}\"", path.display())])
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
    }

    pub(super) fn create_new_file(&mut self) -> Task<UiMessage> {
        let Some(current_dir) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Création impossible en vue réseau".to_string());
            return Task::none();
        };
        Task::perform(
            async move {
                let base_name = "Nouveau fichier";
                let ext = ".txt";
                let mut target = current_dir.join(format!("{base_name}{ext}"));
                let mut counter = 1u32;
                while target.exists() && counter < 1000 {
                    counter += 1;
                    target = current_dir.join(format!("{base_name} ({counter}){ext}"));
                }
                if target.exists() {
                    return Err("Impossible de trouver un nom disponible".to_string());
                }
                match std::fs::File::create(&target) {
                    Ok(_) => Ok(target),
                    Err(error) => Err(error.to_string()),
                }
            },
            UiMessage::NewFileCreated,
        )
    }

    pub(super) fn trash_selection(&mut self) -> Task<UiMessage> {
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

    pub(super) fn execute_undo(&mut self) -> Task<UiMessage> {
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
                            Ok(format!("Annulé : {} copie(s) supprimée(s)", report.succeeded.len()))
                        } else {
                            Err(format!("{} erreur(s) lors de l'annulation", report.failed.len()))
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
            UndoAction::FileCreated { path } => {
                Task::perform(
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
                )
            }
            UndoAction::FolderCreated { path } => {
                Task::perform(
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
                )
            }
            UndoAction::Renamed { old_path, new_path } => {
                Task::perform(
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
                )
            }
        }
    }

    // ── Feature 3: Properties dialog ─────────────────────────────────────────

    pub(super) fn open_properties(&mut self, path: PathBuf) -> Task<UiMessage> {
        use std::fs;
        use chrono::{DateTime, Local};

        let meta = fs::metadata(&path).ok();
        let size_bytes = meta.as_ref().map(|m| m.len());
        let readonly = meta.as_ref().map(|m| m.permissions().readonly()).unwrap_or(false);
        let created = meta.as_ref().and_then(|m| m.created().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });

        self.properties_dialog = Some(PropertiesDialog {
            path: path.clone(),
            size_bytes,
            created,
            modified,
            readonly,
            sha256: None,
            computing_hash: true,
            selection_count: None,
        });

        // Spawn async SHA-256 computation (only for files)
        if path.is_file() {
            let hash_path = path.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        use sha2::{Digest, Sha256};
                        use std::io::Read;
                        let mut file = std::fs::File::open(&hash_path).ok()?;
                        let mut hasher = Sha256::new();
                        let mut buf = vec![0u8; 65536];
                        loop {
                            let n = file.read(&mut buf).ok()?;
                            if n == 0 { break; }
                            hasher.update(&buf[..n]);
                        }
                        let result = hasher.finalize();
                        Some((hash_path, format!("{:x}", result)))
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |result| match result {
                    Some((path, hash)) => UiMessage::PropertiesHashComputed { path, hash },
                    None => UiMessage::Noop,
                },
            )
        } else {
            // For directories, mark hash as not applicable
            if let Some(dialog) = &mut self.properties_dialog {
                dialog.computing_hash = false;
                dialog.sha256 = Some("N/A (dossier)".to_string());
            }
            Task::none()
        }
    }

    // ── Feature 5: Bulk rename ────────────────────────────────────────────────

    pub(super) fn recompute_bulk_rename_previews(&mut self) {
        let Some(state) = &mut self.bulk_rename else { return; };
        state.error = None;
        if state.use_regex {
            match super::regex_replace_preview(&state.paths, &state.find, &state.replace) {
                Ok(previews) => state.previews = previews,
                Err(e) => {
                    state.error = Some(e);
                    state.previews = state.paths.iter().map(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        (name.clone(), name)
                    }).collect();
                }
            }
        } else {
            state.previews = state.paths.iter().map(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let new_name = if state.find.is_empty() {
                    name.clone()
                } else {
                    name.replace(&state.find, &state.replace)
                };
                (name, new_name)
            }).collect();
        }
    }

    pub(super) fn apply_bulk_rename(&mut self) -> Task<UiMessage> {
        let Some(state) = &self.bulk_rename else {
            return Task::none();
        };
        if state.error.is_some() {
            return Task::none();
        }
        let renames: Vec<(PathBuf, String)> = state.paths.iter().zip(state.previews.iter())
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

    pub(super) fn open_archive(&mut self, path: PathBuf) -> Task<UiMessage> {
        let archive_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let ext = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let path_str = archive_path.to_string_lossy().to_lowercase();
                    let entries = if ext == "7z" {
                        archive::list_7z(&archive_path)
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        archive::list_tar_gz(&archive_path)
                    } else {
                        // ZIP (default)
                        let file = std::fs::File::open(&archive_path).ok()?;
                        let mut zip = zip::ZipArchive::new(file).ok()?;
                        let mut entries = Vec::new();
                        for i in 0..zip.len() {
                            if let Ok(entry) = zip.by_index(i) {
                                let inner_path = entry.name().to_string();
                                let name = inner_path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&inner_path).to_string();
                                let is_dir = entry.is_dir();
                                let size = entry.size();
                                let compressed_size = entry.compressed_size();
                                entries.push(ArchiveEntry { name, inner_path, is_dir, size, compressed_size });
                            }
                        }
                        return Some((archive_path, entries));
                    };
                    let entries = entries?;
                    Some((archive_path, entries))
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((archive_path, entries)) => UiMessage::ArchiveListLoaded {
                    archive_path,
                    inner_path: String::new(),
                    entries,
                },
                None => UiMessage::Noop,
            },
        )
    }

    pub(super) fn extract_archive_entry(&self, archive: PathBuf, inner_path: String, dest_dir: PathBuf) -> Task<UiMessage> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                    let ext = archive.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let path_str = archive.to_string_lossy().to_lowercase();

                    if ext == "7z" {
                        // 7Z: inline because sevenz-rust has no single-entry extract
                        if inner_path.contains("..") {
                            return Err("Chemin d'archive invalide (path traversal)".to_string());
                        }
                        let file_name = inner_path.rsplit('/').find(|s| !s.is_empty()).unwrap_or("file");
                        let out_path = dest_dir.join(file_name);
                        let mut found = false;
                        let mut arch = sevenz_rust::SevenZReader::open(&archive, sevenz_rust::Password::empty())
                            .map_err(|e| e.to_string())?;
                        arch.for_each_entries(|entry, reader| {
                            if entry.name() == inner_path {
                                let mut out_file = std::fs::File::create(&out_path)
                                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                                std::io::copy(reader, &mut out_file)?;
                                found = true;
                            }
                            Ok(true)
                        }).map_err(|e| e.to_string())?;
                        if found { Ok(out_path) } else { Err("Fichier non trouvé dans l'archive".to_string()) }
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        archive::extract_tar_gz_entry(&archive, &inner_path, &dest_dir)
                    } else {
                        archive::extract_zip_entry(&archive, &inner_path, &dest_dir)
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::ExtractComplete,
        )
    }

    // ── Feature 11: Dual pane ─────────────────────────────────────────────────

    pub(super) fn toggle_dual_pane(&mut self) -> Task<UiMessage> {
        self.dual_pane.enabled = !self.dual_pane.enabled;
        if self.dual_pane.enabled {
            let path = self.state.route.local_path().cloned()
                .unwrap_or_else(|| self.state.config.start_path.clone());
            self.dual_pane.pane_b = Some(PaneB {
                path: path.clone(),
                entries: Vec::new(),
                is_loading: true,
            });
            self.pane_b_navigate(path)
        } else {
            self.dual_pane.pane_b = None;
            self.dual_pane.active = 0;
            Task::none()
        }
    }

    pub(super) fn pane_b_navigate(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(pane) = &mut self.dual_pane.pane_b {
            pane.path = path.clone();
            pane.is_loading = true;
            pane.entries.clear();
        }
        let list_config = self.state.config.list.clone();
        let filesystem_config = self.state.config.filesystem.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use crate::filesystem::{LocalFileSystem, ListOptions, EntryFilter, FileSystem, SortKey, SortOrder};
                    let fs = LocalFileSystem::from_config(filesystem_config);
                    let options = ListOptions {
                        show_hidden: list_config.show_hidden,
                        sort_by: SortKey::Name,
                        sort_order: SortOrder::Asc,
                        directories_first: true,
                        filter: EntryFilter::All,
                        name_query: None,
                        respect_gitignore: false,
                    };
                    let entries = fs.list_dir(&path, options).unwrap_or_default();
                    (path, entries)
                })
                .await
                .ok()
                .unwrap_or_else(|| (PathBuf::new(), Vec::new()))
            },
            |(path, entries)| UiMessage::PaneBLoaded { path, entries },
        )
    }

    /// Returns true if the file extension suggests a text/code file suitable for preview.
    pub(super) fn is_text_previewable(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "txt" | "md" | "log" | "nfo" | "readme"
                | "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp"
                | "cs" | "java" | "go" | "rb" | "php" | "swift" | "kt" | "lua" | "zig"
                | "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd"
                | "html" | "htm" | "css" | "scss" | "sass" | "less"
                | "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "cfg" | "conf"
                | "env" | "properties" | "csv" | "sql"
                | "gitignore" | "gitmodules" | "gitattributes"
                | "dockerfile" | "makefile" | "cmake"
                | "r" | "dart" | "scala" | "vue" | "svelte"
                | "pdf"
        )
    }
}
