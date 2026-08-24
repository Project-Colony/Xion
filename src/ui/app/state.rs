//! State management methods for XionApp.
//!
//! Handles directory loading, search indexing, navigation, tab management,
//! and file watcher synchronization.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::filesystem::{
    EntryFilter, ListOptions, LocalFileSystem, PageRequest, SortKey, SortOrder, WatchEvent,
};
use crate::services::{SearchIndexOptions, SearchQuery, SearchService};
use crate::ui::theme::layout::TREE_MAX_DEPTH;
use crate::ui::theme::timing::LOADING_INDICATOR_DELAY;
use crate::ui::{GitFileStatus, UiMessage};

use super::XionApp;
use super::helpers::{build_tree_nodes, list_options_from_config};
use super::types::{DirSize, PagedEntries, TabState, root_path_for, route_kind_from_path};

/// How deep a directory-size walk goes.
///
/// A folder is not a number you can read off an inode: it has to be walked.
/// Both limits below exist so that pointing at `/` does not start an unbounded
/// traversal — but a result that hit one of them is a floor, and the UI is told
/// so rather than showing a total that is quietly wrong.
const MAX_DIR_SIZE_DEPTH: usize = 6;

/// How many files a directory-size walk counts before giving up.
const MAX_DIR_SIZE_FILES: usize = 50_000;

/// Concurrency cap on recursive directory-size walks.
static DIR_SIZE_LIMIT: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(3));

impl XionApp {
    /// Reload the current directory, discarding the user's query.
    ///
    /// This is the *navigation* refresh: arriving somewhere new resets the
    /// search box, the quick filter and the scroll position.
    pub(super) fn refresh_entries(&mut self) -> Task<UiMessage> {
        self.refresh_entries_inner(true)
    }

    /// Reload the current directory *in place*, keeping what the user typed.
    ///
    /// Copying a file used to wipe the active search and quick filter, because
    /// every file operation went through the navigation refresh. The list has
    /// to be re-read, but the query the user is in the middle of is not stale.
    pub(super) fn refresh_entries_in_place(&mut self) -> Task<UiMessage> {
        self.refresh_entries_inner(false)
    }

    fn refresh_entries_inner(&mut self, reset_query: bool) -> Task<UiMessage> {
        let page_size = self.entries.page_size;
        if self.entries.total > 0 {
            self.stale_entries = Some(std::mem::replace(
                &mut self.entries,
                Arc::new(PagedEntries::new(0, page_size)),
            ));
        } else {
            Arc::make_mut(&mut self.entries).reset();
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
        if reset_query {
            self.search.input.clear();
            self.search.matches = None;
        }
        self.search.index = None;
        self.search.index_path = None;
        self.search.indexing = false;
        self.selection_snapshot = None;
        self.drag_state = None;
        self.drag_candidate = None;
        self.drag_start_position = None;
        self.mouse_pressed = false;
        self.selection_box_start = None;
        self.selection_box_current = None;
        if reset_query {
            self.quick_filter.clear();
            self.quick_filter_active = false;
        }
        // Clear stale git/dir-size data immediately so old indicators don't persist
        self.git_statuses.clear();
        self.dir_sizes_loading.clear();
        // `dir_sizes` deliberately survives: walking a folder costs real disk
        // I/O, and this ran on every navigation, so going into a folder and
        // back re-walked everything the user had just waited for. It is dropped
        // only when something is known to have changed — see the `reset_query`
        // path below, which is the after-a-file-operation refresh.
        if !reset_query {
            self.dir_sizes.clear();
        }
        // Thumbnail maps used to be cleared only when the config changed, so
        // they grew monotonically for the whole session: every decoded handle
        // of every directory ever visited stayed resident.
        self.media.thumbnail_handles.clear();
        self.media.thumbnail_misses.clear();
        self.media.thumbnails_in_flight.clear();
        self.rebuild_tree_cache();

        Task::batch(vec![
            self.request_page(0),
            self.schedule_loading_indicator(self.loading_generation),
            // No-op unless a query survived the navigation — a refresh keeps
            // the query, and its match count has to be rebuilt with it.
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
                    tokio::task::spawn_blocking(
                        crate::services::network::NetworkDiscoveryService::run_scan,
                    ),
                )
                .await
                {
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

    /// Builds the search index for the current directory, if a query needs it.
    ///
    /// This used to run on every navigation, whether or not the user ever
    /// searched. The index is a second copy of the directory — 12,4 Mo for
    /// 50 000 entries, measured, more than the listing itself — and up to eight
    /// of them are kept in the LRU cache, so simply walking through eight large
    /// folders cost a hundred megabytes for a count nobody had asked for.
    ///
    /// It now runs on the first query, and only then. What it buys, once the
    /// listing itself is capped at `MAX_SEARCH_ENTRIES`, is a match count over
    /// the whole directory rather than over the part that fits.
    pub(super) fn start_search_indexing(&mut self) -> Task<UiMessage> {
        if !self.has_search_query() {
            self.search.index = None;
            self.search.matches = None;
            self.search.index_path = None;
            self.search.indexing = false;
            return Task::none();
        }
        let Some(path) = self.state.route.local_path().cloned() else {
            self.search.index = None;
            self.search.matches = None;
            self.search.index_path = None;
            self.search.indexing = false;
            return Task::none();
        };

        // Now that this is driven by the query rather than by navigation, it is
        // reached on every keystroke: without this guard, typing "rapport"
        // would launch seven concurrent walks of the same directory.
        if self.search.index_path.as_ref() == Some(&path)
            && (self.search.indexing || self.search.index.is_some())
        {
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
            // The index is built on demand; until it lands, the status bar falls
            // back to counting the rows it can see.
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
        let generation = self.loading_generation;
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
                        Ok(Page {
                            items: vec![],
                            total: 0,
                            offset: 0,
                            limit: 0,
                        })
                    }
                };
                (route_key, page_index, generation, result)
            },
            |(path, page_index, generation, result)| UiMessage::PageLoaded {
                path,
                page_index,
                generation,
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
            .tab_manager
            .active_path()
            .cloned()
            .unwrap_or_else(|| self.state.config.start_path.clone());
        self.update_active_tab_path(active_path.clone());
        self.history.record(active_path);
        self.save_tabs_to_config();
        self.refresh_entries()
    }

    /// Opens `path` in a new, active tab.
    ///
    /// This is what a later launch of the binary asks for. Unlike `add_tab` it
    /// names the tab after the folder rather than "Ce PC N": the user asked for
    /// somewhere in particular, so the tab strip should say where.
    pub(super) fn open_in_new_tab(&mut self, path: PathBuf) -> Task<UiMessage> {
        // Already open: go to it. Xion is the default file manager now, so a
        // folder gets opened from the desktop over and over; without this, the
        // tab strip filled with duplicates of the same directory — observed,
        // twice for Téléchargements.
        if let Some(index) = self
            .tab_manager
            .tabs
            .iter()
            .position(|tab| tab.path == path)
        {
            return self.switch_tab(index);
        }

        self.tab_manager.tabs.push(TabState {
            title: crate::ui::app::helpers::tree_label_for_path(&path),
            path: path.clone(),
        });
        self.tab_manager.active = self.tab_manager.count().saturating_sub(1);
        self.update_active_tab_path(path.clone());
        self.history.record(path);
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
            .tab_manager
            .tabs
            .iter()
            .map(|t| crate::core::TabPersistConfig {
                path: t.path.clone(),
            })
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
                    // `statuses(None)` walks the entire repository. In a large
                    // checkout that is seconds of work, redone on every single
                    // navigation, to colour at most a screenful of rows. Scope
                    // it to the directory being displayed.
                    let mut options = git2::StatusOptions::new();
                    options
                        .include_untracked(true)
                        .recurse_untracked_dirs(false)
                        .include_ignored(false)
                        .include_unmodified(false);
                    if let Ok(relative) = path.strip_prefix(&root) {
                        if !relative.as_os_str().is_empty() {
                            options.pathspec(relative);
                        }
                    }
                    let statuses = repo.statuses(Some(&mut options)).ok()?;
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
                        } else if s.intersects(git2::Status::WT_MODIFIED | git2::Status::WT_DELETED)
                        {
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

    /// Records a folder's size, keeping the map bounded.
    ///
    /// Now that it survives navigation, an unbounded map would hold an entry
    /// for every folder seen in a session. The cap is generous — the entries
    /// are a path and sixteen bytes — and eviction is arbitrary rather than
    /// least-recently-used, because at this size the difference is not worth a
    /// second data structure to maintain.
    pub(super) fn remember_dir_size(&mut self, path: PathBuf, size: DirSize) {
        const MAX_REMEMBERED: usize = 4096;

        if self.dir_sizes.len() >= MAX_REMEMBERED && !self.dir_sizes.contains_key(&path) {
            if let Some(victim) = self.dir_sizes.keys().next().cloned() {
                self.dir_sizes.remove(&victim);
            }
        }
        self.dir_sizes.insert(path, size);
    }

    pub(super) fn request_dir_sizes(&mut self) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        // Only the rows on screen. Walking every loaded directory spawned one
        // unbounded recursive walk per subfolder — a directory of 500 folders
        // launched 500 concurrent disk traversals nobody had asked for.
        let window = self.entry_virtual_window();
        let entries_snapshot: Vec<PathBuf> = (window.start..window.end)
            .filter_map(|index| self.entries.get(index))
            .filter(|entry| entry.entry_type == crate::filesystem::FsEntryType::Directory)
            .map(|entry| entry.path.clone())
            .collect();

        for path in entries_snapshot {
            if self.dir_sizes.contains_key(&path) || self.dir_sizes_loading.contains(&path) {
                continue;
            }
            self.dir_sizes_loading.insert(path.clone());
            let task_path = path.clone();
            tasks.push(Task::perform(
                async move {
                    // At most a few traversals at a time: each one saturates a
                    // blocking worker, and they would otherwise starve the
                    // thumbnail and preview tasks that the user is waiting on.
                    // RAII — the permit is held until this future completes.
                    let _permit = DIR_SIZE_LIMIT.acquire().await;
                    let (bytes, truncated) = tokio::task::spawn_blocking(move || {
                        let mut total: u64 = 0;
                        let mut count = 0usize;
                        let mut truncated = false;
                        let walker = walkdir::WalkDir::new(&task_path)
                            .min_depth(1)
                            .max_depth(MAX_DIR_SIZE_DEPTH);
                        for entry in walker {
                            if count >= MAX_DIR_SIZE_FILES {
                                truncated = true;
                                break;
                            }
                            let Ok(entry) = entry else { continue };
                            // A directory sitting exactly on the depth limit has
                            // children this walk will never see, so the total is
                            // a floor. Saying so is the difference between an
                            // approximation and a wrong number.
                            if entry.depth() >= MAX_DIR_SIZE_DEPTH && entry.file_type().is_dir() {
                                truncated = true;
                                continue;
                            }
                            // `file_type` vient de `readdir`, `metadata` est un
                            // appel système par entrée. Tester le type d'abord
                            // évite d'en payer un pour chaque sous-dossier, qui
                            // ne contribue rien au total.
                            if !entry.file_type().is_file() {
                                continue;
                            }
                            if let Ok(metadata) = entry.metadata() {
                                total += metadata.len();
                                count += 1;
                            }
                        }
                        (total, truncated)
                    })
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!("Taille dossier: tâche paniquée: {e}");
                        (0, false)
                    });
                    (path, bytes, truncated)
                },
                |(path, bytes, truncated)| UiMessage::DirSizeLoaded {
                    path,
                    bytes,
                    truncated,
                },
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
                node.expanded = current_path.starts_with(&node.path) && node.depth < TREE_MAX_DEPTH;
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
        self.cached_tree_nodes =
            build_tree_nodes(&tree_root, current_path, TREE_MAX_DEPTH, &tree_options);
    }
}
