//! State management methods for XionApp.
//!
//! Handles directory loading, search indexing, navigation, tab management,
//! and file watcher synchronization.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::filesystem::{EntryFilter, ListOptions, LocalFileSystem, PageRequest, SortKey, SortOrder, WatchEvent};
use crate::services::{SearchIndexOptions, SearchQuery, SearchService};
use crate::ui::{GitFileStatus, UiMessage};
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
        self.search.input.clear();
        self.search.index = None;
        self.search.matches = None;
        self.search.index_path = None;
        self.search.indexing = false;
        self.selection_snapshot = None;
        self.drag_state = None;
        self.drag_candidate = None;
        self.drag_start_position = None;
        self.mouse_pressed = false;
        self.selection_box_start = None;
        self.selection_box_current = None;
        // Clear quick filter when navigating to a new directory
        self.quick_filter.clear();
        self.quick_filter_active = false;
        // Clear stale git/dir-size data immediately so old indicators don't persist
        self.git_statuses.clear();
        self.dir_sizes.clear();
        self.dir_sizes_loading.clear();
        self.rebuild_tree_cache();

        Task::batch(vec![
            self.request_page(0),
            self.schedule_loading_indicator(self.loading_generation),
            self.start_search_indexing(),
            self.load_git_status(),
            self.start_network_scan_if_needed(),
            // Scroll to top to trigger on_scroll, which captures list_viewport_bounds.
            // Without this, rubber band selection won't work until the user scrolls.
            iced::widget::operation::scroll_to(
                iced::widget::Id::new(super::LIST_SCROLLABLE_ID),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
        ])
    }

    pub(super) fn start_network_scan_if_needed(&mut self) -> Task<UiMessage> {
        if !self.state.route.is_network() {
            return Task::none();
        }
        if !self.network_discovery.needs_scan() {
            return Task::none();
        }
        self.network_discovery.mark_scanning();
        Task::perform(
            async {
                // 15-second timeout to prevent network discovery from blocking indefinitely
                match tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    tokio::task::spawn_blocking(crate::services::network::NetworkDiscoveryService::run_scan),
                ).await {
                    Ok(Ok(results)) => results,
                    _ => Vec::new(),
                }
            },
            UiMessage::NetworkScanCompleted,
        )
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

        // #10: Check LRU cache first
        if let Some(pos) = self.search_index_cache.iter().position(|(p, _)| p == &path) {
            let (_, cached_index) = self.search_index_cache.remove(pos).unwrap();
            // Move to back (most recently used)
            self.search_index_cache.push_back((path.clone(), cached_index.clone()));
            self.search.index = Some(cached_index);
            self.search.matches = None;
            self.search.index_path = Some(path);
            self.search.indexing = false;
            return Task::none();
        }

        let filesystem_config = self.state.config.filesystem.clone();
        let list_config = self.state.config.list.clone();
        let respect_gitignore = self.state.config.respect_gitignore;
        let list_options = list_options_from_config(list_config, respect_gitignore);
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
        self.tab_manager.set_active_path(path.clone());
        self.state.route.kind = route_kind_from_path(&path);
        self.address_input = self.state.route.address_label();
        self.sync_watcher();
    }

    pub(super) fn navigate_to(&mut self, path: PathBuf) -> Task<UiMessage> {
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.save_tabs_to_config();
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

        let offset = page_index.saturating_mul(self.entries.page_size);
        let page_request = PageRequest::new(offset, self.entries.page_size);
        let route_key = self.state.route.key();
        let route_kind = self.state.route.kind.clone();
        // These clones are intentional — config structs are small and must be owned by the async task
        let list_config = self.state.config.list.clone();
        let respect_gitignore = self.state.config.respect_gitignore;
        let filesystem_config = self.state.config.filesystem.clone();
        let loader = Arc::clone(&self.directory_loader);
        let network_discovery = self.network_discovery.clone();

        self.pending_pages.insert(page_index);
        self.is_loading = true;

        Task::perform(
            async move {
                let options = list_options_from_config(list_config, respect_gitignore);
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
                    crate::ui::RouteKind::Recent => {
                        // Recent view handled separately; return empty page
                        use crate::filesystem::Page;
                        Ok(Page { items: vec![], total: 0, offset: 0, limit: 0 })
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
        let new_index = self.tab_manager.count() + 1;
        let title = if new_index == 1 {
            "Ce PC".to_string()
        } else {
            format!("Ce PC {}", new_index)
        };
        let path = self.state.config.start_path.clone();
        self.tab_manager.tabs.push(TabState { title, path });
        self.tab_manager.active = self.tab_manager.count().saturating_sub(1);
        let active_path = self
            .tab_manager.active_path()
            .cloned()
            .unwrap_or_else(|| self.state.config.start_path.clone());
        self.update_active_tab_path(active_path.clone());
        self.history.record(active_path);
        self.save_tabs_to_config();
        self.refresh_entries()
    }

    pub(super) fn switch_tab(&mut self, index: usize) -> Task<UiMessage> {
        if index >= self.tab_manager.count() {
            return Task::none();
        }
        self.tab_manager.active = index;
        let path = self.tab_manager.tabs[index].path.clone();
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.save_tabs_to_config();
        self.refresh_entries()
    }

    pub(super) fn close_tab(&mut self, index: usize) -> Task<UiMessage> {
        // Tab 0 is intentionally protected — always keep at least one "home" tab open.
        if self.tab_manager.count() <= 1 || index == 0 || index >= self.tab_manager.count() {
            return Task::none();
        }
        self.tab_manager.tabs.remove(index);
        if self.tab_manager.active == index {
            self.tab_manager.active = index.saturating_sub(1);
        } else if self.tab_manager.active > index {
            self.tab_manager.active = self.tab_manager.active.saturating_sub(1);
        }
        if let Some(tab) = self.tab_manager.tabs.get(self.tab_manager.active) {
            let path = tab.path.clone();
            self.update_active_tab_path(path.clone());
            self.history.record(path);
            self.save_tabs_to_config();
            return self.refresh_entries();
        }
        Task::none()
    }

    /// Persists current tab state to config on disk.
    pub(super) fn save_tabs_to_config(&mut self) {
        self.state.config.tabs = self
            .tab_manager.tabs
            .iter()
            .map(|t| crate::core::TabPersistConfig { path: t.path.clone() })
            .collect();
        self.state.config.active_tab_index = self.tab_manager.active;
        self.config_manager.save(&self.state.config);
    }

    pub(super) fn load_git_status(&self) -> Task<UiMessage> {
        let Some(path) = self.state.route.local_path().cloned() else {
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let repo = git2::Repository::discover(&path).ok()?;
                    let root = repo.workdir()?.to_path_buf();
                    let statuses = repo.statuses(None).ok()?;
                    let mut map: HashMap<PathBuf, GitFileStatus> = HashMap::new();
                    for entry in statuses.iter() {
                        let s = entry.status();
                        let rel = entry.path()?;
                        let file_path = root.join(rel);
                        let status = if s.contains(git2::Status::CONFLICTED) {
                            GitFileStatus::Conflict
                        } else if s.intersects(
                            git2::Status::INDEX_NEW
                                | git2::Status::INDEX_MODIFIED
                                | git2::Status::INDEX_DELETED,
                        ) {
                            GitFileStatus::Staged
                        } else if s.intersects(
                            git2::Status::WT_MODIFIED | git2::Status::WT_DELETED,
                        ) {
                            GitFileStatus::Modified
                        } else if s.contains(git2::Status::WT_NEW) {
                            GitFileStatus::Untracked
                        } else {
                            return None;
                        };
                        map.insert(file_path, status);
                    }
                    Some((root, map))
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((root, statuses)) => UiMessage::GitStatusLoaded { root, statuses },
                None => UiMessage::Noop,
            },
        )
    }

    pub(super) fn request_dir_sizes(&mut self) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        let entries_snapshot: Vec<PathBuf> = self
            .entries
            .items
            .iter()
            .flatten()
            .filter(|e| e.entry_type == crate::filesystem::FsEntryType::Directory)
            .map(|e| e.path.clone())
            .collect();

        for path in entries_snapshot {
            if self.dir_sizes.contains_key(&path) || self.dir_sizes_loading.contains(&path) {
                continue;
            }
            self.dir_sizes_loading.insert(path.clone());
            let task_path = path.clone();
            tasks.push(Task::perform(
                async move {
                    let bytes = tokio::task::spawn_blocking(move || {
                        let mut total: u64 = 0;
                        let mut count = 0usize;
                        for e in walkdir::WalkDir::new(&task_path).min_depth(1).max_depth(6) {
                            if count >= 50_000 {
                                break;
                            }
                            if let Ok(e) = e {
                                if let Ok(m) = e.metadata() {
                                    if m.is_file() {
                                        total += m.len();
                                        count += 1;
                                    }
                                }
                            }
                        }
                        total
                    })
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!("Taille dossier: tâche paniquée: {e}");
                        0
                    });
                    (path, bytes)
                },
                |(path, bytes)| UiMessage::DirSizeLoaded { path, bytes },
            ));
        }
        Task::batch(tasks)
    }

    pub(super) fn rebuild_tree_cache(&mut self) {
        let Some(current_path) = self.state.route.local_path() else {
            self.cached_tree_nodes = Vec::new();
            return;
        };
        let tree_root = root_path_for(current_path).unwrap_or_else(|| current_path.clone());

        // If the root hasn't changed, try to update expanded/selected flags
        // in-place instead of re-reading the filesystem.
        if !self.cached_tree_nodes.is_empty()
            && self.cached_tree_nodes.first().map(|n| &n.path) == Some(&tree_root)
        {
            for node in &mut self.cached_tree_nodes {
                node.selected = node.path == *current_path;
                node.expanded =
                    current_path.starts_with(&node.path) && node.depth < TREE_MAX_DEPTH;
            }
            // If the current path (or one of its ancestors below the root)
            // isn't already in the cached nodes, we need a full rebuild so
            // the new directories appear in the tree.
            let needs_full_rebuild = current_path.starts_with(&tree_root)
                && !self
                    .cached_tree_nodes
                    .iter()
                    .any(|n| n.path == *current_path);
            if !needs_full_rebuild {
                return;
            }
        }

        // Full rebuild - reads the filesystem for every expanded node.
        let tree_options = ListOptions {
            show_hidden: self.state.config.list.show_hidden,
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            filter: EntryFilter::OnlyDirectories,
            name_query: None,
            respect_gitignore: self.state.config.respect_gitignore,
        };
        self.cached_tree_nodes = build_tree_nodes(
            &tree_root,
            current_path,
            TREE_MAX_DEPTH,
            &tree_options,
        );
    }
}
