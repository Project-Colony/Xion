//! The window header: one bar, and a tab strip when there is more than one tab.
//!
//! It used to be three stacked bands, tabs, then navigation with the address
//! and the search, then a command bar of seven buttons. On a 720-pixel window
//! that was 92 pixels of chrome before the first file, and the command bar gave
//! the same visual weight to six different kinds of thing: verbs acting on the
//! selection, view toggles, a theme switch, a panel opener and a menu.
//!
//! Now: navigation, address, search and one `⋯` button. Everything the command
//! bar held moved into that menu, grouped and separated. 56 pixels.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, mouse_area, row};
use iced::{Alignment, Background, Element, Length, Theme};

use crate::i18n::t;
use crate::ui::theme::icons;
use crate::ui::theme::layout::TAB_STRIP_HEIGHT;
use crate::ui::{KeyboardCommand, UiMessage};

use super::XionApp;
use super::widgets::{self, TipVariant, ViewCtx};

use super::widgets::RADIUS;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::glyph_text;
use super::widgets::header_style;

impl XionApp {
    /// Returns the header and, separately, the address-history dropdown: the
    /// dropdown is built here but drawn as an overlay layer on top of
    /// everything else, so `view()` places it itself.
    pub(super) fn render_header(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Option<Element<'_, UiMessage>>) {
        let ViewCtx {
            colors, spacing, ..
        } = ctx;

        let toolbar_button = |label: String| widgets::toolbar_button(ctx, label);

        let back_button = if self.history.can_back() {
            toolbar_button(icons::BACK.to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button(icons::BACK.to_string())
        };
        let back_button = widgets::tip(ctx, back_button, t("header_back"), TipVariant::Nav);

        let forward_button = if self.history.can_forward() {
            toolbar_button(icons::FORWARD.to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button(icons::FORWARD.to_string())
        };
        let forward_button =
            widgets::tip(ctx, forward_button, t("header_forward"), TipVariant::Nav);

        let has_parent = self
            .state
            .route
            .local_path()
            .and_then(|path| path.parent())
            .is_some();
        let go_up_button = toolbar_button("\u{2191}".to_string());
        let go_up_button = if has_parent {
            go_up_button.on_press(UiMessage::KeyboardCommand(KeyboardCommand::GoToParent))
        } else {
            go_up_button
        };
        let go_up_button = widgets::tip(ctx, go_up_button, t("header_parent"), TipVariant::Nav);

        // Refresh left the bar: it is one press of F5, it is in the `⋯` menu,
        // and it was the least-used of the four arrows.
        let navigation = row![back_button, forward_button, go_up_button].spacing(spacing.xs);

        let (address_section, history_menu) = self.render_address_section(ctx);
        let (search_bar, loading_badge) = self.render_command_bar(ctx);

        let overflow_button = widgets::tip(
            ctx,
            toolbar_button("\u{22ef}".to_string())
                .on_press(UiMessage::ToggleOverflowMenu(!self.menus.overflow_open)),
            t("header_more_actions"),
            TipVariant::Nav,
        );

        let bar = row![
            navigation,
            address_section,
            search_bar,
            loading_badge,
            overflow_button
        ]
        .spacing(spacing.md)
        .align_y(Alignment::Center);

        let mut header_column = column![bar].spacing(spacing.xs);

        // Always visible. It used to be hidden while there was only one tab, to
        // save thirty pixels, but it carries the only "+" of the interface, so
        // hiding the bar removed the only visible way to open a second tab. A
        // keyboard shortcut exists; it is not discoverable.
        header_column = header_column.push(self.render_tab_strip(ctx));

        let header = container(header_column)
            .padding(iced::Padding {
                top: spacing.sm,
                right: spacing.md,
                bottom: 0.0,
                left: spacing.md,
            })
            .style(header_style(colors));

        (header.into(), history_menu)
    }

    /// The "Xion" button, which opens and closes the preferences.
    ///
    /// The Colony convention wants the program's identity to be the path to its
    /// settings: "not a gear floating in a toolbar, not an entry buried in a
    /// list of sections". Xion has Digger's chrome shape, a horizontal bar, and
    /// so follows its implementation: a text button, not a card background.
    ///
    /// It lives in the tab strip, and the whole point is that it must not read
    /// as one more tab. What makes a button *become* a tab is touching the
    /// bottom of the strip, as written at [`TAB_STRIP_HEIGHT`]. This one is
    /// therefore centred vertically, never touching that bottom, and keeps full
    /// rounding where tabs are square at the bottom. Both differences show at a
    /// glance.
    fn render_identity_button(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let open = self.menus.preferences.open;

        let label = row![
            caption_text(typography, icons::SETTINGS),
            body_text(typography, "Xion")
        ]
        .spacing(spacing.xs)
        .align_y(Alignment::Center);

        let identity = button(label)
            .padding([spacing.xs, spacing.sm])
            .style(move |_theme: &Theme, status: ButtonStatus| {
                // Open: the accent and a background. Closed: discreet, because
                // the program name should not compete with the tabs.
                let background = if open {
                    Some(Background::Color(colors.selection))
                } else if matches!(status, ButtonStatus::Hovered) {
                    Some(Background::Color(colors.hover))
                } else {
                    None
                };

                iced::widget::button::Style {
                    background,
                    text_color: if open {
                        colors.accent
                    } else {
                        colors.text_muted
                    },
                    border: iced::Border {
                        radius: RADIUS.md.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(UiMessage::TogglePreferences);

        let tip = if open {
            t("header_close_preferences")
        } else {
            t("settings_title")
        };

        container(widgets::tip(ctx, identity, tip, TipVariant::Nav))
            .height(Length::Fixed(TAB_STRIP_HEIGHT))
            .align_y(Alignment::Center)
            .padding(iced::Padding {
                right: spacing.md,
                ..iced::Padding::ZERO
            })
            .into()
    }

    /// The tab strip.
    ///
    /// The close cross used to be built with the very same widget as the tab,
    /// same padding, same shape, same size, so a tab and its cross read as two
    /// tabs, and so did the `+`. Here the cross lives inside the tab, the `+` is
    /// a lighter glyph, and the active tab reaches the bottom of the strip so
    /// that it joins the list underneath.
    fn render_tab_strip(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        let mut strip = row![self.render_identity_button(ctx)]
            .spacing(0)
            .align_y(Alignment::End);

        for (index, tab) in self.tab_manager.tabs.iter().enumerate() {
            let active = index == self.tab_manager.active;

            // The folder icon, not `PC`: tabs carry the name of the directory
            // they show, so a computer glyph on every one of them said nothing.
            let mut inner = row![
                caption_text(typography, icons::FOLDER).color(if active {
                    colors.accent
                } else {
                    colors.text_muted
                }),
                body_text(typography, tab.title.clone())
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center);

            // Tab 0 is the home tab and is never closable, which is why it gets
            // no cross rather than a disabled one.
            if index != 0 {
                inner = inner.push(
                    button(glyph_text(typography, icons::CLOSE).color(colors.text_muted))
                        .padding(2.0)
                        // Theme tokens, not hard-coded white: the cross would
                        // have vanished on a light background.
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            iced::widget::button::Style {
                                background: None,
                                text_color: match status {
                                    ButtonStatus::Hovered => colors.text_primary,
                                    _ => colors.text_muted,
                                },
                                ..Default::default()
                            }
                        })
                        .on_press(UiMessage::CloseTab(index)),
                );
            }

            let tab_body = button(inner)
                .padding([spacing.xs, spacing.md])
                .height(Length::Fixed(TAB_STRIP_HEIGHT))
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: if active {
                            colors.text_primary
                        } else {
                            colors.text_muted
                        },
                        ..Default::default()
                    };

                    if active {
                        // Rounded at the top only, square at the bottom: the tab
                        // has to meet the list without a seam.
                        style.background = Some(Background::Color(colors.panel_background));
                        style.border = iced::Border {
                            radius: iced::border::Radius::default()
                                .top_left(RADIUS.md)
                                .top_right(RADIUS.md),
                            ..Default::default()
                        };
                    } else if matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(colors.hover));
                        style.border = iced::Border {
                            radius: iced::border::Radius::default()
                                .top_left(RADIUS.md)
                                .top_right(RADIUS.md),
                            ..Default::default()
                        };
                    }

                    style
                });

            let tab_body = if active {
                tab_body
            } else {
                tab_body.on_press(UiMessage::SwitchTab(index))
            };

            // #18: dragging a tab reorders it.
            let tab_element: Element<'_, UiMessage> = mouse_area(tab_body)
                .on_press(UiMessage::TabDragStart(index))
                .on_release(UiMessage::TabDragDrop)
                .into();
            strip = strip.push(tab_element);
        }

        strip = strip.push(
            button(body_text(typography, icons::NEW).color(colors.text_muted))
                .padding([spacing.xs, spacing.sm])
                .style(widgets::hover_button_style(
                    colors,
                    colors.text_muted,
                    Some(RADIUS.md),
                ))
                .on_press(UiMessage::AddTab),
        );

        container(row![strip, horizontal_space()].align_y(Alignment::End))
            .height(Length::Fixed(TAB_STRIP_HEIGHT))
            .width(Length::Fill)
            .into()
    }
}
