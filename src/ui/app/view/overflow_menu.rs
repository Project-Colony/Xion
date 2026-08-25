//! The `⋯` menu at the end of the header bar.
//!
//! It holds what the command bar used to show as a permanent third row. That
//! row gave the same visual weight to six different kinds of thing — verbs
//! acting on the selection, view toggles, a theme switch, a panel opener and a
//! menu — so nothing in it was findable except by reading all of it.
//!
//! Here they are grouped and separated: what acts on the selection, what
//! changes what you see, what opens something else. Les préférences n'y
//! figurent pas : la convention Colony veut qu'on y entre par le nom du
//! programme, « not an entry buried in a list of sections ». A disabled entry stays
//! visible rather than disappearing, so its shortcut is still discoverable when
//! nothing is selected.

use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{column, container, mouse_area, opaque, row, stack};
use iced::{Element, Length};

use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::menu_items;
use super::widgets::HEADER_DROPDOWN_TOP;
use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use super::widgets::surface_style;

const MENU_RIGHT_MARGIN: f32 = 12.0;
const MENU_WIDTH: f32 = 260.0;

impl XionApp {
    /// The `⋯` menu, or `None` when it is closed.
    pub(super) fn render_overflow_menu(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        if !self.menus.overflow_open {
            return None;
        }

        let ViewCtx {
            colors, spacing, ..
        } = ctx;

        let has_selection = !self.state.navigation.selection.selected.is_empty();
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        // Closing on activation: every one of these either acts once or opens
        // something, so leaving the menu up would cover the result.
        let entry = |icon: &str, label: &str, shortcut: &str, message: UiMessage| {
            menu_items::item(ctx, icon, label, shortcut, message)
        };
        let disabled = |icon: &str, label: &str, shortcut: &str| {
            menu_items::disabled_item(ctx, icon, label, shortcut)
        };

        let mut items = column![].spacing(spacing.xs).width(Length::Fill);

        items = items.push(entry(
            icons::NEW,
            "Nouveau dossier",
            "Ctrl+Maj+N",
            UiMessage::NewFolder,
        ));
        items = items.push(menu_items::separator(ctx));

        for (icon, label, shortcut, message, enabled) in [
            (
                icons::CUT,
                "Couper",
                "Ctrl+X",
                UiMessage::ClipboardCut,
                has_selection,
            ),
            (
                icons::COPY,
                "Copier",
                "Ctrl+C",
                UiMessage::ClipboardCopy,
                has_selection,
            ),
            (
                icons::PASTE,
                "Coller",
                "Ctrl+V",
                UiMessage::ClipboardPaste,
                has_clipboard,
            ),
        ] {
            items = items.push(if enabled {
                entry(icon, label, shortcut, message)
            } else {
                disabled(icon, label, shortcut)
            });
        }

        items = items.push(menu_items::separator(ctx));

        let theme_label = if self.state.config.dark_mode {
            "Thème clair"
        } else {
            "Thème sombre"
        };
        let compact_label = if self.state.config.compact_mode {
            "Densité normale"
        } else {
            "Densité compacte"
        };
        let gitignore_label = if self.state.config.respect_gitignore {
            "Ignorer .gitignore"
        } else {
            "Respecter .gitignore"
        };

        items = items.push(entry(
            icons::THEME,
            theme_label,
            "",
            UiMessage::ToggleDarkMode,
        ));
        items = items.push(entry(
            "\u{22a1}",
            compact_label,
            "",
            UiMessage::ToggleCompactMode,
        ));
        items = items.push(entry(
            icons::FILE_GIT,
            gitignore_label,
            "",
            UiMessage::ToggleGitignore,
        ));

        items = items.push(menu_items::separator(ctx));
        items = items.push(entry(
            icons::SEARCH,
            "Chercher dans le contenu",
            "Ctrl+Maj+F",
            UiMessage::OpenGrep,
        ));
        items = items.push(entry(
            icons::REFRESH,
            "Actualiser",
            "F5",
            UiMessage::Refresh,
        ));

        let menu = container(items)
            .padding(spacing.xs)
            .width(Length::Fixed(MENU_WIDTH))
            .style(surface_style(colors, RADIUS.lg));

        // A click anywhere else closes it. Without this layer the menu would
        // stay up until its own button was pressed again.
        let dismiss: Element<'_, UiMessage> =
            mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                .on_press(UiMessage::ToggleOverflowMenu(false))
                .into();

        let anchored: Element<'_, UiMessage> = container(
            column![
                vertical_space().height(Length::Fixed(HEADER_DROPDOWN_TOP)),
                row![
                    horizontal_space(),
                    opaque(menu),
                    horizontal_space().width(Length::Fixed(MENU_RIGHT_MARGIN))
                ]
            ]
            .spacing(0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        Some(stack![dismiss, anchored].into())
    }
}
