//! Shared building blocks for the [`XionApp::view`](super::XionApp::view) tree.
//!
//! Holds the design tokens that are not part of [`UiTokens`](crate::ui::theme::UiTokens)
//! yet (corner radii), the [`ViewCtx`] bundle passed down to every `render_*`
//! method, and the button/tooltip constructors that used to be duplicated as
//! local closures inside `view()`.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{Button, button, container, text, tooltip};
use iced::{Background, Element, Theme, border};

use crate::ui::{UiColors, UiMessage, UiSpacing, UiTypography};

/// Corner radii used across the widget tree.
///
/// These belong in `ui::theme` next to [`UiSpacing`], but `theme.rs` is out of
/// scope for this refactor, so they live here until they can be folded in.
pub(super) struct UiRadius {
    /// Chips, sort handles, tooltips.
    pub(super) sm: f32,
    /// Buttons and inline inputs.
    pub(super) md: f32,
    /// Menus, dialogs and floating surfaces.
    pub(super) lg: f32,
    /// Full-height panels (sidebar, list, preview).
    pub(super) xl: f32,
}

/// The single source of truth for corner radii.
pub(super) const RADIUS: UiRadius = UiRadius {
    sm: 4.0,
    md: 6.0,
    lg: 8.0,
    xl: 10.0,
};

/// Radius large enough to turn any box into a pill.
pub(super) const RADIUS_PILL: f32 = 999.0;

/// Theme tokens resolved once per frame and threaded through the `render_*`
/// methods, so no method re-derives them from the config.
#[derive(Debug, Clone, Copy)]
pub(super) struct ViewCtx {
    pub(super) colors: UiColors,
    pub(super) spacing: UiSpacing,
    pub(super) typography: UiTypography,
}

/// Container style for a floating surface: menus, dropdowns, dialogs.
pub(super) fn surface_style(
    colors: UiColors,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(colors.panel_background)),
        border: border::rounded(radius).color(colors.border).width(1.0),
        ..Default::default()
    }
}

/// Container style for a recessed strip: list header, archive header, filter bar.
pub(super) fn chrome_style(
    colors: UiColors,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(colors.chrome_background)),
        border: border::rounded(radius).color(colors.border).width(1.0),
        ..Default::default()
    }
}

/// Style shared by the toolbar, the context menus and the address suggestions:
/// transparent at rest, tinted and outlined on hover and on press.
///
/// `mute_disabled` reproduces the toolbar's greyed-out look for buttons without
/// an `on_press`; the menus never disable an entry and left it out.
pub(super) fn raised_button_style(
    colors: UiColors,
    mute_disabled: bool,
) -> impl Fn(&Theme, ButtonStatus) -> button::Style + Copy {
    move |_theme: &Theme, status: ButtonStatus| {
        let mut style = button::Style {
            text_color: colors.text_primary,
            ..Default::default()
        };

        match status {
            ButtonStatus::Hovered => {
                style.background = Some(Background::Color(colors.hover));
                style.border = border::rounded(RADIUS.md).color(colors.border).width(1.0);
            }
            ButtonStatus::Pressed => {
                style.background = Some(Background::Color(colors.pressed));
                style.border = border::rounded(RADIUS.md).color(colors.border).width(1.0);
            }
            ButtonStatus::Disabled if mute_disabled => {
                style.text_color = colors.text_muted;
            }
            ButtonStatus::Active | ButtonStatus::Disabled => {}
        }

        style
    }
}

/// The toolbar button used by the navigation arrows, the command bar and the
/// rename prompt.
pub(super) fn toolbar_button<'a>(ctx: ViewCtx, label: String) -> Button<'a, UiMessage> {
    button(
        text(label)
            .size(ctx.typography.body)
            .font(ctx.typography.body_font),
    )
    .padding([ctx.spacing.xs, ctx.spacing.sm])
    .style(raised_button_style(ctx.colors, true))
}

/// Which of the two historical tooltip looks a call site wants.
///
/// The navigation arrows and the command bar grew their own tooltip code and
/// ended up with different surfaces, text sizes and offsets.  Both now go
/// through [`tip`]; the variant only carries the pixels that differ, so nothing
/// moves on screen.
#[derive(Debug, Clone, Copy)]
pub(super) enum TipVariant {
    /// Navigation arrows: chrome surface, default text, 4px inner padding.
    Nav,
    /// Command bar: panel surface, caption text, 4px gap to the anchor.
    Command,
}

/// The single tooltip constructor.
pub(super) fn tip<'a>(
    ctx: ViewCtx,
    content: impl Into<Element<'a, UiMessage>>,
    label: &'a str,
    variant: TipVariant,
) -> Element<'a, UiMessage> {
    let colors = ctx.colors;
    match variant {
        TipVariant::Nav => tooltip(content, label, tooltip::Position::Bottom)
            .style(chrome_style(colors, RADIUS.sm))
            .padding(4)
            .into(),
        TipVariant::Command => tooltip(
            content,
            text(label)
                .size(ctx.typography.caption)
                .font(ctx.typography.caption_font),
            tooltip::Position::Bottom,
        )
        .gap(4.0)
        .style(surface_style(colors, RADIUS.sm))
        .into(),
    }
}
