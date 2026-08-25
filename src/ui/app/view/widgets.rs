//! Shared building blocks for the [`XionApp::view`](super::XionApp::view) tree.
//!
//! Holds the design tokens that are not part of [`UiTokens`](crate::ui::theme::UiTokens)
//! yet (corner radii), the [`ViewCtx`] bundle passed down to every `render_*`
//! method, and the button/tooltip constructors that used to be duplicated as
//! local closures inside `view()`.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{Button, Text, button, container, text, tooltip};
use iced::{Background, Color, Element, Theme, border};

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

/// Où pendent les menus déroulants issus de la barre d'en-tête.
///
/// Mesuré depuis le haut de la fenêtre : le rembourrage extérieur du contenu,
/// puis le rembourrage haut de l'en-tête, puis la hauteur d'un bouton de barre
/// d'outils. C'est une estimation, pas une valeur dérivée de la mise en page —
/// iced ne rend pas la position calculée disponible ici.
///
/// Elle est unique et partagée pour une raison précise : le menu du fil
/// d'Ariane utilisait 90, calibré sur l'ancien en-tête à trois bandes. Celui-ci
/// n'en a plus qu'une, et le menu flottait cinquante pixels sous le bouton
/// auquel il appartient. Deux nombres pour la même chose divergent dès que la
/// chose change.
pub(super) const HEADER_DROPDOWN_TOP: f32 = 54.0;

/// A solid fill with the standard one-pixel outline and a rounded corner.
///
/// Thirty call sites across the view tree had written this same struct literal
/// inline, which is how three of them drifted onto different radii for the same
/// kind of surface.
pub(super) fn filled_style(
    colors: UiColors,
    fill: Color,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: border::rounded(radius).color(colors.border).width(1.0),
        ..Default::default()
    }
}

/// La bande qui porte des onglets : remplie, arrondie en haut seulement, et
/// sans contour.
///
/// Two reasons it has no border. The tab strip sits flush against the bottom of
/// this container, and the active tab is painted in the list's own background —
/// so a line drawn under the tabs would cut the join that makes a row of
/// buttons read as tabs at all. And the window already had a border around the
/// header, one around the sidebar, one around the list and one around the
/// status bar, which is what made the screen read as a stack of panels rather
/// than an application.
///
/// Partagée entre l'en-tête de fenêtre et le panneau terminal : la bande du
/// terminal utilisait `filled_style`, qui trace justement cette ligne, et ses
/// onglets ne se rattachaient donc à rien.
pub(super) fn tab_strip_style(
    colors: UiColors,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    // Sauf quand l'aplat ne sépare rien. Le thème « contraste élevé » peint
    // `chrome_background` et `panel_background` tous deux en noir : il ne
    // distingue les zones que par ses bordures blanches. Retirer celle-ci y
    // rendait l'en-tête purement invisible — précisément dans le thème destiné
    // aux personnes qui ont besoin de séparations franches.
    let fills_separate = colors.chrome_background != colors.panel_background;
    move |_: &Theme| container::Style {
        background: Some(Background::Color(colors.chrome_background)),
        border: iced::Border {
            radius: iced::border::Radius::default()
                .top_left(radius)
                .top_right(radius),
            color: colors.border,
            width: if fills_separate { 0.0 } else { 1.0 },
        },
        ..Default::default()
    }
}

/// La bande de l'en-tête de fenêtre.
pub(super) fn header_style(colors: UiColors) -> impl Fn(&Theme) -> container::Style + Copy {
    tab_strip_style(colors, RADIUS.xl)
}

/// Container style for a floating surface: menus, dropdowns, dialogs.
pub(super) fn surface_style(
    colors: UiColors,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    filled_style(colors, colors.panel_background, radius)
}

/// Container style for a recessed strip: list header, archive header, filter bar.
pub(super) fn chrome_style(
    colors: UiColors,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style + Copy {
    filled_style(colors, colors.chrome_background, radius)
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

/// The four typographic roles the interface actually uses.
///
/// Every one of the 140 pieces of text in the view tree used to name its size
/// and its font separately, which meant the pairing was a convention rather
/// than a rule, and reading a call site told you two numbers instead of a role.
///
/// `glyph_text` is not a mistake: the ✕, +, ⛔ and Nerd Font marks are drawn at
/// caption size in the body font, deliberately.
pub(super) fn body_text<'a>(
    typography: UiTypography,
    content: impl text::IntoFragment<'a>,
) -> Text<'a> {
    text(content)
        .size(typography.body)
        .font(typography.body_font)
}

pub(super) fn caption_text<'a>(
    typography: UiTypography,
    content: impl text::IntoFragment<'a>,
) -> Text<'a> {
    text(content)
        .size(typography.caption)
        .font(typography.caption_font)
}

pub(super) fn glyph_text<'a>(
    typography: UiTypography,
    content: impl text::IntoFragment<'a>,
) -> Text<'a> {
    text(content)
        .size(typography.caption)
        .font(typography.body_font)
}

pub(super) fn title_text<'a>(
    typography: UiTypography,
    content: impl text::IntoFragment<'a>,
) -> Text<'a> {
    text(content)
        .size(typography.title)
        .font(typography.title_font)
}

/// A flat control that only reacts on hover: address-bar chips, sort handles,
/// the network entry. `outline` adds the one-pixel border some of them draw.
pub(super) fn hover_button_style(
    colors: UiColors,
    text_color: Color,
    outline: Option<f32>,
) -> impl Fn(&Theme, ButtonStatus) -> button::Style + Copy {
    move |_theme: &Theme, status: ButtonStatus| {
        let mut style = button::Style {
            text_color,
            ..Default::default()
        };

        if matches!(status, ButtonStatus::Hovered) {
            style.background = Some(Background::Color(colors.hover));
            if let Some(radius) = outline {
                style.border = border::rounded(radius).color(colors.border).width(1.0);
            }
        }

        style
    }
}

/// A row or tile that can be selected and focused: list rows, grid tiles, tree
/// nodes. The focus ring is the selection outline drawn twice as thick.
pub(super) fn selectable_button_style(
    colors: UiColors,
    selected: bool,
    focused: bool,
    radius: f32,
) -> impl Fn(&Theme, ButtonStatus) -> button::Style + Copy {
    move |_theme: &Theme, status: ButtonStatus| {
        let mut style = button::Style {
            text_color: colors.text_primary,
            ..Default::default()
        };

        if selected {
            style.background = Some(Background::Color(colors.selection));
            style.border = border::rounded(radius)
                .color(colors.selection_border)
                .width(if focused { 2.0 } else { 1.0 });
        }

        match status {
            ButtonStatus::Hovered => style.background = Some(Background::Color(colors.hover)),
            ButtonStatus::Pressed => style.background = Some(Background::Color(colors.pressed)),
            ButtonStatus::Active | ButtonStatus::Disabled => {}
        }

        style
    }
}

/// A status-bar toggle: accented and filled while on, muted while off.
pub(super) fn toggle_button_style(
    colors: UiColors,
    active: bool,
) -> impl Fn(&Theme, ButtonStatus) -> button::Style + Copy {
    move |_theme: &Theme, status: ButtonStatus| {
        let mut style = button::Style {
            text_color: if active {
                colors.accent
            } else {
                colors.text_muted
            },
            ..Default::default()
        };

        if active {
            style.background = Some(Background::Color(colors.selection));
            style.border = border::rounded(RADIUS.sm)
                .color(colors.selection_border)
                .width(1.0);
        }

        if matches!(status, ButtonStatus::Hovered) {
            style.background = Some(Background::Color(colors.hover));
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

/// Which tooltip look a call site wants.
///
/// There were two, because the navigation arrows and the command bar had each
/// grown their own tooltip code. The command bar is gone — its contents moved
/// into the `⋯` menu, where entries carry their shortcut inline and need no
/// tooltip — so only one look is left.
#[derive(Debug, Clone, Copy)]
pub(super) enum TipVariant {
    /// Navigation arrows: chrome surface, default text, 4px inner padding.
    Nav,
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
    }
}
