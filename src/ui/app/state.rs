//! State management methods for XionApp.
//!
//! Handles directory loading, search indexing, navigation, tab management,
//! and file watcher synchronization.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::filesystem::{EntryFilter, ListOptions, LocalFileSystem, PageRequest, SortKey, SortOrder, WatchEvent};
use crate::services::{SearchIndexOptions, SearchQuery, SearchService};
use crate::ui::UiMessage;
use crate::ui::theme::layout::TREE_MAX_DEPTH;
use crate::ui::theme::timing::LOADING_INDICATOR_DELAY;

use super::XionApp;
use super::helpers::{build_tree_nodes, list_options_from_config};
use super::types::{PagedEntries, TabState, root_path_for, route_kind_from_path};

impl XionApp {
    pub(super) fn refresh_entries(&mut self) -> Task<UiMessage> {
        let page_size = self.entries.page_size;
        if self.entries.total > 0 {
            self.stale_entries = Some(std::mem::replace(
                &mut self.entries,
                PagedEntries::new(0, page_size),
            ));
        } else {
            self.entries.reset();
            self.stale_entries = None;
        }
        self.error = None;
        self.scroll.offset = 0.0;
        self.pending_pages.clear();
        self.is_loading = true;
        self.is_refreshing = true;
        self.show_loading_indicator = false;
        self.loading_generation = self.loading_generation.wrapping_add(1);
        self.clear_selection();
        self.search.index = None;
        self.search.matches = None;
        self.search.index_path = None;
        self.search.indexing = false;
        self.drag_state = None;
        self.drag_candidate = None;
        self.drag_start_position = None;
        self.mouse_pressed = false;
        self.selection_box_start = None;
        self.selection_box_current = None;
        self.rebuild_tree_cache();

        Task::batch(vec![
            self.request_page(0),
            self.schedule_loading_indicator(self.loading_generation),
            self.start_search_indexing(),
        ])
    }

    pub(super) fn sync_watcher(&mut self) {
        let target = self.state.route.local_path().map(|path| path.to_path_buf());
        if target == self.watched_path {
            return;
        }
        if let Some(path) = self.watched_path.take() {
            let _ = self.file_watcher.unwatch(&path);
        }
        self.watched_path = target.clone();
        if let Some(path) = target {
            if let Err(error) = self.file_watcher.watch(&path) {
                self.last_action = Some(format!(
                    "Observateur FS: impossible de surveiller {} ({})",
                    path.display(),
                    error
                ));
            }
        }
    }

    pub(super) fn start_search_indexing(&mut self) -> Task<UiMessage> {
        let Some(path) = self.state.route.local_path().cloned() else {
            self.search.index = None;
            self.search.matches = None;
            self.search.index_path = None;
            self.search.indexing = false;
            return Task::none();
        };

        let filesystem_config = self.state.config.filesystem.clone();
        let list_config = self.state.config.list.clone();
        let list_options = list_options_from_config(list_config);
        let options = SearchIndexOptions {
            include_hidden: list_options.show_hidden,
            recursive: false,
            ..SearchIndexOptions::default()
        };

        self.search.index = None;
        self.search.matches = None;
        self.search.index_path = Some(path.clone());
        self.search.indexing = true;

        let message_path = path.clone();
        Task::perform(
            async move {
                let filesystem = LocalFileSystem::from_config(filesystem_config);
                SearchService
                    .build_index_with_options(&filesystem, &path, options, list_options)
                    .map_err(|error| error.to_string())
            },
            move |result| UiMessage::SearchIndexBuilt {
                path: message_path.clone(),
                result,
            },
        )
    }

    pub(super) fn update_search_index_matches(&mut self) {
        let Some(query) = self.normalized_search_query() else {
            self.search.matches = None;
            return;
        };

        let Some(index) = self.search.index.as_ref() else {
            self.search.matches = None;
            return;
        };

        let search_query = SearchQuery {
            text: Some(query),
            ..SearchQuery::default()
        };
        let count = SearchService.count_index_matches(index, &search_query);
        self.search.matches = Some(count);
    }

    pub(super) fn poll_watcher(&mut self) -> Option<Vec<WatchEvent>> {
        match self.file_watcher.poll() {
            Ok(events) if !events.is_empty() => Some(events),
            Ok(_) => None,
            Err(error) => {
                self.last_action = Some(format!("Observateur FS: {error}"));
                None
            }
        }
    }

    pub(super) fn update_active_tab_path(&mut self, path: PathBuf) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.path = path.clone();
        }
        self.state.route.kind = route_kind_from_path(&path);
        self.address_input = self.state.route.address_label();
        self.sync_watcher();
    }

    pub(super) fn navigate_to(&mut self, path: PathBuf) -> Task<UiMessage> {
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.refresh_entries()
    }

    pub(super) fn address_target_from_input(&self) -> Option<PathBuf> {
        let trimmed = self.address_input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut target = PathBuf::from(trimmed);
        if !target.is_absolute() {
            if let Some(base_path) = self.state.route.local_path() {
                target = base_path.join(target);
            }
        }
        Some(target)
    }

    pub(super) fn address_suggestions(&self) -> Vec<PathBuf> {
        let query = self.address_input.trim().to_lowercase();
        let mut suggestions = Vec::new();
        let mut seen = HashSet::new();
        for entry in self.history.entries().rev() {
            if entry == &self.state.route.key() {
                continue;
            }
            let display = entry.display().to_string();
            if !query.is_empty() && !display.to_lowercase().contains(&query) {
                continue;
            }
            if seen.insert(entry.clone()) {
                suggestions.push(entry.clone());
            }
            if suggestions.len() >= 6 {
                break;
            }
        }
        suggestions
    }

    pub(super) fn request_page(&mut self, page_index: usize) -> Task<UiMessage> {
        if self.pending_pages.contains(&page_index) {
            return Task::none();
        }

        let offset = page_index * self.entries.page_size;
        let page_request = PageRequest::new(offset, self.entries.page_size);
        let route_key = self.state.route.key();
        let route_kind = self.state.route.kind.clone();
        let list_config = self.state.config.list.clone();
        let filesystem_config = self.state.config.filesystem.clone();
        let loader = Arc::clone(&self.directory_loader);
        let network_discovery = self.network_discovery.clone();

        self.pending_pages.insert(page_index);
        self.is_loading = true;

        Task::perform(
            async move {
                let options = list_options_from_config(list_config);
                let result = match route_kind {
                    crate::ui::RouteKind::Local(path) => {
                        let filesystem = LocalFileSystem::from_config(filesystem_config);
                        match loader.lock() {
                            Ok(mut loader) => loader
                                .load_page(&filesystem, &path, options, page_request)
                                .map_err(|error| error.to_string()),
                            Err(_) => Err("Le chargeur de dossiers est indisponible.".to_string()),
                        }
                    }
                    crate::ui::RouteKind::Network => {
                        Ok(network_discovery.list_page(options, page_request))
                    }
                };
                (route_key, page_index, result)
            },
            |(path, page_index, result)| UiMessage::PageLoaded {
                path,
                page_index,
                result,
            },
        )
    }

    pub(super) fn schedule_loading_indicator(&self, generation: u64) -> Task<UiMessage> {
        let delay = LOADING_INDICATOR_DELAY;
        Task::perform(
            async move {
                tokio::time::sleep(delay).await;
                generation
            },
            UiMessage::LoadingDelayElapsed,
        )
    }

    pub(super) fn add_tab(&mut self) -> Task<UiMessage> {
        let new_index = self.tabs.len() + 1;
        let title = if new_index == 1 {
            "Ce PC".to_string()
        } else {
            format!("Ce PC {}", new_index)
        };
        let path = self.state.config.start_path.clone();
        self.tabs.push(TabState { title, path });
        self.active_tab = self.tabs.len().saturating_sub(1);
        let active_path = self
            .tabs
            .get(self.active_tab)
            .map(|tab| tab.path.clone())
            .unwrap_or_else(|| self.state.config.start_path.clone());
        self.update_active_tab_path(active_path.clone());
        self.history.record(active_path);
        self.refresh_entries()
    }

    pub(super) fn switch_tab(&mut self, index: usize) -> Task<UiMessage> {
        if index >= self.tabs.len() {
            return Task::none();
        }
        self.active_tab = index;
        let path = self.tabs[index].path.clone();
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.refresh_entries()
    }

    pub(super) fn close_tab(&mut self, index: usize) -> Task<UiMessage> {
        if self.tabs.len() <= 1 || index == 0 || index >= self.tabs.len() {
            return Task::none();
        }
        self.tabs.remove(index);
        if self.active_tab == index {
            self.active_tab = index.saturating_sub(1);
        } else if self.active_tab > index {
            self.active_tab = self.active_tab.saturating_sub(1);
        }
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let path = tab.path.clone();
            self.update_active_tab_path(path.clone());
            self.history.record(path);
            return self.refresh_entries();
        }
        Task::none()
    }

    pub(super) fn rebuild_tree_cache(&mut self) {
        self.cached_tree_nodes = if let Some(current_path) = self.state.route.local_path() {
            let tree_root = root_path_for(current_path).unwrap_or_else(|| current_path.clone());
            let tree_options = ListOptions {
                show_hidden: self.state.config.list.show_hidden,
                sort_by: SortKey::Name,
                sort_order: SortOrder::Asc,
                directories_first: true,
                filter: EntryFilter::OnlyDirectories,
                name_query: None,
            };
            build_tree_nodes(
                &tree_root,
                current_path,
                TREE_MAX_DEPTH,
                &tree_options,
            )
        } else {
            Vec::new()
        };
    }
}
