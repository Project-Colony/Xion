//! Which entry buffer is current (live, stale during a refresh, or frozen
//! during a gesture), plus config reload and preview animation.
//!
//! Moved verbatim out of the single `impl XionApp` block in `mod.rs`.

use crate::filesystem::WatchEvent;
use crate::services::{DirectoryLoader, PreviewImageService, ThumbnailService};
use crate::ui::app::XionApp;
use crate::ui::app::types::PagedEntries;
use crate::ui::{RouteKind, UiMessage};
use iced::Task;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

impl XionApp {
    pub(in crate::ui::app) fn base_display_entries(&self) -> &PagedEntries {
        if self.is_refreshing {
            self.stale_entries.as_ref().unwrap_or(&self.entries)
        } else {
            &self.entries
        }
    }

    pub(in crate::ui::app) fn display_entries(&self) -> &PagedEntries {
        if self.is_user_selecting {
            if let Some(snapshot) = &self.selection_snapshot {
                snapshot
            } else {
                self.base_display_entries()
            }
        } else {
            self.base_display_entries()
        }
    }

    pub(in crate::ui::app) fn reload_config(&mut self) -> Task<UiMessage> {
        let load = self.config_manager.load();
        let new_config = load.config;
        if new_config == self.state.config {
            return Task::none();
        }

        let should_reset_loader = new_config.cache.directory_entries
            != self.state.config.cache.directory_entries
            || new_config.cache.directory_ttl_seconds
                != self.state.config.cache.directory_ttl_seconds
            || new_config.paging.page_size != self.state.config.paging.page_size;

        if should_reset_loader {
            self.directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
                new_config.cache.directory_entries,
                Duration::from_secs(new_config.cache.directory_ttl_seconds),
                new_config.paging.page_size,
            )));
            self.entries = PagedEntries::new(0, new_config.paging.page_size);
            self.pending_pages.clear();
        }

        if new_config.cache.thumbnail_entries != self.state.config.cache.thumbnail_entries
            || new_config.cache.thumbnail_ttl_seconds
                != self.state.config.cache.thumbnail_ttl_seconds
        {
            self.media.thumbnails = ThumbnailService::new(
                new_config.cache.thumbnail_entries,
                Duration::from_secs(new_config.cache.thumbnail_ttl_seconds),
            );
            self.media.thumbnail_handles.clear();
            self.media.thumbnail_misses.clear();
            self.media.thumbnails_in_flight.clear();
            let preview_cache_entries = new_config.cache.thumbnail_entries.clamp(1, 8);
            self.media.previews = PreviewImageService::new(
                preview_cache_entries,
                Duration::from_secs(new_config.cache.thumbnail_ttl_seconds),
            );
            self.media.preview_handles.clear();
            self.media.preview_misses.clear();
            self.media.previews_in_flight.clear();
        }

        if self
            .state
            .route
            .local_path()
            .is_some_and(|path| path == &self.state.config.start_path)
        {
            self.state.route.kind = RouteKind::Local(new_config.start_path.clone());
        }

        if !load.warnings.is_empty() {
            self.last_action = Some(format!(
                "Config: {}",
                load.warnings
                    .iter()
                    .map(|warning| warning.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        } else {
            self.last_action = Some("Config rechargée".to_string());
        }

        self.state.config = new_config;
        Task::none()
    }

    pub(in crate::ui::app) fn is_event_relevant(event: &WatchEvent, watched_path: &Path) -> bool {
        event.path == watched_path
            || event
                .path
                .parent()
                .is_some_and(|parent| parent == watched_path)
    }

    pub(in crate::ui::app) fn reset_preview_state(&mut self) {
        self.media.preview_handles.clear();
        self.media.preview_misses.clear();
        self.media.previews_in_flight.clear();
        self.media.previews.clear();
        self.media.animated = None;
        self.cached_text_preview = None;
    }

    pub(in crate::ui::app) fn advance_animated_preview(&mut self, now: Instant) {
        let Some(animated) = &mut self.media.animated else {
            return;
        };

        if animated.frames.is_empty() || now < animated.next_frame_at {
            return;
        }

        animated.current = (animated.current + 1) % animated.frames.len();
        let frame = &animated.frames[animated.current];
        animated.handle = frame.handle.clone(); // cheap Arc clone, no pixel copy
        animated.next_frame_at = now + frame.delay;
    }
}
