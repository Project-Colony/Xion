//! Row and tile builders for the file list.
//!
//! These were four closures inside `render_list`. As closures their return type
//! elided to the `&FsEntry` parameter rather than to `&self`, which is why the
//! name had to be cloned with `into_owned()`; as methods with an explicit `'a`
//! the borrow is fine.

use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, image, row, text};
use iced::{Alignment, Background, Element, Length, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::core::{ViewColumn, ViewMode};
use crate::filesystem::{FsEntry, FsEntryType};
use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;
use crate::ui::app::ColumnSpec;
use crate::ui::app::helpers::{
    entry_type_label, format_entry_size, format_modified, truncate_name,
};
use crate::ui::app::types::*;

/// Everything the row builders need beyond `&self`, resolved once per frame.
///
/// These used to be captured by four closures declared inside `render_list`,
/// which is what forced the whole thing to be one 660-line function.
#[derive(Clone, Copy)]
pub(super) struct RowCtx<'c> {
    pub(super) ui: ViewCtx,
    pub(super) view_mode: ViewMode,
    pub(super) row_height: f32,
    pub(super) columns: &'c [ColumnSpec],
}

use super::widgets::RADIUS;

impl XionApp {
    /// The icon or thumbnail shown at the start of a row or tile.
    pub(super) fn entry_leading<'a>(
        &'a self,
        cx: RowCtx<'_>,
        entry: &'a FsEntry,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing: _,
            typography,
        } = cx.ui;
        let view_mode = cx.view_mode;
        match entry.entry_type {
            FsEntryType::Directory => text(icons::FOLDER)
                .size(typography.body)
                .font(typography.body_font)
                .into(),
            FsEntryType::File if matches!(view_mode, ViewMode::Grid) => {
                // Grid mode: show thumbnail preview if available
                self.media
                    .thumbnail_handles
                    .get(&entry.path)
                    .map(|handle| {
                        image(handle.clone())
                            .width(Length::Fixed(self.state.config.view.thumbnail_size as f32))
                            .height(Length::Fixed(self.state.config.view.thumbnail_size as f32))
                            .into()
                    })
                    .unwrap_or_else(|| {
                        let icon = entry
                            .path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(icons::icon_for_extension)
                            .unwrap_or(icons::FILE);
                        text(icon)
                            .size(typography.body)
                            .font(typography.body_font)
                            .into()
                    })
            }
            FsEntryType::File => {
                // List mode: always use icon for uniform row height
                let icon = entry
                    .path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(icons::icon_for_extension)
                    .unwrap_or(icons::FILE);
                text(icon)
                    .size(typography.body)
                    .font(typography.body_font)
                    .into()
            }
            FsEntryType::Symlink => text(icons::SYMLINK)
                .size(typography.body)
                .font(typography.body_font)
                .into(),
            FsEntryType::Other => text(icons::UNKNOWN)
                .size(typography.body)
                .font(typography.body_font)
                .into(),
        }
    }

    /// One row of the detail view, one cell per configured column.
    pub(super) fn list_row<'a>(
        &'a self,
        cx: RowCtx<'_>,
        entry: &'a FsEntry,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = cx.ui;
        let column_specs = cx.columns;
        let mut entry_row = row![].spacing(spacing.md).align_y(Alignment::Center);
        for spec in column_specs {
            let cell: Element<'_, UiMessage> = match spec.column {
                ViewColumn::Name => {
                    // Feature F: File label dot
                    let label_dot: Option<Element<'_, UiMessage>> =
                        self.state.config.labels.get(&entry.path).map(|label| {
                            let dot_color = label.color();
                            container(row![])
                                .width(Length::Fixed(8.0))
                                .height(Length::Fixed(8.0))
                                .style(move |_| iced::widget::container::Style {
                                    background: Some(Background::Color(dot_color)),
                                    border: border::rounded(RADIUS.sm).width(0.0),
                                    ..Default::default()
                                })
                                .into()
                        });
                    // Feature 7: Git status badge
                    let git_badge: Option<Element<'_, UiMessage>> =
                        self.git_statuses.get(&entry.path).map(|status| {
                            use crate::ui::GitFileStatus;
                            let (label, color) = match status {
                                GitFileStatus::Modified => ("M", colors.accent),
                                GitFileStatus::Untracked => ("?", colors.text_muted),
                                GitFileStatus::Staged => ("S", colors.git_staged),
                                GitFileStatus::Conflict => ("!", colors.git_conflict),
                                GitFileStatus::Deleted => ("D", colors.git_deleted),
                            };
                            text(label)
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .color(color)
                                .into()
                        });
                    let is_cut = matches!(self.clipboard.kind, Some(ClipboardKind::Cut))
                        && self.clipboard.items.contains(&entry.path);
                    let mut name_row = row![self.entry_leading(cx, entry)]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center);
                    if let Some(dot) = label_dot {
                        name_row = name_row.push(dot);
                    }
                    // Borrowed: `text` takes a `Cow`, and now that this is a
                    // method with an explicit `'a` tied to `&self`, the
                    // borrow outlives the returned tree. Names under the
                    // limit — nearly all of them — no longer allocate.
                    let display_name = truncate_name(&entry.name, 60);
                    let name_text = text(display_name)
                        .size(typography.body)
                        .font(typography.body_font);
                    let name_text = if is_cut {
                        name_text.color(colors.text_muted)
                    } else {
                        name_text
                    };
                    name_row = name_row.push(name_text);
                    if let Some(badge) = git_badge {
                        name_row = name_row.push(badge);
                    }
                    name_row = name_row.push(horizontal_space());
                    container(name_row)
                        .width(spec.width)
                        .align_x(spec.align)
                        .into()
                }
                ViewColumn::Type => container(
                    text(entry_type_label(entry.entry_type))
                        .size(typography.caption)
                        .font(typography.caption_font),
                )
                .width(spec.width)
                .align_x(spec.align)
                .into(),
                ViewColumn::Size => {
                    // Feature 8: Show dir sizes
                    let size_str = if entry.entry_type == FsEntryType::Directory {
                        if let Some(&bytes) = self.dir_sizes.get(&entry.path) {
                            crate::ui::app::helpers::format_bytes(bytes)
                        } else if self.dir_sizes_loading.contains(&entry.path) {
                            "…".to_string()
                        } else {
                            "—".to_string()
                        }
                    } else {
                        format_entry_size(entry)
                    };
                    container(
                        text(size_str)
                            .size(typography.caption)
                            .font(typography.caption_font),
                    )
                    .width(spec.width)
                    .align_x(spec.align)
                    .into()
                }
                ViewColumn::Modified => container(
                    text(format_modified(entry.metadata.modified))
                        .size(typography.caption)
                        .font(typography.caption_font),
                )
                .width(spec.width)
                .align_x(spec.align)
                .into(),
            };
            entry_row = entry_row.push(cell);
        }
        entry_row.into()
    }

    /// Placeholder shown while a page is still loading.
    pub(super) fn loading_row<'a>(&'a self, cx: RowCtx<'_>) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing,
            typography,
        } = cx.ui;
        let column_specs = cx.columns;
        let row_height = cx.row_height;
        let mut placeholder_row = row![].spacing(spacing.md).align_y(Alignment::Center);
        for (index, spec) in column_specs.iter().enumerate() {
            let cell: Element<'_, UiMessage> = if index == 0 {
                let content = row![
                    text(icons::LOADING)
                        .size(typography.body)
                        .font(typography.body_font),
                    text("Chargement…")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center);
                container(content)
                    .width(spec.width)
                    .align_x(spec.align)
                    .into()
            } else {
                container(row![])
                    .width(spec.width)
                    .align_x(spec.align)
                    .into()
            };
            placeholder_row = placeholder_row.push(cell);
        }
        button(placeholder_row)
            .height(Length::Fixed(row_height))
            .into()
    }

    /// One tile of the grid view.
    pub(super) fn grid_tile<'a>(
        &'a self,
        cx: RowCtx<'_>,
        entry: &'a FsEntry,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing,
            typography,
        } = cx.ui;
        let grid_name = truncate_name(&entry.name, 20);
        let mut name_row = row![].spacing(spacing.xs).align_y(Alignment::Center);
        // Label dot in grid view
        if let Some(label) = self.state.config.labels.get(&entry.path) {
            let dot_color = label.color();
            name_row = name_row.push(
                container(row![])
                    .width(Length::Fixed(6.0))
                    .height(Length::Fixed(6.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(dot_color)),
                        border: border::rounded(3.0).width(0.0),
                        ..Default::default()
                    }),
            );
        }
        name_row = name_row.push(
            text(grid_name.into_owned())
                .size(typography.caption)
                .font(typography.caption_font),
        );
        let tile_content = column![self.entry_leading(cx, entry), name_row,]
            .spacing(spacing.xs)
            .align_x(Alignment::Center);
        container(tile_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    }
}
