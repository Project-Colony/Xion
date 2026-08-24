//! View rendering for XionApp.
//!
//! Contains the [`XionApp::view`] method which builds the entire widget tree.
//! Modal overlay rendering is in [`overlays`].

mod address_bar;
mod command_bar;
mod context_entries;
mod context_menu;
mod dual_pane;
mod header;
mod list;
mod list_body;
mod main_area;
mod menu_items;
mod overflow_menu;
mod overlays;
mod preview;
mod rows;
mod sidebar;
mod sidebar_sections;
mod status_bar;
mod terminal_panel;
mod widgets;

use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, stack, text_input,
};
use iced::{Alignment, Background, Color, Element, Length, border, mouse};

use crate::ui::UiMessage;
use crate::ui::theme::UiTokens;
use crate::ui::theme::layout::PREVIEW_RESIZE_BAR_WIDTH;

use super::XionApp;
use widgets::ViewCtx;

use widgets::body_text;
use widgets::caption_text;
use widgets::filled_style;
use widgets::glyph_text;
use widgets::hover_button_style;
use widgets::surface_style;
use widgets::{RADIUS, RADIUS_PILL};

impl XionApp {
    pub(super) fn view(&self) -> Element<'_, UiMessage> {
        let tokens = UiTokens::for_theme(&self.state.config.theme);
        let colors = tokens.colors;
        let spacing = tokens.spacing;
        let typography = tokens.typography;
        let ctx = ViewCtx {
            colors,
            spacing,
            typography,
        };

        let (header, history_menu) = self.render_header(ctx);
        let context_menu = self.render_context_menu(ctx);

        let display_entries = self.display_entries();

        let filtered_indices = self.filtered_indices_for(display_entries);
        let list = self.render_list(ctx, display_entries, &filtered_indices);

        // Accordion collapsed state per section name
        let sidebar = self.render_sidebar(ctx);

        let preview_progress = self.preview_anim_progress;
        let preview_visible = preview_progress > 0.001;

        // Feature 10: Archive browser overlay
        // Feature G: Recent files list when route is Recent
        let main_list_element = self.render_main_area(ctx, list);

        // ── Quick Filter bar (Feature E) ──────────────────────────────────────
        let mut list_col = if self.quick_filter_active {
            let filter_bar: Element<'_, UiMessage> = container(
                row![
                    caption_text(typography, "Filtrer :"),
                    text_input("", &self.quick_filter)
                        .on_input(UiMessage::QuickFilterChanged)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                    button(glyph_text(typography, "✕"))
                        .on_press(UiMessage::QuickFilterClear)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(filled_style(colors, colors.hover, RADIUS.md))
            .into();
            column![filter_bar, main_list_element]
        } else {
            column![main_list_element]
        };

        if let Some(term_panel) = self.render_terminal_panel(ctx) {
            list_col = list_col.push(term_panel);
        }

        let list_col = list_col
            .width(Length::FillPortion(4))
            .height(Length::Fill)
            .spacing(spacing.xs);

        let mut body = row![sidebar, list_col,];

        // Feature 11: Dual pane
        if let Some((divider, panel)) = self.render_pane_b(ctx) {
            body = body.push(divider);
            body = body.push(panel);
        }

        if preview_visible {
            // Built here, not above: the panel is fully collapsed most of the
            // time, and constructing it cost a scan of the entries plus a dozen
            // allocations on every rebuild for something nothing drew.
            let preview_body = self.render_preview_body(ctx, display_entries);
            let animated_width = self.pane_resize.preview_width * preview_progress;

            let preview_resize_bar: Element<'_, UiMessage> = mouse_area(
                container(row![])
                    .width(Length::Fixed(PREVIEW_RESIZE_BAR_WIDTH))
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: if self.pane_resize.preview_resizing {
                            Some(Background::Color(colors.hover))
                        } else {
                            None
                        },
                        border: if self.pane_resize.preview_resizing {
                            border::rounded(RADIUS.md).color(colors.border).width(1.0)
                        } else {
                            border::rounded(RADIUS.md).width(0.0)
                        },
                        ..Default::default()
                    }),
            )
            .on_press(UiMessage::PreviewResizeStart)
            .on_release(UiMessage::PreviewResizeEnd)
            .interaction(mouse::Interaction::ResizingHorizontally)
            .into();

            let preview_header = mouse_area(caption_text(typography, "Prévisualisation").style(
                move |_| iced::widget::text::Style {
                    color: Some(colors.text_muted),
                },
            ))
            .on_press(UiMessage::Noop) // absorb single click
            .on_double_click(UiMessage::PreviewDoubleClick);

            let preview_panel =
                container(column![preview_header, preview_body].spacing(spacing.md))
                    .padding(spacing.md)
                    .width(Length::Fixed(animated_width))
                    .style(surface_style(colors, RADIUS.xl));

            body = body.push(preview_resize_bar);
            body = body.push(preview_panel);
        }

        let body = body.height(Length::Fill).spacing(spacing.xs);

        let status_bar = self.render_status_bar(ctx, display_entries, &filtered_indices);

        // No gap between the header and the body: the active tab has to reach
        // the list it belongs to. A uniform `spacing` on the whole column would
        // put twelve pixels there and leave the tab floating, which is what made
        // the strip read as a row of unrelated buttons.
        let content = column![column![header, body].spacing(0), status_bar]
            .spacing(spacing.md)
            .padding(spacing.lg)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        let base: Element<'_, UiMessage> = container(content)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                ..Default::default()
            })
            .into();

        let drag_overlay: Option<Element<'_, UiMessage>> = self
            .drag_state
            .as_ref()
            .zip(self.cursor_position)
            .map(|(drag_state, position)| {
                let count = drag_state.items.len();
                let action = if self.modifiers.control {
                    "Copier"
                } else {
                    "Déplacer"
                };
                let label = if count == 1 {
                    format!("{action} 1 élément")
                } else {
                    format!("{action} {count} éléments")
                };
                let overlay = container(caption_text(typography, label))
                    .padding([spacing.xs, spacing.sm])
                    .style(surface_style(colors, RADIUS_PILL));
                let position_x = (position.x + spacing.md).max(0.0);
                let position_y = (position.y + spacing.md).max(0.0);
                let layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(overlay)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                layer
            });

        let selection_overlay: Option<Element<'_, UiMessage>> = self
            .selection_box_rect()
            .zip(self.list_viewport_bounds)
            .map(|(rect, bounds)| {
                let selection_x = (bounds.x + rect.x).max(0.0);
                let selection_y = (bounds.y + rect.y - self.scroll.offset).max(0.0);
                let selection_width = rect.width.max(0.0);
                let selection_height = rect.height.max(0.0);
                if selection_width == 0.0 || selection_height == 0.0 {
                    return container(row![]).into();
                }
                let fill = Color {
                    a: 0.2,
                    ..colors.accent
                };
                let outline = container(row![])
                    .width(Length::Fixed(selection_width))
                    .height(Length::Fixed(selection_height))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(fill)),
                        border: border::rounded(2.0).color(colors.accent).width(1.0),
                        ..Default::default()
                    });
                container(
                    column![
                        vertical_space().height(Length::Fixed(selection_y)),
                        row![
                            horizontal_space().width(Length::Fixed(selection_x)),
                            opaque(outline)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            });

        let selection_layer: Element<'_, UiMessage> =
            selection_overlay.unwrap_or_else(|| container(row![]).into());
        let drag_layer: Element<'_, UiMessage> =
            drag_overlay.unwrap_or_else(|| container(row![]).into());
        let history_layer: Element<'_, UiMessage> =
            history_menu.unwrap_or_else(|| container(row![]).into());
        let context_layer: Element<'_, UiMessage> =
            context_menu.unwrap_or_else(|| container(row![]).into());
        let overflow_layer: Element<'_, UiMessage> = self
            .render_overflow_menu(ctx)
            .unwrap_or_else(|| container(row![]).into());

        // Feature R: Breadcrumb dropdown
        let breadcrumb_dropdown_layer: Element<'_, UiMessage> =
            if self.breadcrumb_dropdown.is_some() {
                let subdirs = &self.breadcrumb_dropdown_items;
                if subdirs.is_empty() {
                    container(row![]).into()
                } else {
                    let mut items_col = column![].spacing(spacing.xs);
                    for dir in subdirs {
                        let label = dir
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let dir_clone = dir.clone();
                        items_col = items_col.push(
                            button(body_text(typography, label))
                                .width(Length::Fill)
                                .padding([spacing.xs, spacing.sm])
                                .style(hover_button_style(colors, colors.text_primary, None))
                                .on_press(UiMessage::NavigateTo(dir_clone)),
                        );
                    }
                    if self.breadcrumb_dropdown_has_more {
                        items_col = items_col.push(
                            caption_text(typography, "\u{2026}") // "…"
                                .color(colors.text_muted),
                        );
                    }
                    let menu = container(scrollable(items_col).height(Length::Shrink))
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fixed(280.0))
                        .max_height(400.0)
                        .style(surface_style(colors, RADIUS.lg));
                    // Position below the header area
                    let menu_layer: Element<'_, UiMessage> = container(
                        column![
                            vertical_space().height(Length::Fixed(90.0)),
                            row![horizontal_space().width(Length::Fixed(120.0)), opaque(menu)]
                        ]
                        .spacing(0),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
                    let dismiss_layer: Element<'_, UiMessage> =
                        mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                            .on_press(UiMessage::CloseBreadcrumbDropdown)
                            .into();
                    stack![dismiss_layer, menu_layer].into()
                }
            } else {
                container(row![]).into()
            };

        // Feature 3, 5, C, K, O, P: overlay modals (see view/overlays.rs)
        let properties_layer = self.render_properties_layer(colors, spacing, typography);
        let bulk_rename_layer = self.render_bulk_rename_layer(colors, spacing, typography);
        let diff_layer = self.render_diff_layer(colors, spacing, typography);
        let hex_layer = self.render_hex_layer(colors, spacing, typography);
        let grep_layer = self.render_grep_layer(colors, spacing, typography);
        let permissions_layer = self.render_permissions_layer(colors, spacing, typography);
        // Last in the stack: a confirmation must sit above every other modal.
        let confirm_layer = self.render_confirm_layer(colors, spacing, typography);

        stack![
            base,
            selection_layer,
            drag_layer,
            breadcrumb_dropdown_layer,
            history_layer,
            context_layer,
            overflow_layer,
            properties_layer,
            bulk_rename_layer,
            diff_layer,
            hex_layer,
            grep_layer,
            permissions_layer,
            confirm_layer
        ]
        .into()
    }
}
