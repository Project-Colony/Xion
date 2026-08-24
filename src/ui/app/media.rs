//! Thumbnail and preview requests for the visible rows and the selection.
//!
//! Moved verbatim out of the single 809-line `impl XionApp` block in `mod.rs`.

use crate::filesystem::FsEntryType;
use crate::services::{generate_preview, generate_thumbnail};
use crate::ui::UiMessage;
use iced::Task;
use iced::widget::image;
use std::path::PathBuf;

use super::XionApp;

impl XionApp {
    pub(in crate::ui::app) fn request_visible_thumbnails(&mut self) -> Task<UiMessage> {
        // The only reader of `media.thumbnail_handles` is the grid tile
        // builder, behind `matches!(view_mode, ViewMode::Grid)`. In list mode —
        // the default — every decode was thrown away: up to 40 concurrent
        // `spawn_blocking` image decodes per navigation, for pixels nothing
        // would ever draw.
        if !matches!(self.state.config.view.mode, crate::core::ViewMode::Grid) {
            return Task::none();
        }
        let thumbnail_size = self.state.config.view.thumbnail_size;

        // Resolve the on-screen rows first and copy out their paths: everything
        // afterwards mutates `self.media`, which cannot coexist with a borrow of
        // the entry buffer. The two branches this replaces were byte-identical
        // apart from index resolution, and the filtered one consulted a
        // search-only helper — so thumbnails were still generated for rows the
        // quick filter had hidden.
        let candidates: Vec<std::path::PathBuf> = {
            let visible = self.visible_indices();
            let entries = self.display_entries();
            let count = visible.as_ref().map_or(entries.total, Vec::len);
            if count == 0 {
                return Task::none();
            }
            let window = self.entry_virtual_window_for(count);
            if window.is_empty() {
                return Task::none();
            }
            (window.start..window.end)
                .filter_map(|position| match &visible {
                    Some(indices) => indices.get(position).copied(),
                    None => Some(position),
                })
                .filter_map(|index| entries.get(index))
                .filter(|entry| entry.entry_type == FsEntryType::File)
                .map(|entry| entry.path.clone())
                .collect()
        };

        let mut tasks = Vec::new();
        for path in candidates {
            if let Some(thumbnail) = self.media.thumbnails.get(&path) {
                if !self.media.thumbnail_handles.contains_key(&path) {
                    // `Bytes::clone` is a refcount bump: the handle and the cache now
                    // point at the same buffer instead of holding one each.
                    let handle = image::Handle::from_bytes(thumbnail.bytes.clone());
                    self.media.thumbnail_handles.insert(path.clone(), handle);
                }
                continue;
            }

            self.media.thumbnail_handles.remove(&path);

            if self.media.thumbnail_misses.contains(&path)
                || self.media.thumbnails_in_flight.contains(&path)
            {
                continue;
            }

            self.media.thumbnails_in_flight.insert(path.clone());
            tasks.push(Task::perform(
                async move {
                    // Decoding an image and shelling out to ffmpeg/mutool both
                    // block. Running them directly inside the future occupied a
                    // tokio worker for the whole decode, starving every other
                    // load; `spawn_blocking` is what the rest of the codebase
                    // already uses for this.
                    let generated = tokio::task::spawn_blocking({
                        let path = path.clone();
                        move || {
                            generate_thumbnail(path.as_path(), thumbnail_size)
                                .or_else(|| {
                                    crate::services::generate_video_thumbnail(
                                        path.as_path(),
                                        thumbnail_size,
                                    )
                                })
                                .or_else(|| {
                                    crate::services::generate_pdf_thumbnail(
                                        path.as_path(),
                                        thumbnail_size,
                                    )
                                })
                        }
                    })
                    .await
                    .ok()
                    .flatten();
                    (path, generated)
                },
                |(path, thumbnail)| UiMessage::ThumbnailLoaded { path, thumbnail },
            ));
        }

        Task::batch(tasks)
    }

    pub(in crate::ui::app) fn preview_image_size(&self) -> u32 {
        let base_size = self.state.config.view.thumbnail_size;
        base_size.saturating_mul(4).clamp(256, 512)
    }

    pub(in crate::ui::app) fn request_selected_preview(&mut self) -> Task<UiMessage> {
        let Some(entry) = self.selected_entry(&self.entries) else {
            return Task::none();
        };
        let entry_path = entry.path.clone();
        let entry_type = entry.entry_type;

        if entry_type != FsEntryType::File {
            return Task::none();
        }

        let mut tasks = Vec::new();

        // Request text preview for text-previewable files
        if Self::is_text_previewable(&entry_path) {
            let already_cached = self
                .cached_text_preview
                .as_ref()
                .is_some_and(|(p, _)| p == &entry_path);
            if !already_cached {
                let path = entry_path.clone();
                let is_pdf = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);
                if is_pdf {
                    // Feature 12: PDF text extraction
                    let err_path = path.clone();
                    tasks.push(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                pdf_extract::extract_text(&path)
                                    .ok()
                                    .map(|text| (path, text))
                            })
                            .await
                            .ok()
                            .flatten()
                        },
                        move |result| match result {
                            Some((path, content)) => UiMessage::TextPreviewLoaded {
                                path,
                                content,
                                encoding: Some("PDF".to_string()),
                            },
                            None => UiMessage::TextPreviewLoaded {
                                path: err_path,
                                content: "Impossible d'extraire le texte du PDF".to_string(),
                                encoding: None,
                            },
                        },
                    ));
                } else {
                    tasks.push(Task::perform(
                        async move {
                            let bytes = super::helpers::read_file_capped(
                                &path,
                                super::helpers::MAX_PREVIEW_IMAGE_BYTES,
                            )
                            .ok();
                            (path, bytes)
                        },
                        |(path, bytes): (PathBuf, Option<Vec<u8>>)| match bytes {
                            Some(bytes) => {
                                // Detect encoding
                                let (content, encoding) = if let Ok(s) = std::str::from_utf8(&bytes)
                                {
                                    (s.to_string(), "UTF-8".to_string())
                                } else {
                                    let (cow, encoding, _) =
                                        encoding_rs::WINDOWS_1252.decode(&bytes);
                                    (cow.into_owned(), encoding.name().to_string())
                                };
                                // Limit to first ~4000 chars, snapping to a char boundary
                                let truncated = if content.len() > 4000 {
                                    let end = content
                                        .char_indices()
                                        .map(|(i, _)| i)
                                        .take_while(|&i| i <= 4000)
                                        .last()
                                        .unwrap_or(0);
                                    format!("{}…", &content[..end])
                                } else {
                                    content
                                };
                                UiMessage::TextPreviewLoaded {
                                    path,
                                    content: truncated,
                                    encoding: Some(encoding),
                                }
                            }
                            None => {
                                tracing::warn!("Preview: lecture fichier {:?} échouée", path);
                                UiMessage::TextPreviewLoaded {
                                    path,
                                    content: "Impossible de lire le fichier".to_string(),
                                    encoding: None,
                                }
                            }
                        },
                    ));
                }
            }
        }

        // Request image preview
        if let Some(animated) = &self.media.animated {
            if animated.path == entry_path {
                return Task::batch(tasks);
            }
        }

        if let Some(preview) = self.media.previews.get(&entry_path) {
            if !self.media.preview_handles.contains_key(&entry_path) {
                self.media.preview_handles.insert(
                    entry_path.clone(),
                    image::Handle::from_bytes(preview.bytes.clone()),
                );
            }
            return Task::batch(tasks);
        }

        if self.media.preview_handles.contains_key(&entry_path)
            || self.media.preview_misses.contains(&entry_path)
            || self.media.previews_in_flight.contains(&entry_path)
        {
            return Task::batch(tasks);
        }

        let path = entry_path.clone();
        let preview_size = self.preview_image_size();
        self.media.previews_in_flight.insert(path.clone());
        tasks.push(Task::perform(
            async move {
                let preview = generate_preview(path.as_path(), preview_size);
                (path, preview)
            },
            |(path, preview)| UiMessage::PreviewLoaded { path, preview },
        ));
        Task::batch(tasks)
    }

    // ── Test-only state accessors ─────────────────────────────────────────────
    // These are compiled unconditionally so that integration tests in tests/
    // can call them. They are named *_for_test or have obvious test semantics.
}
