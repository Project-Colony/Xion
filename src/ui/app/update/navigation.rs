//! Routes, tabs, history, address bar, sorting and page loading.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;

use iced::Task;

use crate::core::SortOrderConfig;
use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;
use std::sync::Arc;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_navigation(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::NavigateTo(path) => {
                if self
                    .ignore_next_navigation
                    .as_ref()
                    .is_some_and(|ignore| ignore == &path)
                {
                    self.ignore_next_navigation = None;
                    return Ok(Flow::Stop(Task::none()));
                }
                self.menus.history_open = false;
                self.menus.history_position = None;
                self.address_editing = false;
                self.breadcrumb_dropdown = None;
                self.breadcrumb_dropdown_items.clear();
                tasks.push(self.navigate_to(path));
            }
            // A later launch of the binary handed us a path.
            UiMessage::OpenRequestTick => {
                // Drained, not read once: several launches can land between two
                // ticks, and the last one is the folder the user is looking at.
                let mut requested = None;
                while let Some(path) = self
                    .single_instance
                    .as_ref()
                    .and_then(|primary| primary.try_recv())
                {
                    // An empty path means "you are already running, come to the
                    // front" — a launch with no argument at all.
                    if path.as_os_str().is_empty() {
                        continue;
                    }
                    requested = Some(path);
                }
                if let Some(path) = requested {
                    // Sans ça la fenêtre ouvre bien l'onglet mais reste
                    // derrière celle où l'utilisateur vient de taper la
                    // commande, et le dossier demandé n'apparaît jamais.
                    tasks.push(iced::window::latest().and_then(iced::window::gain_focus));
                    tasks.push(self.open_in_new_tab(path));
                }
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
            UiMessage::AddressEditStart => {
                self.address_editing = true;
            }
            UiMessage::AddressEditCancel => {
                self.address_editing = false;
                self.address_input = self.state.route.address_label();
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
                generation,
                result,
            } => {
                if path != self.state.route.key() || generation != self.loading_generation {
                    return Ok(Flow::Stop(Task::batch(std::mem::take(tasks))));
                }

                self.pending_pages.remove(&page_index);
                match result {
                    Ok(page) => {
                        Arc::make_mut(&mut self.entries).apply_page(page_index, page);
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

                if self.has_search_query() {
                    tasks.push(self.request_all_pages());
                }
                // Feature 8: Load dir sizes after page load (only when Size column is visible)
                if self
                    .state
                    .config
                    .view
                    .columns
                    .contains(&crate::core::config::types::ViewColumn::Size)
                {
                    tasks.push(self.request_dir_sizes());
                }
            }
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
                        a.file_name()
                            .unwrap_or_default()
                            .to_ascii_lowercase()
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
            UiMessage::TabDragStart(index) => {
                self.tab_drag_source = Some(index);
            }
            UiMessage::TabDragOver(target) => {
                if let Some(source) = self.tab_drag_source {
                    if source != target
                        && source < self.tab_manager.tabs.len()
                        && target < self.tab_manager.tabs.len()
                    {
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
            UiMessage::ToggleSidebarSection(section) => {
                if !self.sidebar_collapsed.remove(section) {
                    self.sidebar_collapsed.insert(section);
                }
            }
            // ── UX: Compact mode ──────────────────────────────────────────────
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
