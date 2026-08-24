//! The preview panel body: metadata, image, text or hex rendering for the
//! selected entry.
//!
//! Moved out of `view()` unchanged. The surrounding panel and its slide
//! animation stay in `view()`, which owns the layout.

use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{column, container, image, row, scrollable, text};
use iced::{Alignment, Background, Color, Element, Length, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::filesystem::FsEntryType;
use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;
use crate::ui::app::helpers::{entry_type_label, format_entry_size, format_modified};
use crate::ui::app::types::*;

use super::widgets::RADIUS;

impl XionApp {
    /// `display_entries` is resolved once in `view()` and passed in rather than
    /// re-resolved: it arbitrates between the live, stale and frozen buffers.
    pub(super) fn render_preview_body<'a>(
        &'a self,
        ctx: ViewCtx,
        display_entries: &'a PagedEntries,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let preview_row = |label: String, value: String| {
            row![
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                )
                .width(Length::Fixed(90.0)),
                text(value)
                    .size(typography.caption)
                    .font(typography.caption_font)
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center)
        };

        let preview_entry = self.selected_entry(display_entries);
        let preview_body: Element<'_, UiMessage> = match preview_entry {
            Some(entry) => {
                let icon = match entry.entry_type {
                    FsEntryType::Directory => icons::FOLDER,
                    FsEntryType::File => entry
                        .path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(icons::icon_for_extension)
                        .unwrap_or(icons::FILE),
                    FsEntryType::Symlink => icons::SYMLINK,
                    FsEntryType::Other => icons::UNKNOWN,
                };
                let preview_media_size =
                    (self.state.config.view.thumbnail_size as f32 * 3.0).clamp(120.0, 220.0);
                let preview_media: Element<'_, UiMessage> = match entry.entry_type {
                    FsEntryType::File => {
                        if let Some(animated) = self
                            .media
                            .animated
                            .as_ref()
                            .filter(|animated| animated.path == entry.path)
                        {
                            image(animated.handle.clone())
                                .width(Length::Fixed(preview_media_size))
                                .height(Length::Fixed(preview_media_size))
                                .into()
                        } else {
                            self.media
                                .preview_handles
                                .get(&entry.path)
                                .map(|handle| {
                                    image(handle.clone())
                                        .width(Length::Fixed(preview_media_size))
                                        .height(Length::Fixed(preview_media_size))
                                        .into()
                                })
                                .unwrap_or_else(|| {
                                    text(icons::LOADING)
                                        .size(typography.title)
                                        .font(typography.title_font)
                                        .into()
                                })
                        }
                    }
                    _ => text(icon)
                        .size(typography.title)
                        .font(typography.title_font)
                        .into(),
                };

                let metadata = column![
                    preview_row(
                        "Type".to_string(),
                        entry_type_label(entry.entry_type).to_string()
                    ),
                    preview_row("Taille".to_string(), format_entry_size(entry)),
                    preview_row(
                        "Modifié".to_string(),
                        format_modified(entry.metadata.modified),
                    ),
                    preview_row("Créé".to_string(), format_modified(entry.metadata.created)),
                    preview_row(
                        "Accès".to_string(),
                        format_modified(entry.metadata.accessed)
                    ),
                    preview_row(
                        "Lecture seule".to_string(),
                        if entry.metadata.readonly {
                            "Oui".to_string()
                        } else {
                            "Non".to_string()
                        },
                    ),
                ]
                .spacing(spacing.xs);

                // Feature 6: Text preview with syntax highlighting
                let text_preview_section: Element<'_, UiMessage> = self
                    .cached_text_preview
                    .as_ref()
                    .filter(|(p, _)| p == &entry.path)
                    .map(|(_, content)| {
                        // Check if we have highlighted lines for this path
                        let is_pdf = entry
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("pdf"))
                            .unwrap_or(false);
                        let has_highlight = self
                            .cached_highlighted_preview
                            .as_ref()
                            .is_some_and(|(p, _)| p == &entry.path);

                        let inner: Element<'_, UiMessage> = if has_highlight {
                            if let Some((_, lines)) = &self.cached_highlighted_preview {
                                let mut lines_col = column![].spacing(0);
                                for line in lines {
                                    let mut line_row = row![].spacing(0);
                                    for (rgba, span_text) in &line.spans {
                                        let r = (rgba >> 24) as u8;
                                        let g = (rgba >> 16) as u8;
                                        let b = (rgba >> 8) as u8;
                                        let a = *rgba as u8;
                                        let color = Color::from_rgba8(r, g, b, a as f32 / 255.0);
                                        line_row = line_row.push(
                                            text(span_text.as_str())
                                                .size(typography.caption)
                                                .font(typography.body_font)
                                                .color(color),
                                        );
                                    }
                                    lines_col = lines_col.push(line_row);
                                }
                                lines_col.into()
                            } else {
                                text(content.as_str())
                                    .size(typography.caption)
                                    .font(typography.body_font)
                                    .into()
                            }
                        } else {
                            let label = if is_pdf {
                                format!("(PDF - texte extrait)\n{}", content)
                            } else {
                                content.clone()
                            };
                            text(label)
                                .size(typography.caption)
                                .font(typography.body_font)
                                .into()
                        };

                        let preview_text = container(
                            scrollable(container(inner).padding(spacing.sm).width(Length::Fill))
                                .height(Length::Fixed(200.0)),
                        )
                        .style(move |_| iced::widget::container::Style {
                            background: Some(Background::Color(colors.chrome_background)),
                            border: border::rounded(RADIUS.md).color(colors.border).width(1.0),
                            ..Default::default()
                        })
                        .width(Length::Fill);
                        let el: Element<'_, UiMessage> = preview_text.into();
                        el
                    })
                    .unwrap_or_else(|| container(row![]).into());

                // Feature L: encoding badge
                let encoding_badge: Element<'_, UiMessage> =
                    if let Some(enc) = &self.preview_encoding {
                        text(enc.as_str())
                            .size(typography.caption)
                            .font(typography.caption_font)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(colors.text_muted),
                            })
                            .into()
                    } else {
                        row![].into()
                    };

                column![
                    preview_media,
                    row![
                        text(&entry.name)
                            .size(typography.body)
                            .font(typography.body_font),
                        horizontal_space(),
                        encoding_badge,
                    ]
                    .align_y(Alignment::Center)
                    .spacing(spacing.xs),
                    text(entry.path.display().to_string())
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                    text_preview_section,
                    metadata
                ]
                .spacing(spacing.sm)
                .align_x(Alignment::Center)
                .into()
            }
            None if self.state.navigation.selection.selected.is_empty() => column![
                text("Sélectionnez un élément")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
            None => column![
                text("Aperçu en cours de chargement…")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
        };

        preview_body
    }
}
