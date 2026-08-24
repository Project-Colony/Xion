//! The scrollable body of the file list, and the clickable column header above
//! it.
//!
//! Moved out of `render_list`, which now only assembles what these return.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::vertical as vertical_space;
use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Alignment, Background, Element, Length, Theme, border, mouse};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::core::{SortOrderConfig, ViewMode};
use crate::ui::UiMessage;

use super::XionApp;
use super::rows::RowCtx;
use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use crate::ui::app::helpers::column_specs;
use crate::ui::app::types::*;

impl XionApp {
    /// The rows or tiles themselves: the error state, the empty state, the
    /// loading state, and the virtualised window of real entries.
    pub(super) fn render_list_content<'a>(
        &'a self,
        ctx: ViewCtx,
        cx: RowCtx<'_>,
        display_entries: &'a PagedEntries,
        filtered_indices: &Option<Vec<usize>>,
        total_entries: usize,
        is_filtered: bool,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let view_mode = cx.view_mode;
        let row_height = cx.row_height;
        let entry_index_for = |display_index: usize| -> Option<usize> {
            if let Some(indices) = filtered_indices {
                indices.get(display_index).copied()
            } else {
                Some(display_index)
            }
        };

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier")
                    .size(typography.title)
                    .font(typography.title_font),
                text(message)
                    .size(typography.body)
                    .font(typography.body_font),
                button(
                    text("Réessayer")
                        .size(typography.body)
                        .font(typography.body_font),
                )
                .on_press(UiMessage::Refresh)
            ]
            .spacing(spacing.sm)
        } else if total_entries == 0 && !self.is_loading {
            if is_filtered {
                column![
                    text("Aucun résultat")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
            } else {
                column![
                    text("Dossier vide")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
            }
        } else if total_entries == 0 {
            column![]
        } else {
            match view_mode {
                ViewMode::List => {
                    let window = self.list_virtual_window_for(total_entries);
                    let mut list = column![];

                    if window.padding_top > 0.0 {
                        list =
                            list.push(vertical_space().height(Length::Fixed(window.padding_top)));
                    }

                    for display_index in window.start..window.end {
                        let Some(actual_index) = entry_index_for(display_index) else {
                            continue;
                        };
                        let entry = display_entries.get(actual_index);
                        if let Some(entry) = entry {
                            let is_selected = self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&entry.path);
                            let is_focused = self
                                .state
                                .navigation
                                .selection
                                .focused
                                .as_ref()
                                .map(|path| path == &entry.path)
                                .unwrap_or(false);
                            let message = UiMessage::SelectEntry {
                                path: entry.path.clone(),
                                kind: self.selection_kind_from_modifiers(),
                            };
                            let context_path = entry.path.clone();
                            let pressed_path = entry.path.clone();
                            list = list.push(
                                mouse_area(
                                    button(self.list_row(cx, entry))
                                        .padding([spacing.xs, spacing.sm])
                                        .height(Length::Fixed(row_height))
                                        .style(move |_theme: &Theme, status: ButtonStatus| {
                                            let mut style = iced::widget::button::Style {
                                                text_color: colors.text_primary,
                                                ..Default::default()
                                            };

                                            if is_selected {
                                                style.background =
                                                    Some(Background::Color(colors.selection));
                                                style.border = border::rounded(RADIUS.md)
                                                    .color(colors.selection_border)
                                                    .width(if is_focused { 2.0 } else { 1.0 });
                                            }

                                            if matches!(status, ButtonStatus::Hovered) {
                                                style.background =
                                                    Some(Background::Color(colors.hover));
                                            }

                                            if matches!(status, ButtonStatus::Pressed) {
                                                style.background =
                                                    Some(Background::Color(colors.pressed));
                                            }

                                            style
                                        })
                                        .on_press(message),
                                )
                                .on_press(UiMessage::EntryPressed(pressed_path))
                                .on_right_press(UiMessage::OpenContextMenuForEntry(context_path)),
                            );
                        } else {
                            list = list.push(self.loading_row(cx));
                        }
                    }

                    if window.padding_bottom > 0.0 {
                        list = list
                            .push(vertical_space().height(Length::Fixed(window.padding_bottom)));
                    }

                    list
                }
                ViewMode::Grid => {
                    let grid = self.grid_window_for(total_entries);
                    let mut list = column![];
                    let tile_height = self.state.config.view.grid_row_height;

                    if grid.window.padding_top > 0.0 {
                        list = list
                            .push(vertical_space().height(Length::Fixed(grid.window.padding_top)));
                    }

                    for row_index in grid.window.start..grid.window.end {
                        let mut tile_row = row![].spacing(spacing.md);
                        for column_index in 0..grid.columns {
                            let display_index = row_index * grid.columns + column_index;
                            if display_index >= grid.total {
                                tile_row = tile_row.push(
                                    container(row![])
                                        .width(Length::FillPortion(1))
                                        .height(Length::Fixed(tile_height)),
                                );
                                continue;
                            }
                            let Some(actual_index) = entry_index_for(display_index) else {
                                continue;
                            };
                            let entry = display_entries.get(actual_index);
                            let tile_element: Element<'_, UiMessage> = match entry {
                                Some(entry) => {
                                    let is_selected = self
                                        .state
                                        .navigation
                                        .selection
                                        .selected
                                        .contains(&entry.path);
                                    let is_focused = self
                                        .state
                                        .navigation
                                        .selection
                                        .focused
                                        .as_ref()
                                        .map(|path| path == &entry.path)
                                        .unwrap_or(false);
                                    let message = UiMessage::SelectEntry {
                                        path: entry.path.clone(),
                                        kind: self.selection_kind_from_modifiers(),
                                    };
                                    let context_path = entry.path.clone();
                                    let pressed_path = entry.path.clone();
                                    mouse_area(
                                        container(
                                            button(self.grid_tile(cx, entry))
                                                .width(Length::Fill)
                                                .height(Length::Fill)
                                                .padding(spacing.sm)
                                                .style(
                                                    move |_theme: &Theme, status: ButtonStatus| {
                                                        let mut style =
                                                            iced::widget::button::Style {
                                                                text_color: colors.text_primary,
                                                                ..Default::default()
                                                            };

                                                        if is_selected {
                                                            style.background = Some(
                                                                Background::Color(colors.selection),
                                                            );
                                                            style.border =
                                                                border::rounded(RADIUS.lg)
                                                                    .color(colors.selection_border)
                                                                    .width(if is_focused {
                                                                        2.0
                                                                    } else {
                                                                        1.0
                                                                    });
                                                        }

                                                        if matches!(status, ButtonStatus::Hovered) {
                                                            style.background = Some(
                                                                Background::Color(colors.hover),
                                                            );
                                                        }

                                                        if matches!(status, ButtonStatus::Pressed) {
                                                            style.background = Some(
                                                                Background::Color(colors.pressed),
                                                            );
                                                        }

                                                        style
                                                    },
                                                )
                                                .on_press(message),
                                        )
                                        .width(Length::FillPortion(1))
                                        .height(Length::Fixed(tile_height)),
                                    )
                                    .on_press(UiMessage::EntryPressed(pressed_path))
                                    .on_right_press(UiMessage::OpenContextMenuForEntry(
                                        context_path,
                                    ))
                                    .into()
                                }
                                None => container(row![])
                                    .width(Length::FillPortion(1))
                                    .height(Length::Fixed(tile_height))
                                    .into(),
                            };
                            tile_row = tile_row.push(tile_element);
                        }
                        list = list.push(tile_row);
                    }

                    if grid.window.padding_bottom > 0.0 {
                        list = list.push(
                            vertical_space().height(Length::Fixed(grid.window.padding_bottom)),
                        );
                    }

                    list
                }
            }
        };

        list_content.into()
    }

    /// The sortable column header. Absent in grid mode and while the list is
    /// empty.
    pub(super) fn render_list_header(&self, ctx: ViewCtx, visible: bool) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let column_specs = column_specs(&self.state.config.view.columns);
        let row_height = self.state.config.view.row_height;
        let list_header_visible = visible;

        let list_header: Element<'_, UiMessage> = if list_header_visible {
            let mut header_row = row![].spacing(spacing.md).align_y(Alignment::Center);
            for spec in &column_specs {
                let is_active_sort = spec
                    .sort_key
                    .is_some_and(|key| key == self.state.config.list.sort_key);
                let sort_indicator = if is_active_sort {
                    match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => "↑",
                        SortOrderConfig::Desc => "↓",
                    }
                } else {
                    ""
                };
                let label = if sort_indicator.is_empty() {
                    spec.label.to_string()
                } else {
                    format!("{} {}", spec.label, sort_indicator)
                };
                let header_text = text(label)
                    .size(typography.caption)
                    .font(typography.caption_font);
                let cell: Element<'_, UiMessage> = if let Some(sort_key) = spec.sort_key {
                    button(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                            }

                            style
                        })
                        .on_press(UiMessage::ChangeSort(sort_key))
                        .into()
                } else {
                    container(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .into()
                };
                header_row = header_row.push(container(cell).width(spec.width).align_x(spec.align));
                // Feature H: resize handle between columns
                let col_name = spec.label.to_string();
                let is_resizing = self
                    .column_resize_state
                    .as_ref()
                    .is_some_and(|r| r.column == col_name);
                let handle_color = if is_resizing {
                    colors.accent
                } else {
                    colors.border
                };
                header_row = header_row.push(
                    mouse_area(
                        container(row![])
                            .width(Length::Fixed(4.0))
                            .height(Length::Fill)
                            .style(move |_| iced::widget::container::Style {
                                background: Some(Background::Color(handle_color)),
                                ..Default::default()
                            }),
                    )
                    .on_press(UiMessage::ColumnResizeStart(col_name))
                    .on_release(UiMessage::ColumnResizeEnd)
                    .interaction(mouse::Interaction::ResizingHorizontally),
                );
            }

            container(header_row)
                .padding([spacing.xs, spacing.sm])
                .height(Length::Fixed(row_height))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.chrome_background)),
                    border: border::rounded(RADIUS.md).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            container(row![]).into()
        };

        list_header
    }
}
