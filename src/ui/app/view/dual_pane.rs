//! The optional second pane.
//!
//! Moved out of `view()` unchanged, with the two `body.push` calls left to the
//! caller so this module does not need to know the surrounding layout.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::filesystem::FsEntryType;
use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;

use super::widgets::RADIUS;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::chrome_style;
use super::widgets::glyph_text;
use super::widgets::surface_style;

impl XionApp {
    /// The divider and the pane B panel, or `None` when the dual pane is off or
    /// has no directory loaded yet.
    pub(super) fn render_pane_b(
        &self,
        ctx: ViewCtx,
    ) -> Option<(Element<'_, UiMessage>, Element<'_, UiMessage>)> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        if !self.dual_pane.enabled {
            return None;
        }
        let pane_b = self.dual_pane.pane_b.as_ref()?;

        let pane_b_path = pane_b.path.display().to_string();
        // The loader already stopped at `PANE_B_LIMIT`; this pane has no virtual
        // windowing, so that limit is what it can draw.
        let pane_b_entries: Vec<Element<'_, UiMessage>> = pane_b
            .entries
            .iter()
            .map(|entry| {
                let icon = match entry.entry_type {
                    FsEntryType::Directory => icons::FOLDER,
                    FsEntryType::File => entry
                        .path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(icons::icon_for_extension)
                        .unwrap_or(icons::FILE),
                    FsEntryType::Symlink => icons::SYMLINK,
                    FsEntryType::Other => icons::UNKNOWN,
                };
                let activate_path = entry.path.clone();
                button(
                    row![
                        body_text(typography, icon),
                        body_text(typography, entry.name.clone()),
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
                .on_press(UiMessage::PaneBActivate(activate_path))
                .into()
            })
            .collect();

        let pane_b_header = container(
            row![
                glyph_text(typography, icons::FOLDER),
                caption_text(typography, pane_b_path),
                horizontal_space(),
                button(glyph_text(typography, "✕"))
                    .on_press(UiMessage::ToggleDualPane)
                    .padding([spacing.xs, spacing.sm])
                    .style(move |_: &Theme, _: ButtonStatus| {
                        iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        }
                    }),
            ]
            .spacing(spacing.sm)
            .align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.sm])
        .style(chrome_style(colors, RADIUS.md));

        let pane_b_list: Element<'_, UiMessage> = if pane_b.is_loading {
            container(caption_text(typography, "Chargement…"))
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            let mut body = column(pane_b_entries).spacing(0);
            if pane_b.truncated > 0 {
                // Silently dropping them made a truncated listing look complete.
                body = body.push(
                    container(caption_text(
                        typography,
                        format!("… et {} autres", pane_b.truncated),
                    ))
                    .padding([spacing.xs, spacing.sm]),
                );
            }
            scrollable(body).height(Length::Fill).into()
        };
        let pane_b_panel = container(column![pane_b_header, pane_b_list].spacing(spacing.xs))
            .width(Length::FillPortion(1))
            .height(Length::Fill)
            .style(surface_style(colors, RADIUS.xl));

        // Divider between panes
        let pane_divider = container(row![])
            .width(Length::Fixed(2.0))
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.border)),
                ..Default::default()
            });

        Some((pane_divider.into(), pane_b_panel.into()))
    }
}
