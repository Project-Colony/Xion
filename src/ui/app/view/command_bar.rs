//! The search field and the loading badge.
//!
//! The command bar that used to live here — seven buttons on a permanent third
//! row — is gone: its contents moved into the `⋯` menu, in
//! `render_overflow_menu`. Only what belongs on the bar itself remains.

use iced::widget::{container, progress_bar, row, text_input};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};

use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;
use super::widgets::caption_text;
use super::widgets::filled_style;
use super::widgets::surface_style;
use super::widgets::{RADIUS, RADIUS_PILL};

impl XionApp {
    /// The search field and the loading badge, in that order — the caller lays
    /// them out.
    pub(super) fn render_command_bar(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Element<'_, UiMessage>) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

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
            row![caption_text(typography, icons::SEARCH), search_input]
                .spacing(spacing.xs)
                .align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .width(Length::Fixed(240.0))
        .style(surface_style(colors, RADIUS.md));

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
                    caption_text(typography, progress_label),
                    progress_bar(0.0..=1.0, ratio)
                        .girth(Length::Fixed(4.0))
                        .length(Length::Fixed(80.0)),
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center)
                .into();

                container(content)
                    .padding([spacing.xs, spacing.sm])
                    .style(filled_style(colors, colors.hover, RADIUS_PILL))
                    .into()
            } else {
                container(row![]).into()
            };

        (search_bar.into(), loading_badge)
    }
}
