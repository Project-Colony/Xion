//! L'écran d'apparence : thème, accent, contraste.
//!
//! Les deux sélecteurs viennent de `colony-ui` et se rendent seuls depuis le
//! catalogue partagé. Sa documentation le dit sans détour : « ajouter une
//! famille de thèmes ne demande aucun changement ici, ni dans le programme
//! hôte — c'est tout l'intérêt de la caisse ». Xion n'écrit donc ni les
//! vignettes, ni les noms, ni la liste.
//!
//! Une modale plutôt qu'une entrée de menu : vingt-cinq familles en cartes ne
//! tiennent pas dans un menu déroulant.

use iced::widget::{column, container, row, scrollable};
use iced::{Alignment, Element, Length};

use crate::ui::UiMessage;

use super::XionApp;
use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::raised_button_style;
use super::widgets::surface_style;
use super::widgets::title_text;

impl XionApp {
    /// L'écran d'apparence, ou `None` s'il est fermé.
    pub(super) fn render_appearance_layer(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        if !self.menus.appearance_open {
            return None;
        }

        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let choice = &self.state.config.theme;
        let typo = typography.colony();

        let heading =
            |label: &'static str| caption_text(typography, label).color(colors.text_muted);

        let themes = colony_ui::widgets::theme_picker(
            &typo,
            &choice.family,
            &choice.variant,
            UiMessage::SetThemeVariant,
        );

        let accents = colony_ui::widgets::accent_picker(
            &typo,
            choice.accent.as_deref(),
            UiMessage::SetAccent,
        );

        let contrast_label = if choice.high_contrast {
            "Contraste élevé : activé"
        } else {
            "Contraste élevé : désactivé"
        };
        let contrast = iced::widget::button(body_text(typography, contrast_label))
            .padding([spacing.xs, spacing.sm])
            .style(raised_button_style(colors, false))
            .on_press(UiMessage::ToggleHighContrast);

        let close = iced::widget::button(body_text(typography, "Fermer"))
            .padding([spacing.xs, spacing.md])
            .style(raised_button_style(colors, false))
            .on_press(UiMessage::ToggleAppearance(false));

        let body = column![
            row![
                title_text(typography, "Apparence"),
                iced::widget::space::horizontal(),
                close
            ]
            .align_y(Alignment::Center),
            heading("THÈME"),
            themes,
            heading("ACCENT"),
            accents,
            caption_text(typography, "Sans accent choisi, celui du thème s'applique.",)
                .color(colors.text_muted),
            heading("ACCESSIBILITÉ"),
            contrast,
            caption_text(
                typography,
                "Le contraste élevé s'applique à n'importe quelle palette.",
            )
            .color(colors.text_muted),
        ]
        .spacing(spacing.md)
        .padding(spacing.lg);

        let panel = container(scrollable(body).height(Length::Shrink))
            .width(Length::Fixed(720.0))
            .max_height(620.0)
            .style(surface_style(colors, RADIUS.lg));

        // Un clic hors du panneau ferme, comme les autres surcouches.
        let dismiss: Element<'_, UiMessage> =
            iced::widget::mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::ToggleAppearance(false))
                .into();

        let centred: Element<'_, UiMessage> = container(
            column![
                iced::widget::space::vertical(),
                row![
                    iced::widget::space::horizontal(),
                    iced::widget::opaque(panel),
                    iced::widget::space::horizontal()
                ],
                iced::widget::space::vertical(),
            ]
            .spacing(0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        Some(iced::widget::stack![dismiss, centred].into())
    }
}
