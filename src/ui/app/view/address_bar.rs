//! The address bar: the editable field, the breadcrumb, the validation badge
//! and the history dropdown.
//!
//! Moved out of `render_header` unchanged.

use std::path::PathBuf;

use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{button, column, container, mouse_area, opaque, row, stack, text, text_input};
use iced::{Alignment, Element, Length, Point};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::UiMessage;

use super::XionApp;
use super::widgets::ViewCtx;
use super::widgets::hover_button_style;
use super::widgets::raised_button_style;
use super::widgets::surface_style;
use super::widgets::{self, RADIUS, RADIUS_PILL};

impl XionApp {
    /// The address section and, separately, the history dropdown: the dropdown
    /// is drawn as an overlay layer on top of everything else, so the caller
    /// places it itself.
    pub(super) fn render_address_section(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Option<Element<'_, UiMessage>>) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let toolbar_button = |label: String| widgets::toolbar_button(ctx, label);

        let address_bar: Element<'_, UiMessage> = if self.address_editing {
            // Editable text input mode
            let address_input = text_input("Chemin…", &self.address_input)
                .on_input(UiMessage::AddressInputChanged)
                .on_submit(UiMessage::AddressInputSubmitted)
                .size(typography.body)
                .font(typography.body_font)
                .padding([spacing.xs, spacing.md])
                .id(iced::widget::Id::new("address_input"));

            container(address_input)
                .width(Length::Fill)
                .style(surface_style(colors, RADIUS.md))
                .into()
        } else {
            // Breadcrumb mode — clickable path segments
            let current_path = self.state.route.address_label();
            let mut breadcrumb_row = row![].spacing(0).align_y(Alignment::Center);

            let segments: Vec<&str> = current_path.split('\\').filter(|s| !s.is_empty()).collect();
            let mut accumulated = String::new();

            for (i, segment) in segments.iter().enumerate() {
                let parent_path = accumulated.clone();
                if i == 0 && segment.ends_with(':') {
                    // Drive root, e.g. "C:"
                    accumulated = format!("{}\\", segment);
                } else {
                    accumulated = format!("{}{}\\", accumulated, segment);
                }

                if i > 0 {
                    // The separator lists siblings = subdirectories of parent_path
                    let dropdown_target = PathBuf::from(&parent_path);
                    breadcrumb_row = breadcrumb_row.push(
                        button(
                            text("❯")
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(colors.text_muted),
                                }),
                        )
                        .padding([spacing.xs, 2.0])
                        .style(hover_button_style(
                            colors,
                            colors.text_muted,
                            Some(RADIUS.sm),
                        ))
                        .on_press(UiMessage::BreadcrumbDropdown(dropdown_target)),
                    );
                }

                let target = PathBuf::from(&accumulated);
                let is_last = i == segments.len() - 1;
                let label = segment.to_string();

                if is_last {
                    // Also add a trailing separator for the last segment to list its children
                    breadcrumb_row = breadcrumb_row
                        .push(text(label).size(typography.body).font(typography.body_font));
                    let dropdown_target = PathBuf::from(&accumulated);
                    breadcrumb_row = breadcrumb_row.push(
                        button(
                            text("❯")
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(colors.text_muted),
                                }),
                        )
                        .padding([spacing.xs, 2.0])
                        .style(hover_button_style(
                            colors,
                            colors.text_muted,
                            Some(RADIUS.sm),
                        ))
                        .on_press(UiMessage::BreadcrumbDropdown(dropdown_target)),
                    );
                } else {
                    breadcrumb_row = breadcrumb_row.push(
                        button(text(label).size(typography.body).font(typography.body_font))
                            .padding([spacing.xs, spacing.xs])
                            .style(hover_button_style(colors, colors.accent, Some(RADIUS.sm)))
                            .on_press(UiMessage::NavigateTo(target)),
                    );
                }
            }

            let breadcrumb_button = mouse_area(
                container(breadcrumb_row)
                    .width(Length::Fill)
                    .padding([spacing.xs, spacing.md])
                    .style(surface_style(colors, RADIUS.md)),
            )
            .on_press(UiMessage::AddressEditStart);

            breadcrumb_button.into()
        };

        let address_validation = {
            use crate::ui::app::types::AddressValidation;
            if self.address_editing {
                self.address_validation_cache
                    .get(&self.address_input, || self.address_target_from_input())
                    .map(|v| match v {
                        AddressValidation::Directory => {
                            ("Dossier".to_string(), colors.address_directory)
                        }
                        AddressValidation::File => ("Fichier".to_string(), colors.address_file),
                        AddressValidation::NotFound => {
                            ("Introuvable".to_string(), colors.address_not_found)
                        }
                    })
            } else {
                None
            }
        };

        let address_status: Element<'_, UiMessage> =
            if let Some((label, status_color)) = address_validation {
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(status_color),
                        }),
                )
                .padding([spacing.xs, spacing.sm])
                .style(surface_style(colors, RADIUS_PILL))
                .into()
            } else {
                container(row![]).into()
            };

        let suggestion_button = |label: String, target: PathBuf| {
            button(
                text(label)
                    .size(typography.caption)
                    .font(typography.body_font),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(raised_button_style(colors, false))
            .on_press(UiMessage::AddressSuggestionSelected(target))
        };

        let address_suggestions = self.address_suggestions();
        let history_button = toolbar_button("▼".to_string())
            .on_press(UiMessage::ToggleHistoryMenu(!self.menus.history_open));

        let history_menu: Option<Element<'_, UiMessage>> =
            if self.menus.history_open && !address_suggestions.is_empty() {
                let mut suggestions_list = column![
                    text("Historique")
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        })
                ]
                .spacing(spacing.xs);
                for suggestion in address_suggestions {
                    suggestions_list = suggestions_list.push(suggestion_button(
                        suggestion.display().to_string(),
                        suggestion,
                    ));
                }
                let position = self.menus.history_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);
                let menu = container(suggestions_list)
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fixed(420.0))
                    .style(surface_style(colors, RADIUS.lg));
                let menu_layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(menu)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                let dismiss_layer: Element<'_, UiMessage> =
                    mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                        .on_press(UiMessage::ToggleHistoryMenu(false))
                        .into();
                Some(stack![dismiss_layer, menu_layer].into())
            } else {
                None
            };

        let address_row = row![address_bar, history_button, address_status]
            .spacing(spacing.xs)
            .align_y(Alignment::Center);

        let address_section: Element<'_, UiMessage> =
            container(address_row).width(Length::Fill).into();

        (address_section, history_menu)
    }
}
