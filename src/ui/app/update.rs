//! Message handling for XionApp.
//!
//! Contains the main `update()` method that processes all `UiMessage` variants,
//! and the `update_for_test()` wrapper for integration tests.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use iced::widget::image;
use iced::{keyboard, Point, Task};

use crate::core::{SortOrderConfig, ThemeConfig};
use crate::filesystem::{FileOperationKind, LocalFileOperations};
use crate::ui::{KeyboardCommand, SelectionKind, UiMessage};

use crate::ui::theme::layout::{
    DRAG_START_THRESHOLD, PREVIEW_MAX_WIDTH, PREVIEW_MIN_WIDTH, TREE_MAX_HEIGHT, TREE_MIN_HEIGHT,
};
use crate::ui::theme::timing::DOUBLE_CLICK_THRESHOLD;

use super::archive;
use super::helpers::{build_animated_preview, command_from_key_press_with_shortcuts, is_gif_preview};
use super::permissions;
use super::shell;
use super::types::*;
use super::{detect_archive_type, XionApp};

impl XionApp {
    /// Visible to integration tests; the Iced framework calls this without `pub`.
    #[doc(hidden)]
    pub fn update_for_test(&mut self, message: UiMessage) -> Task<UiMessage> {
        self.update(message)
    }

    pub(crate) fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        if self.is_user_selecting {
            match message {
                UiMessage::Refresh => {
                    self.pending_refresh = true;
                    self.pending_refresh_reload_config = true;
                    return Task::none();
                }
                UiMessage::FileWatchTick => {
                    let events = self.poll_watcher();
                    if let (Some(watched_path), Some(events)) =
                        (self.watched_path.as_deref(), events)
                    {
                        let should_refresh = events
                            .iter()
                            .any(|event| Self::is_event_relevant(event, watched_path));
                        if should_refresh && !self.is_refreshing {
                            self.pending_refresh = true;
                        }
                    }
                    return Task::none();
                }
                // Allow mouse tracking, modifier keys, navigation, and keyboard
                // commands through during selection so they are not silently dropped.
                UiMessage::MouseReleased
                | UiMessage::CursorMoved(_)
                | UiMessage::ModifiersChanged(_)
                | UiMessage::NavigateTo(_)
                | UiMessage::KeyboardCommand(_) => {}
                // All other messages are deferred until selection ends.
                _ => return Task::none(),
            }
        }
        match message {
            UiMessage::Noop => {}
            UiMessage::ExitRequested => {
                // Kill cmd.exe before exiting so no orphan processes remain.
                for tab in &mut self.terminal.tabs {
                    tab.process = None;
                }
                return iced::exit();
            }
            UiMessage::CursorMoved(position) => {
                self.cursor_position = Some(position);
                // Feature H: column resize tracking
                if let Some(ref resize) = self.column_resize_state {
                    let delta = position.x - resize.start_x;
                    let new_width = (resize.start_width + delta).max(30.0);
                    if let Some(width) = self.state.config.column_widths.get_mut(&resize.column) {
                        *width = new_width;
                    } else {
                        let column = resize.column.clone();
                        self.state.config.column_widths.insert(column, new_width);
                    }
                }
                if self.pane_resize.tree_resizing {
                    if let Some((start_y, start_height)) = self.pane_resize.tree_resize_anchor {
                        let next_height = start_height + (position.y - start_y);
                        self.pane_resize.tree_height = next_height.clamp(TREE_MIN_HEIGHT, TREE_MAX_HEIGHT);
                        self.scroll.tree_height = self.pane_resize.tree_height.max(1.0);
                    }
                }
                if self.pane_resize.preview_resizing {
                    if let Some((start_x, start_width)) = self.pane_resize.preview_resize_anchor {
                        let delta = position.x - start_x;
                        let next_width =
                            (start_width - delta).clamp(PREVIEW_MIN_WIDTH, PREVIEW_MAX_WIDTH);
                        self.pane_resize.preview_width = next_width;
                    }
                }
                if self.selection_box_start.is_some() && self.drag_candidate.is_none() {
                    self.begin_user_selection();
                    let content_point = self.list_content_point(position, true)
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
            UiMessage::NavigateTo(path) => {
                if self
                    .ignore_next_navigation
                    .as_ref()
                    .is_some_and(|ignore| ignore == &path)
                {
                    self.ignore_next_navigation = None;
                    return Task::none();
                }
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                self.breadcrumb_dropdown = None;
                self.breadcrumb_dropdown_items.clear();
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddTab => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                tasks.push(self.add_tab());
            }
            UiMessage::SwitchTab(index) => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                tasks.push(self.switch_tab(index));
            }
            UiMessage::CloseTab(index) => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                tasks.push(self.close_tab(index));
            }
            UiMessage::Back => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                if let Some(path) = self.history.back() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Forward => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                if let Some(path) = self.history.forward() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Refresh => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                tasks.push(self.reload_config());
                tasks.push(self.refresh_entries());
            }
            UiMessage::FocusPane(pane) => {
                self.state.navigation.focused_pane = pane;
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
                    return Task::none();
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
                    let content_point = self.list_content_point(position, false)
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
                    return Task::batch(tasks);
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
                    } else if self.breadcrumb_dropdown.is_some() && matches!(command, KeyboardCommand::ClearSelection) {
                        self.breadcrumb_dropdown = None;
                        self.breadcrumb_dropdown_items.clear();
                    } else if self.address_editing && matches!(command, KeyboardCommand::ClearSelection) {
                        self.address_editing = false;
                        self.address_input = self.state.route.address_label();
                    } else if self.quick_filter_active && matches!(command, KeyboardCommand::ClearSelection) {
                        self.quick_filter.clear();
                        self.quick_filter_active = false;
                    } else {
                        tasks.push(self.handle_keyboard_command(command));
                    }
                }
            }
            UiMessage::KeyboardCommand(command) => {
                // Escape while breadcrumb dropdown is open closes it
                if self.breadcrumb_dropdown.is_some() && matches!(command, KeyboardCommand::ClearSelection) {
                    self.breadcrumb_dropdown = None;
                    self.breadcrumb_dropdown_items.clear();
                // Escape while editing the address bar cancels editing
                } else if self.address_editing && matches!(command, KeyboardCommand::ClearSelection) {
                    self.address_editing = false;
                    self.address_input = self.state.route.address_label();
                } else if self.quick_filter_active && matches!(command, KeyboardCommand::ClearSelection) {
                    self.quick_filter.clear();
                    self.quick_filter_active = false;
                } else {
                    tasks.push(self.handle_keyboard_command(command));
                }
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
                use super::types::ContextSubmenu;
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
            UiMessage::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            UiMessage::AddressInputChanged(value) => {
                self.address_input = value;
            }
            UiMessage::AddressSuggestionSelected(path) => {
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_input = path.display().to_string();
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddressInputSubmitted => {
                self.address_editing = false;
                self.menus.history_open = false;
                self.menus.history_position = None;
                if let Some(target) = self.address_target_from_input() {
                    if is_network_path(&target) || target.is_dir() {
                        tasks.push(self.navigate_to(target));
                    } else if target.exists() {
                        self.last_action = Some(format!(
                            "Le chemin pointe vers un fichier : {}",
                            target.display()
                        ));
                    } else {
                        self.last_action =
                            Some(format!("Chemin introuvable : {}", target.display()));
                    }
                }
            }
            UiMessage::SearchInputChanged(value) => {
                self.search.input = value;
                self.scroll.offset = 0.0;
                self.clear_selection();
                self.update_search_index_matches();
                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                } else {
                    tasks.push(self.ensure_visible_pages());
                }
            }
            UiMessage::SearchInputSubmitted => {
                // #23: Ctrl held = full-text content search
                if self.modifiers.control {
                    tasks.push(self.update(UiMessage::FullTextSearchSubmit));
                } else {
                    self.update_search_index_matches();
                    if self.normalized_search_query().is_some() {
                        tasks.push(self.request_all_pages());
                    }
                }
            }
            UiMessage::Scroll(viewport) => {
                self.scroll.offset = viewport.offset_y;
                self.scroll.height = viewport.viewport_height.max(1.0);
                self.scroll.content_height = viewport.content_height;
                self.list_viewport_bounds = Some(viewport.bounds);
                tasks.push(self.ensure_visible_pages());
            }
            UiMessage::TreeScroll(viewport) => {
                self.scroll.tree_offset = viewport.offset_y;
                self.scroll.tree_height = viewport.viewport_height.max(1.0);
            }
            UiMessage::ChangeSort(sort_key) => {
                if self.state.config.list.sort_key == sort_key {
                    self.state.config.list.sort_order = match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => SortOrderConfig::Desc,
                        SortOrderConfig::Desc => SortOrderConfig::Asc,
                    };
                } else {
                    self.state.config.list.sort_key = sort_key;
                    self.state.config.list.sort_order = SortOrderConfig::Asc;
                }
                self.last_action = Some(format!(
                    "Tri : {:?} ({:?})",
                    self.state.config.list.sort_key, self.state.config.list.sort_order
                ));
                tasks.push(self.refresh_entries());
            }
            UiMessage::ToggleViewMode => {
                self.state.config.view.mode = self.state.config.view.mode.toggle();
                self.scroll.offset = 0.0;
                self.clear_selection();
                tasks.push(self.ensure_visible_pages());
                tasks.push(self.request_visible_thumbnails());
            }
            UiMessage::LoadingDelayElapsed(generation) => {
                if self.is_refreshing
                    && self.is_loading
                    && generation == self.loading_generation
                    && self.entries.total == 0
                {
                    self.show_loading_indicator = true;
                }
            }
            UiMessage::PageLoaded {
                path,
                page_index,
                result,
            } => {
                if path != self.state.route.key() {
                    return Task::batch(tasks);
                }

                self.pending_pages.remove(&page_index);
                match result {
                    Ok(page) => {
                        self.entries.apply_page(page_index, page);
                        self.error = None;
                        if self.is_refreshing {
                            self.stale_entries = None;
                        }
                        tasks.push(self.ensure_visible_pages());
                    }
                    Err(message) => {
                        self.error = Some(message);
                    }
                }

                if self.pending_pages.is_empty() {
                    self.is_loading = false;
                    self.is_refreshing = false;
                    self.show_loading_indicator = false;
                }

                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                }
                // Feature 8: Load dir sizes after page load (only when Size column is visible)
                if self.state.config.view.columns.contains(&crate::core::config::types::ViewColumn::Size) {
                    tasks.push(self.request_dir_sizes());
                }
            }
            UiMessage::ThumbnailLoaded { path, thumbnail } => {
                self.media.thumbnails_in_flight.remove(&path);
                match thumbnail {
                    Some(thumbnail) => {
                        // Create handle from bytes before moving into cache
                        let handle = image::Handle::from_bytes(thumbnail.bytes.clone());
                        self.media.thumbnail_handles.insert(path.clone(), handle);
                        self.media.thumbnails.insert(path.clone(), thumbnail);
                        self.media.thumbnail_misses.remove(&path);
                    }
                    None => {
                        self.media.thumbnail_misses.insert(path);
                    }
                }
            }
            UiMessage::PreviewLoaded { path, preview } => {
                self.media.previews_in_flight.remove(&path);
                let is_selected = self
                    .state
                    .navigation
                    .selection
                    .focused
                    .as_ref()
                    .is_some_and(|focused| focused == &path)
                    || self.state.navigation.selection.selected.contains(&path);
                if is_selected {
                    match preview {
                        Some(preview) => {
                            if is_gif_preview(&preview) {
                                self.media.preview_handles.remove(&path);
                                self.media.previews.remove(&path);
                                if let Some(animated) =
                                    build_animated_preview(path.clone(), &preview)
                                {
                                    self.media.animated = Some(animated);
                                    self.media.preview_misses.remove(&path);
                                } else {
                                    self.media.preview_misses.insert(path);
                                }
                            } else {
                                let handle = image::Handle::from_bytes(preview.bytes.clone());
                                self.media.preview_handles.insert(path.clone(), handle);
                                self.media.previews.insert(path.clone(), preview);
                                self.media.preview_misses.remove(&path);
                            }
                        }
                        None => {
                            self.media.preview_misses.insert(path);
                        }
                    }
                }
            }
            UiMessage::SearchIndexBuilt { path, result } => {
                if self.search.index_path.as_ref() == Some(&path) {
                    match result {
                        Ok(index) => {
                            // #10: Cache the search index for LRU reuse
                            const MAX_SEARCH_CACHE: usize = 8;
                            self.search_index_cache.retain(|(p, _)| p != &path);
                            if self.search_index_cache.len() >= MAX_SEARCH_CACHE {
                                self.search_index_cache.pop_front();
                            }
                            self.search_index_cache.push_back((path.clone(), index.clone()));
                            self.search.index = Some(index);
                            self.search.indexing = false;
                            self.update_search_index_matches();
                            self.last_action = Some("Indexation terminée".to_string());
                        }
                        Err(error) => {
                            self.search.index = None;
                            self.search.indexing = false;
                            self.search.matches = None;
                            self.last_action = Some(format!(
                                "Indexation impossible pour {} ({})",
                                path.display(),
                                error
                            ));
                        }
                    }
                }
            }
            UiMessage::AnimatedPreviewTick(now) => {
                self.advance_animated_preview(now);
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
                    tasks.push(self.refresh_entries());
                }
            }
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
                if self.pane_resize.preview_resizing {
                    self.pane_resize.preview_resizing = false;
                    self.pane_resize.preview_resize_anchor = None;
                }
                self.mouse_pressed = false;
                self.drag_candidate = None;
                self.drag_start_position = None;
                self.is_user_selecting = false;
                self.selection_snapshot = None;
                if self.selection_box_start.is_some() {
                    let kind = self.selection_kind_from_modifiers();
                    let selected = self.entries_in_selection_box();
                    self.apply_box_selection(selected, kind);
                    self.selection_box_start = None;
                    self.selection_box_current = None;
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
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::FinalizeDrag => {
                self.drag_state = None;
            }
            UiMessage::FileOperationFinished(report) => {
                self.operation_progress = None;
                // Record undo action based on pending context
                if let Some(ctx) = self.pending_undo_context.take() {
                    if !report.succeeded.is_empty() {
                        match ctx {
                            PendingUndoContext::Copy { sources, destination } => {
                                let created: Vec<PathBuf> = sources.iter()
                                    .filter_map(|s| s.file_name().map(|n| destination.join(n)))
                                    .filter(|p| report.succeeded.contains(p))
                                    .collect();
                                if !created.is_empty() {
                                    self.undo_stack.push(UndoAction::Copy { created });
                                }
                            }
                            PendingUndoContext::Move { sources, destination } => {
                                let pairs: Vec<(PathBuf, PathBuf)> = sources.iter()
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
                tasks.push(self.refresh_entries());
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
            UiMessage::NewFolderCreated(result) => {
                match result {
                    Ok(path) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        self.last_action = Some(format!("Dossier créé : {name}"));
                        self.undo_stack.push(UndoAction::FolderCreated { path: path.clone() });
                        tasks.push(self.refresh_entries());
                        self.rename_dialog = Some(RenameDialog {
                            path,
                            input: name,
                        });
                    }
                    Err(error) => {
                        self.last_action = Some(format!("Erreur création dossier : {error}"));
                    }
                }
            }
            UiMessage::NewFile => {
                tasks.push(self.create_new_file());
            }
            UiMessage::NewFileCreated(result) => {
                match result {
                    Ok(path) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        self.last_action = Some(format!("Fichier créé : {name}"));
                        self.undo_stack.push(UndoAction::FileCreated { path: path.clone() });
                        tasks.push(self.refresh_entries());
                        self.rename_dialog = Some(RenameDialog {
                            path,
                            input: name,
                        });
                    }
                    Err(error) => {
                        self.last_action = Some(format!("Erreur création fichier : {error}"));
                    }
                }
            }
            UiMessage::AddressEditStart => {
                self.address_editing = true;
            }
            UiMessage::AddressEditCancel => {
                self.address_editing = false;
                self.address_input = self.state.route.address_label();
            }
            UiMessage::ToggleDarkMode => {
                // Cycle through themes
                let next_theme = match self.state.config.theme {
                    ThemeConfig::Light => ThemeConfig::Dark,
                    ThemeConfig::Dark => ThemeConfig::Nord,
                    ThemeConfig::Nord => ThemeConfig::Solarized,
                    ThemeConfig::Solarized => ThemeConfig::HighContrast,
                    ThemeConfig::HighContrast => ThemeConfig::Light,
                };
                self.state.config.dark_mode = matches!(next_theme, ThemeConfig::Dark | ThemeConfig::Nord | ThemeConfig::Solarized | ThemeConfig::HighContrast);
                self.state.config.theme = next_theme;
                self.config_manager.save(&self.state.config);
            }
            UiMessage::TextPreviewLoaded { path, content, encoding } => {
                self.preview_encoding = encoding;
                // Feature 6: spawn syntax highlighting
                let dark = self.state.config.dark_mode;
                let hl_path = path.clone();
                let hl_content = content.clone();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::services::highlight::highlight_text(&hl_path, &hl_content, dark)
                                .map(|lines| (hl_path, lines))
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |result| match result {
                        Some((path, lines)) => UiMessage::TextHighlightComplete { path, lines },
                        None => UiMessage::Noop,
                    },
                ));
                self.cached_text_preview = Some((path, content));
            }
            UiMessage::PreviewAnimTick => {
                // Ease-out interpolation: fast start, smooth stop
                let speed = 0.15;
                let diff = self.preview_anim_target - self.preview_anim_progress;
                if diff.abs() < 0.005 {
                    self.preview_anim_progress = self.preview_anim_target;
                } else {
                    self.preview_anim_progress += diff * speed;
                }
            }
            UiMessage::ToggleTerminal => {
                if self.terminal_anim_target > 0.5 {
                    // Close: drop processes on all tabs
                    for tab in &mut self.terminal.tabs {
                        tab.process = None;
                    }
                    self.terminal_anim_target = 0.0;
                } else {
                    // Open: spawn a persistent cmd.exe session
                    self.terminal_anim_target = 1.0;
                    let cwd = self
                        .state
                        .route
                        .local_path()
                        .cloned()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    self.terminal.active().cwd = Some(cwd.clone());
                    let shell = self.state.config.terminal_shell.clone();
                    tasks.push(Task::perform(
                        async move {
                            shell::spawn_shell_process(&shell, &cwd)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        UiMessage::TerminalSpawned,
                    ));
                }
            }
            UiMessage::TerminalSpawned(result) => match result {
                Ok(process) => {
                    self.terminal.active().process = Some(process);
                    self.terminal.active().lines.push("Terminal prêt.".to_string());
                }
                Err(e) => {
                    self.terminal.active().lines.push(format!("Erreur démarrage terminal : {e}"));
                    self.terminal_anim_target = 0.0;
                }
            },
            UiMessage::TerminalInputChanged(input) => {
                self.terminal.active().input = input;
            }
            UiMessage::TerminalInputSubmitted => {
                let cmd = self.terminal.active_ref().input.trim().to_string();
                if cmd.is_empty() {
                    return Task::batch(tasks);
                }
                let fallback = self
                    .state
                    .route
                    .local_path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let cwd = self.terminal.effective_cwd(&fallback).to_path_buf();
                self.terminal.push_prompt(&cwd, &cmd);
                self.terminal.apply_cd(&cmd, &cwd);
                self.terminal.active().input.clear();
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    tasks.push(Task::perform(
                        async move { process.write_line(&cmd).await },
                        |_| UiMessage::Noop,
                    ));
                } else {
                    self.terminal.active().lines.push("Erreur : terminal non démarré.".to_string());
                }
            }
            UiMessage::TerminalPollOutput => {
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    let lines = process.poll();
                    if !lines.is_empty() {
                        self.terminal.push_lines(lines);
                        // Auto-scroll terminal to bottom
                        tasks.push(iced::widget::operation::scroll_to(
                            iced::widget::Id::new("terminal_output"),
                            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: f32::MAX },
                        ));
                    }
                }
            }
            UiMessage::TerminalAddTab => {
                const MAX_TERMINAL_TABS: usize = 10;
                if self.terminal.tabs.len() >= MAX_TERMINAL_TABS {
                    self.last_action = Some(format!("Maximum {MAX_TERMINAL_TABS} onglets terminal"));
                    return Task::none();
                }
                let count = self.terminal.tabs.len() + 1;
                let tab = TerminalTab { title: format!("Terminal {}", count), ..Default::default() };
                self.terminal.tabs.push(tab);
                self.terminal.active_tab = self.terminal.tabs.len() - 1;
                let cwd = self
                    .state
                    .route
                    .local_path()
                    .cloned()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                self.terminal.active().cwd = Some(cwd.clone());
                let shell = self.state.config.terminal_shell.clone();
                tasks.push(Task::perform(
                    async move {
                        shell::spawn_shell_process(&shell, &cwd)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    UiMessage::TerminalSpawned,
                ));
            }
            UiMessage::TerminalCloseTab(index) => {
                if self.terminal.tabs.len() > 1 {
                    if let Some(tab) = self.terminal.tabs.get_mut(index) {
                        tab.process = None;
                    }
                    if index < self.terminal.tabs.len() {
                        self.terminal.tabs.remove(index);
                    }
                    if self.terminal.active_tab >= self.terminal.tabs.len() {
                        self.terminal.active_tab = self.terminal.tabs.len() - 1;
                    }
                }
            }
            UiMessage::TerminalSwitchTab(index) => {
                if index < self.terminal.tabs.len() {
                    self.terminal.active_tab = index;
                }
            }
            UiMessage::TerminalAnimTick => {
                let speed = 0.15;
                let diff = self.terminal_anim_target - self.terminal_anim_progress;
                if diff.abs() < 0.005 {
                    self.terminal_anim_progress = self.terminal_anim_target;
                } else {
                    self.terminal_anim_progress += diff * speed;
                }
            }
            // Feature 1: Trash
            UiMessage::TrashCompleted(result) => {
                match result {
                    Ok(()) => {
                        self.last_action = Some("Déplacé vers la corbeille".to_string());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur corbeille : {}", e));
                    }
                }
                tasks.push(self.refresh_entries());
            }
            // Feature 3: Properties dialog
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
                self.state.config.dark_mode = matches!(theme, ThemeConfig::Dark | ThemeConfig::Nord | ThemeConfig::Solarized | ThemeConfig::HighContrast);
                self.state.config.theme = theme;
                // Invalidate syntax highlight cache so it's regenerated with the new theme colors
                self.cached_highlighted_preview = None;
                self.config_manager.save(&self.state.config);
            }
            // Feature 5: Bulk rename
            UiMessage::OpenBulkRename => {
                let paths: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if paths.len() > 1 {
                    let previews = paths.iter().map(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        (name.clone(), name)
                    }).collect();
                    self.bulk_rename = Some(BulkRenameState {
                        paths,
                        find: String::new(),
                        replace: String::new(),
                        use_regex: false,
                        previews,
                        error: None,
                    });
                } else {
                    self.last_action = Some("Sélectionnez plusieurs fichiers pour renommer".to_string());
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
                        tasks.push(self.refresh_entries());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur renommage : {}", e));
                    }
                }
            }
            // Feature 6: Syntax highlighting
            UiMessage::TextHighlightComplete { path, lines } => {
                self.cached_highlighted_preview = Some((path, lines));
            }
            // Feature 7: Git status
            UiMessage::GitStatusLoaded { statuses, .. } => {
                self.git_statuses = statuses;
            }
            // Feature 8: Disk usage
            UiMessage::DirSizeLoaded { path, bytes } => {
                self.dir_sizes_loading.remove(&path);
                self.dir_sizes.insert(path, bytes);
            }
            // Feature 10: Archive browser
            UiMessage::ArchiveListLoaded { archive_path, inner_path, entries } => {
                let archive_type = detect_archive_type(&archive_path);
                self.archive_browser = Some(ArchiveBrowserState {
                    archive_path,
                    inner_path,
                    entries,
                    archive_type,
                });
            }
            UiMessage::ArchiveFolderOpen { inner_path } => {
                if let Some(browser) = &mut self.archive_browser {
                    browser.inner_path = inner_path;
                }
            }
            UiMessage::CloseArchiveBrowser => {
                self.archive_browser = None;
            }
            UiMessage::ExtractArchiveEntry { archive, inner_path, dest_dir } => {
                tasks.push(self.extract_archive_entry(archive, inner_path, dest_dir));
            }
            UiMessage::ExtractComplete(result) => {
                match result {
                    Ok(path) => {
                        self.last_action = Some(format!("Extrait : {}", path.display()));
                        Self::shell_open(&path);
                        // Refresh file list to show extracted files
                        tasks.push(self.refresh_entries());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur extraction : {}", e));
                    }
                }
            }
            // Feature 11: Dual pane
            UiMessage::ToggleDualPane => {
                tasks.push(self.toggle_dual_pane());
            }
            UiMessage::PaneBNavigate(path) => {
                tasks.push(self.pane_b_navigate(path));
            }
            UiMessage::PaneBLoaded { path, entries } => {
                if let Some(pane) = &mut self.dual_pane.pane_b {
                    if pane.path == path {
                        pane.entries = entries;
                        pane.is_loading = false;
                    }
                }
            }
            UiMessage::PaneBActivate(path) => {
                if path.is_dir() {
                    tasks.push(self.pane_b_navigate(path));
                } else {
                    Self::shell_open(&path);
                }
            }
            UiMessage::SwitchActivePane => {
                if self.dual_pane.enabled {
                    self.dual_pane.active = 1 - self.dual_pane.active;
                }
            }
            // ── Feature A: Compress to ZIP ──────────────────────────────────
            UiMessage::CompressToZip => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if selected.is_empty() {
                    self.last_action = Some("Aucune sélection à compresser".to_string());
                } else {
                    let first = selected[0].clone();
                    let output_dir = self.state.route.local_path().cloned()
                        .unwrap_or_else(|| first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
                    let archive_name = first.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("archive")
                        .to_string();
                    let out_path = output_dir.join(format!("{}.zip", archive_name));
                    self.last_action = Some("Compression en cours...".to_string());
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                                fn zip_add(
                                    zip: &mut zip::ZipWriter<std::fs::File>,
                                    src: &std::path::Path,
                                    name_in_zip: &str,
                                    options: zip::write::FileOptions<()>,
                                ) -> Result<(), String> {
                                    if src.is_file() {
                                        zip.start_file(name_in_zip, options).map_err(|e| e.to_string())?;
                                        let mut f = std::fs::File::open(src).map_err(|e| e.to_string())?;
                                        let mut buf = Vec::new();
                                        use std::io::Read;
                                        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                                        use std::io::Write;
                                        zip.write_all(&buf).map_err(|e| e.to_string())?;
                                    } else if src.is_dir() {
                                        zip.add_directory(format!("{}/", name_in_zip), options).map_err(|e| e.to_string())?;
                                        for child in std::fs::read_dir(src).map_err(|e| e.to_string())? {
                                            let child = child.map_err(|e| e.to_string())?;
                                            let child_name = child.file_name();
                                            let child_zip_name = format!("{}/{}", name_in_zip, child_name.to_string_lossy());
                                            zip_add(zip, &child.path(), &child_zip_name, options)?;
                                        }
                                    }
                                    Ok(())
                                }
                                let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                                let mut zip = zip::ZipWriter::new(file);
                                let options = zip::write::FileOptions::<()>::default()
                                    .compression_method(zip::CompressionMethod::Deflated);
                                for src in &selected {
                                    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
                                    zip_add(&mut zip, src, &name, options)?;
                                }
                                zip.finish().map_err(|e| e.to_string())?;
                                Ok(out_path)
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        UiMessage::CompressCompleted,
                    ));
                }
            }
            UiMessage::CompressToTarGz => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if selected.is_empty() {
                    self.last_action = Some("Aucune sélection à compresser".to_string());
                } else {
                    let first = selected[0].clone();
                    let output_dir = self.state.route.local_path().cloned()
                        .unwrap_or_else(|| first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
                    let archive_name = first.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("archive")
                        .to_string();
                    let out_path = output_dir.join(format!("{}.tar.gz", archive_name));
                    self.last_action = Some("Compression tar.gz en cours...".to_string());
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                archive::create_tar_gz(&selected, &out_path)
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        UiMessage::CompressCompleted,
                    ));
                }
            }
            UiMessage::CompressTo7z => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
                if selected.is_empty() {
                    self.last_action = Some("Aucune sélection à compresser".to_string());
                } else {
                    let first = selected[0].clone();
                    let output_dir = self.state.route.local_path().cloned()
                        .unwrap_or_else(|| first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
                    let archive_name = first.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("archive")
                        .to_string();
                    let out_path = output_dir.join(format!("{}.7z", archive_name));
                    self.last_action = Some("Compression 7z en cours...".to_string());
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                archive::create_7z(&selected, &out_path)
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        UiMessage::CompressCompleted,
                    ));
                }
            }
            UiMessage::CompressCompleted(result) => {
                match result {
                    Ok(path) => {
                        self.last_action = Some(format!("{} créé", path.file_name().and_then(|n| n.to_str()).unwrap_or("archive")));
                        tasks.push(self.refresh_entries());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur compression : {}", e));
                    }
                }
            }
            // ── Feature B: Open With ─────────────────────────────────────────
            UiMessage::OpenWith(path) => {
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            if let Err(e) = std::process::Command::new("rundll32")
                                .args(["shell32.dll,OpenAs_RunDLL", &format!("\"{}\"", path.display())])
                                .spawn()
                            {
                                tracing::warn!("OpenWith spawn failed: {e}");
                            }
                        })
                        .await
                        .ok();
                    },
                    |_| UiMessage::Noop,
                ));
            }
            // ── Feature C: File Diff ──────────────────────────────────────────
            UiMessage::OpenDiff => {
                let selected: Vec<PathBuf> = self.state.navigation.selection.selected.iter().cloned().collect();
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
                                let content_a = std::fs::read_to_string(&path_a).unwrap_or_default();
                                let content_b = std::fs::read_to_string(&path_b).unwrap_or_default();
                                let lines_a: Vec<&str> = content_a.lines().collect();
                                let lines_b: Vec<&str> = content_b.lines().collect();
                                let mut diff_lines = Vec::new();
                                diff_lines.push(crate::ui::DiffLine::Header(format!("--- {}", path_a.display())));
                                diff_lines.push(crate::ui::DiffLine::Header(format!("+++ {}", path_b.display())));
                                let max = lines_a.len().max(lines_b.len());
                                for i in 0..max {
                                    match (lines_a.get(i), lines_b.get(i)) {
                                        (Some(a), Some(b)) if a == b => {
                                            diff_lines.push(crate::ui::DiffLine::Same(a.to_string()));
                                        }
                                        (Some(a), Some(b)) => {
                                            diff_lines.push(crate::ui::DiffLine::Removed(a.to_string()));
                                            diff_lines.push(crate::ui::DiffLine::Added(b.to_string()));
                                        }
                                        (Some(a), None) => {
                                            diff_lines.push(crate::ui::DiffLine::Removed(a.to_string()));
                                        }
                                        (None, Some(b)) => {
                                            diff_lines.push(crate::ui::DiffLine::Added(b.to_string()));
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
                            Some((path_a, path_b, lines)) => UiMessage::DiffLoaded { path_a, path_b, lines },
                            None => UiMessage::Noop,
                        },
                    ));
                } else {
                    self.last_action = Some("Sélectionnez exactement 2 fichiers pour comparer".to_string());
                }
            }
            UiMessage::DiffLoaded { path_a, path_b, lines } => {
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
            UiMessage::SelectionSizeComputed(size) => {
                if let Some(ref mut dialog) = self.properties_dialog {
                    dialog.size_bytes = Some(size);
                }
            }
            // ── Feature E: Quick Filter ──────────────────────────────────────
            UiMessage::QuickFilterChanged(c) => {
                if !self.quick_filter_active {
                    self.quick_filter_active = true;
                    self.quick_filter = c;
                } else {
                    self.quick_filter.push_str(&c);
                }
            }
            UiMessage::QuickFilterClear => {
                self.quick_filter.clear();
                self.quick_filter_active = false;
            }
            // ── Feature F: File Labels ───────────────────────────────────────
            UiMessage::SetLabel(path, label_opt) => {
                match label_opt {
                    Some(label) => { self.state.config.labels.insert(path, label); }
                    None => { self.state.config.labels.remove(&path); }
                }
                self.config_manager.save(&self.state.config);
            }
            // ── Feature G: Recent Files ──────────────────────────────────────
            UiMessage::NavigateToRecent => {
                self.state.route.kind = crate::ui::RouteKind::Recent;
                self.address_input = crate::ui::RECENT_ROUTE.to_string();
                self.clear_selection();
            }
            UiMessage::ClearRecents => {
                self.recents.entries.clear();
                self.last_action = Some("Historique récents effacé".to_string());
            }
            // ── Feature H: Column Resizing ────────────────────────────────────
            UiMessage::ColumnResizeStart(column) => {
                let current_width = self.state.config.column_widths
                    .get(&column).copied().unwrap_or(150.0);
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
                self.state.config.column_widths.insert(column, width.max(30.0));
            }
            // ── Feature J: Shell Switcher ─────────────────────────────────────
            UiMessage::SetShell(shell) => {
                self.state.config.terminal_shell = shell;
                // Kill current process so next open uses new shell
                self.terminal.active().process = None;
                self.config_manager.save(&self.state.config);
            }
            // ── Feature K: Hex Viewer ─────────────────────────────────────────
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
                self.hex_view = Some(HexViewState { path, data, offset: 0 });
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
            UiMessage::NetworkScanCompleted(resources) => {
                let count = resources.len();
                self.network_discovery.update(resources);
                if self.state.route.is_network() {
                    tasks.push(self.request_page(0));
                }
                self.last_action = Some(format!(
                    "Scan réseau terminé — {} partage{} trouvé{}",
                    count,
                    if count > 1 { "s" } else { "" },
                    if count > 1 { "s" } else { "" },
                ));
            }
            // ── Feature N: Gitignore ──────────────────────────────────────────
            UiMessage::ToggleGitignore => {
                self.state.config.respect_gitignore = !self.state.config.respect_gitignore;
                self.last_action = Some(if self.state.config.respect_gitignore {
                    ".gitignore activé".to_string()
                } else {
                    ".gitignore désactivé".to_string()
                });
                tasks.push(self.refresh_entries());
            }
            // ── UX: Sidebar accordion ─────────────────────────────────────────
            UiMessage::ToggleSidebarSection(section) => {
                if !self.sidebar_collapsed.remove(&section) {
                    self.sidebar_collapsed.insert(section);
                }
            }
            // ── UX: Compact mode ──────────────────────────────────────────────
            UiMessage::ToggleCompactMode => {
                self.state.config.compact_mode = !self.state.config.compact_mode;
                self.state.config.view.row_height = if self.state.config.compact_mode { 22.0 } else { 32.0 };
                self.config_manager.save(&self.state.config);
            }
            // ── Feature O: Grep ───────────────────────────────────────────────
            UiMessage::OpenGrep => {
                let root = self.state.route.local_path().cloned()
                    .unwrap_or_else(|| PathBuf::from("."));
                self.grep_state = Some(GrepState {
                    query: String::new(),
                    root,
                    results: Vec::new(),
                    searching: false,
                });
            }
            UiMessage::GrepQueryChanged(q) => {
                if let Some(ref mut gs) = self.grep_state {
                    gs.query = q;
                }
            }
            UiMessage::GrepSearch => {
                if let Some(ref mut gs) = self.grep_state {
                    if gs.query.is_empty() {
                        return Task::batch(tasks);
                    }
                    gs.searching = true;
                    let root = gs.root.clone();
                    let query = gs.query.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut results = Vec::new();
                                let walker = walkdir::WalkDir::new(&root).into_iter();
                                let mut file_count = 0usize;
                                let query_lower = query.to_lowercase();
                                for entry in walker.filter_map(|e| e.ok()) {
                                    if file_count >= 10_000 || results.len() >= 100 {
                                        break;
                                    }
                                    if entry.file_type().is_file() {
                                        file_count += 1;
                                        let path = entry.path().to_path_buf();
                                        if let Ok(content) = std::fs::read(&path) {
                                            // Skip binary files
                                            if content[..content.len().min(8192)].contains(&0u8) {
                                                continue;
                                            }
                                            if let Ok(text) = String::from_utf8(content) {
                                                for (ln, line) in text.lines().enumerate() {
                                                    if line.to_lowercase().contains(&query_lower) {
                                                        results.push(crate::ui::GrepResult {
                                                            path: path.clone(),
                                                            line_number: ln + 1,
                                                            line: line.to_string(),
                                                        });
                                                        if results.len() >= 100 {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                results
                            })
                            .await
                            .unwrap_or_default()
                        },
                        UiMessage::GrepResultsLoaded,
                    ));
                }
            }
            UiMessage::GrepResultsLoaded(results) => {
                if let Some(ref mut gs) = self.grep_state {
                    gs.results = results;
                    gs.searching = false;
                }
            }
            UiMessage::CloseGrep => {
                self.grep_state = None;
            }
            // ── Feature P: NTFS Permissions ───────────────────────────────────
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
                            let output = std::process::Command::new("icacls")
                                .arg(&path)
                                .output();
                            match output {
                                Ok(out) => {
                                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                                    let entries = permissions::parse_icacls_output(&text);
                                    Ok((path, entries))
                                }
                                Err(e) => Err((path, format!("Impossible de lire les permissions : {e}"))),
                            }
                        })
                        .await
                        .unwrap_or_else(|_| Err((path_fallback, "Erreur interne lors de la lecture des permissions".to_string())))
                    },
                    |result| match result {
                        Ok((path, entries)) => UiMessage::PermissionsLoaded { path, entries, error: None },
                        Err((path, error)) => UiMessage::PermissionsLoaded { path, entries: vec![], error: Some(error) },
                    },
                ));
            }
            UiMessage::PermissionsLoaded { path, entries, error } => {
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
                    self.pending_undo_context = Some(if is_copy {
                        PendingUndoContext::Copy {
                            sources: items.clone(),
                            destination: destination.clone(),
                        }
                    } else {
                        PendingUndoContext::Move {
                            sources: items.clone(),
                            destination: destination.clone(),
                        }
                    });
                    tasks.push(Task::perform(
                        async move {
                            let operations = LocalFileOperations::new();
                            if matches!(operation, FileOperationKind::Copy) {
                                operations.copy_items(&items, &destination, Some(counter))
                            } else {
                                operations.move_items(&items, &destination, Some(counter))
                            }
                        },
                        UiMessage::FileOperationFinished,
                    ));
                }
            }
            // Feature Q: Undo
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
                tasks.push(self.refresh_entries());
            }
            // Feature R: Breadcrumb dropdown
            UiMessage::BreadcrumbDropdown(path) => {
                if self.breadcrumb_dropdown.as_ref() == Some(&path) {
                    self.breadcrumb_dropdown = None;
                    self.breadcrumb_dropdown_items.clear();
                } else {
                    // Cache subdirs on open so view() doesn't do I/O per frame
                    let mut subdirs: Vec<std::path::PathBuf> = match std::fs::read_dir(&path) {
                        Ok(entries) => entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                            .map(|e| e.path())
                            .collect(),
                        Err(e) => {
                            tracing::warn!("Breadcrumb dropdown: lecture {:?} échouée: {e}", path);
                            Vec::new()
                        }
                    };
                    subdirs.sort_by(|a, b| {
                        a.file_name().unwrap_or_default().to_ascii_lowercase()
                            .cmp(&b.file_name().unwrap_or_default().to_ascii_lowercase())
                    });
                    let has_more = subdirs.len() > 15;
                    subdirs.truncate(15);
                    self.breadcrumb_dropdown_items = subdirs;
                    self.breadcrumb_dropdown_has_more = has_more;
                    self.breadcrumb_dropdown = Some(path);
                }
            }
            UiMessage::CloseBreadcrumbDropdown => {
                self.breadcrumb_dropdown = None;
                self.breadcrumb_dropdown_items.clear();
            }
            UiMessage::RemoveFavorite(path) => {
                self.favorites.remove(&path);
                self.state.config.user_favorites.retain(|p| p != &path);
                self.config_manager.save(&self.state.config);
                self.last_action = Some("Retiré des favoris".to_string());
            }
            // ── #25: Terminal autocomplete ────────────────────────────────────
            UiMessage::TerminalAutoComplete => {
                let input = self.terminal.active_ref().input.clone();
                if !input.is_empty() {
                    let fallback = self
                        .state
                        .route
                        .local_path()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("."));
                    let cwd = self.terminal.effective_cwd(&fallback).to_path_buf();
                    // Try to complete the last word as a path
                    let last_word = input.split_whitespace().last().unwrap_or("");
                    let (search_dir, prefix) = if let Some(sep_pos) = last_word.rfind(['\\', '/']) {
                        let dir_part = &last_word[..=sep_pos];
                        let file_part = &last_word[sep_pos + 1..];
                        let search_path = if std::path::Path::new(dir_part).is_absolute() {
                            PathBuf::from(dir_part)
                        } else {
                            cwd.join(dir_part)
                        };
                        (search_path, file_part.to_lowercase())
                    } else {
                        (cwd.clone(), last_word.to_lowercase())
                    };
                    if let Ok(entries) = std::fs::read_dir(&search_dir) {
                        let mut matches: Vec<String> = entries
                            .flatten()
                            .filter_map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                if name.to_lowercase().starts_with(&prefix) {
                                    Some(name)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        matches.sort();
                        if let Some(first_match) = matches.first() {
                            // Replace the last word with the completion
                            let words: Vec<&str> = input.split_whitespace().collect();
                            let completed = if words.len() > 1 {
                                let leading = &words[..words.len() - 1];
                                let last_parts: Vec<&str> = last_word.rsplitn(2, ['\\', '/']).collect();
                                if last_parts.len() > 1 {
                                    // Has directory prefix: preserve it
                                    let dir_prefix = &last_word[..last_word.len() - last_parts[0].len()];
                                    format!("{} {}{}", leading.join(" "), dir_prefix, first_match)
                                } else {
                                    format!("{} {}", leading.join(" "), first_match)
                                }
                            } else {
                                let last_parts: Vec<&str> = last_word.rsplitn(2, ['\\', '/']).collect();
                                if last_parts.len() > 1 {
                                    let dir_prefix = &last_word[..last_word.len() - last_parts[0].len()];
                                    format!("{}{}", dir_prefix, first_match)
                                } else {
                                    first_match.clone()
                                }
                            };
                            self.terminal.active().input = completed;
                        }
                    }
                }
            }
            // ── #21: Trash browsing ──────────────────────────────────────────
            UiMessage::NavigateToTrash => {
                tasks.push(Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| -> Vec<(String, PathBuf)> {
                            let mut items = Vec::new();
                            if let Ok(trash_items) = trash::os_limited::list() {
                                for item in trash_items.into_iter().take(200) {
                                    let name = item.name.to_string_lossy().to_string();
                                    let original = item.original_parent.join(&item.name);
                                    items.push((name, original));
                                }
                            }
                            items
                        })
                        .await
                        .unwrap_or_default()
                    },
                    UiMessage::TrashListLoaded,
                ));
            }
            UiMessage::TrashListLoaded(items) => {
                let count = items.len();
                self.last_action = Some(format!("{count} élément(s) dans la corbeille"));
                // Store trash items for display in sidebar
                // We'll use the recents service pattern — display as a notification for now
            }
            // ── #23: Full-text content search ────────────────────────────────
            UiMessage::FullTextSearchSubmit => {
                let query = self.search.input.clone();
                if query.len() >= 2 {
                    if let Some(path) = self.state.route.local_path().cloned() {
                        let query_display = query.clone();
                        tasks.push(Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    let mut results = Vec::new();
                                    let walker = ignore::WalkBuilder::new(&path)
                                        .max_depth(Some(5))
                                        .build();
                                    for entry in walker.flatten() {
                                        if results.len() >= 100 { break; }
                                        let p = entry.path();
                                        if !p.is_file() { continue; }
                                        // Only search text files (< 1MB)
                                        if let Ok(meta) = p.metadata() {
                                            if meta.len() > 1_048_576 { continue; }
                                        }
                                        if let Ok(content) = std::fs::read_to_string(p) {
                                            for (i, line) in content.lines().enumerate() {
                                                if results.len() >= 100 { break; }
                                                if line.to_lowercase().contains(&query.to_lowercase()) {
                                                    results.push(crate::ui::GrepResult {
                                                        path: p.to_path_buf(),
                                                        line_number: i + 1,
                                                        line: line.to_string(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    results
                                })
                                .await
                                .unwrap_or_default()
                            },
                            UiMessage::FullTextSearchResults,
                        ));
                        self.last_action = Some(format!("Recherche en cours : '{query_display}'…"));
                    }
                }
            }
            UiMessage::FullTextSearchResults(results) => {
                let count = results.len();
                self.last_action = Some(format!("{count} résultat(s) de recherche dans le contenu"));
                // Reuse grep state for display
                self.grep_state = Some(GrepState {
                    query: self.search.input.clone(),
                    root: self.state.route.local_path().cloned().unwrap_or_default(),
                    results,
                    searching: false,
                });
            }
            // ── #24: Folder comparison ───────────────────────────────────────
            UiMessage::FolderCompare => {
                if self.dual_pane.enabled {
                    let path_a = self.state.route.local_path().cloned();
                    let path_b = self.dual_pane.pane_b.as_ref().map(|p| p.path.clone());
                    if let (Some(a), Some(b)) = (path_a, path_b) {
                        tasks.push(Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    let entries_a: std::collections::HashSet<String> = std::fs::read_dir(&a)
                                        .into_iter()
                                        .flatten()
                                        .flatten()
                                        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                                        .collect();
                                    let entries_b: std::collections::HashSet<String> = std::fs::read_dir(&b)
                                        .into_iter()
                                        .flatten()
                                        .flatten()
                                        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                                        .collect();
                                    let only_a: Vec<String> = entries_a.difference(&entries_b).cloned().collect();
                                    let only_b: Vec<String> = entries_b.difference(&entries_a).cloned().collect();
                                    let common_names: Vec<&String> = entries_a.intersection(&entries_b).collect();
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
                                only_a, only_b, different, common,
                            },
                        ));
                    }
                }
            }
            UiMessage::FolderCompareResults { only_a, only_b, different, common } => {
                self.last_action = Some(format!(
                    "Comparaison : {} communs, {} différents, {} unique A, {} unique B",
                    common, different.len(), only_a.len(), only_b.len()
                ));
                // Build diff lines for display in diff viewer
                let mut lines = Vec::new();
                lines.push(crate::ui::DiffLine::Header(format!("--- {} fichiers en commun ---", common)));
                for name in &different {
                    lines.push(crate::ui::DiffLine::Header(format!("≠ {name} (taille différente)")));
                }
                for name in &only_a {
                    lines.push(crate::ui::DiffLine::Removed(format!("Seulement dans A: {name}")));
                }
                for name in &only_b {
                    lines.push(crate::ui::DiffLine::Added(format!("Seulement dans B: {name}")));
                }
                let path_a = self.state.route.local_path().cloned().unwrap_or_default();
                let path_b = self.dual_pane.pane_b.as_ref().map(|p| p.path.clone()).unwrap_or_default();
                self.diff_view = Some(DiffViewState {
                    path_a,
                    path_b,
                    lines,
                    loading: false,
                });
            }
            // ── #18: Tab drag reorder ────────────────────────────────────────
            UiMessage::TabDragStart(index) => {
                self.tab_drag_source = Some(index);
            }
            UiMessage::TabDragOver(target) => {
                if let Some(source) = self.tab_drag_source {
                    if source != target && source < self.tab_manager.tabs.len() && target < self.tab_manager.tabs.len() {
                        self.tab_manager.tabs.swap(source, target);
                        // Update active index if needed
                        if self.tab_manager.active == source {
                            self.tab_manager.active = target;
                        } else if self.tab_manager.active == target {
                            self.tab_manager.active = source;
                        }
                        self.tab_drag_source = Some(target);
                    }
                }
            }
            UiMessage::TabDragDrop => {
                self.tab_drag_source = None;
                self.save_tabs_to_config();
            }
            // ── #12: External drag & drop from Windows Explorer ──────────
            UiMessage::ExternalFileHovered(path) => {
                self.last_action = Some(format!("Fichier survolé : {}", path.display()));
            }
            UiMessage::ExternalFileDropped(path) => {
                // Copy the dropped file/folder into the current directory
                if let Some(current_dir) = self.state.route.local_path().cloned() {
                    let file_name = path.file_name().unwrap_or_default().to_os_string();
                    let source = path.clone();
                    self.last_action = Some(format!("Copie de {} ...", file_name.to_string_lossy()));
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                crate::filesystem::LocalFileOperations::new()
                                    .copy_items(&[source], &current_dir, None)
                            })
                            .await
                            .unwrap_or_else(|e| {
                                let report = crate::filesystem::OperationReport {
                                    action: crate::filesystem::FileOperationKind::Copy,
                                    succeeded: Vec::new(),
                                    failed: vec![crate::filesystem::OperationFailure {
                                        path: std::path::PathBuf::new(),
                                        error: e.to_string(),
                                    }],
                                };
                                report
                            })
                        },
                        UiMessage::FileOperationFinished,
                    ));
                } else {
                    self.last_action = Some("Drop non supporté ici".to_string());
                }
            }
            UiMessage::ExternalFileCancelled => {
                self.last_action = None;
            }
        }

        tasks.push(self.request_visible_thumbnails());
        tasks.push(self.request_selected_preview());
        Task::batch(tasks)
    }
}
