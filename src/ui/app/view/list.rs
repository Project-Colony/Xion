//! The file list itself: column header, rows, grid tiles, loading placeholders
//! and the rename prompt drawn in place.
//!
//! Moved out of `view()` unchanged. This was the largest single section of that
//! function, and the closures it defines are used nowhere else.

use iced::widget::{column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Alignment, Background, Element, Length, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

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
        let _entry_index_for = |display_index: usize| -> Option<usize> {
            if let Some(indices) = &filtered_indices {
                indices.get(display_index).copied()
            } else {
                Some(display_index)
            }
        };

        let list_content = self.render_list_content(
            ctx,
            cx,
            display_entries,
            filtered_indices,
            total_entries,
            is_filtered,
        );

        let list_header_visible =
            self.list_header_visible(view_mode, display_entries, filtered_indices.as_ref());
        let list_header = self.render_list_header(ctx, list_header_visible);

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
