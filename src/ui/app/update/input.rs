//! Pointer, keyboard, selection, context menus, resizing and drops.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

use iced::{Point, Task, keyboard};

use crate::filesystem::{FileOperationKind, LocalFileOperations};
use crate::ui::{KeyboardCommand, SelectionKind, UiMessage};

use crate::ui::theme::layout::{
    DRAG_START_THRESHOLD, PREVIEW_MAX_WIDTH, PREVIEW_MIN_WIDTH, TERMINAL_MAX_HEIGHT,
    TERMINAL_MIN_HEIGHT, TREE_MAX_HEIGHT, TREE_MIN_HEIGHT,
};
use crate::ui::theme::timing::DOUBLE_CLICK_THRESHOLD;

use crate::ui::app::helpers::command_from_key_press_with_shortcuts;
use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_input(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::Noop => {}
            UiMessage::ExitRequested => {
                // Les shells partent avec le programme. Pas par le cimetière :
                // l'application s'arrête, il n'y aura pas de tour suivant pour
                // le vider, et le processus meurt avant qu'un flux ne s'en
                // aperçoive.
                for tab in &mut self.terminal.tabs {
                    tab.terminal = None;
                }
                return Ok(Flow::Stop(iced::exit()));
            }
            UiMessage::CursorMoved(position) => {
                self.cursor_position = Some(position);
                // Feature H: column resize tracking
                if let Some(ref resize) = self.column_resize_state {
                    let delta = position.x - resize.start_x;
                    let new_width = (resize.start_width + delta).max(30.0);
                    if let Some(width) = self.state.config.column_widths.get_mut(resize.column) {
                        *width = new_width;
                    } else {
                        self.state
                            .config
                            .column_widths
                            .insert(resize.column.to_string(), new_width);
                    }
                }
                if self.pane_resize.tree_resizing
                    && let Some((start_y, start_height)) = self.pane_resize.tree_resize_anchor
                {
                    let next_height = start_height + (position.y - start_y);
                    self.pane_resize.tree_height =
                        next_height.clamp(TREE_MIN_HEIGHT, TREE_MAX_HEIGHT);
                    self.scroll.tree_height = self.pane_resize.tree_height.max(1.0);
                }
                if self.pane_resize.terminal_resizing
                    && let Some((start_y, start_height)) = self.pane_resize.terminal_resize_anchor
                {
                    // La poignée est en haut du panneau : le tirer vers le
                    // haut l'agrandit, donc la hauteur croît quand y décroît.
                    let delta = position.y - start_y;
                    self.pane_resize.terminal_height =
                        (start_height - delta).clamp(TERMINAL_MIN_HEIGHT, TERMINAL_MAX_HEIGHT);
                }
                if self.pane_resize.preview_resizing
                    && let Some((start_x, start_width)) = self.pane_resize.preview_resize_anchor
                {
                    let delta = position.x - start_x;
                    let next_width =
                        (start_width - delta).clamp(PREVIEW_MIN_WIDTH, PREVIEW_MAX_WIDTH);
                    self.pane_resize.preview_width = next_width;
                }
                if self.selection_box_start.is_some() && self.drag_candidate.is_none() {
                    self.begin_user_selection();
                    let content_point = self
                        .list_content_point(position, true)
                        .unwrap_or(Point::new(position.x, position.y + self.scroll.offset));
                    self.selection_box_current = Some(content_point);
                    let selected = self.entries_in_selection_box();
                    self.apply_box_selection(selected, self.selection_kind_from_modifiers());
                }
                if self.mouse_pressed && self.drag_state.is_none() {
                    if self.drag_start_position.is_none() && self.drag_candidate.is_some() {
                        self.drag_start_position = Some(position);
                    }
                    if let (Some(candidate), Some(start_pos)) =
                        (self.drag_candidate.clone(), self.drag_start_position)
                    {
                        let dx = position.x - start_pos.x;
                        let dy = position.y - start_pos.y;
                        if dx * dx + dy * dy >= DRAG_START_THRESHOLD * DRAG_START_THRESHOLD {
                            self.begin_user_selection();
                            if !self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&candidate)
                            {
                                self.last_click_time = None;
                                self.last_clicked_path = None;
                                self.apply_selection(candidate.clone(), SelectionKind::Single);
                            }
                            let items = if self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&candidate)
                            {
                                self.state
                                    .navigation
                                    .selection
                                    .selected
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                            } else {
                                vec![candidate.clone()]
                            };
                            self.drag_state = Some(DragState { items });
                        }
                    }
                }
            }
            UiMessage::EntryPressed(path) => {
                self.begin_user_selection();
                self.mouse_pressed = true;
                self.drag_candidate = Some(path);
                self.drag_start_position = self.cursor_position;
                self.selection_box_start = None;
                self.selection_box_current = None;
            }
            UiMessage::ListBackgroundPressed => {
                if self.drag_state.is_some() || self.drag_candidate.is_some() {
                    return Ok(Flow::Stop(Task::none()));
                }
                self.begin_user_selection();
                self.mouse_pressed = true;
                self.drag_candidate = None;
                self.drag_start_position = None;
                if let Some(position) = self.cursor_position {
                    // Try proper coordinate mapping first; if viewport bounds
                    // aren't captured yet (no scroll event has fired), use the
                    // cursor position directly with zero scroll offset so that
                    // rubber band selection works even before first scroll.
                    let content_point = self
                        .list_content_point(position, false)
                        .unwrap_or(Point::new(position.x, position.y + self.scroll.offset));
                    self.selection_box_start = Some(content_point);
                    self.selection_box_current = Some(content_point);
                }
            }
            UiMessage::SelectEntry { path, kind } => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                let now = Instant::now();
                if kind != SelectionKind::Single {
                    self.last_click_time = None;
                    self.last_clicked_path = None;
                    self.apply_selection(path, kind);
                } else {
                    let is_double_click = self
                        .last_clicked_path
                        .as_ref()
                        .is_some_and(|last_path| last_path == &path)
                        && self.last_click_time.is_some_and(|last_click| {
                            now.duration_since(last_click) <= DOUBLE_CLICK_THRESHOLD
                        });
                    self.apply_selection(path.clone(), kind);
                    if is_double_click {
                        self.last_click_time = None;
                        self.last_clicked_path = None;
                        tasks.push(self.activate_entry(path));
                    } else {
                        self.last_click_time = Some(now);
                        self.last_clicked_path = Some(path);
                    }
                }
            }
            UiMessage::ActivateEntry(path) => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                tasks.push(self.activate_entry(path));
            }
            UiMessage::RawKeyPressed { key, modifiers } => {
                // #25: Tab autocomplete in terminal
                if self.terminal_anim_target > 0.5
                    && matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
                    && !modifiers.control()
                    && !modifiers.alt()
                {
                    tasks.push(self.update(UiMessage::TerminalAutoComplete));
                    return Ok(Flow::Stop(Task::batch(std::mem::take(tasks))));
                }
                if let Some(command) = command_from_key_press_with_shortcuts(
                    &self.state.config.shortcuts,
                    key,
                    modifiers,
                ) {
                    // Block bare-key quick filter when a text input has focus
                    let has_text_focus = self.address_editing
                        || self.terminal_anim_target > 0.5
                        || self.rename_dialog.is_some()
                        || self.bulk_rename.is_some()
                        || self.grep_state.is_some()
                        || self.quick_filter_active
                        || !self.search.input.is_empty();
                    if has_text_focus && matches!(command, KeyboardCommand::QuickFilterChanged(_)) {
                        // Swallow: let the text_input widget handle the keystroke
                    } else if self.breadcrumb_dropdown.is_some()
                        && matches!(command, KeyboardCommand::ClearSelection)
                    {
                        self.breadcrumb_dropdown = None;
                        self.breadcrumb_dropdown_items.clear();
                    } else if self.address_editing
                        && matches!(command, KeyboardCommand::ClearSelection)
                    {
                        self.address_editing = false;
                        self.address_input = self.state.route.address_label();
                    } else if self.quick_filter_active
                        && matches!(command, KeyboardCommand::ClearSelection)
                    {
                        self.quick_filter.clear();
                        self.quick_filter_active = false;
                    } else {
                        tasks.push(self.handle_keyboard_command(command));
                    }
                }
            }
            UiMessage::KeyboardCommand(command) => {
                // Escape while breadcrumb dropdown is open closes it
                if self.breadcrumb_dropdown.is_some()
                    && matches!(command, KeyboardCommand::ClearSelection)
                {
                    self.breadcrumb_dropdown = None;
                    self.breadcrumb_dropdown_items.clear();
                // Escape while editing the address bar cancels editing
                } else if self.address_editing && matches!(command, KeyboardCommand::ClearSelection)
                {
                    self.address_editing = false;
                    self.address_input = self.state.route.address_label();
                } else if self.quick_filter_active
                    && matches!(command, KeyboardCommand::ClearSelection)
                {
                    self.quick_filter.clear();
                    self.quick_filter_active = false;
                } else {
                    tasks.push(self.handle_keyboard_command(command));
                }
            }
            UiMessage::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            UiMessage::ToggleContextMenu(force_open) => {
                self.menus.context_open = force_open;
                self.menus.background_context_open = false;
                self.menus.context_submenu = None;
                if force_open {
                    self.menus.context_position = self.cursor_position;
                } else {
                    self.menus.context_position = None;
                }
                self.menus.history_open = false;
                self.menus.history_position = None;
            }
            UiMessage::ToggleHistoryMenu(force_open) => {
                self.menus.history_open = force_open;
                if force_open {
                    self.menus.history_position = self.cursor_position;
                } else {
                    self.menus.history_position = None;
                }
            }
            UiMessage::OpenContextMenuForEntry(path) => {
                let is_selected = self.state.navigation.selection.selected.contains(&path);
                if !is_selected {
                    self.apply_selection(path, SelectionKind::Single);
                }
                self.menus.context_open = true;
                self.menus.context_submenu = None;
                self.menus.context_position = self.cursor_position;
                self.menus.history_open = false;
                self.menus.history_position = None;
            }
            UiMessage::BackgroundContextMenu => {
                self.clear_selection();
                self.menus.background_context_open = true;
                self.menus.context_position = self.cursor_position;
                self.menus.context_open = false;
                self.menus.context_submenu = None;
                self.menus.history_open = false;
                self.menus.history_position = None;
            }
            UiMessage::ToggleContextSubmenu(id) => {
                use crate::ui::app::types::ContextSubmenu;
                let target = match id {
                    0 => ContextSubmenu::Compress,
                    _ => ContextSubmenu::Label,
                };
                // Always open on hover/click (not toggle) — closing is handled
                // by hovering a different area or dismissing the menu
                self.menus.context_submenu = Some(target);
            }
            UiMessage::CloseContextSubmenu => {
                self.menus.context_submenu = None;
            }
            UiMessage::ContextAction(action) => {
                self.menus.background_context_open = false;
                tasks.push(self.apply_context_action(action));
            }
            UiMessage::TreeResizeStart => {
                self.pane_resize.tree_resizing = true;
                self.pane_resize.tree_resize_anchor = self
                    .cursor_position
                    .map(|position| (position.y, self.pane_resize.tree_height));
            }
            UiMessage::TreeResizeEnd => {
                self.pane_resize.tree_resizing = false;
                self.pane_resize.tree_resize_anchor = None;
            }
            UiMessage::TerminalResizeStart => {
                self.pane_resize.terminal_resizing = true;
                self.pane_resize.terminal_resize_anchor = self
                    .cursor_position
                    .map(|position| (position.y, self.pane_resize.terminal_height));
            }
            UiMessage::TerminalResizeEnd => {
                self.pane_resize.terminal_resizing = false;
                self.pane_resize.terminal_resize_anchor = None;
            }
            UiMessage::PreviewResizeStart => {
                self.pane_resize.preview_resizing = true;
                self.pane_resize.preview_resize_anchor = self
                    .cursor_position
                    .map(|position| (position.x, self.pane_resize.preview_width));
            }
            UiMessage::PreviewResizeEnd => {
                self.pane_resize.preview_resizing = false;
                self.pane_resize.preview_resize_anchor = None;
            }
            UiMessage::PreviewDoubleClick => {
                // Toggle between min and max width
                let mid = (PREVIEW_MIN_WIDTH + PREVIEW_MAX_WIDTH) / 2.0;
                if self.pane_resize.preview_width < mid {
                    self.pane_resize.preview_width = PREVIEW_MAX_WIDTH;
                } else {
                    self.pane_resize.preview_width = PREVIEW_MIN_WIDTH;
                }
            }
            UiMessage::MouseReleased => {
                // Feature H: release column resize
                if self.column_resize_state.is_some() {
                    self.column_resize_state = None;
                    self.config_manager.save(&self.state.config);
                }
                if self.pane_resize.tree_resizing {
                    self.pane_resize.tree_resizing = false;
                    self.pane_resize.tree_resize_anchor = None;
                }
                if self.pane_resize.terminal_resizing {
                    self.pane_resize.terminal_resizing = false;
                    self.pane_resize.terminal_resize_anchor = None;
                }
                if self.pane_resize.preview_resizing {
                    self.pane_resize.preview_resizing = false;
                    self.pane_resize.preview_resize_anchor = None;
                }
                self.mouse_pressed = false;
                self.drag_candidate = None;
                self.drag_start_position = None;
                // The rubber-band result must be computed BEFORE the snapshot is
                // released: `entries_in_selection_box` goes through
                // `display_entries()`, which only returns the frozen buffer while
                // `is_user_selecting` is set. Clearing both first meant the
                // rectangle was resolved against the live list, so a refresh
                // landing mid-drag selected the wrong rows.
                if self.selection_box_start.is_some() {
                    let kind = self.selection_kind_from_modifiers();
                    let selected = self.entries_in_selection_box();
                    self.apply_box_selection(selected, kind);
                    self.selection_box_start = None;
                    self.selection_box_current = None;
                }
                self.is_user_selecting = false;
                self.selection_snapshot = None;
                // A drop that never produced a navigation used to leave this
                // armed forever, swallowing the next unrelated navigation.
                self.ignore_next_navigation = None;

                // Replay whatever arrived while the button was down.
                for deferred in std::mem::take(&mut self.deferred_messages) {
                    tasks.push(Task::done(deferred));
                }
                if self.drag_state.is_some() {
                    tasks.push(Task::perform(async {}, |_| UiMessage::FinalizeDrag));
                }
                if self.pending_refresh {
                    let reload_config = self.pending_refresh_reload_config;
                    self.pending_refresh = false;
                    self.pending_refresh_reload_config = false;
                    if reload_config {
                        tasks.push(self.reload_config());
                    }
                    tasks.push(self.refresh_entries_in_place());
                }
            }
            UiMessage::FinalizeDrag => {
                self.drag_state = None;
            }
            UiMessage::ColumnResizeStart(column) => {
                let current_width = self
                    .state
                    .config
                    .column_widths
                    .get(column)
                    .copied()
                    .unwrap_or(150.0);
                let start_x = self.cursor_position.map(|p| p.x).unwrap_or(0.0);
                self.column_resize_state = Some(ColumnResizeState {
                    column,
                    start_x,
                    start_width: current_width,
                });
            }
            UiMessage::ColumnResizeEnd => {
                self.column_resize_state = None;
                self.config_manager.save(&self.state.config);
            }
            UiMessage::ColumnResized(column, width) => {
                self.state
                    .config
                    .column_widths
                    .insert(column.to_string(), width.max(30.0));
            }
            // ── Feature J: Shell Switcher ─────────────────────────────────────
            UiMessage::DropOnPath(path) => {
                if let Some(drag_state) = self.drag_state.take() {
                    let destination = path.clone();
                    let items = drag_state.items;
                    let is_copy = self.modifiers.control;
                    let action_label = if is_copy { "Copie" } else { "Déplacement" };
                    self.last_action = Some(format!(
                        "{} (glisser-déposer) vers {}",
                        action_label,
                        destination.display()
                    ));
                    self.ignore_next_navigation = Some(destination.clone());
                    let operation = if is_copy {
                        FileOperationKind::Copy
                    } else {
                        FileOperationKind::Move
                    };
                    let total = items.len();
                    let counter = Arc::new(AtomicUsize::new(0));
                    self.operation_progress = Some(FileOpProgress {
                        counter: counter.clone(),
                        total,
                        kind: operation,
                    });
                    // Stash undo context for drag-and-drop
                    let operation_id = self.next_operation_id();
                    self.pending_undo_context.insert(
                        operation_id,
                        if is_copy {
                            PendingUndoContext::Copy {
                                sources: items.clone(),
                                destination: destination.clone(),
                            }
                        } else {
                            PendingUndoContext::Move {
                                sources: items.clone(),
                                destination: destination.clone(),
                            }
                        },
                    );
                    tasks.push(Task::perform(
                        async move {
                            // Blocking work: this is a real tree copy, not a
                            // handful of syscalls.
                            tokio::task::spawn_blocking(move || {
                                let operations = LocalFileOperations::new();
                                if matches!(operation, FileOperationKind::Copy) {
                                    operations.copy_items(&items, &destination, Some(counter))
                                } else {
                                    operations.move_items(&items, &destination, Some(counter))
                                }
                            })
                            .await
                            .unwrap_or_else(|error| {
                                tracing::error!("Tâche de dépôt interrompue : {error}");
                                crate::filesystem::OperationReport::empty(operation)
                            })
                        },
                        move |report| UiMessage::FileOperationFinished(operation_id, report),
                    ));
                }
            }
            // Feature Q: Undo
            UiMessage::ExternalFileHovered(path) => {
                self.last_action = Some(format!("Fichier survolé : {}", path.display()));
            }
            UiMessage::ExternalFileDropped(path) => {
                // Copy the dropped file/folder into the current directory
                if let Some(current_dir) = self.state.route.local_path().cloned() {
                    let file_name = path.file_name().unwrap_or_default().to_os_string();
                    let source = path.clone();
                    let external_drop_id = self.next_operation_id();
                    self.last_action =
                        Some(format!("Copie de {} ...", file_name.to_string_lossy()));
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                crate::filesystem::LocalFileOperations::new().copy_items(
                                    &[source],
                                    &current_dir,
                                    None,
                                )
                            })
                            .await
                            .unwrap_or_else(|e| {
                                crate::filesystem::OperationReport {
                                    action: crate::filesystem::FileOperationKind::Copy,
                                    succeeded: Vec::new(),
                                    failed: vec![crate::filesystem::OperationFailure {
                                        path: std::path::PathBuf::new(),
                                        error: e.to_string(),
                                    }],
                                }
                            })
                        },
                        move |report| UiMessage::FileOperationFinished(external_drop_id, report),
                    ));
                } else {
                    self.last_action = Some("Drop non supporté ici".to_string());
                }
            }
            UiMessage::ExternalFileCancelled => {
                self.last_action = None;
            }
            UiMessage::FileWatchTick => {
                let events = self.poll_watcher();
                let should_refresh = match (self.watched_path.as_deref(), &events) {
                    (Some(watched_path), Some(events)) => {
                        let relevant = events
                            .iter()
                            .any(|event| Self::is_event_relevant(event, watched_path));
                        if relevant {
                            // Invalidate directory cache so refresh reads fresh data from disk
                            if let Ok(mut loader) = self.directory_loader.lock() {
                                loader.invalidate(watched_path);
                            }
                        }
                        relevant
                    }
                    _ => false,
                };
                if should_refresh && !self.is_refreshing {
                    tasks.push(self.refresh_entries_in_place());
                }
            }
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
