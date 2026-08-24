//! The search field, the command bar and the loading badge.
//!
//! Moved out of `render_header` unchanged.

use iced::widget::{container, progress_bar, row, text, text_input};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::{self, RADIUS, RADIUS_PILL};
use super::widgets::{TipVariant, ViewCtx};

impl XionApp {
    /// The search field, the command bar row and the loading badge, in that
    /// order — the caller lays them out.
    pub(super) fn render_command_bar(
        &self,
        ctx: ViewCtx,
    ) -> (
        Element<'_, UiMessage>,
        Element<'_, UiMessage>,
        Element<'_, UiMessage>,
    ) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let toolbar_button = |label: String| widgets::toolbar_button(ctx, label);

        let active_tab_title = self
            .tab_manager
            .tabs
            .get(self.tab_manager.active)
            .map(|tab| tab.title.as_str())
            .unwrap_or("Ce PC");
        let search_input = text_input(
            &format!("Rechercher dans : {}", active_tab_title),
            &self.search.input,
        )
        .id(iced::widget::Id::new("search_input"))
        .on_input(UiMessage::SearchInputChanged)
        .on_submit(UiMessage::SearchInputSubmitted)
        .size(typography.caption)
        .font(typography.caption_font)
        .padding([spacing.xs, spacing.sm])
        .width(Length::Fill)
        .style(
            move |_theme: &Theme, _status| iced::widget::text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: border::rounded(0.0).width(0.0),
                icon: colors.text_muted,
                placeholder: colors.text_muted,
                value: colors.text_primary,
                selection: colors.selection,
            },
        );
        let search_bar = container(
            row![
                text(icons::SEARCH)
                    .size(typography.caption)
                    .font(typography.caption_font),
                search_input
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .width(Length::Fixed(240.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(RADIUS.md).color(colors.border).width(1.0),
            ..Default::default()
        });

        let has_selection = !self.state.navigation.selection.selected.is_empty();
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        let cut_button = if has_selection {
            toolbar_button(format!("{} Couper", icons::CUT)).on_press(UiMessage::ClipboardCut)
        } else {
            toolbar_button(format!("{} Couper", icons::CUT))
        };

        let copy_button = if has_selection {
            toolbar_button(format!("{} Copier", icons::COPY)).on_press(UiMessage::ClipboardCopy)
        } else {
            toolbar_button(format!("{} Copier", icons::COPY))
        };

        let paste_button = if has_clipboard {
            toolbar_button(format!("{} Coller", icons::PASTE)).on_press(UiMessage::ClipboardPaste)
        } else {
            toolbar_button(format!("{} Coller", icons::PASTE))
        };

        let dark_mode_label = if self.state.config.dark_mode {
            format!("{} Clair", icons::THEME)
        } else {
            format!("{} Sombre", icons::THEME)
        };

        let gitignore_label = if self.state.config.respect_gitignore {
            format!("{} .gitignore ✓", icons::FILE)
        } else {
            format!("{} .gitignore", icons::FILE)
        };
        let grep_label = format!("{} Chercher", icons::FILE);
        let compact_label = if self.state.config.compact_mode {
            "⊞ Normal"
        } else {
            "⊟ Compact"
        };
        // #32: Accessibility — every command-bar button carries a tooltip.
        macro_rules! tt {
            ($widget:expr, $label:expr) => {
                widgets::tip(ctx, $widget, $label, TipVariant::Command)
            };
        }
        let command_bar = row![
            tt!(
                toolbar_button(format!("{} Nouveau", icons::NEW)).on_press(UiMessage::NewFolder),
                "Nouveau dossier (Ctrl+Shift+N)"
            ),
            tt!(cut_button, "Couper (Ctrl+X)"),
            tt!(copy_button, "Copier (Ctrl+C)"),
            tt!(paste_button, "Coller (Ctrl+V)"),
            tt!(
                toolbar_button(dark_mode_label).on_press(UiMessage::ToggleDarkMode),
                "Basculer thème clair/sombre"
            ),
            tt!(
                toolbar_button(gitignore_label).on_press(UiMessage::ToggleGitignore),
                "Respecter .gitignore"
            ),
            tt!(
                toolbar_button(compact_label.to_string()).on_press(UiMessage::ToggleCompactMode),
                "Mode compact/normal"
            ),
            tt!(
                toolbar_button(grep_label).on_press(UiMessage::OpenGrep),
                "Chercher dans le contenu (Ctrl+Shift+F)"
            ),
            tt!(
                toolbar_button(format!("{} Actions", icons::ACTIONS))
                    .on_press(UiMessage::ToggleContextMenu(!self.menus.context_open)),
                "Menu contextuel (F10)"
            )
        ]
        .spacing(spacing.sm);

        let loading_badge: Element<'_, UiMessage> =
            if self.show_loading_indicator || self.operation_progress.is_some() {
                // #16: Enhanced progress bar with operation label
                let (progress_label, ratio) = if let Some(op) = &self.operation_progress {
                    let done = op.counter.load(std::sync::atomic::Ordering::Relaxed);
                    let r = if op.total > 0 {
                        done as f32 / op.total as f32
                    } else {
                        0.5
                    };
                    let kind_label = match op.kind {
                        crate::filesystem::FileOperationKind::Copy => "Copie",
                        crate::filesystem::FileOperationKind::Move => "Déplacement",
                        crate::filesystem::FileOperationKind::Delete => "Suppression",
                        crate::filesystem::FileOperationKind::Rename => "Renommage",
                    };
                    (format!("{} : {}/{}", kind_label, done, op.total), r)
                } else if self.entries.total > 0 {
                    let loaded = self.entries.items.iter().filter(|e| e.is_some()).count();
                    (
                        "Chargement…".to_string(),
                        loaded as f32 / self.entries.total as f32,
                    )
                } else {
                    ("Chargement…".to_string(), 0.5)
                };
                let content: Element<'_, UiMessage> = row![
                    text(progress_label)
                        .size(typography.caption)
                        .font(typography.caption_font),
                    progress_bar(0.0..=1.0, ratio)
                        .girth(Length::Fixed(4.0))
                        .length(Length::Fixed(80.0)),
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center)
                .into();

                container(content)
                    .padding([spacing.xs, spacing.sm])
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.hover)),
                        border: border::rounded(RADIUS_PILL).color(colors.border).width(1.0),
                        ..Default::default()
                    })
                    .into()
            } else {
                container(row![]).into()
            };

        (search_bar.into(), command_bar.into(), loading_badge)
    }
}
