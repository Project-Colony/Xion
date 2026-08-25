//! The integrated terminal panel, which slides up below the file list.
//!
//! Moved out of `view()` unchanged, except that the `if` became an early
//! return so the caller decides where to place the panel.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, column, container, row, scrollable, text_input, tooltip};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};

use crate::ui::UiMessage;
use crate::ui::theme::layout::TERMINAL_DEFAULT_HEIGHT;

use super::XionApp;
use super::widgets::ViewCtx;

use super::widgets::RADIUS;
use super::widgets::caption_text;
use super::widgets::filled_style;
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

        let animated_height = TERMINAL_DEFAULT_HEIGHT * term_progress;

        let active_tab = self.terminal.active_ref();
        let output_text: Element<'_, UiMessage> = if active_tab.lines.is_empty() {
            caption_text(typography, "Entrez une commande…")
                .style(move |_: &Theme| iced::widget::text::Style {
                    color: Some(colors.text_muted),
                })
                .into()
        } else {
            let combined = active_tab.cached_output.as_deref().unwrap_or("");
            // Borrowed: the whole scrollback was copied on every rebuild.
            caption_text(typography, combined)
                .style(move |_: &Theme| iced::widget::text::Style {
                    color: Some(colors.text_primary),
                })
                .into()
        };

        let output_area: Element<'_, UiMessage> = container(
            scrollable(
                container(output_text)
                    .width(Length::Fill)
                    .padding([spacing.xs, spacing.sm]),
            )
            .id(iced::widget::Id::new("terminal_output"))
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        let fallback_cwd = self
            .state
            .route
            .local_path()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("C:\\"))
            });
        let cwd_display = self
            .terminal
            .effective_cwd(&fallback_cwd)
            .display()
            .to_string();

        let prompt_label =
            glyph_text(typography, format!("{cwd_display} >")).style(move |_: &Theme| {
                iced::widget::text::Style {
                    color: Some(colors.accent),
                }
            });

        let input_field = text_input("commande…", &self.terminal.active_ref().input)
            .id(iced::widget::Id::new("terminal_input"))
            .size(typography.caption)
            .font(typography.caption_font)
            .on_input(UiMessage::TerminalInputChanged)
            .on_submit(UiMessage::TerminalInputSubmitted)
            .style(
                move |_theme: &Theme, _status| iced::widget::text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: border::rounded(0.0).color(Color::TRANSPARENT).width(0.0),
                    icon: colors.text_muted,
                    placeholder: colors.text_muted,
                    value: colors.text_primary,
                    selection: colors.selection,
                },
            );

        let input_row: Element<'_, UiMessage> = container(
            row![prompt_label, input_field]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.sm])
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.sidebar_background)),
            border: iced::Border {
                color: colors.border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into();

        // Feature I: Terminal tab bar
        let mut tab_bar = row![];
        for (i, tab) in self.terminal.tabs.iter().enumerate() {
            let is_active = i == self.terminal.active_tab;
            let tab_label = tab.title.clone();
            let tab_btn = button(caption_text(typography, tab_label.clone()))
                .padding([spacing.xs, spacing.sm])
                .style(
                    move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                        text_color: if is_active {
                            colors.accent
                        } else {
                            colors.text_muted
                        },
                        background: if is_active {
                            Some(Background::Color(colors.hover))
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                );
            let tab_btn = if is_active {
                tab_btn
            } else {
                tab_btn.on_press(UiMessage::TerminalSwitchTab(i))
            };
            tab_bar = tab_bar.push(tab_btn);
            if self.terminal.tabs.len() > 1 {
                let muted = colors.text_muted;
                tab_bar = tab_bar.push(
                    button(glyph_text(typography, "✕"))
                        .padding([spacing.xs, spacing.xs])
                        .on_press(UiMessage::TerminalCloseTab(i))
                        .style(
                            move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                                text_color: muted,
                                ..Default::default()
                            },
                        ),
                );
            }
        }
        let muted_color = colors.text_muted;
        // Feature J: Shell switcher buttons
        //
        // Découverts, plus écrits en dur : la liste figée affichait « CMD » sous
        // Linux pour un bouton qui lançait zsh, et un « PS » qui ne pouvait rien
        // lancer du tout.
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
        tab_bar = tab_bar.push(
            button(glyph_text(typography, "+"))
                .padding([spacing.xs, spacing.sm])
                .on_press(UiMessage::TerminalAddTab)
                .style(
                    move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                        text_color: muted_color,
                        ..Default::default()
                    },
                ),
        );
        // Ctrl-C. A pipe-backed shell had no way to interrupt anything; a
        // pty does, and a runaway command needs a visible way out.
        tab_bar = tab_bar.push(horizontal_space());
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
        let tab_bar_element: Element<'_, UiMessage> = container(tab_bar.spacing(spacing.xs))
            .width(Length::Fill)
            .padding([spacing.xs, spacing.sm])
            .style(filled_style(colors, colors.sidebar_background, 0.0))
            .into();

        let term_panel: Element<'_, UiMessage> =
            container(column![tab_bar_element, output_area, input_row].spacing(0))
                .width(Length::Fill)
                .height(Length::Fixed(animated_height))
                .style(surface_style(colors, RADIUS.lg))
                .into();

        Some(term_panel)
    }
}
