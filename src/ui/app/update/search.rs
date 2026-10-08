//! Search box, quick filter, grep and full-text search.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::helpers::{self};
use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_search(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::SearchInputChanged(value) => {
                self.search.input = value;
                self.scroll.offset = 0.0;
                self.clear_selection();
                self.update_search_index_matches();
                if self.has_search_query() {
                    tasks.push(self.start_search_indexing());
                    tasks.push(self.request_all_pages());
                } else {
                    self.search.index = None;
                    self.search.index_path = None;
                    self.search.indexing = false;
                    tasks.push(self.ensure_visible_pages());
                }
            }
            UiMessage::SearchInputSubmitted => {
                // #23: Ctrl held = full-text content search
                if self.modifiers.control {
                    tasks.push(self.update(UiMessage::FullTextSearchSubmit));
                } else {
                    self.update_search_index_matches();
                    if self.has_search_query() {
                        tasks.push(self.start_search_indexing());
                        tasks.push(self.request_all_pages());
                    }
                }
            }
            UiMessage::SearchIndexBuilt { path, result } => {
                if self.search.index_path.as_ref() == Some(&path) {
                    match result {
                        Ok(index) => {
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
            UiMessage::OpenGrep => {
                let root = self
                    .state
                    .route
                    .local_path()
                    .cloned()
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
                        return Ok(Flow::Stop(Task::batch(std::mem::take(tasks))));
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
                                        if let Ok(content) = helpers::read_file_capped(
                                            &path,
                                            helpers::MAX_TEXT_PREVIEW_BYTES,
                                        ) {
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
            UiMessage::FullTextSearchSubmit => {
                let query = self.search.input.clone();
                if query.len() >= 2
                    && let Some(path) = self.state.route.local_path().cloned()
                {
                    let query_display = query.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let mut results = Vec::new();
                                let walker =
                                    ignore::WalkBuilder::new(&path).max_depth(Some(5)).build();
                                for entry in walker.flatten() {
                                    if results.len() >= 100 {
                                        break;
                                    }
                                    let p = entry.path();
                                    if !p.is_file() {
                                        continue;
                                    }
                                    // Only search text files (< 1MB)
                                    if let Ok(meta) = p.metadata()
                                        && meta.len() > 1_048_576
                                    {
                                        continue;
                                    }
                                    if let Ok(content) = helpers::read_text_capped(
                                        p,
                                        helpers::MAX_TEXT_PREVIEW_BYTES,
                                    ) {
                                        for (i, line) in content.lines().enumerate() {
                                            if results.len() >= 100 {
                                                break;
                                            }
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
            UiMessage::FullTextSearchResults(results) => {
                let count = results.len();
                self.last_action =
                    Some(format!("{count} résultat(s) de recherche dans le contenu"));
                // Reuse grep state for display
                self.grep_state = Some(GrepState {
                    query: self.search.input.clone(),
                    root: self.state.route.local_path().cloned().unwrap_or_default(),
                    results,
                    searching: false,
                });
            }
            // ── #24: Folder comparison ───────────────────────────────────────
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
                // La page de préférences affiche « enregistrées
                // automatiquement » ; sans ceci le réglage revenait au
                // démarrage suivant et la promesse était fausse.
                self.config_manager.save(&self.state.config);
                tasks.push(self.refresh_entries());
            }
            // ── UX: Sidebar accordion ─────────────────────────────────────────
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
