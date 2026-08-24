//! The file list itself: column header, rows, grid tiles, loading placeholders
//! and the rename prompt drawn in place.
//!
//! Moved out of `view()` unchanged. This was the largest single section of that
//! function, and the closures it defines are used nowhere else.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::vertical as vertical_space;
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Alignment, Background, Element, Length, Theme, border, mouse};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::core::{SortOrderConfig, ViewMode};
use crate::ui::{ScrollViewport, UiMessage};

use super::XionApp;
use super::widgets::{self, ViewCtx};
use crate::ui::app::helpers::column_specs;
use crate::ui::app::types::*;

use super::widgets::RADIUS;

use super::rows::RowCtx;

impl XionApp {
    /// `display_entries` and `filtered_indices` are resolved once by `view()`
    /// and passed in: both are O(n) over the directory, and the status bar
    /// needs the same values.
    pub(super) fn render_list<'a>(
        &'a self,
        ctx: ViewCtx,
        display_entries: &'a PagedEntries,
        // Deliberately not tied to `'a`: the rows copy the indices they need,
        // so the returned tree must not borrow the caller's Vec.
        filtered_indices: &Option<Vec<usize>>,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let column_specs = column_specs(&self.state.config.view.columns);
        let view_mode = self.state.config.view.mode;
        let row_height = self.state.config.view.row_height;
        let cx = RowCtx {
            ui: ctx,
            view_mode,
            row_height,
            columns: &column_specs,
        };
        let is_filtered = filtered_indices.is_some();
        let total_entries = filtered_indices
            .as_ref()
            .map_or(display_entries.total, |indices| indices.len());
        let entry_index_for = |display_index: usize| -> Option<usize> {
            if let Some(indices) = &filtered_indices {
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

        let list_header_visible =
            self.list_header_visible(view_mode, display_entries, filtered_indices.as_ref());
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

        let rename_prompt = if let Some(dialog) = &self.rename_dialog {
            let input = text_input("Nouveau nom…", &dialog.input)
                .on_input(UiMessage::RenameInputChanged)
                .on_submit(UiMessage::RenameSubmit)
                .size(typography.body)
                .font(typography.body_font)
                .padding([spacing.xs, spacing.md]);
            container(
                row![
                    text("Renommer :")
                        .size(typography.body)
                        .font(typography.body_font),
                    input,
                    widgets::toolbar_button(ctx, "Valider".to_string())
                        .on_press(UiMessage::RenameSubmit),
                    widgets::toolbar_button(ctx, "Annuler".to_string())
                        .on_press(UiMessage::RenameCancel)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
            )
            .padding([spacing.sm, spacing.md])
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(RADIUS.lg).color(colors.border).width(1.0),
                ..Default::default()
            })
        } else {
            container(row![])
        };

        let mut list_column = column![].spacing(spacing.xl);
        if self.rename_dialog.is_some() {
            list_column = list_column.push(rename_prompt);
        }
        if list_header_visible {
            list_column = list_column.push(list_header);
        }
        list_column = list_column.push(list_content);

        // Add a clickable spacer below entries so rubber band selection can start
        // from empty space.  Without this, clicks below the last entry land on the
        // scrollable's dead zone and never reach the outer mouse_area.
        let bg_spacer_height = if self.scroll.height > 1.0 {
            self.scroll.height
        } else {
            500.0
        };
        list_column = list_column.push(
            mouse_area(
                container(row![])
                    .width(Length::Fill)
                    .height(Length::Fixed(bg_spacer_height)),
            )
            .on_press(UiMessage::ListBackgroundPressed)
            .on_right_press(UiMessage::BackgroundContextMenu),
        );

        let list = mouse_area(
            scrollable(container(list_column).padding(spacing.md))
                .id(iced::widget::Id::new(crate::ui::app::LIST_SCROLLABLE_ID))
                .on_scroll(|viewport| {
                    UiMessage::Scroll(ScrollViewport {
                        offset_y: viewport.absolute_offset().y,
                        viewport_height: viewport.bounds().height,
                        content_height: viewport.content_bounds().height,
                        bounds: viewport.bounds(),
                    })
                }),
        )
        .on_press(UiMessage::ListBackgroundPressed)
        .on_right_press(UiMessage::BackgroundContextMenu);

        list.into()
    }
}
