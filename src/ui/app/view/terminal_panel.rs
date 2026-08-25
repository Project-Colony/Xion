//! The integrated terminal panel, which slides up below the file list.
//!
//! Moved out of `view()` unchanged, except that the `if` became an early
//! return so the caller decides where to place the panel.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, mouse_area, row, tooltip};
use iced::{Alignment, Background, Element, Length, Theme, border};

use crate::ui::UiMessage;
use crate::ui::theme::icons;
use crate::ui::theme::layout::{TAB_STRIP_HEIGHT, TERMINAL_RESIZE_BAR_HEIGHT};

/// L'espace laissé sous les onglets, entre eux et le bas de leur bande.
const TAB_BOTTOM_GAP: f32 = 4.0;

use super::XionApp;
use super::widgets::{self, ViewCtx};

use super::widgets::RADIUS;
use super::widgets::caption_text;
use super::widgets::glyph_text;
use super::widgets::surface_style;

impl XionApp {
    /// The terminal panel, or `None` while it is fully collapsed.
    pub(super) fn render_terminal_panel(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let term_progress = self.terminal_anim_progress;
        if term_progress <= 0.001 {
            return None;
        }

        let animated_height = self.pane_resize.terminal_height * term_progress;

        // La grille de l'émulateur, ou le mot de Xion quand il n'y en a pas.
        //
        // `TerminalView` est un widget à part entière : il dessine les cellules,
        // reçoit les frappes quand il a le focus et prévient le pseudo-terminal
        // de sa taille. Il remplace le texte défilant et le champ de saisie —
        // taper se fait maintenant dans le terminal, comme dans tout terminal.
        let output_area: Element<'_, UiMessage> = match self
            .terminal
            .active_ref()
            .and_then(|tab| tab.terminal.as_ref())
        {
            Some(terminal) => {
                container(iced_term::TerminalView::show(terminal).map(UiMessage::TerminalEvent))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    // Le texte a besoin d'air en haut. La jonction avec
                    // l'onglet actif est déjà assurée par le fond du panneau,
                    // continu sous les deux ; retirer ce rembourrage ne l'a pas
                    // renforcée, ça a seulement collé la première ligne du shell
                    // contre la barre d'onglets.
                    .padding([spacing.xs, spacing.sm])
                    // Surtout pas de `clip(true)` ici.
                    //
                    // Le widget dessine sa grille dans un cache de canevas, et
                    // le rognage d'iced la fait disparaître entièrement — écran
                    // noir, aucune cellule. Constaté deux fois. Le débordement
                    // d'une ligne sur la bordure basse est le moindre mal ; s'il
                    // faut le corriger, ce sera en donnant au panneau une
                    // hauteur multiple de la hauteur de cellule, pas en rognant.
                    .into()
            }
            None => {
                let message = self
                    .terminal
                    .active_ref()
                    .and_then(|tab| tab.notice.clone())
                    .unwrap_or_else(|| "Démarrage de l'interpréteur…".to_string());
                container(caption_text(typography, message).style(move |_: &Theme| {
                    iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    }
                }))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([spacing.xs, spacing.sm])
                .into()
            }
        };

        // Les onglets, dessinés comme ceux de la fenêtre.
        //
        // Ils étaient plats, avec leur croix de fermeture posée *à côté* — même
        // taille, même forme que l'onglet, donc un onglet et sa croix se
        // lisaient comme deux onglets. C'est le défaut que la bande du haut
        // avait déjà corrigé, et qu'il restait ici.
        //
        // Ce qui fait qu'une rangée de boutons devient une rangée d'onglets :
        // l'actif touche le bas de la bande, arrondi en haut seulement, et sa
        // couleur est celle de la zone qu'il ouvre.
        let mut tab_bar = row![].spacing(0).align_y(Alignment::End);
        let closable = self.terminal.tabs.len() > 1;

        for (index, tab) in self.terminal.tabs.iter().enumerate() {
            let is_active = index == self.terminal.active_tab;

            let mut inner = row![caption_text(typography, shorten(&tab.title))]
                .spacing(spacing.xs)
                .align_y(Alignment::Center);

            if closable {
                inner = inner.push(
                    button(glyph_text(typography, icons::CLOSE).color(colors.text_muted))
                        .padding(2.0)
                        .style(
                            move |_: &Theme, status: ButtonStatus| iced::widget::button::Style {
                                background: None,
                                text_color: match status {
                                    ButtonStatus::Hovered => colors.text_primary,
                                    _ => colors.text_muted,
                                },
                                ..Default::default()
                            },
                        )
                        .on_press(UiMessage::TerminalCloseTab(index)),
                );
            }

            let tab_body = button(inner)
                .padding([spacing.xs, spacing.md])
                // Plus court que la bande, pour laisser un espace sous lui.
                //
                // Il la remplissait de haut en bas et touchait son bord
                // inférieur : c'est ce contact qui raccorde un onglet à la zone
                // qu'il ouvre. Le détacher est un choix assumé — ça aère, au
                // prix de cette jonction.
                .height(Length::Fixed(TAB_STRIP_HEIGHT - TAB_BOTTOM_GAP))
                .style(move |_: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: if is_active {
                            colors.text_primary
                        } else {
                            colors.text_muted
                        },
                        ..Default::default()
                    };
                    let rounded_top = iced::Border {
                        radius: iced::border::Radius::default()
                            .top_left(RADIUS.md)
                            .top_right(RADIUS.md),
                        ..Default::default()
                    };
                    if is_active {
                        style.background = Some(Background::Color(colors.panel_background));
                        style.border = rounded_top;
                    } else if matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(colors.hover));
                        style.border = rounded_top;
                    }
                    style
                });

            let tab_body = if is_active {
                tab_body
            } else {
                tab_body.on_press(UiMessage::TerminalSwitchTab(index))
            };
            tab_bar = tab_bar.push(tab_body);
        }

        tab_bar = tab_bar.push(
            button(caption_text(typography, icons::NEW).color(colors.text_muted))
                .padding([spacing.xs, spacing.sm])
                .style(widgets::hover_button_style(
                    colors,
                    colors.text_muted,
                    Some(RADIUS.md),
                ))
                .on_press(UiMessage::TerminalAddTab),
        );

        let muted_color = colors.text_muted;

        // Les commandes passent à droite, séparées des onglets.
        //
        // Le choix de l'interpréteur se trouvait au milieu des onglets, ce qui
        // en faisait des onglets d'apparence sans en être : cliquer « bash » ne
        // change pas de session, ça change le programme de la session courante.
        tab_bar = tab_bar.push(horizontal_space());

        let current_shell = &self.state.config.terminal_shell;
        for choice in crate::ui::app::shell::available_shells() {
            let is_active = *current_shell == choice.config;
            let shell_cfg_clone = choice.config.clone();
            tab_bar = tab_bar.push(
                button(caption_text(typography, choice.label))
                    .padding([spacing.xs, spacing.xs])
                    .on_press(UiMessage::SetShell(shell_cfg_clone))
                    .style(
                        move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: if is_active {
                                colors.accent
                            } else {
                                muted_color
                            },
                            background: if is_active {
                                Some(Background::Color(colors.hover))
                            } else {
                                None
                            },
                            border: if is_active {
                                border::rounded(RADIUS.sm).color(colors.accent).width(1.0)
                            } else {
                                border::rounded(RADIUS.sm).width(0.0)
                            },
                            ..Default::default()
                        },
                    ),
            );
        }

        // Ctrl-C. A pipe-backed shell had no way to interrupt anything; a
        // pty does, and a runaway command needs a visible way out.
        tab_bar = tab_bar.push(tooltip(
            button(glyph_text(typography, "⛔"))
                .padding([spacing.xs, spacing.sm])
                .on_press(UiMessage::TerminalInterrupt)
                .style(
                    move |_: &Theme, status: ButtonStatus| iced::widget::button::Style {
                        text_color: match status {
                            ButtonStatus::Hovered => colors.text_primary,
                            _ => muted_color,
                        },
                        ..Default::default()
                    },
                ),
            // Cette infobulle était la seule à ne pas nommer sa police : elle
            // s'affichait donc dans la police par défaut d'iced, pas dans celle
            // des légendes comme toutes les autres.
            container(caption_text(typography, "Interrompre (Ctrl-C)"))
                .padding(spacing.xs)
                .style(surface_style(colors, RADIUS.sm)),
            tooltip::Position::Top,
        ));

        // Rembourrage en bas nul : l'onglet actif doit atteindre la grille qu'il
        // ouvre. Douze pixels d'écart suffisaient à le faire flotter, et toute
        // la rangée redevenait une rangée de boutons.
        let tab_bar_element: Element<'_, UiMessage> = container(tab_bar)
            .width(Length::Fill)
            .height(Length::Fixed(TAB_STRIP_HEIGHT))
            .padding(iced::Padding {
                top: 0.0,
                right: spacing.sm,
                bottom: TAB_BOTTOM_GAP,
                left: spacing.sm,
            })
            .into();

        // La poignée : tirer la bordure haute agrandit le panneau.
        //
        // Onze lignes suffisaient à une commande et sa réponse, mais un
        // programme plein écran s'y trouvait écrasé sans recours — et l'écran
        // alterné n'a pas d'historique, donc défiler n'était pas une issue.
        //
        // Transparente au repos : elle vit *dans* la bande d'onglets et en
        // partage donc l'aplat. Lui donner un fond propre insérait une troisième
        // couleur entre le panneau et la bande, et c'est ce liseré qui faisait
        // flotter la rangée.
        let resize_handle: Element<'_, UiMessage> = mouse_area(
            container(row![])
                .width(Length::Fill)
                .height(Length::Fixed(TERMINAL_RESIZE_BAR_HEIGHT))
                .style(move |_| iced::widget::container::Style {
                    background: self
                        .pane_resize
                        .terminal_resizing
                        .then_some(Background::Color(colors.hover)),
                    ..Default::default()
                }),
        )
        .on_press(UiMessage::TerminalResizeStart)
        .on_release(UiMessage::TerminalResizeEnd)
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .into();

        // Les onglets seuls sous l'aplat, sans contour — le même style que
        // l'en-tête de fenêtre, pour la même raison : une ligne tracée sous les
        // onglets couperait la jonction qui les fait lire comme des onglets.
        let strip: Element<'_, UiMessage> = container(tab_bar_element)
            .width(Length::Fill)
            .style(widgets::tab_strip_style(colors, RADIUS.lg))
            .into();

        let term_panel: Element<'_, UiMessage> = container(column![strip, output_area].spacing(0))
            .width(Length::Fill)
            .height(Length::Fixed(animated_height))
            .style(surface_style(colors, RADIUS.lg))
            .into();

        // La poignée vit au-dessus du panneau, dans l'espace qui le sépare de la
        // liste — pas dedans.
        //
        // À l'intérieur, elle laissait quatre pixels d'aplat au-dessus de
        // l'onglet actif : celui-ci remplissait le bas de la bande sans en
        // atteindre le haut, et flottait donc dans son propre noir. Ici la bande
        // ne contient plus que les onglets, et l'onglet la remplit entièrement.
        Some(column![resize_handle, term_panel].spacing(0).into())
    }
}

/// Raccourcit un titre d'onglet trop long pour la bande.
///
/// Les titres viennent maintenant du programme lui-même — le shell y met son
/// dossier courant, `claude` y met le sujet de la conversation. Rien ne borne
/// leur longueur, et un seul onglet bavard chassait tous les autres hors de la
/// bande.
fn shorten(title: &str) -> String {
    const MAX_CHARS: usize = 22;
    let characters: Vec<char> = title.chars().collect();
    if characters.len() <= MAX_CHARS {
        return title.to_string();
    }
    // Couper sur les caractères et non les octets : un titre accentué se
    // tronquait au milieu d'un point de code.
    let kept: String = characters[..MAX_CHARS.saturating_sub(1)].iter().collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn a_short_title_is_left_alone() {
        assert_eq!(shorten("Terminal 1"), "Terminal 1");
    }

    #[test]
    fn a_long_title_is_cut_and_marked() {
        let cut = shorten("mothersphere@Colony:~/Documents/Repositories/Xion");
        assert!(cut.ends_with('…'));
        assert_eq!(cut.chars().count(), 22);
    }

    /// Le titre vient du programme, donc il peut être accentué : couper sur les
    /// octets casserait un point de code en deux.
    #[test]
    fn an_accented_title_survives_the_cut() {
        let cut = shorten("éééééééééééééééééééééééééééé");
        assert_eq!(cut.chars().count(), 22);
        assert!(cut.starts_with('é'));
    }
}
