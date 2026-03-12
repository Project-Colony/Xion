//! Modal overlay rendering methods for [`XionApp`].
//!
//! Each method returns an [`Element`] layer that is stacked on top of the main
//! content by [`XionApp::view`].  All methods are `pub(super)` so they are
//! accessible only within the `view` module.

use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable,
    stack, text, text_input,
};
use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::button::Status as ButtonStatus;
use iced::{Alignment, Background, Color, Element, Length, Theme, border};

use crate::ui::{UiMessage, UiColors, UiSpacing, UiTypography};

use super::XionApp;

impl XionApp {
    // ── Properties dialog ─────────────────────────────────────────────────────

    pub(super) fn render_properties_layer(
        &self,
        colors: UiColors,
        spacing: UiSpacing,
        typography: UiTypography,
    ) -> Element<'_, UiMessage> {
        let modal_opt: Option<Element<'_, UiMessage>> = self.properties_dialog.as_ref().map(|dialog| {
            let size_str = dialog.size_bytes.map(|b| super::super::helpers::format_bytes(b)).unwrap_or_else(|| "—".to_string());
            let hash_str = if dialog.computing_hash {
                "Calcul en cours...".to_string()
            } else {
                dialog.sha256.clone().unwrap_or_else(|| "—".to_string())
            };
            let title_str = if let Some(count) = dialog.selection_count {
                format!("Propriétés : {} éléments sélectionnés", count)
            } else {
                format!("Propriétés : {}", dialog.path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
            };
            let mut modal_content = column![
                text(title_str).size(typography.body).font(typography.body_font),
                text(format!("Chemin : {}", dialog.path.display())).size(typography.caption).font(typography.caption_font),
                text(format!("Taille : {}", size_str)).size(typography.caption).font(typography.caption_font),
                text(format!("Créé : {}", dialog.created.as_deref().unwrap_or("—"))).size(typography.caption).font(typography.caption_font),
                text(format!("Modifié : {}", dialog.modified.as_deref().unwrap_or("—"))).size(typography.caption).font(typography.caption_font),
                text(format!("Lecture seule : {}", if dialog.readonly { "Oui" } else { "Non" })).size(typography.caption).font(typography.caption_font),
            ];
            if dialog.selection_count.is_none() {
                modal_content = modal_content.push(
                    text(format!("SHA-256 : {}", hash_str)).size(typography.caption).font(typography.caption_font)
                );
            }
            let modal_content = modal_content.push(
                button(text("Fermer").size(typography.body).font(typography.body_font))
                    .on_press(UiMessage::CloseProperties)
                    .padding([spacing.xs, spacing.sm])
            )
            .spacing(spacing.sm)
            .padding(spacing.lg);
            let modal = container(modal_content)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
            let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::CloseProperties);
            let layer: Element<'_, UiMessage> = stack![
                opaque(dismiss),
                container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()])
                    .width(Length::Fill)
                    .height(Length::Fill)
            ].into();
            layer
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
            let preview_rows: Vec<Element<'_, UiMessage>> = state.previews.iter().map(|(old, new)| {
                row![
                    text(old.as_str()).size(typography.caption).font(typography.caption_font).width(Length::FillPortion(1)),
                    text("→").size(typography.caption).font(typography.caption_font),
                    text(new.as_str()).size(typography.caption).font(typography.caption_font).width(Length::FillPortion(1)),
                ]
                .spacing(spacing.sm)
                .into()
            }).collect();

            let mut modal_content = column![
                text("Renommage multiple").size(typography.body).font(typography.body_font),
                row![
                    text("Chercher :").size(typography.caption).font(typography.caption_font),
                    text_input("", &state.find)
                        .on_input(UiMessage::BulkRenameFindChanged)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
                row![
                    text("Remplacer :").size(typography.caption).font(typography.caption_font),
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
                modal_content = modal_content.push(
                    text(err.as_str()).size(typography.caption).font(typography.caption_font),
                );
            }

            modal_content = modal_content.push(
                row![
                    button(text("Appliquer").size(typography.body).font(typography.body_font))
                        .on_press(UiMessage::BulkRenameApply)
                        .padding([spacing.xs, spacing.sm]),
                    button(text("Annuler").size(typography.body).font(typography.body_font))
                        .on_press(UiMessage::BulkRenameCancel)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm),
            );

            let modal = container(modal_content)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
            let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::BulkRenameCancel);
            let layer: Element<'_, UiMessage> = stack![
                opaque(dismiss),
                container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()])
                    .width(Length::Fill)
                    .height(Length::Fill)
            ].into();
            layer
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
                let modal = container(
                    text("Chargement du diff…").size(typography.caption).font(typography.caption_font)
                )
                .padding(spacing.lg)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
                let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                    .on_press(UiMessage::CloseDiff);
                stack![opaque(dismiss), container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()]).width(Length::Fill).height(Length::Fill)].into()
            } else {
                let diff_rows: Vec<Element<'_, UiMessage>> = diff.lines.iter().map(|line| {
                    let (line_text, bg_color) = match line {
                        crate::ui::DiffLine::Same(s)    => (format!("  {}", s), None),
                        crate::ui::DiffLine::Added(s)   => (format!("+ {}", s), Some(Color::from_rgba(0.1, 0.6, 0.1, 0.3))),
                        crate::ui::DiffLine::Removed(s) => (format!("- {}", s), Some(Color::from_rgba(0.7, 0.1, 0.1, 0.3))),
                        crate::ui::DiffLine::Header(s)  => (format!("  {}", s), Some(Color::from_rgba(0.5, 0.5, 0.5, 0.2))),
                    };
                    let t = text(line_text).size(typography.caption).font(typography.caption_font);
                    if let Some(bg) = bg_color {
                        container(t).width(Length::Fill).style(move |_| iced::widget::container::Style {
                            background: Some(Background::Color(bg)),
                            ..Default::default()
                        }).into()
                    } else {
                        container(t).width(Length::Fill).into()
                    }
                }).collect();
                let header_a = diff.path_a.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let header_b = diff.path_b.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let modal_content = column![
                    row![
                        text(format!("Diff : {} ↔ {}", header_a, header_b)).size(typography.body).font(typography.body_font),
                        horizontal_space(),
                        button(text("✕").size(typography.caption).font(typography.body_font))
                            .on_press(UiMessage::CloseDiff)
                            .padding([spacing.xs, spacing.sm]),
                    ].spacing(spacing.sm).align_y(Alignment::Center),
                    scrollable(column(diff_rows).spacing(0)).height(Length::Fixed(400.0)),
                ]
                .spacing(spacing.sm)
                .padding(spacing.md);
                let modal = container(modal_content)
                    .width(Length::Fixed(700.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    });
                let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                    .on_press(UiMessage::CloseDiff);
                stack![opaque(dismiss), container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()]).width(Length::Fill).height(Length::Fill)].into()
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
            let byte_rows: Vec<Vec<u8>> = hv.data.chunks(16)
                .skip(hv.offset)
                .take(rows_per_view)
                .map(|chunk| chunk.to_vec())
                .collect();
            let hex_rows: Vec<Element<'_, UiMessage>> = byte_rows.iter().enumerate().map(|(row_idx, row_bytes)| {
                let offset_val = (hv.offset + row_idx) * 16;
                let hex_str: String = row_bytes.iter().map(|b| format!("{:02X} ", b)).collect();
                let ascii_str: String = row_bytes.iter().map(|&b| {
                    if b >= 0x20 && b < 0x7f { b as char } else { '.' }
                }).collect();
                row![
                    text(format!("{:08X}", offset_val)).size(typography.caption).font(typography.caption_font).width(Length::Fixed(80.0)),
                    text(hex_str).size(typography.caption).font(typography.caption_font).width(Length::Fixed(380.0)),
                    text(ascii_str).size(typography.caption).font(typography.caption_font),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center)
                .into()
            }).collect();
            let filename = hv.path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let modal_content = column![
                row![
                    text(format!("Hex : {}", filename)).size(typography.body).font(typography.body_font),
                    horizontal_space(),
                    button(text("✕").size(typography.caption).font(typography.body_font))
                        .on_press(UiMessage::CloseHexView)
                        .padding([spacing.xs, spacing.sm]),
                ].spacing(spacing.sm).align_y(Alignment::Center),
                scrollable(column(hex_rows).spacing(2)).height(Length::Fixed(380.0)),
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);
            let modal = container(modal_content)
                .width(Length::Fixed(600.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
            let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::CloseHexView);
            stack![opaque(dismiss), container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()]).width(Length::Fill).height(Length::Fill)].into()
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
            let result_rows: Vec<Element<'_, UiMessage>> = gs.results.iter().map(|res| {
                let path_str = res.path.to_string_lossy().to_string();
                let label = format!("{}:{} — {}", path_str, res.line_number, res.line);
                let parent = res.path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                button(
                    text(label).size(typography.caption).font(typography.caption_font)
                )
                .padding([spacing.xs, spacing.sm])
                .width(Length::Fill)
                .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                    text_color: colors.text_primary,
                    ..Default::default()
                })
                .on_press(UiMessage::NavigateTo(parent))
                .into()
            }).collect();
            let modal_content = column![
                row![
                    text("Chercher dans les fichiers").size(typography.body).font(typography.body_font),
                    horizontal_space(),
                    button(text("✕").size(typography.caption).font(typography.body_font))
                        .on_press(UiMessage::CloseGrep)
                        .padding([spacing.xs, spacing.sm]),
                ].spacing(spacing.sm).align_y(Alignment::Center),
                row![
                    text_input("Terme de recherche…", &gs.query)
                        .on_input(UiMessage::GrepQueryChanged)
                        .on_submit(UiMessage::GrepSearch)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                    button(text(if gs.searching { "…" } else { "Chercher" }).size(typography.caption).font(typography.caption_font))
                        .on_press(UiMessage::GrepSearch)
                        .padding([spacing.xs, spacing.sm]),
                ].spacing(spacing.sm).align_y(Alignment::Center),
                scrollable(column(result_rows).spacing(0)).height(Length::Fixed(350.0)),
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);
            let modal = container(modal_content)
                .width(Length::Fixed(650.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
            let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::CloseGrep);
            stack![opaque(dismiss), container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()]).width(Length::Fill).height(Length::Fill)].into()
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
            let perm_rows: Vec<Element<'_, UiMessage>> = pv.entries.iter().map(|entry| {
                let allow_str = if entry.allow { "Autoriser" } else { "Refuser" };
                let perms_str = entry.permissions.join(", ");
                row![
                    text(entry.principal.clone()).size(typography.caption).font(typography.caption_font).width(Length::Fixed(200.0)),
                    text(allow_str).size(typography.caption).font(typography.caption_font).width(Length::Fixed(80.0)),
                    text(perms_str).size(typography.caption).font(typography.caption_font),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center)
                .into()
            }).collect();
            let filename = pv.path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let content_elem: Element<'_, UiMessage> = if pv.loading {
                text("Chargement des permissions…").size(typography.caption).font(typography.caption_font).into()
            } else if pv.entries.is_empty() {
                text("Aucune entrée ACL trouvée").size(typography.caption).font(typography.caption_font).into()
            } else {
                scrollable(column(perm_rows).spacing(2)).height(Length::Fixed(300.0)).into()
            };
            let modal_content = column![
                row![
                    text(format!("Permissions : {}", filename)).size(typography.body).font(typography.body_font),
                    horizontal_space(),
                    button(text("✕").size(typography.caption).font(typography.body_font))
                        .on_press(UiMessage::ClosePermissions)
                        .padding([spacing.xs, spacing.sm]),
                ].spacing(spacing.sm).align_y(Alignment::Center),
                content_elem,
            ]
            .spacing(spacing.sm)
            .padding(spacing.md);
            let modal = container(modal_content)
                .width(Length::Fixed(600.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
            let dismiss = mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::ClosePermissions);
            stack![opaque(dismiss), container(column![vertical_space(), row![horizontal_space(), opaque(modal), horizontal_space()], vertical_space()]).width(Length::Fill).height(Length::Fill)].into()
        } else {
            container(row![]).into()
        }
    }
}
