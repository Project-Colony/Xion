//! The window header: navigation buttons, tabs, address bar, search and the
//! command bar.
//!
//! Moved out of `view()` unchanged, together with the two closures only it
//! used (`toolbar_button`, `tab_button`).

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{button, column, container, mouse_area, row};
use iced::{Alignment, Background, Element, Theme, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::theme::icons;
use crate::ui::{KeyboardCommand, UiMessage};

use super::XionApp;
use super::widgets::{self, TipVariant, ViewCtx};

use super::widgets::RADIUS;
use super::widgets::body_text;
use super::widgets::chrome_style;

impl XionApp {
    /// Returns the header row and, separately, the address-history dropdown:
    /// the dropdown is built here but drawn as an overlay layer on top of
    /// everything else, so `view()` places it itself.
    pub(super) fn render_header(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Option<Element<'_, UiMessage>>) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let toolbar_button = |label: String| widgets::toolbar_button(ctx, label);

        let tab_button = |label: String, active: bool| {
            button(body_text(typography, label))
                .padding([spacing.xs, spacing.md])
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };

                    if active {
                        style.background = Some(Background::Color(colors.panel_background));
                        style.border = border::rounded(RADIUS.lg).color(colors.border).width(1.0);
                    }

                    if !active && matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(colors.hover));
                    }

                    style
                })
        };

        let back_button = if self.history.can_back() {
            toolbar_button(icons::BACK.to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button(icons::BACK.to_string())
        };
        let back_button = widgets::tip(ctx, back_button, "Précédent (Alt+←)", TipVariant::Nav);

        let forward_button = if self.history.can_forward() {
            toolbar_button(icons::FORWARD.to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button(icons::FORWARD.to_string())
        };
        let forward_button = widgets::tip(ctx, forward_button, "Suivant (Alt+→)", TipVariant::Nav);

        let refresh_button =
            toolbar_button(icons::REFRESH.to_string()).on_press(UiMessage::Refresh);
        let refresh_button = widgets::tip(ctx, refresh_button, "Actualiser (F5)", TipVariant::Nav);

        // Go Up button
        let has_parent = self
            .state
            .route
            .local_path()
            .and_then(|p| p.parent())
            .is_some();
        let go_up_btn = toolbar_button(format!("{} \u{2191}", icons::FOLDER));
        let go_up_btn = if has_parent {
            go_up_btn.on_press(UiMessage::KeyboardCommand(KeyboardCommand::GoToParent))
        } else {
            go_up_btn
        };
        let go_up_btn = widgets::tip(ctx, go_up_btn, "Dossier parent (Alt+↑)", TipVariant::Nav);

        let navigation =
            row![back_button, forward_button, refresh_button, go_up_btn].spacing(spacing.sm);

        let mut tabs = row![];
        for (index, tab) in self.tab_manager.tabs.iter().enumerate() {
            let label = format!("{} {}", icons::PC, tab.title);
            let mut button = tab_button(label, index == self.tab_manager.active);
            if index != self.tab_manager.active {
                button = button.on_press(UiMessage::SwitchTab(index));
            }
            let mut tab_row = row![button].spacing(spacing.xs).align_y(Alignment::Center);
            if index != 0 {
                tab_row = tab_row.push(
                    tab_button(icons::CLOSE.to_string(), false)
                        .on_press(UiMessage::CloseTab(index)),
                );
            }
            // #18: Tab drag reorder — wrap in mouse_area
            let tab_element: Element<'_, UiMessage> = mouse_area(tab_row)
                .on_press(UiMessage::TabDragStart(index))
                .on_release(UiMessage::TabDragDrop)
                .into();
            tabs = tabs.push(tab_element);
        }
        tabs = tabs.push(tab_button(icons::NEW.to_string(), false).on_press(UiMessage::AddTab));
        let tabs = tabs.spacing(spacing.sm);

        let (address_section, history_menu) = self.render_address_section(ctx);
        let (search_bar, command_bar, loading_badge) = self.render_command_bar(ctx);

        let header = container(
            column![
                row![tabs].spacing(8).align_y(Alignment::Center),
                row![navigation, address_section, search_bar, loading_badge]
                    .spacing(spacing.md)
                    .align_y(Alignment::Center),
                command_bar
            ]
            .spacing(spacing.sm),
        )
        .padding(iced::Padding {
            top: spacing.sm + 2.0,
            right: spacing.md,
            bottom: spacing.sm,
            left: spacing.md,
        })
        .style(chrome_style(colors, RADIUS.xl));

        (header.into(), history_menu)
    }
}
