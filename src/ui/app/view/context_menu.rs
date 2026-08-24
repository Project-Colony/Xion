//! Context menu rendering: the entry menu, the background menu and their
//! submenus.
//!
//! Moved out of `view()` unchanged. Nothing here was used anywhere else in that
//! function, which is what made it the cleanest cut to take first.

use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{column, container, mouse_area, opaque, row, stack};
use iced::{Background, Element, Length, Point, Theme, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::ui::UiMessage;

use super::XionApp;

use super::widgets::ViewCtx;

use super::widgets::RADIUS;

impl XionApp {
    /// The context-menu layer, or `None` when no menu is open.
    pub(super) fn render_context_menu(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        let ViewCtx {
            colors,
            spacing,
            typography: _,
        } = ctx;

        let submenu_panel = self.context_submenu_panel(ctx);
        // ── Context menu rendering ───────────────────────────────────────────
        let show_entry_menu =
            self.menus.context_open && !self.state.navigation.selection.selected.is_empty();
        let show_bg_menu = self.menus.background_context_open;

        let context_menu: Option<Element<'_, UiMessage>> = if show_entry_menu || show_bg_menu {
            let main_menu = if show_entry_menu {
                self.context_entry_menu(ctx)
            } else {
                self.context_background_menu(ctx)
            };
            let position = self.menus.context_position.unwrap_or(Point::ORIGIN);
            let position_x = position.x.max(0.0);
            let position_y = position.y.max(0.0);

            let menu_style = move |_: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(RADIUS.lg).color(colors.border).width(1.0),
                ..Default::default()
            };
            let menu = container(main_menu)
                .padding([spacing.sm, spacing.md])
                .width(Length::Fixed(260.0))
                .style(menu_style);

            // Combine main menu + optional submenu in a row
            let menu_row: Element<'_, UiMessage> = if let Some(sub) = submenu_panel {
                let combined = row![opaque(menu), sub].spacing(2);
                mouse_area(combined)
                    .on_exit(UiMessage::CloseContextSubmenu)
                    .into()
            } else {
                opaque(menu)
            };

            let menu_layer: Element<'_, UiMessage> = container(
                column![
                    vertical_space().height(Length::Fixed(position_y)),
                    row![
                        horizontal_space().width(Length::Fixed(position_x)),
                        menu_row
                    ]
                ]
                .spacing(0),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
            let dismiss_layer: Element<'_, UiMessage> =
                mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                    .on_press(UiMessage::ToggleContextMenu(false))
                    .into();
            Some(stack![dismiss_layer, menu_layer].into())
        } else {
            None
        };

        context_menu
    }
}
