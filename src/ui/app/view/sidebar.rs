//! The left sidebar: tree, quick access, favourites and drives.
//!
//! Moved out of `view()` unchanged, together with the three closures that only
//! it ever used (`sidebar_button`, `section_header`, `format_sidebar_label`).

use iced::widget::{column, container, scrollable};
use iced::{Background, Border, Element, Length};

use crate::ui::UiMessage;

use super::XionApp;

use super::widgets::ViewCtx;

impl XionApp {
    /// The sidebar column. Its width is set here rather than by the caller,
    /// so the module owns its own layout.
    pub(super) fn render_sidebar(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography: _,
        } = ctx;

        let (tree_panel, tree_resize_bar) = self.sidebar_tree(ctx);
        let (quick_access, favorites_section) = self.sidebar_quick_access(ctx);
        let drive_section = self.sidebar_drives(ctx);

        let sidebar = container(
            scrollable(
                column![
                    tree_panel,
                    tree_resize_bar,
                    quick_access,
                    favorites_section,
                    drive_section,
                ]
                .spacing(spacing.sm)
                .padding([spacing.xs, spacing.xs]),
            )
            .height(Length::Fill),
        )
        .padding(spacing.md)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.sidebar_background)),
            border: Border {
                color: colors.border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        sidebar.width(Length::FillPortion(1)).into()
    }
}
