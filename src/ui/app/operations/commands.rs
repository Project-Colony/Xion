//! Keyboard commands and context-menu actions: the two entry points that fan out to everything else.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;

use iced::Task;

use crate::ui::{ContextAction, KeyboardCommand, SelectionKind, UiMessage};

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    pub(in crate::ui::app) fn handle_keyboard_command(
        &mut self,
        command: KeyboardCommand,
    ) -> Task<UiMessage> {
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
                if self.history.can_back()
                    && let Some(path) = self.history.back()
                {
                    self.update_active_tab_path(path);
                    return self.refresh_entries();
                }
                Task::none()
            }
            KeyboardCommand::Forward => {
                if self.history.can_forward()
                    && let Some(path) = self.history.forward()
                {
                    self.update_active_tab_path(path);
                    return self.refresh_entries();
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
            // Delete moves to the trash: recoverable, so no prompt.
            KeyboardCommand::Delete => self.trash_selection(),
            // Shift+Delete is the irreversible one and always asks first.
            KeyboardCommand::DeletePermanently => self.request_permanent_delete(),
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
            KeyboardCommand::BulkRename => Task::done(UiMessage::OpenBulkRename),
            KeyboardCommand::ToggleDualPane => self.toggle_dual_pane(),
            KeyboardCommand::SwitchActivePane => {
                if self.dual_pane.enabled {
                    self.dual_pane.active = 1 - self.dual_pane.active;
                }
                Task::none()
            }
            KeyboardCommand::OpenDiff => Task::done(UiMessage::OpenDiff),
            KeyboardCommand::QuickFilterChanged(c) => Task::done(UiMessage::QuickFilterChanged(c)),
            KeyboardCommand::QuickFilterClear => Task::done(UiMessage::QuickFilterClear),
            KeyboardCommand::OpenGrep => Task::done(UiMessage::OpenGrep),
            KeyboardCommand::Undo => Task::done(UiMessage::Undo),
            KeyboardCommand::FocusAddress => {
                self.address_editing = true;
                iced::widget::operation::focus(iced::widget::Id::new("address_input"))
            }
            KeyboardCommand::GoToParent => {
                if let Some(path) = self.state.route.local_path().cloned()
                    && let Some(parent) = path.parent()
                {
                    return self.navigate_to(parent.to_path_buf());
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

    pub(in crate::ui::app) fn apply_context_action(
        &mut self,
        action: ContextAction,
    ) -> Task<UiMessage> {
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
            ContextAction::SetLabelRed
            | ContextAction::SetLabelOrange
            | ContextAction::SetLabelYellow
            | ContextAction::SetLabelGreen
            | ContextAction::SetLabelBlue
            | ContextAction::SetLabelPurple
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
            ContextAction::Delete => self.request_permanent_delete(),
            ContextAction::CopyPath => {
                self.copy_selection_path();
                Task::none()
            }
            ContextAction::OpenProperties => {
                let selection_count = self.state.navigation.selection.selected.len();
                if selection_count > 1 {
                    // Multi-selection properties
                    let paths: Vec<PathBuf> = self
                        .state
                        .navigation
                        .selection
                        .selected
                        .iter()
                        .cloned()
                        .collect();
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
                                paths
                                    .iter()
                                    .filter_map(|p| std::fs::metadata(p).ok())
                                    .map(|m| m.len())
                                    .sum::<u64>()
                            })
                            .await
                            .unwrap_or(0)
                        },
                        UiMessage::SelectionSizeComputed,
                    );
                }
                let focused = self.state.navigation.selection.focused.clone().or_else(|| {
                    self.state
                        .navigation
                        .selection
                        .selected
                        .iter()
                        .next()
                        .cloned()
                });
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenProperties(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenBulkRename => Task::done(UiMessage::OpenBulkRename),
            ContextAction::CompressToZip => Task::done(UiMessage::CompressToZip),
            ContextAction::CompressToTarGz => Task::done(UiMessage::CompressToTarGz),
            ContextAction::CompressTo7z => Task::done(UiMessage::CompressTo7z),
            ContextAction::OpenWith => {
                let focused = self.state.navigation.selection.focused.clone().or_else(|| {
                    self.state
                        .navigation
                        .selection
                        .selected
                        .iter()
                        .next()
                        .cloned()
                });
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenWith(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenDiff => Task::done(UiMessage::OpenDiff),
            ContextAction::OpenHexView => {
                let focused = self.state.navigation.selection.focused.clone().or_else(|| {
                    self.state
                        .navigation
                        .selection
                        .selected
                        .iter()
                        .next()
                        .cloned()
                });
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenHexView(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::OpenPermissions => {
                let focused = self.state.navigation.selection.focused.clone().or_else(|| {
                    self.state
                        .navigation
                        .selection
                        .selected
                        .iter()
                        .next()
                        .cloned()
                });
                if let Some(path) = focused {
                    Task::done(UiMessage::OpenPermissions(path))
                } else {
                    Task::none()
                }
            }
            ContextAction::SetLabelRed => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Red))
            }
            ContextAction::SetLabelOrange => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Orange))
            }
            ContextAction::SetLabelYellow => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Yellow))
            }
            ContextAction::SetLabelGreen => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Green))
            }
            ContextAction::SetLabelBlue => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Blue))
            }
            ContextAction::SetLabelPurple => {
                self.set_label_for_focused(Some(crate::ui::FileLabel::Purple))
            }
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

    pub(in crate::ui::app) fn set_label_for_focused(
        &mut self,
        label: Option<crate::ui::FileLabel>,
    ) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone().or_else(|| {
            self.state
                .navigation
                .selection
                .selected
                .iter()
                .next()
                .cloned()
        });
        if let Some(path) = focused {
            Task::done(UiMessage::SetLabel(path, label))
        } else {
            Task::none()
        }
    }
}
