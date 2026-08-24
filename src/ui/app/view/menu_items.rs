//! The three building blocks every context menu is made of.
//!
//! These were closures inside `render_context_menu`, which is what kept the
//! entry menu, the background menu and the submenus welded into one function.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::{ContextAction, UiMessage};

use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use super::widgets::raised_button_style;

/// One clickable menu entry, with its icon, label and optional shortcut hint.
pub(super) fn item<'a>(
    ctx: ViewCtx,
    icon: &str,
    label: &str,
    shortcut: &str,
    msg: UiMessage,
) -> Element<'a, UiMessage> {
    let ViewCtx {
        colors,
        spacing,
        typography,
    } = ctx;
    let main_text = text(format!("{} {}", icon, label))
        .size(typography.body)
        .font(typography.body_font);
    let content: Element<'_, UiMessage> = if shortcut.is_empty() {
        main_text.into()
    } else {
        let shortcut_text = text(shortcut.to_string())
            .size(typography.caption)
            .font(typography.caption_font)
            .color(colors.text_muted);
        row![main_text, horizontal_space(), shortcut_text]
            .spacing(spacing.lg)
            .align_y(Alignment::Center)
            .into()
    };
    button(content)
        .padding([spacing.xs, spacing.sm])
        .width(Length::Fill)
        .style(raised_button_style(colors, false))
        .on_press(msg)
        .into()
}

/// A one-pixel separator line.
pub(super) fn separator<'a>(ctx: ViewCtx) -> Element<'a, UiMessage> {
    let ViewCtx { colors, .. } = ctx;
    container(row![])
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.border)),
            ..Default::default()
        })
        .into()
}

/// A colour-label entry: a coloured dot plus its name.
pub(super) fn label_dot<'a>(
    ctx: ViewCtx,
    dot_color: Color,
    label_text: String,
    action: ContextAction,
) -> Element<'a, UiMessage> {
    let ViewCtx {
        colors,
        spacing,
        typography,
    } = ctx;
    let dot = container(row![])
        .width(Length::Fixed(10.0))
        .height(Length::Fixed(10.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(dot_color)),
            border: border::rounded(5.0).width(0.0),
            ..Default::default()
        });
    button(
        row![
            dot,
            text(label_text)
                .size(typography.body)
                .font(typography.body_font)
        ]
        .spacing(spacing.sm)
        .align_y(Alignment::Center),
    )
    .padding([spacing.xs, spacing.sm])
    .width(Length::Fill)
    .style(move |_theme: &Theme, status: ButtonStatus| {
        let mut style = iced::widget::button::Style {
            text_color: colors.text_primary,
            ..Default::default()
        };
        if matches!(status, ButtonStatus::Hovered | ButtonStatus::Pressed) {
            style.background = Some(Background::Color(colors.hover));
            style.border = border::rounded(RADIUS.md).color(colors.border).width(1.0);
        }
        style
    })
    .on_press(UiMessage::ContextAction(action))
    .into()
}
