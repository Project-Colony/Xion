//! La page de préférences.
//!
//! Elle **remplace la zone de contenu**. Ce n'est ni une modale, ni une
//! fenêtre, ni une bulle — la convention Colony nomme les trois comme
//! contre-exemples, et la première écriture de cet écran était précisément une
//! modale. Le chrome du programme, lui, ne bouge pas : l'en-tête et la barre
//! d'état restent en place, on ne remplace que ce qu'il y a entre les deux.
//!
//! On y entre et on en sort par le bouton « Xion » de la bande d'onglets. Le
//! bouton « Fermer » de la page envoie le même message que lui : deux sorties,
//! un seul message, donc aucun moyen de les faire diverger.
//!
//! Il n'y a pas de bouton « Enregistrer », et il ne doit pas y en avoir : un
//! changement s'applique au moment où il est fait.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{button, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme};

use crate::ui::theme::icons;
use crate::ui::{PreferencesCategory, UiMessage};

use super::XionApp;
use super::widgets::ViewCtx;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::title_text;

/// La colonne des catégories. Assez large pour « Accessibilité », qui est le
/// plus long des cinq intitulés, sans que la colonne ne se redimensionne quand
/// la catégorie change.
const CATEGORY_COLUMN_WIDTH: f32 = 190.0;

impl PreferencesCategory {
    /// L'intitulé affiché dans la colonne de gauche.
    fn label(self) -> &'static str {
        match self {
            Self::General => "Général",
            Self::Appearance => "Apparence",
            Self::Accessibility => "Accessibilité",
            Self::Files => "Fichiers",
            Self::About => "À propos",
        }
    }

    /// La ligne sous le titre de la catégorie : ce qu'elle change, pas son nom
    /// répété.
    fn description(self) -> &'static str {
        match self {
            // La convention impose cette phrase-ci, mot pour mot, parce qu'elle
            // porte le contrat qui compte : il n'y a pas de bouton
            // « Enregistrer », et son absence doit être expliquée quelque part.
            Self::General => "Les préférences sont enregistrées automatiquement.",
            Self::Appearance => "Le thème s'applique immédiatement, sans redémarrage.",
            Self::Accessibility => "Ces réglages se combinent avec le thème choisi.",
            Self::Files => "Ce que la liste affiche, et dans quel ordre.",
            Self::About => "Version, licence et provenance des composants.",
        }
    }
}

impl XionApp {
    /// La page entière, ou `None` quand elle est fermée.
    ///
    /// Rendue par `view()` à la place du corps — la barre latérale des fichiers
    /// disparaît avec lui. Deux colonnes de gauche, l'une de dossiers et
    /// l'autre de catégories, ne se distingueraient pas au premier regard.
    pub(super) fn render_preferences_page(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        if !self.menus.preferences.open {
            return None;
        }

        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let close = button(caption_text(typography, "Fermer").color(colors.text_muted))
            .padding([6.0, 14.0])
            .style(
                move |_theme: &Theme, status: ButtonStatus| iced::widget::button::Style {
                    background: matches!(status, ButtonStatus::Hovered)
                        .then(|| Background::Color(colors.hover)),
                    text_color: colors.text_muted,
                    border: iced::Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .on_press(UiMessage::TogglePreferences);

        let heading = row![
            title_text(typography, "Préférences").color(colors.text_primary),
            iced::widget::space::horizontal(),
            close,
        ]
        .align_y(Alignment::Center);

        let selected = self.menus.preferences.category;
        let mut categories = column![].spacing(spacing.xs).width(Length::Fill);
        for category in PreferencesCategory::ALL {
            let active = category == selected;
            categories = categories.push(
                button(body_text(typography, category.label()))
                    .width(Length::Fill)
                    .padding([8.0, 14.0])
                    .style(move |_theme: &Theme, status: ButtonStatus| {
                        iced::widget::button::Style {
                            background: if active {
                                Some(Background::Color(colors.accent))
                            } else if matches!(status, ButtonStatus::Hovered) {
                                Some(Background::Color(colors.hover))
                            } else {
                                None
                            },
                            text_color: if active {
                                colors.text_primary
                            } else {
                                colors.text_muted
                            },
                            border: iced::Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .on_press(UiMessage::SelectPreferencesCategory(category)),
            );
        }

        let category_column = container(categories)
            .width(Length::Fixed(CATEGORY_COLUMN_WIDTH))
            .height(Length::Fill);

        let panel = column![
            title_text(typography, selected.label()).color(colors.text_primary),
            caption_text(typography, selected.description()).color(colors.text_muted),
            self.render_preferences_category(ctx, selected),
        ]
        .spacing(spacing.md)
        .width(Length::Fill);

        let body = row![
            category_column,
            scrollable(container(panel).padding(iced::Padding {
                left: spacing.lg,
                ..iced::Padding::ZERO
            }))
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .height(Length::Fill);

        Some(
            container(column![heading, body].spacing(spacing.lg))
                .padding(spacing.lg)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        )
    }

    /// Le contenu d'une catégorie : une liste de sections repliées.
    fn render_preferences_category(
        &self,
        ctx: ViewCtx,
        category: PreferencesCategory,
    ) -> Element<'_, UiMessage> {
        match category {
            PreferencesCategory::General => self.preferences_general(ctx),
            PreferencesCategory::Appearance => self.preferences_appearance(ctx),
            PreferencesCategory::Accessibility => self.preferences_accessibility(ctx),
            PreferencesCategory::Files => self.preferences_files(ctx),
            PreferencesCategory::About => self.preferences_about(ctx),
        }
    }

    /// Une section repliable, dont l'état d'ouverture vit dans `menus`.
    fn section<'a>(
        &self,
        ctx: ViewCtx,
        key: &'static str,
        title: &str,
        content: Element<'a, UiMessage>,
    ) -> Element<'a, UiMessage> {
        colony_ui::widgets::collapsible_section(
            &ctx.typography.colony(),
            title,
            self.menus.preferences.expanded.contains(key),
            UiMessage::TogglePreferencesSection(key),
            content,
        )
    }

    /// Une ligne « intitulé : valeur » pour ce qui se lit sans se modifier.
    fn readout<'a>(ctx: ViewCtx, label: &str, value: String) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        row![
            caption_text(typography, label.to_string()).color(colors.text_muted),
            iced::widget::space::horizontal(),
            caption_text(typography, value).color(colors.text_primary),
        ]
        .spacing(spacing.md)
        .align_y(Alignment::Center)
        .into()
    }

    fn preferences_general(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let spacing = ctx.spacing;
        let config = &self.state.config;

        let startup = column![
            Self::readout(
                ctx,
                "Dossier d'ouverture",
                config.start_path.display().to_string(),
            ),
            Self::readout(
                ctx,
                "Onglets restaurés au démarrage",
                config.tabs.len().to_string(),
            ),
        ]
        .spacing(spacing.sm);

        // Les mêmes intitulés que le sélecteur du panneau terminal, qui est
        // l'endroit où ce réglage se change. Deux noms pour le même
        // interpréteur donneraient l'impression de deux réglages distincts.
        let shell = match &config.terminal_shell {
            crate::core::ShellConfig::Cmd => "CMD".to_string(),
            crate::core::ShellConfig::PowerShell => "PowerShell".to_string(),
            crate::core::ShellConfig::GitBash => "Bash".to_string(),
            crate::core::ShellConfig::Custom(path) => path.clone(),
        };

        let terminal = column![
            Self::readout(ctx, "Interpréteur", shell),
            caption_text(
                ctx.typography,
                "Se change depuis le panneau terminal, en bas de la fenêtre.",
            )
            .color(ctx.colors.text_muted),
        ]
        .spacing(spacing.sm);

        column![
            self.section(ctx, "general.startup", "Démarrage", startup.into()),
            self.section(ctx, "general.terminal", "Terminal", terminal.into()),
        ]
        .spacing(spacing.sm)
        .into()
    }

    fn preferences_appearance(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let typo = typography.colony();
        let choice = &self.state.config.theme;

        // Les deux sélecteurs se rendent seuls depuis le catalogue partagé :
        // ajouter une famille de thèmes ne demande aucun changement ici.
        let theme = colony_ui::widgets::theme_picker(
            &typo,
            &choice.family,
            &choice.variant,
            UiMessage::SetThemeVariant,
        );

        let accent = column![
            colony_ui::widgets::accent_picker(
                &typo,
                choice.accent.as_deref(),
                UiMessage::SetAccent
            ),
            caption_text(typography, "Sans accent choisi, celui du thème s'applique.")
                .color(colors.text_muted),
        ]
        .spacing(spacing.sm);

        let density = colony_ui::widgets::functional_toggle(
            &typo,
            "Densité compacte",
            "Des lignes de 22 pixels au lieu de 32 : plus de fichiers à l'écran.",
            self.state.config.compact_mode,
            UiMessage::ToggleCompactMode,
        );

        column![
            self.section(ctx, "appearance.theme", "Thème", theme),
            self.section(ctx, "appearance.accent", "Couleurs", accent.into()),
            self.section(ctx, "appearance.density", "Densité", density),
        ]
        .spacing(spacing.sm)
        .into()
    }

    fn preferences_accessibility(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let spacing = ctx.spacing;
        let typo = ctx.typography.colony();

        let vision = colony_ui::widgets::functional_toggle(
            &typo,
            "Contraste élevé",
            "Renforce la séparation des surfaces et du texte, sur n'importe quelle palette.",
            self.state.config.theme.high_contrast,
            UiMessage::ToggleHighContrast,
        );

        column![self.section(ctx, "a11y.vision", "Vision", vision)]
            .spacing(spacing.sm)
            .into()
    }

    fn preferences_files(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let spacing = ctx.spacing;
        let typo = ctx.typography.colony();
        let config = &self.state.config;

        let display = column![
            colony_ui::widgets::functional_toggle(
                &typo,
                "Afficher les fichiers cachés",
                "Ceux dont le nom commence par un point.",
                config.list.show_hidden,
                UiMessage::ToggleShowHidden,
            ),
            colony_ui::widgets::functional_toggle(
                &typo,
                "Dossiers en premier",
                "Regroupe les dossiers en tête de liste plutôt que de tout trier ensemble.",
                config.list.directories_first,
                UiMessage::ToggleDirectoriesFirst,
            ),
        ]
        .spacing(spacing.sm);

        let git = colony_ui::widgets::functional_toggle(
            &typo,
            "Respecter .gitignore",
            "Masque dans un dépôt les fichiers que Git ignore.",
            config.respect_gitignore,
            UiMessage::ToggleGitignore,
        );

        column![
            self.section(ctx, "files.display", "Affichage", display.into()),
            self.section(ctx, "files.git", "Git", git),
        ]
        .spacing(spacing.sm)
        .into()
    }

    fn preferences_about(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let identity = column![
            row![
                body_text(typography, icons::SETTINGS).color(colors.accent),
                title_text(typography, "Xion").color(colors.text_primary),
            ]
            .spacing(spacing.sm)
            .align_y(Alignment::Center),
            Self::readout(ctx, "Version", env!("CARGO_PKG_VERSION").to_string()),
            Self::readout(ctx, "Licence", "GPL-3.0-or-later".to_string()),
        ]
        .spacing(spacing.sm);

        let credits = column![
            caption_text(
                typography,
                "Les thèmes, les accents et les sélecteurs de cette page viennent de \
                 colony-ui, la caisse partagée de l'écosystème Colony.",
            )
            .color(colors.text_muted),
            Self::readout(
                ctx,
                "Familles de thèmes",
                colony_ui::THEME_FAMILIES.len().to_string(),
            ),
        ]
        .spacing(spacing.sm);

        column![
            self.section(ctx, "about.identity", "Xion", identity.into()),
            self.section(ctx, "about.credits", "Composants", credits.into()),
        ]
        .spacing(spacing.sm)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::PreferencesCategory;

    /// La position qu'occupe chaque catégorie, énoncée à part de `ALL`.
    ///
    /// Ce `match` est exhaustif, donc ajouter une variante sans lui donner de
    /// rang ne compile pas. C'est là tout l'intérêt : `ALL` est un tableau
    /// écrit à la main, et rien d'autre n'obligerait à l'y ajouter.
    fn expected_rank(category: PreferencesCategory) -> usize {
        match category {
            PreferencesCategory::General => 0,
            PreferencesCategory::Appearance => 1,
            PreferencesCategory::Accessibility => 2,
            PreferencesCategory::Files => 3,
            PreferencesCategory::About => 4,
        }
    }

    /// « Do not reorder the first three. They are what a user hunting for a
    /// setting scans first » — la convention de l'écosystème est explicite, et
    /// c'est le genre de contrainte qu'un remaniement casse sans s'en rendre
    /// compte. Le test la rend visible.
    #[test]
    fn the_first_three_categories_are_the_imposed_ones_in_order() {
        assert_eq!(
            &PreferencesCategory::ALL[..3],
            &[
                PreferencesCategory::General,
                PreferencesCategory::Appearance,
                PreferencesCategory::Accessibility,
            ]
        );
    }

    /// « About last where it exists ».
    #[test]
    fn about_comes_last() {
        assert_eq!(
            PreferencesCategory::ALL.last(),
            Some(&PreferencesCategory::About)
        );
    }

    #[test]
    fn every_category_appears_once_and_at_its_rank() {
        for (rank, category) in PreferencesCategory::ALL.into_iter().enumerate() {
            assert_eq!(rank, expected_rank(category), "{category:?} mal placée");
        }
        assert_eq!(PreferencesCategory::ALL.len(), 5);
    }

    /// La catégorie ouverte à l'arrivée est la première, pas une au hasard.
    #[test]
    fn the_default_category_is_the_first_one() {
        assert_eq!(PreferencesCategory::default(), PreferencesCategory::ALL[0]);
    }

    /// Une description qui répète le titre n'apprend rien ; la convention
    /// demande qu'elle dise la conséquence du réglage.
    #[test]
    fn each_category_says_something_different() {
        let mut descriptions: Vec<&str> = PreferencesCategory::ALL
            .into_iter()
            .map(|category| category.description())
            .collect();
        descriptions.sort_unstable();
        let before = descriptions.len();
        descriptions.dedup();
        assert_eq!(before, descriptions.len(), "deux descriptions identiques");

        for category in PreferencesCategory::ALL {
            assert_ne!(category.label(), category.description());
        }
    }

    /// Le contrat que porte la page : pas de bouton « Enregistrer », donc la
    /// phrase qui l'explique doit être là.
    #[test]
    fn general_carries_the_automatic_saving_contract() {
        assert!(
            PreferencesCategory::General
                .description()
                .contains("automatiquement")
        );
    }
}
