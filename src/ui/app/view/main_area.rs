//! The centre of the window: the Recent view, the archive browser, or the
//! ordinary file list, whichever the current route calls for.
//!
//! Moved out of `view()` unchanged.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Element, Length, Theme};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;

use super::widgets::RADIUS;
use super::widgets::chrome_style;
use super::widgets::surface_style;

impl XionApp {
    /// `list` is built by the caller and consumed here: only one of the three
    /// branches actually shows it.
    pub(super) fn render_main_area<'a>(
        &'a self,
        ctx: ViewCtx,
        list: Element<'a, UiMessage>,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let main_list_element: Element<'_, UiMessage> = if self.state.route.is_recent() {
            let recents = self.recents.list();
            let recent_rows: Vec<Element<'_, UiMessage>> = if recents.is_empty() {
                vec![
                    text("Aucun fichier récent")
                        .size(typography.body)
                        .font(typography.body_font)
                        .into(),
                ]
            } else {
                recents
                    .iter()
                    .map(|entry| {
                        let icon = entry
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(icons::icon_for_extension)
                            .unwrap_or(icons::FILE);
                        let name = entry
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        let parent = entry
                            .path
                            .parent()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        let accessed_str = {
                            let dt: chrono::DateTime<chrono::Local> = entry.accessed.into();
                            dt.format("%d/%m/%Y %H:%M").to_string()
                        };
                        let path = entry.path.clone();
                        button(
                            row![
                                text(icon).size(typography.body).font(typography.body_font),
                                column![
                                    text(name).size(typography.body).font(typography.body_font),
                                    row![
                                        text(parent)
                                            .size(typography.caption)
                                            .font(typography.caption_font)
                                            .style(move |_| iced::widget::text::Style {
                                                color: Some(colors.text_muted)
                                            }),
                                        text(accessed_str)
                                            .size(typography.caption)
                                            .font(typography.caption_font)
                                            .style(move |_| iced::widget::text::Style {
                                                color: Some(colors.text_muted)
                                            }),
                                    ]
                                    .spacing(spacing.md),
                                ]
                                .spacing(2),
                            ]
                            .spacing(spacing.sm)
                            .align_y(Alignment::Center),
                        )
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(
                            move |_: &Theme, status: ButtonStatus| iced::widget::button::Style {
                                text_color: colors.text_primary,
                                background: if matches!(status, ButtonStatus::Hovered) {
                                    Some(Background::Color(colors.hover))
                                } else {
                                    None
                                },
                                ..Default::default()
                            },
                        )
                        .on_press(UiMessage::ActivateEntry(path))
                        .into()
                    })
                    .collect()
            };
            let header = container(
                row![
                    text("Fichiers récents")
                        .size(typography.body)
                        .font(typography.body_font),
                    horizontal_space(),
                    button(
                        text("Effacer")
                            .size(typography.caption)
                            .font(typography.caption_font)
                    )
                    .on_press(UiMessage::ClearRecents)
                    .padding([spacing.xs, spacing.sm])
                    .style(
                        move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        }
                    ),
                ]
                .align_y(Alignment::Center)
                .spacing(spacing.sm),
            )
            .padding([spacing.xs, spacing.sm])
            .style(chrome_style(colors, RADIUS.md));
            container(
                column![
                    header,
                    scrollable(column(recent_rows).spacing(0)).height(Length::Fill)
                ]
                .spacing(spacing.xs),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(surface_style(colors, RADIUS.xl))
            .into()
        } else if let Some(archive) = &self.archive_browser {
            let archive_path_str = archive
                .archive_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Archive")
                .to_string();
            let inner_str = if archive.inner_path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", archive.inner_path)
            };

            let archive_entries: Vec<Element<'_, UiMessage>> = archive
                .entries
                .iter()
                .filter(|e| {
                    // Show only entries directly inside the current inner_path folder.
                    // Normalize backslashes to forward slashes for consistent handling.
                    let path = e.inner_path.replace('\\', "/");
                    if archive.inner_path.is_empty() {
                        // Root level: no '/' (files) or exactly one trailing '/' (dirs)
                        !path.contains('/')
                            || (path.ends_with('/') && path.matches('/').count() == 1)
                    } else {
                        let inner = archive.inner_path.replace('\\', "/");
                        // Must start with "<inner_path>/" to avoid matching siblings
                        let prefix = format!("{}/", inner.trim_end_matches('/'));
                        path.starts_with(&prefix)
                    }
                })
                .map(|entry| {
                    let icon = if entry.is_dir {
                        icons::FOLDER
                    } else {
                        entry
                            .inner_path
                            .rsplit('.')
                            .next()
                            .map(icons::icon_for_extension)
                            .unwrap_or(icons::FILE)
                    };
                    let size_str = if entry.is_dir {
                        "—".to_string()
                    } else {
                        crate::ui::app::helpers::format_bytes(entry.size)
                    };
                    let entry_inner = entry.inner_path.clone();
                    let archive_path_clone = archive.archive_path.clone();
                    let dest_dir = std::env::temp_dir();
                    let on_press = if entry.is_dir {
                        UiMessage::ArchiveFolderOpen {
                            inner_path: entry_inner,
                        }
                    } else {
                        UiMessage::ExtractArchiveEntry {
                            archive: archive_path_clone,
                            inner_path: entry_inner,
                            dest_dir,
                        }
                    };
                    button(
                        row![
                            text(icon).size(typography.body).font(typography.body_font),
                            text(entry.name.clone())
                                .size(typography.body)
                                .font(typography.body_font),
                            horizontal_space(),
                            text(size_str)
                                .size(typography.caption)
                                .font(typography.caption_font),
                        ]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center),
                    )
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fill)
                    .style(
                        move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_primary,
                            ..Default::default()
                        },
                    )
                    .on_press(on_press)
                    .into()
                })
                .collect();

            // Build parent inner_path for the "go up" button
            let parent_inner_path: Option<String> = if archive.inner_path.is_empty() {
                None
            } else {
                let parts: Vec<&str> = archive
                    .inner_path
                    .trim_end_matches('/')
                    .split('/')
                    .collect();
                if parts.len() <= 1 {
                    Some(String::new())
                } else {
                    Some(parts[..parts.len() - 1].join("/"))
                }
            };
            let mut archive_header_row = row![].spacing(spacing.sm).align_y(Alignment::Center);
            if let Some(parent) = parent_inner_path {
                archive_header_row = archive_header_row.push(
                    button(text("↑").size(typography.body).font(typography.body_font))
                        .on_press(UiMessage::ArchiveFolderOpen { inner_path: parent })
                        .padding([spacing.xs, spacing.sm])
                        .style(
                            move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                                text_color: colors.text_muted,
                                ..Default::default()
                            },
                        ),
                );
            }
            archive_header_row = archive_header_row
                .push(
                    text(format!(
                        "{} {} [{}]{}",
                        icons::FILE_ARCHIVE,
                        archive_path_str,
                        match archive.archive_type {
                            crate::ui::app::types::ArchiveType::Zip => "ZIP",
                            crate::ui::app::types::ArchiveType::TarGz => "TAR.GZ",
                            crate::ui::app::types::ArchiveType::SevenZ => "7Z",
                        },
                        inner_str
                    ))
                    .size(typography.body)
                    .font(typography.body_font),
                )
                .push(horizontal_space())
                .push(
                    button(
                        text(icons::CLOSE)
                            .size(typography.body)
                            .font(typography.body_font),
                    )
                    .on_press(UiMessage::CloseArchiveBrowser)
                    .padding([spacing.xs, spacing.sm])
                    .style(
                        move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        },
                    ),
                );
            let archive_header = container(archive_header_row)
                .padding([spacing.xs, spacing.sm])
                .style(chrome_style(colors, RADIUS.md));

            container(
                column![
                    archive_header,
                    scrollable(column(archive_entries).spacing(0)).height(Length::Fill),
                ]
                .spacing(spacing.xs),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(surface_style(colors, RADIUS.xl))
            .into()
        } else {
            container(list)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(surface_style(colors, RADIUS.xl))
                .into()
        };

        main_list_element
    }
}
