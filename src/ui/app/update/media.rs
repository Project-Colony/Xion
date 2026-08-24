//! Thumbnails, previews and syntax highlighting results.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use iced::Task;
use iced::widget::image;

use crate::ui::UiMessage;

use crate::ui::app::helpers::{build_animated_preview, is_gif_preview};

use super::Flow;
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_media(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
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
            UiMessage::AnimatedPreviewTick(now) => {
                self.advance_animated_preview(now);
            }
            UiMessage::TextPreviewLoaded {
                path,
                content,
                encoding,
            } => {
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
            UiMessage::TextHighlightComplete { path, lines } => {
                self.cached_highlighted_preview = Some((path, lines));
            }
            // Feature 7: Git status
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
