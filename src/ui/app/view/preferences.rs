//! The preferences page.
//!
//! It **replaces the content area**. It is not a modal, a window or a popover:
//! the Colony convention names all three as counter-examples, and the first
//! version of this screen was precisely a modal. The program's chrome stays
//! put: the header and the status bar remain, only what sits between them is
//! replaced.
//!
//! It is entered and left through the "Xion" button in the top-left corner of
//! the header. The page's Close button sends the same message: two exits, one
//! message, so no way for them to diverge.
//!
//! There is no Save button, and there must not be one: a change applies the
//! moment it is made.
//!
//! The frame, the category list, General's heading and the whole of
//! Appearance and Accessibility come from colony-ui's preferences kit, so they
//! look and behave as in every other Colony program. Xion adds its own
//! sections to General and Appearance, and its own Files and About categories.

use iced::widget::{column, row};
use iced::{Alignment, Element};

use colony_ui::widgets::{self, ABOUT_CATEGORY, PreferencesView, STANDARD_CATEGORIES};

use crate::core::Language;
use crate::i18n::t;
use crate::ui::{PreferencesCategory, UiMessage};

use super::XionApp;
use super::widgets::ViewCtx;
use super::widgets::caption_text;

/// The program's name, as the identity button and About show it. A proper
/// noun: the same in every language.
pub(super) const PROGRAM_NAME: &str = "Xion";

impl PreferencesCategory {
    /// The locale key of the label in the category list.
    ///
    /// The first three and About are colony-ui's shared labels, so they read
    /// the same in every Colony program.
    fn label_key(self) -> &'static str {
        match self {
            Self::General => STANDARD_CATEGORIES[0],
            Self::Appearance => STANDARD_CATEGORIES[1],
            Self::Accessibility => STANDARD_CATEGORIES[2],
            Self::Files => "preferences_cat_files",
            Self::About => ABOUT_CATEGORY,
        }
    }
}

/// How a language is named in the picker.
fn language_label_key(language: Language) -> &'static str {
    match language {
        Language::Auto => "preferences_language_auto",
        Language::English => "preferences_language_en",
        Language::French => "preferences_language_fr",
    }
}

impl XionApp {
    /// The whole page, or `None` when it is closed.
    ///
    /// Rendered by `view()` in place of the body, and the file sidebar goes
    /// with it. Two left columns, one of folders and one of categories, would
    /// not be told apart at first glance.
    pub(super) fn render_preferences_page(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        let preferences = &self.menus.preferences;
        if !preferences.open {
            return None;
        }

        let typo = ctx.typography.colony();
        let kit = PreferencesView::new(
            typo,
            &self.state.config.standard,
            &preferences.expanded,
            UiMessage::Preference,
            UiMessage::TogglePreferencesSection,
        );

        let content: Element<'_, UiMessage> = match preferences.category {
            PreferencesCategory::General => self.preferences_general(ctx, kit.general()),
            PreferencesCategory::Appearance => self.preferences_appearance(ctx, kit.appearance()),
            PreferencesCategory::Accessibility => kit.accessibility().into(),
            PreferencesCategory::Files => self.preferences_files(ctx),
            PreferencesCategory::About => self.preferences_about(ctx),
        };

        let selected = PreferencesCategory::ALL
            .iter()
            .position(|category| *category == preferences.category)
            .unwrap_or(0);
        let categories = widgets::category_list(
            &typo,
            PreferencesCategory::ALL.map(|category| t(category.label_key())),
            selected,
            |index| UiMessage::SelectPreferencesCategory(PreferencesCategory::ALL[index]),
        );

        Some(widgets::preferences_page(
            &typo,
            categories,
            content,
            UiMessage::TogglePreferences,
        ))
    }

    /// A collapsible section, whose open state lives in `menus`, opening on a
    /// line that says what its settings change.
    fn section<'a>(
        &self,
        ctx: ViewCtx,
        key: &'static str,
        title: &str,
        description: &str,
        content: Element<'a, UiMessage>,
    ) -> Element<'a, UiMessage> {
        let typo = ctx.typography.colony();
        colony_ui::widgets::collapsible_section(
            &typo,
            title,
            self.menus.preferences.expanded.contains(key),
            UiMessage::TogglePreferencesSection(key),
            column![widgets::section_description(&typo, description), content].into(),
        )
    }

    /// A "label: value" line for what is read and not edited.
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

    /// General: the kit's heading and contract, then Language, Startup and
    /// Terminal.
    fn preferences_general<'a>(
        &'a self,
        ctx: ViewCtx,
        page: iced::widget::Column<'a, UiMessage>,
    ) -> Element<'a, UiMessage> {
        let spacing = ctx.spacing;
        let config = &self.state.config;
        let typo = ctx.typography.colony();

        let languages =
            Language::ALL.map(|language| (language, t(language_label_key(language)).to_string()));
        let language = widgets::setting_row(
            &typo,
            t("preferences_language"),
            t("preferences_language_desc"),
            widgets::dropdown(&typo, languages, &config.language, UiMessage::SetLanguage),
        );

        let startup = column![
            Self::readout(
                ctx,
                t("preferences_start_folder"),
                config.start_path.display().to_string(),
            ),
            Self::readout(
                ctx,
                t("preferences_restored_tabs"),
                self.tab_manager.count().to_string(),
            ),
        ]
        .spacing(spacing.sm);

        // The same label as the terminal panel's picker, produced by the same
        // function: two names for the same shell would look like two separate
        // settings.
        let shell = crate::ui::app::shell::label_for(&config.terminal_shell);
        let terminal = Self::readout(ctx, t("preferences_shell"), shell);

        page.push(self.section(
            ctx,
            "general.language",
            t("preferences_section_language"),
            t("preferences_section_language_desc"),
            language,
        ))
        .push(self.section(
            ctx,
            "general.startup",
            t("preferences_section_startup"),
            t("preferences_section_startup_desc"),
            startup.into(),
        ))
        .push(self.section(
            ctx,
            "general.terminal",
            t("preferences_section_terminal"),
            t("preferences_section_terminal_desc"),
            terminal,
        ))
        .into()
    }

    /// Appearance: the kit's Theme, Colors, Typography, Effects and Preview,
    /// then the list density, which is Xion's own.
    fn preferences_appearance<'a>(
        &'a self,
        ctx: ViewCtx,
        page: iced::widget::Column<'a, UiMessage>,
    ) -> Element<'a, UiMessage> {
        let density = widgets::functional_toggle(
            &ctx.typography.colony(),
            t("preferences_compact"),
            t("preferences_compact_desc"),
            self.state.config.compact_mode,
            UiMessage::ToggleCompactMode,
        );

        page.push(self.section(
            ctx,
            "appearance.density",
            t("preferences_section_density"),
            t("preferences_section_density_desc"),
            density,
        ))
        .into()
    }

    fn preferences_files(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let spacing = ctx.spacing;
        let typo = ctx.typography.colony();
        let config = &self.state.config;

        let display = column![
            widgets::functional_toggle(
                &typo,
                t("preferences_show_hidden"),
                t("preferences_show_hidden_desc"),
                config.list.show_hidden,
                UiMessage::ToggleShowHidden,
            ),
            widgets::functional_toggle(
                &typo,
                t("preferences_directories_first"),
                t("preferences_directories_first_desc"),
                config.list.directories_first,
                UiMessage::ToggleDirectoriesFirst,
            ),
        ]
        .spacing(spacing.sm);

        let git = widgets::functional_toggle(
            &typo,
            t("preferences_respect_gitignore"),
            t("preferences_respect_gitignore_desc"),
            config.respect_gitignore,
            UiMessage::ToggleGitignore,
        );

        widgets::category_heading(
            &typo,
            t("preferences_cat_files"),
            t("preferences_cat_files_desc"),
        )
        .push(self.section(
            ctx,
            "files.display",
            t("preferences_section_display"),
            t("preferences_section_display_desc"),
            display.into(),
        ))
        .push(self.section(
            ctx,
            "files.git",
            t("preferences_section_git"),
            t("preferences_section_git_desc"),
            git,
        ))
        .into()
    }

    fn preferences_about(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let spacing = ctx.spacing;
        let typo = ctx.typography.colony();

        let identity = column![
            Self::readout(
                ctx,
                t("preferences_version"),
                env!("CARGO_PKG_VERSION").to_string()
            ),
            Self::readout(
                ctx,
                t("preferences_license"),
                env!("CARGO_PKG_LICENSE").to_string()
            ),
        ]
        .spacing(spacing.sm);

        let components = Self::readout(
            ctx,
            t("preferences_theme_families"),
            colony_ui::THEME_FAMILIES.len().to_string(),
        );

        widgets::category_heading(&typo, t(ABOUT_CATEGORY), t("preferences_cat_about_desc"))
            .push(self.section(
                ctx,
                "about.identity",
                PROGRAM_NAME,
                t("preferences_section_identity_desc"),
                identity.into(),
            ))
            .push(self.section(
                ctx,
                "about.components",
                t("preferences_section_components"),
                t("preferences_section_components_desc"),
                components,
            ))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::PreferencesCategory;

    /// The position each category holds, stated apart from `ALL`.
    ///
    /// This `match` is exhaustive, so adding a variant without giving it a rank
    /// does not compile. That is the whole point: `ALL` is a hand-written array,
    /// and nothing else would force adding the variant to it.
    fn expected_rank(category: PreferencesCategory) -> usize {
        match category {
            PreferencesCategory::General => 0,
            PreferencesCategory::Appearance => 1,
            PreferencesCategory::Accessibility => 2,
            PreferencesCategory::Files => 3,
            PreferencesCategory::About => 4,
        }
    }

    /// "Do not reorder the first three. They are what a user hunting for a
    /// setting scans first": the ecosystem convention is explicit, and the
    /// first three carry colony-ui's own labels, in its order.
    #[test]
    fn the_first_three_categories_are_the_shared_ones_in_order() {
        let keys: Vec<&str> = PreferencesCategory::ALL[..3]
            .iter()
            .map(|category| category.label_key())
            .collect();
        assert_eq!(keys, colony_ui::widgets::STANDARD_CATEGORIES);
    }

    /// "About last where it exists".
    #[test]
    fn about_comes_last() {
        assert_eq!(
            PreferencesCategory::ALL.last(),
            Some(&PreferencesCategory::About)
        );
        assert_eq!(
            PreferencesCategory::About.label_key(),
            colony_ui::widgets::ABOUT_CATEGORY
        );
    }

    #[test]
    fn every_category_appears_once_and_at_its_rank() {
        for (rank, category) in PreferencesCategory::ALL.into_iter().enumerate() {
            assert_eq!(rank, expected_rank(category), "{category:?} out of place");
        }
        assert_eq!(PreferencesCategory::ALL.len(), 5);
    }

    /// The category open on arrival is the first one, not a random one.
    #[test]
    fn the_default_category_is_the_first_one() {
        assert_eq!(PreferencesCategory::default(), PreferencesCategory::ALL[0]);
    }
}
