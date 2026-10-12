//! Modal overlay rendering methods for [`XionApp`].
//!
//! Each method returns an [`Element`] layer that is stacked on top of the main
//! content by [`XionApp::view`].  All methods are `pub(super)` so they are
//! accessible only within the `view` module.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, stack, text_input,
};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};

use crate::i18n::{t, tf};
use crate::ui::{UiColors, UiMessage, UiSpacing, UiTypography};

use super::XionApp;

// ── Helper: modal overlay pattern ────────────────────────────────────────────

/// A dismiss layer with the modal content centred on top of it.
///
/// `width` pins the dialog; `None` lets it size to its content. This existed
/// twice, the two copies differing by that one call to `.width()`.
fn modal_overlay<'a>(
    content: impl Into<Element<'a, UiMessage>>,
    dismiss: UiMessage,
    colors: UiColors,
    width: Option<f32>,
) -> Element<'a, UiMessage> {
    let mut modal = container(content).style(surface_style(colors, RADIUS.lg));
    if let Some(width) = width {
        modal = modal.width(Length::Fixed(width));
    }
    let dismiss_layer =
        mouse_area(container(row![]).width(Length::Fill).height(Length::Fill)).on_press(dismiss);
    stack![
        opaque(dismiss_layer),
        container(column![
            vertical_space(),
            row![horizontal_space(), opaque(modal), horizontal_space()],
            vertical_space(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
    ]
    .into()
}

use super::widgets::RADIUS;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::glyph_text;
use super::widgets::surface_style;

impl XionApp {
    // ── Properties dialog ─────────────────────────────────────────────────────

    pub(super) fn render_properties_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        let modal_opt: Option<Element<'_, UiMessage>> =
            self.properties_dialog.as_ref().map(|dialog| {
                let size_str = dialog
                    .size_bytes
                    .map(super::super::helpers::format_bytes)
                    .unwrap_or_else(|| "—".to_string());
                let hash_str = if dialog.computing_hash {
                    t("properties_computing").to_string()
                } else {
                    dialog.sha256.clone().unwrap_or_else(|| "—".to_string())
                };
                let title_str = if let Some(count) = dialog.selection_count {
                    tf("properties_title_many", &[("count", &count)])
                } else {
                    tf(
                        "properties_title",
                        &[(
                            "name",
                            &dialog
                                .path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(""),
                        )],
                    )
                };

                let field = |label: &str, value: String| -> Element<'_, UiMessage> {
                    row![
                        container(glyph_text(typography, label.to_string()))
                            .width(Length::Fixed(120.0)),
                        caption_text(typography, value),
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center)
                    .into()
                };

                let mut modal_content = column![
                    body_text(typography, title_str),
                    field(t("properties_path"), {
                        let p = dialog.path.display().to_string();
                        let char_count = p.chars().count();
                        if char_count > 80 {
                            let tail: String = p.chars().skip(char_count - 75).collect();
                            format!("…{tail}")
                        } else {
                            p
                        }
                    }),
                    field(t("properties_size"), size_str),
                    field(
                        t("properties_created"),
                        dialog.created.as_deref().unwrap_or("—").to_string()
                    ),
                    field(
                        t("properties_modified"),
                        dialog.modified.as_deref().unwrap_or("—").to_string()
                    ),
                    field(
                        t("properties_read_only"),
                        t(if dialog.readonly { "yes" } else { "no" }).to_string()
                    ),
                ];
                if dialog.selection_count.is_none() {
                    modal_content = modal_content.push(field("SHA-256", hash_str));
                }
                let modal_content = modal_content
                    .push(
                        button(body_text(typography, t("action_close")))
                            .on_press(UiMessage::CloseProperties)
                            .padding([spacing.xs, spacing.sm]),
                    )
                    .spacing(spacing.sm)
                    .padding(spacing.lg);

                modal_overlay(modal_content, UiMessage::CloseProperties, colors, None)
            });
        modal_opt.unwrap_or_else(|| container(row![]).into())
    }

    // ── Bulk rename dialog ────────────────────────────────────────────────────

    pub(super) fn render_bulk_rename_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        let modal_opt: Option<Element<'_, UiMessage>> = self.bulk_rename.as_ref().map(|state| {
            let preview_rows: Vec<Element<'_, UiMessage>> = state
                .previews
                .iter()
                .map(|(old_name, new_name)| {
                    let old_color = if old_name != new_name {
                        colors.diff_removed
                    } else {
                        colors.text_primary
                    };
                    let new_color = if old_name != new_name {
                        colors.diff_added
                    } else {
                        colors.text_primary
                    };
                    row![
                        caption_text(typography, old_name.as_str())
                            .color(old_color)
                            .width(Length::FillPortion(1)),
                        caption_text(typography, "→"),
                        caption_text(typography, new_name.as_str())
                            .color(new_color)
                            .width(Length::FillPortion(1)),
                    ]
                    .spacing(spacing.sm)
                    .into()
                })
                .collect();

            let mut modal_content = column![
                body_text(typography, t("bulk_rename_title")),
                row![
                    caption_text(typography, t("bulk_rename_find")),
                    text_input("", &state.find)
                        .on_input(UiMessage::BulkRenameFindChanged)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                row![
                    caption_text(typography, t("bulk_rename_replace")),
                    text_input("", &state.replace)
                        .on_input(UiMessage::BulkRenameReplaceChanged)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
            ]
            .spacing(spacing.sm)
            .padding(spacing.lg);

            for row_elem in preview_rows {
                modal_content = modal_content.push(row_elem);
            }

            if let Some(err) = &state.error {
                modal_content = modal_content.push(caption_text(typography, err.as_str()));
            }

            modal_content = modal_content.push(
                row![
                    button(body_text(typography, t("action_apply")))
                        .on_press(UiMessage::BulkRenameApply)
                        .padding([spacing.xs, spacing.sm]),
                    button(body_text(typography, t("action_cancel")))
                        .on_press(UiMessage::BulkRenameCancel)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm),
            );

            modal_overlay(modal_content, UiMessage::BulkRenameCancel, colors, None)
        });
        modal_opt.unwrap_or_else(|| container(row![]).into())
    }

    // ── Diff viewer overlay ───────────────────────────────────────────────────

    pub(super) fn render_diff_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        if let Some(diff) = &self.diff_view {
            if diff.loading {
                let loading_content =
                    column![caption_text(typography, t("diff_loading"))].padding(spacing.lg);

                modal_overlay(loading_content, UiMessage::CloseDiff, colors, None)
            } else {
                let diff_rows: Vec<Element<'_, UiMessage>> = diff
                    .lines
                    .iter()
                    .map(|line| {
                        // The theme's status colours, faded so the line
                        // stays readable on top of them.
                        let (line_text, bg_color) = match line {
                            crate::ui::DiffLine::Same(s) => (format!("  {}", s), None),
                            crate::ui::DiffLine::Added(s) => (
                                format!("+ {}", s),
                                Some(Color {
                                    a: 0.3,
                                    ..colors.diff_added
                                }),
                            ),
                            crate::ui::DiffLine::Removed(s) => (
                                format!("- {}", s),
                                Some(Color {
                                    a: 0.3,
                                    ..colors.diff_removed
                                }),
                            ),
                            crate::ui::DiffLine::Header(s) => (
                                format!("  {}", s),
                                Some(Color {
                                    a: 0.2,
                                    ..colors.text_muted
                                }),
                            ),
                        };
                        let t = caption_text(typography, line_text);
                        if let Some(bg) = bg_color {
                            container(t)
                                .width(Length::Fill)
                                .style(move |_| iced::widget::container::Style {
                                    background: Some(Background::Color(bg)),
                                    ..Default::default()
                                })
                                .into()
                        } else {
                            container(t).width(Length::Fill).into()
                        }
                    })
                    .collect();
                let header_a = diff
                    .path_a
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let header_b = diff
                    .path_b
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let modal_content = column![
                    row![
                        body_text(
                            typography,
                            tf("diff_title", &[("left", &header_a), ("right", &header_b)])
                        ),
                        horizontal_space(),
                        button(glyph_text(typography, "✕"))
                            .on_press(UiMessage::CloseDiff)
                            .padding([spacing.xs, spacing.sm]),
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center),
                    scrollable(column(diff_rows).spacing(0)).height(Length::Fixed(400.0)),
                ]
                .spacing(spacing.sm)
                .padding(spacing.md);

                modal_overlay(modal_content, UiMessage::CloseDiff, colors, Some(700.0))
            }
        } else {
            container(row![]).into()
        }
    }

    // ── Hex viewer overlay ────────────────────────────────────────────────────

    pub(super) fn render_hex_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        if let Some(hv) = &self.hex_view {
            let rows_per_view = 16usize;
            let hex_rows: Vec<Element<'_, UiMessage>> = hv
                .data
                .chunks(16)
                .skip(hv.offset)
                .take(rows_per_view)
                .enumerate()
                .map(|(row_idx, row_bytes)| {
                    let offset_val = (hv.offset + row_idx) * 16;
                    let hex_str: String = row_bytes.iter().map(|b| format!("{:02X} ", b)).collect();
                    let ascii_str: String = row_bytes
                        .iter()
                        .map(|&b| {
                            if (0x20..0x7f).contains(&b) {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    row![
                        caption_text(typography, format!("{:08X}", offset_val))
                            .width(Length::Fixed(80.0)),
                        caption_text(typography, hex_str).width(Length::Fixed(380.0)),
                        caption_text(typography, ascii_str),
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect();
            let filename = hv
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            let modal_content = column![
                row![
                    body_text(typography, tf("hex_title", &[("name", &filename)])),
                    horizontal_space(),
                    button(glyph_text(typography, "✕"))
                        .on_press(UiMessage::CloseHexView)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                scrollable(column(hex_rows).spacing(2)).height(Length::Fixed(380.0)),
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);

            modal_overlay(modal_content, UiMessage::CloseHexView, colors, Some(600.0))
        } else {
            container(row![]).into()
        }
    }

    // ── Grep overlay ──────────────────────────────────────────────────────────

    pub(super) fn render_grep_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        if let Some(gs) = &self.grep_state {
            let result_rows: Vec<Element<'_, UiMessage>> = gs
                .results
                .iter()
                .map(|res| {
                    let path_str = res
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_else(|| res.path.to_str().unwrap_or("?"));
                    let line_preview: String = res.line.chars().take(80).collect();
                    let label = format!("{}:{} — {}", path_str, res.line_number, line_preview);
                    let parent = res
                        .path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_default();
                    button(caption_text(typography, label))
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(
                            move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            },
                        )
                        .on_press(UiMessage::NavigateTo(parent))
                        .into()
                })
                .collect();
            let modal_content = column![
                row![
                    body_text(typography, t("grep_title")),
                    horizontal_space(),
                    button(glyph_text(typography, "✕"))
                        .on_press(UiMessage::CloseGrep)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                row![
                    text_input(t("grep_placeholder"), &gs.query)
                        .on_input(UiMessage::GrepQueryChanged)
                        .on_submit(UiMessage::GrepSearch)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                    button(caption_text(
                        typography,
                        if gs.searching {
                            "…"
                        } else {
                            t("grep_search")
                        }
                    ))
                    .on_press(UiMessage::GrepSearch)
                    .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                scrollable(column(result_rows).spacing(0)).height(Length::Fixed(350.0)),
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);

            modal_overlay(modal_content, UiMessage::CloseGrep, colors, Some(650.0))
        } else {
            container(row![]).into()
        }
    }

    // ── Permissions overlay ───────────────────────────────────────────────────

    pub(super) fn render_permissions_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        if let Some(pv) = &self.permissions_view {
            let perm_rows: Vec<Element<'_, UiMessage>> = pv
                .entries
                .iter()
                .map(|entry| {
                    let allow_str = t(if entry.allow {
                        "permissions_allow"
                    } else {
                        "permissions_deny"
                    });
                    let perms_str = entry.permissions.join(", ");
                    row![
                        caption_text(
                            typography,
                            crate::ui::app::helpers::truncate_name(&entry.principal, 30)
                                .into_owned()
                        )
                        .width(Length::Fixed(200.0)),
                        caption_text(typography, allow_str).width(Length::Fixed(80.0)),
                        caption_text(typography, perms_str),
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect();
            let filename = pv
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let content_elem: Element<'_, UiMessage> = if pv.loading {
                caption_text(typography, t("permissions_loading")).into()
            } else if let Some(err) = &pv.error {
                caption_text(typography, err.as_str())
                    .color(colors.diff_removed)
                    .into()
            } else if pv.entries.is_empty() {
                caption_text(typography, t("permissions_empty")).into()
            } else {
                scrollable(column(perm_rows).spacing(2))
                    .height(Length::Fixed(300.0))
                    .into()
            };
            let modal_content = column![
                row![
                    body_text(typography, tf("permissions_title", &[("name", &filename)])),
                    horizontal_space(),
                    button(glyph_text(typography, "✕"))
                        .on_press(UiMessage::ClosePermissions)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                content_elem,
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);

            modal_overlay(
                modal_content,
                UiMessage::ClosePermissions,
                colors,
                Some(600.0),
            )
        } else {
            container(row![]).into()
        }
    }

    // ── Confirmation dialog ───────────────────────────────────────────────────

    pub(super) fn render_confirm_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        let Some(dialog) = &self.confirm_dialog else {
            return container(row![]).into();
        };

        let confirm_button = button(body_text(typography, dialog.confirm_label.as_str()))
            .padding([spacing.xs, spacing.md])
            .on_press(UiMessage::ConfirmAccept)
            .style(
                move |_theme: &Theme, status: ButtonStatus| iced::widget::button::Style {
                    background: Some(Background::Color(match status {
                        ButtonStatus::Hovered => colors.diff_removed,
                        _ => Color {
                            a: 0.85,
                            ..colors.diff_removed
                        },
                    })),
                    text_color: colors.text_primary,
                    border: border::rounded(RADIUS.sm),
                    ..Default::default()
                },
            );

        let cancel_button = button(body_text(typography, t("action_cancel")))
            .padding([spacing.xs, spacing.md])
            .on_press(UiMessage::ConfirmCancel);

        let modal_content = column![
            body_text(typography, dialog.title.as_str()),
            caption_text(typography, dialog.message.as_str()).color(colors.text_muted),
            row![horizontal_space(), cancel_button, confirm_button].spacing(spacing.sm),
        ]
        .spacing(spacing.md)
        .padding(spacing.md);

        modal_overlay(modal_content, UiMessage::ConfirmCancel, colors, Some(420.0))
    }
}
