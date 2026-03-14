//! View rendering for XionApp.
//!
//! Contains the [`XionApp::view`] method which builds the entire widget tree.
//! Modal overlay rendering is in [`overlays`].

mod overlays;

use std::path::PathBuf;

use iced::widget::button::Status as ButtonStatus;
#[allow(unused_imports)]
use tracing::{debug, info, warn};
use iced::widget::{
    button, column, container, image, mouse_area, opaque, progress_bar, row,
    scrollable, stack, text, text_input, tooltip,
};
use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Theme,
    border, mouse,
};

use crate::core::{SortOrderConfig, ViewColumn, ViewMode};
use crate::filesystem::{FsEntry, FsEntryType};
use crate::ui::{
    ContextAction, KeyboardCommand, NETWORK_ROUTE, ScrollViewport, UiMessage,
};
use crate::ui::theme::{UiTokens, icons};
use crate::ui::theme::layout::{
    PREVIEW_RESIZE_BAR_WIDTH,
    TERMINAL_DEFAULT_HEIGHT,
    TREE_RESIZE_BAR_HEIGHT,
};

use super::XionApp;
use super::types::*;
use super::helpers::{
    column_specs, entry_type_label,
    format_entry_size, format_modified, truncate_name,
};

impl XionApp {
    pub(super) fn view(&self) -> Element<'_, UiMessage> {
        let tokens = UiTokens::for_theme(&self.state.config.theme);
        let colors = tokens.colors;
        let spacing = tokens.spacing;
        let typography = tokens.typography;
        let home_dir = self.cached_home_dir.clone();

        let toolbar_button = |label: String| {
            button(text(label).size(typography.body).font(typography.body_font))
                .padding([spacing.xs, spacing.sm])
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };

                    match status {
                        ButtonStatus::Hovered => {
                            style.background = Some(Background::Color(colors.hover));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        ButtonStatus::Pressed => {
                            style.background = Some(Background::Color(colors.pressed));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        ButtonStatus::Disabled => {
                            style.text_color = colors.text_muted;
                        }
                        ButtonStatus::Active => {}
                    }

                    style
                })
        };

        let sidebar_button =
            |icon: &str, label: &str, target: Option<PathBuf>| -> Element<'_, UiMessage> {
                let icon = icon.to_string();
                let label = label.to_string();
                let content: Element<'_, UiMessage> = row![
                    text(icon).size(typography.body).font(typography.body_font),
                    text(label).size(typography.body).font(typography.body_font)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center)
                .into();

                match target {
                    Some(path) => button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            match status {
                                ButtonStatus::Hovered => {
                                    style.background = Some(Background::Color(colors.hover));
                                    style.border =
                                        border::rounded(6.0).color(colors.border).width(1.0);
                                }
                                ButtonStatus::Pressed => {
                                    style.background = Some(Background::Color(colors.pressed));
                                }
                                ButtonStatus::Active | ButtonStatus::Disabled => {}
                            }

                            style
                        })
                        .on_press(UiMessage::NavigateTo(path))
                        .into(),
                    None => container(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .into(),
                }
            };

        // Clickable accordion header — shows ▸/▾ chevron and toggles the section.
        let section_header = |label: &'static str, collapsed: bool| -> Element<'_, UiMessage> {
            let chevron = if collapsed { "▸" } else { "▾" };
            button(
                row![
                    text(chevron).size(typography.caption).font(typography.caption_font),
                    text(label).size(typography.caption).font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style { color: Some(colors.text_muted) }),
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center),
            )
            .padding([2.0, 0.0])
            .width(Length::Fill)
            .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                ..Default::default()
            })
            .on_press(UiMessage::ToggleSidebarSection(label.to_string()))
            .into()
        };

        let format_sidebar_label = |path: &PathBuf| {
            path.file_name()
                .and_then(|name| name.to_str())
                .filter(|label| !label.is_empty())
                .map(|label| label.to_string())
                .unwrap_or_else(|| path.display().to_string())
        };

        let tab_button = |label: String, active: bool| {
            button(text(label).size(typography.body).font(typography.body_font))
                .padding([spacing.xs, spacing.md])
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };

                    if active {
                        style.background = Some(Background::Color(colors.panel_background));
                        style.border = border::rounded(8.0).color(colors.border).width(1.0);
                    }

                    if !active && matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(colors.hover));
                    }

                    style
                })
        };

        let tip_style = move |_: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(colors.chrome_background)),
            border: border::rounded(4.0).color(colors.border).width(1.0),
            ..Default::default()
        };

        let back_button = if self.history.can_back() {
            toolbar_button(icons::BACK.to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button(icons::BACK.to_string())
        };
        let back_button = tooltip(back_button, "Précédent (Alt+←)", tooltip::Position::Bottom)
            .style(tip_style).padding(4);

        let forward_button = if self.history.can_forward() {
            toolbar_button(icons::FORWARD.to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button(icons::FORWARD.to_string())
        };
        let forward_button = tooltip(forward_button, "Suivant (Alt+→)", tooltip::Position::Bottom)
            .style(tip_style).padding(4);

        let refresh_button = toolbar_button(icons::REFRESH.to_string()).on_press(UiMessage::Refresh);
        let refresh_button = tooltip(refresh_button, "Actualiser (F5)", tooltip::Position::Bottom)
            .style(tip_style).padding(4);

        // Go Up button
        let has_parent = self.state.route.local_path()
            .and_then(|p| p.parent())
            .is_some();
        let go_up_btn = toolbar_button(format!("{} \u{2191}", icons::FOLDER));
        let go_up_btn = if has_parent {
            go_up_btn.on_press(UiMessage::KeyboardCommand(KeyboardCommand::GoToParent))
        } else {
            go_up_btn
        };
        let go_up_btn = tooltip(go_up_btn, "Dossier parent (Alt+↑)", tooltip::Position::Bottom)
            .style(tip_style).padding(4);

        let navigation = row![back_button, forward_button, refresh_button, go_up_btn].spacing(spacing.sm);

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
                    tab_button(icons::CLOSE.to_string(), false).on_press(UiMessage::CloseTab(index)),
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

        let address_bar: Element<'_, UiMessage> = if self.address_editing {
            // Editable text input mode
            let address_input = text_input("Chemin…", &self.address_input)
                .on_input(UiMessage::AddressInputChanged)
                .on_submit(UiMessage::AddressInputSubmitted)
                .size(typography.body)
                .font(typography.body_font)
                .padding([spacing.xs, spacing.md])
                .id(iced::widget::Id::new("address_input"));

            container(address_input)
                .width(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(6.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            // Breadcrumb mode — clickable path segments
            let current_path = self.state.route.address_label();
            let mut breadcrumb_row = row![].spacing(0).align_y(Alignment::Center);

            let segments: Vec<&str> = current_path.split('\\').filter(|s| !s.is_empty()).collect();
            let mut accumulated = String::new();

            for (i, segment) in segments.iter().enumerate() {
                let parent_path = accumulated.clone();
                if i == 0 && segment.ends_with(':') {
                    // Drive root, e.g. "C:"
                    accumulated = format!("{}\\", segment);
                } else {
                    accumulated = format!("{}{}\\", accumulated, segment);
                }

                if i > 0 {
                    // The separator lists siblings = subdirectories of parent_path
                    let dropdown_target = PathBuf::from(&parent_path);
                    breadcrumb_row = breadcrumb_row.push(
                        button(
                            text("❯")
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(colors.text_muted),
                                }),
                        )
                        .padding([spacing.xs, 2.0])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_muted,
                                ..Default::default()
                            };
                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                                style.border = border::rounded(4.0).color(colors.border).width(1.0);
                            }
                            style
                        })
                        .on_press(UiMessage::BreadcrumbDropdown(dropdown_target)),
                    );
                }

                let target = PathBuf::from(&accumulated);
                let is_last = i == segments.len() - 1;
                let label = segment.to_string();

                if is_last {
                    // Also add a trailing separator for the last segment to list its children
                    breadcrumb_row = breadcrumb_row.push(
                        text(label)
                            .size(typography.body)
                            .font(typography.body_font),
                    );
                    let dropdown_target = PathBuf::from(&accumulated);
                    breadcrumb_row = breadcrumb_row.push(
                        button(
                            text("❯")
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .style(move |_| iced::widget::text::Style {
                                    color: Some(colors.text_muted),
                                }),
                        )
                        .padding([spacing.xs, 2.0])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_muted,
                                ..Default::default()
                            };
                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                                style.border = border::rounded(4.0).color(colors.border).width(1.0);
                            }
                            style
                        })
                        .on_press(UiMessage::BreadcrumbDropdown(dropdown_target)),
                    );
                } else {
                    breadcrumb_row = breadcrumb_row.push(
                        button(
                            text(label)
                                .size(typography.body)
                                .font(typography.body_font),
                        )
                        .padding([spacing.xs, spacing.xs])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.accent,
                                ..Default::default()
                            };
                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                                style.border = border::rounded(4.0).color(colors.border).width(1.0);
                            }
                            style
                        })
                        .on_press(UiMessage::NavigateTo(target)),
                    );
                }
            }

            let breadcrumb_button = mouse_area(
                container(breadcrumb_row)
                    .width(Length::Fill)
                    .padding([spacing.xs, spacing.md])
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(6.0).color(colors.border).width(1.0),
                        ..Default::default()
                    }),
            )
            .on_press(UiMessage::AddressEditStart);

            breadcrumb_button.into()
        };

        let address_validation = {
            use super::types::AddressValidation;
            if self.address_editing {
                self.address_validation_cache
                    .get(&self.address_input, || self.address_target_from_input())
                    .map(|v| match v {
                        AddressValidation::Directory => ("Dossier".to_string(), colors.address_directory),
                        AddressValidation::File => ("Fichier".to_string(), colors.address_file),
                        AddressValidation::NotFound => ("Introuvable".to_string(), colors.address_not_found),
                    })
            } else {
                None
            }
        };

        let address_status: Element<'_, UiMessage> =
            if let Some((label, status_color)) = address_validation {
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(status_color),
                        }),
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(999.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
            } else {
                container(row![]).into()
            };

        let suggestion_button = |label: String, target: PathBuf| {
            button(
                text(label)
                    .size(typography.caption)
                    .font(typography.body_font),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(move |_theme: &Theme, status: ButtonStatus| {
                let mut style = iced::widget::button::Style {
                    text_color: colors.text_primary,
                    ..Default::default()
                };

                match status {
                    ButtonStatus::Hovered => {
                        style.background = Some(Background::Color(colors.hover));
                        style.border = border::rounded(6.0).color(colors.border).width(1.0);
                    }
                    ButtonStatus::Pressed => {
                        style.background = Some(Background::Color(colors.pressed));
                        style.border = border::rounded(6.0).color(colors.border).width(1.0);
                    }
                    ButtonStatus::Active | ButtonStatus::Disabled => {}
                }

                style
            })
            .on_press(UiMessage::AddressSuggestionSelected(target))
        };

        let address_suggestions = self.address_suggestions();
        let history_button = toolbar_button("▼".to_string())
            .on_press(UiMessage::ToggleHistoryMenu(!self.menus.history_open));

        let history_menu: Option<Element<'_, UiMessage>> =
            if self.menus.history_open && !address_suggestions.is_empty() {
                let mut suggestions_list = column![
                    text("Historique")
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        })
                ]
                .spacing(spacing.xs);
                for suggestion in address_suggestions {
                    suggestions_list = suggestions_list.push(suggestion_button(
                        suggestion.display().to_string(),
                        suggestion,
                    ));
                }
                let position = self.menus.history_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);
                let menu = container(suggestions_list)
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fixed(420.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    });
                let menu_layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(menu)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                let dismiss_layer: Element<'_, UiMessage> =
                    mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                        .on_press(UiMessage::ToggleHistoryMenu(false))
                        .into();
                Some(stack![dismiss_layer, menu_layer].into())
            } else {
                None
            };

        let address_row = row![address_bar, history_button, address_status]
            .spacing(spacing.xs)
            .align_y(Alignment::Center);

        let address_section: Element<'_, UiMessage> =
            container(address_row).width(Length::Fill).into();

        let active_tab_title = self
            .tab_manager.tabs
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
        .style(move |_theme: &Theme, _status| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: border::rounded(0.0).width(0.0),
            icon: colors.text_muted,
            placeholder: colors.text_muted,
            value: colors.text_primary,
            selection: colors.selection,
        });
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
            border: border::rounded(6.0).color(colors.border).width(1.0),
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
        let compact_label = if self.state.config.compact_mode { "⊞ Normal" } else { "⊟ Compact" };
        // #32: Accessibility — tooltip helper macro
        macro_rules! tt {
            ($widget:expr, $label:expr) => {
                tooltip(
                    $widget,
                    text($label).size(typography.caption).font(typography.caption_font),
                    tooltip::Position::Bottom,
                )
                .gap(4.0)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(4.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
            };
        }
        let command_bar = row![
            tt!(toolbar_button(format!("{} Nouveau", icons::NEW)).on_press(UiMessage::NewFolder), "Nouveau dossier (Ctrl+Shift+N)"),
            tt!(cut_button, "Couper (Ctrl+X)"),
            tt!(copy_button, "Copier (Ctrl+C)"),
            tt!(paste_button, "Coller (Ctrl+V)"),
            tt!(toolbar_button(dark_mode_label).on_press(UiMessage::ToggleDarkMode), "Basculer thème clair/sombre"),
            tt!(toolbar_button(gitignore_label).on_press(UiMessage::ToggleGitignore), "Respecter .gitignore"),
            tt!(toolbar_button(compact_label.to_string()).on_press(UiMessage::ToggleCompactMode), "Mode compact/normal"),
            tt!(toolbar_button(grep_label).on_press(UiMessage::OpenGrep), "Chercher dans le contenu (Ctrl+Shift+F)"),
            tt!(toolbar_button(format!("{} Actions", icons::ACTIONS))
                .on_press(UiMessage::ToggleContextMenu(!self.menus.context_open)), "Menu contextuel (F10)")
        ]
        .spacing(spacing.sm);

        let loading_badge: Element<'_, UiMessage> = if self.show_loading_indicator || self.operation_progress.is_some() {
            // #16: Enhanced progress bar with operation label
            let (progress_label, ratio) = if let Some(op) = &self.operation_progress {
                let done = op.counter.load(std::sync::atomic::Ordering::Relaxed);
                let r = if op.total > 0 { done as f32 / op.total as f32 } else { 0.5 };
                let kind_label = match op.kind {
                    crate::filesystem::FileOperationKind::Copy => "Copie",
                    crate::filesystem::FileOperationKind::Move => "Déplacement",
                    crate::filesystem::FileOperationKind::Delete => "Suppression",
                    crate::filesystem::FileOperationKind::Rename => "Renommage",
                };
                (format!("{} : {}/{}", kind_label, done, op.total), r)
            } else if self.entries.total > 0 {
                let loaded = self.entries.items.iter().filter(|e| e.is_some()).count();
                ("Chargement…".to_string(), loaded as f32 / self.entries.total as f32)
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
                    border: border::rounded(999.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            container(row![]).into()
        };

        let selection_count = self.state.navigation.selection.selected.len();
        let selected_is_archive = self.state.navigation.selection.focused.as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .map(|e| {
                let e = e.to_ascii_lowercase();
                e == "zip" || e == "7z" || e == "tgz"
            })
            .unwrap_or_else(|| {
                self.state.navigation.selection.focused.as_ref()
                    .map(|p| p.to_string_lossy().to_lowercase().ends_with(".tar.gz"))
                    .unwrap_or(false)
            });
        let selected_is_file = self.state.navigation.selection.focused.as_ref()
            .map(|p| p.is_file())
            .unwrap_or(false);
        let selected_is_dir = self.state.navigation.selection.focused.as_ref()
            .map(|p| p.is_dir())
            .unwrap_or(false);
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        // ── Context menu helpers ─────────────────────────────────────────────
        let ctx_button = |icon: &str, label: &str, shortcut: &str, msg: UiMessage| -> Element<'_, UiMessage> {
            let main_text = text(format!("{} {}", icon, label))
                .size(typography.body)
                .font(typography.body_font);
            let content: Element<'_, UiMessage> = if shortcut.is_empty() {
                main_text.into()
            } else {
                let shortcut_text = text(shortcut.to_string())
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .color(colors.text_muted);
                row![main_text, horizontal_space(), shortcut_text]
                    .spacing(spacing.lg)
                    .align_y(Alignment::Center)
                    .into()
            };
            button(content)
                .padding([spacing.xs, spacing.sm])
                .width(Length::Fill)
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };
                    match status {
                        ButtonStatus::Hovered => {
                            style.background = Some(Background::Color(colors.hover));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        ButtonStatus::Pressed => {
                            style.background = Some(Background::Color(colors.pressed));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        _ => {}
                    }
                    style
                })
                .on_press(msg)
                .into()
        };
        let ctx_separator = || -> Element<'_, UiMessage> {
            container(row![])
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.border)),
                    ..Default::default()
                })
                .into()
        };
        let ctx_label_dot = |dot_color: Color, label_text: String, action: ContextAction| -> Element<'_, UiMessage> {
            let dot = container(row![])
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(10.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: border::rounded(5.0).width(0.0),
                    ..Default::default()
                });
            button(
                row![dot, text(label_text).size(typography.body).font(typography.body_font)]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(move |_theme: &Theme, status: ButtonStatus| {
                let mut style = iced::widget::button::Style {
                    text_color: colors.text_primary,
                    ..Default::default()
                };
                if matches!(status, ButtonStatus::Hovered | ButtonStatus::Pressed) {
                    style.background = Some(Background::Color(colors.hover));
                    style.border = border::rounded(6.0).color(colors.border).width(1.0);
                }
                style
            })
            .on_press(UiMessage::ContextAction(action))
            .into()
        };

        // ── Entry context menu (right-click on a file/folder) ────────────────
        let mut context_col: _ = column![].spacing(2);

        // Group 1: Open
        context_col = context_col
            .push(ctx_button(icons::OPEN, "Ouvrir", "Entrée", UiMessage::ContextAction(ContextAction::Open)))
            .push(ctx_button(icons::OPEN, "Ouvrir avec\u{2026}", "", UiMessage::ContextAction(ContextAction::OpenWith)));
        if selected_is_dir {
            context_col = context_col.push(
                ctx_button(icons::TERMINAL, "Ouvrir terminal ici", "", UiMessage::ToggleTerminal),
            );
        }
        if selected_is_archive {
            context_col = context_col.push(
                ctx_button(icons::FILE_ARCHIVE, "Ouvrir l'archive", "", UiMessage::ActivateEntry(
                    self.state.navigation.selection.focused.clone().unwrap_or_default()
                )),
            );
        }

        // Separator
        context_col = context_col.push(ctx_separator());

        // Group 2: Clipboard
        context_col = context_col
            .push(ctx_button(icons::CUT, "Couper", "Ctrl+X", UiMessage::ClipboardCut))
            .push(ctx_button(icons::COPY, "Copier", "Ctrl+C", UiMessage::ClipboardCopy));
        if has_clipboard {
            context_col = context_col.push(
                ctx_button(icons::PASTE, "Coller", "Ctrl+V", UiMessage::ClipboardPaste),
            );
        }

        // Separator
        context_col = context_col.push(ctx_separator());

        // Group 3: Edit
        context_col = context_col
            .push(ctx_button(icons::RENAME, "Renommer", "F2", UiMessage::ContextAction(ContextAction::Rename)))
            .push(ctx_button(icons::DELETE, "Supprimer", "Suppr", UiMessage::ContextAction(ContextAction::MoveToTrash)));
        if selection_count > 1 {
            context_col = context_col.push(
                ctx_button(icons::RENAME, "Renommer plusieurs\u{2026}", "", UiMessage::ContextAction(ContextAction::OpenBulkRename)),
            );
        }
        if selection_count == 2 {
            context_col = context_col.push(
                ctx_button(icons::FILE, "Comparer", "Ctrl+D", UiMessage::ContextAction(ContextAction::OpenDiff)),
            );
        }

        // Separator
        context_col = context_col.push(ctx_separator());

        // Group 4: Compress submenu trigger (hover to open)
        context_col = context_col.push(
            mouse_area(ctx_button(icons::FILE_ARCHIVE, "Compresser  \u{276f}", "", UiMessage::ToggleContextSubmenu(0)))
                .on_enter(UiMessage::ToggleContextSubmenu(0)),
        );

        // Group 5: Label submenu trigger (hover to open)
        context_col = context_col.push(
            mouse_area(ctx_button("", "\u{25cf} Étiquette  \u{276f}", "", UiMessage::ToggleContextSubmenu(1)))
                .on_enter(UiMessage::ToggleContextSubmenu(1)),
        );

        // Separator
        context_col = context_col.push(ctx_separator());

        // Group 6: Info
        context_col = context_col
            .push(ctx_button(icons::SYMLINK, "Copier le chemin", "Ctrl+Shift+C", UiMessage::ContextAction(ContextAction::CopyPath)))
            .push(ctx_button(icons::HOME, "Ajouter aux favoris", "", UiMessage::ContextAction(ContextAction::AddToFavorites)));
        if selected_is_file {
            context_col = context_col.push(
                ctx_button(icons::FILE, "Voir en hexadécimal", "", UiMessage::ContextAction(ContextAction::OpenHexView)),
            );
        }
        context_col = context_col
            .push(ctx_button(icons::FILE, "Propriétés", "Alt+Entrée", UiMessage::ContextAction(ContextAction::OpenProperties)));

        // ── Background context menu (right-click on empty space) ─────────────
        let mut bg_context_col: _ = column![].spacing(2);
        if has_clipboard {
            bg_context_col = bg_context_col
                .push(ctx_button(icons::PASTE, "Coller", "Ctrl+V", UiMessage::ClipboardPaste))
                .push(ctx_separator());
        }
        bg_context_col = bg_context_col
            .push(ctx_button(icons::FOLDER, "Nouveau dossier", "Ctrl+Shift+N", UiMessage::ContextAction(ContextAction::NewFolder)))
            .push(ctx_button(icons::FILE, "Nouveau fichier", "", UiMessage::ContextAction(ContextAction::NewFile)))
            .push(ctx_separator())
            .push(ctx_button(icons::TERMINAL, "Ouvrir terminal ici", "", UiMessage::ToggleTerminal))
            .push(ctx_button(icons::REFRESH, "Rafraîchir", "Ctrl+R", UiMessage::Refresh))
            .push(ctx_separator())
            .push(ctx_button(icons::FILE, "Propriétés du dossier", "", UiMessage::ContextAction(ContextAction::OpenProperties)));

        // ── Context submenu panels ───────────────────────────────────────────
        use super::types::ContextSubmenu;
        let submenu_panel: Option<Element<'_, UiMessage>> = match self.menus.context_submenu {
            Some(ContextSubmenu::Compress) => {
                let sub = column![
                    ctx_button(icons::FILE_ARCHIVE, "ZIP", "", UiMessage::ContextAction(ContextAction::CompressToZip)),
                    ctx_button(icons::FILE_ARCHIVE, "TAR.GZ", "", UiMessage::ContextAction(ContextAction::CompressToTarGz)),
                    ctx_button(icons::FILE_ARCHIVE, "7Z", "", UiMessage::ContextAction(ContextAction::CompressTo7z)),
                ].spacing(2);
                Some(container(sub)
                    .padding([spacing.sm, spacing.md])
                    .width(Length::Fixed(180.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    })
                    .into())
            }
            Some(ContextSubmenu::Label) => {
                let sub = column![
                    ctx_label_dot(Color::from_rgb8(220, 50, 50), "Rouge".into(), ContextAction::SetLabelRed),
                    ctx_label_dot(Color::from_rgb8(255, 165, 0), "Orange".into(), ContextAction::SetLabelOrange),
                    ctx_label_dot(Color::from_rgb8(220, 200, 50), "Jaune".into(), ContextAction::SetLabelYellow),
                    ctx_label_dot(Color::from_rgb8(50, 180, 50), "Verte".into(), ContextAction::SetLabelGreen),
                    ctx_label_dot(Color::from_rgb8(50, 100, 220), "Bleue".into(), ContextAction::SetLabelBlue),
                    ctx_label_dot(Color::from_rgb8(150, 50, 200), "Violette".into(), ContextAction::SetLabelPurple),
                    ctx_separator(),
                    ctx_button("", "Supprimer", "", UiMessage::ContextAction(ContextAction::RemoveLabel)),
                ].spacing(2);
                Some(container(sub)
                    .padding([spacing.sm, spacing.md])
                    .width(Length::Fixed(180.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    })
                    .into())
            }
            None => None,
        };

        // ── Context menu rendering ───────────────────────────────────────────
        let show_entry_menu = self.menus.context_open && !self.state.navigation.selection.selected.is_empty();
        let show_bg_menu = self.menus.background_context_open;

        let context_menu: Option<Element<'_, UiMessage>> =
            if show_entry_menu || show_bg_menu {
                let main_menu: Element<'_, UiMessage> = if show_entry_menu {
                    context_col.into()
                } else {
                    bg_context_col.into()
                };
                let position = self.menus.context_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);

                let menu_style = move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(8.0).color(colors.border).width(1.0),
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
                    opaque(menu).into()
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
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.chrome_background)),
            border: border::rounded(10.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let display_entries = self.display_entries();

        let column_specs = column_specs(&self.state.config.view.columns);
        let filtered_indices = self.filtered_indices_for(display_entries);

        let view_mode = self.state.config.view.mode;
        let row_height = self.state.config.view.row_height;
        let is_filtered = filtered_indices.is_some();
        let total_entries = filtered_indices
            .as_ref()
            .map_or(display_entries.total, |indices| indices.len());
        let entry_index_for = |display_index: usize| -> Option<usize> {
            if let Some(indices) = &filtered_indices {
                indices.get(display_index).copied()
            } else {
                Some(display_index)
            }
        };
        let entry_leading = |entry: &FsEntry| -> Element<'_, UiMessage> {
            match entry.entry_type {
                FsEntryType::Directory => text(icons::FOLDER)
                    .size(typography.body)
                    .font(typography.body_font)
                    .into(),
                FsEntryType::File if matches!(view_mode, ViewMode::Grid) => {
                    // Grid mode: show thumbnail preview if available
                    self.media.thumbnail_handles
                        .get(&entry.path)
                        .map(|handle| {
                            image(handle.clone())
                                .width(Length::Fixed(self.state.config.view.thumbnail_size as f32))
                                .height(Length::Fixed(self.state.config.view.thumbnail_size as f32))
                                .into()
                        })
                        .unwrap_or_else(|| {
                            let icon = entry.path.extension()
                                .and_then(|ext| ext.to_str())
                                .map(icons::icon_for_extension)
                                .unwrap_or(icons::FILE);
                            text(icon)
                                .size(typography.body)
                                .font(typography.body_font)
                                .into()
                        })
                },
                FsEntryType::File => {
                    // List mode: always use icon for uniform row height
                    let icon = entry.path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(icons::icon_for_extension)
                        .unwrap_or(icons::FILE);
                    text(icon)
                        .size(typography.body)
                        .font(typography.body_font)
                        .into()
                },
                FsEntryType::Symlink => text(icons::SYMLINK)
                    .size(typography.body)
                    .font(typography.body_font)
                    .into(),
                FsEntryType::Other => text(icons::UNKNOWN)
                    .size(typography.body)
                    .font(typography.body_font)
                    .into(),
            }
        };
        let build_list_row = |entry: &FsEntry| -> Element<'_, UiMessage> {
            let mut entry_row = row![].spacing(spacing.md).align_y(Alignment::Center);
            for spec in &column_specs {
                let cell: Element<'_, UiMessage> = match spec.column {
                    ViewColumn::Name => {
                        // Feature F: File label dot
                        let label_dot: Option<Element<'_, UiMessage>> = self.state.config.labels.get(&entry.path).map(|label| {
                            let dot_color = label.color();
                            container(row![])
                                .width(Length::Fixed(8.0))
                                .height(Length::Fixed(8.0))
                                .style(move |_| iced::widget::container::Style {
                                    background: Some(Background::Color(dot_color)),
                                    border: border::rounded(4.0).width(0.0),
                                    ..Default::default()
                                })
                                .into()
                        });
                        // Feature 7: Git status badge
                        let git_badge: Option<Element<'_, UiMessage>> = self.git_statuses.get(&entry.path).map(|status| {
                            use crate::ui::GitFileStatus;
                            let (label, color) = match status {
                                GitFileStatus::Modified => ("M", colors.accent),
                                GitFileStatus::Untracked => ("?", colors.text_muted),
                                GitFileStatus::Staged => ("S", colors.git_staged),
                                GitFileStatus::Conflict => ("!", colors.git_conflict),
                                GitFileStatus::Deleted => ("D", colors.git_deleted),
                            };
                            text(label)
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .color(color)
                                .into()
                        });
                        let is_cut = matches!(self.clipboard.kind, Some(ClipboardKind::Cut))
                            && self.clipboard.items.contains(&entry.path);
                        let mut name_row = row![entry_leading(entry)]
                            .spacing(spacing.sm)
                            .align_y(Alignment::Center);
                        if let Some(dot) = label_dot {
                            name_row = name_row.push(dot);
                        }
                        let display_name = truncate_name(&entry.name, 60);
                        let name_text = text(display_name.into_owned())
                            .size(typography.body)
                            .font(typography.body_font);
                        let name_text = if is_cut {
                            name_text.color(colors.text_muted)
                        } else {
                            name_text
                        };
                        name_row = name_row.push(name_text);
                        if let Some(badge) = git_badge {
                            name_row = name_row.push(badge);
                        }
                        name_row = name_row.push(horizontal_space());
                        container(name_row)
                            .width(spec.width)
                            .align_x(spec.align)
                            .into()
                    }
                    ViewColumn::Type => container(
                        text(entry_type_label(entry.entry_type))
                            .size(typography.caption)
                            .font(typography.caption_font),
                    )
                    .width(spec.width)
                    .align_x(spec.align)
                    .into(),
                    ViewColumn::Size => {
                        // Feature 8: Show dir sizes
                        let size_str = if entry.entry_type == FsEntryType::Directory {
                            if let Some(&bytes) = self.dir_sizes.get(&entry.path) {
                                super::helpers::format_bytes(bytes)
                            } else if self.dir_sizes_loading.contains(&entry.path) {
                                "…".to_string()
                            } else {
                                "—".to_string()
                            }
                        } else {
                            format_entry_size(entry)
                        };
                        container(
                            text(size_str)
                                .size(typography.caption)
                                .font(typography.caption_font),
                        )
                        .width(spec.width)
                        .align_x(spec.align)
                        .into()
                    }
                    ViewColumn::Modified => container(
                        text(format_modified(entry.metadata.modified))
                            .size(typography.caption)
                            .font(typography.caption_font),
                    )
                    .width(spec.width)
                    .align_x(spec.align)
                    .into(),
                };
                entry_row = entry_row.push(cell);
            }
            entry_row.into()
        };
        let build_loading_row = || -> Element<'_, UiMessage> {
            let mut placeholder_row = row![].spacing(spacing.md).align_y(Alignment::Center);
            for (index, spec) in column_specs.iter().enumerate() {
                let cell: Element<'_, UiMessage> = if index == 0 {
                    let content = row![
                        text(icons::LOADING)
                            .size(typography.body)
                            .font(typography.body_font),
                        text("Chargement…")
                            .size(typography.body)
                            .font(typography.body_font)
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center);
                    container(content)
                        .width(spec.width)
                        .align_x(spec.align)
                        .into()
                } else {
                    container(row![])
                        .width(spec.width)
                        .align_x(spec.align)
                        .into()
                };
                placeholder_row = placeholder_row.push(cell);
            }
            button(placeholder_row)
                .height(Length::Fixed(row_height))
                .into()
        };
        let build_grid_tile = |entry: &FsEntry| -> Element<'_, UiMessage> {
            let grid_name = truncate_name(&entry.name, 20);
            let mut name_row = row![].spacing(spacing.xs).align_y(Alignment::Center);
            // Label dot in grid view
            if let Some(label) = self.state.config.labels.get(&entry.path) {
                let dot_color = label.color();
                name_row = name_row.push(
                    container(row![])
                        .width(Length::Fixed(6.0))
                        .height(Length::Fixed(6.0))
                        .style(move |_| iced::widget::container::Style {
                            background: Some(Background::Color(dot_color)),
                            border: border::rounded(3.0).width(0.0),
                            ..Default::default()
                        }),
                );
            }
            name_row = name_row.push(
                text(grid_name.into_owned())
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
            let tile_content = column![
                entry_leading(entry),
                name_row,
            ]
            .spacing(spacing.xs)
            .align_x(Alignment::Center);
            container(tile_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        };

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier")
                    .size(typography.title)
                    .font(typography.title_font),
                text(message)
                    .size(typography.body)
                    .font(typography.body_font),
                button(
                    text("Réessayer")
                        .size(typography.body)
                        .font(typography.body_font),
                )
                .on_press(UiMessage::Refresh)
            ]
            .spacing(spacing.sm)
        } else if total_entries == 0 && !self.is_loading {
            if is_filtered {
                column![
                    text("Aucun résultat")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
            } else {
                column![
                    text("Dossier vide")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
            }
        } else if total_entries == 0 {
            column![]
        } else {
            match view_mode {
                ViewMode::List => {
                    let window = self.list_virtual_window_for(total_entries);
                    let mut list = column![];

                    if window.padding_top > 0.0 {
                        list =
                            list.push(vertical_space().height(Length::Fixed(window.padding_top)));
                    }

                    for display_index in window.start..window.end {
                        let Some(actual_index) = entry_index_for(display_index) else {
                            continue;
                        };
                        let entry = display_entries.get(actual_index);
                        if let Some(entry) = entry {
                            let is_selected = self
                                .state
                                .navigation
                                .selection
                                .selected
                                .contains(&entry.path);
                            let is_focused = self
                                .state
                                .navigation
                                .selection
                                .focused
                                .as_ref()
                                .map(|path| path == &entry.path)
                                .unwrap_or(false);
                            let message = UiMessage::SelectEntry {
                                path: entry.path.clone(),
                                kind: self.selection_kind_from_modifiers(),
                            };
                            let context_path = entry.path.clone();
                            let pressed_path = entry.path.clone();
                            list = list.push(
                                mouse_area(
                                    button(build_list_row(entry))
                                        .padding([spacing.xs, spacing.sm])
                                        .height(Length::Fixed(row_height))
                                        .style(move |_theme: &Theme, status: ButtonStatus| {
                                            let mut style = iced::widget::button::Style {
                                                text_color: colors.text_primary,
                                                ..Default::default()
                                            };

                                            if is_selected {
                                                style.background =
                                                    Some(Background::Color(colors.selection));
                                                style.border = border::rounded(6.0)
                                                    .color(colors.selection_border)
                                                    .width(if is_focused { 2.0 } else { 1.0 });
                                            }

                                            if matches!(status, ButtonStatus::Hovered) {
                                                style.background =
                                                    Some(Background::Color(colors.hover));
                                            }

                                            if matches!(status, ButtonStatus::Pressed) {
                                                style.background =
                                                    Some(Background::Color(colors.pressed));
                                            }

                                            style
                                        })
                                        .on_press(message),
                                )
                                .on_press(UiMessage::EntryPressed(pressed_path))
                                .on_right_press(UiMessage::OpenContextMenuForEntry(context_path)),
                            );
                        } else {
                            list = list.push(build_loading_row());
                        }
                    }

                    if window.padding_bottom > 0.0 {
                        list = list
                            .push(vertical_space().height(Length::Fixed(window.padding_bottom)));
                    }

                    list
                }
                ViewMode::Grid => {
                    let grid = self.grid_window_for(total_entries);
                    let mut list = column![];
                    let tile_height = self.state.config.view.grid_row_height;

                    if grid.window.padding_top > 0.0 {
                        list = list
                            .push(vertical_space().height(Length::Fixed(grid.window.padding_top)));
                    }

                    for row_index in grid.window.start..grid.window.end {
                        let mut tile_row = row![].spacing(spacing.md);
                        for column_index in 0..grid.columns {
                            let display_index = row_index * grid.columns + column_index;
                            if display_index >= grid.total {
                                tile_row = tile_row.push(
                                    container(row![])
                                        .width(Length::FillPortion(1))
                                        .height(Length::Fixed(tile_height)),
                                );
                                continue;
                            }
                            let Some(actual_index) = entry_index_for(display_index) else {
                                continue;
                            };
                            let entry = display_entries.get(actual_index);
                            let tile_element: Element<'_, UiMessage> = match entry {
                                Some(entry) => {
                                    let is_selected = self
                                        .state
                                        .navigation
                                        .selection
                                        .selected
                                        .contains(&entry.path);
                                    let is_focused = self
                                        .state
                                        .navigation
                                        .selection
                                        .focused
                                        .as_ref()
                                        .map(|path| path == &entry.path)
                                        .unwrap_or(false);
                                    let message = UiMessage::SelectEntry {
                                        path: entry.path.clone(),
                                        kind: self.selection_kind_from_modifiers(),
                                    };
                                    let context_path = entry.path.clone();
                                    let pressed_path = entry.path.clone();
                                    mouse_area(
                                        container(
                                            button(build_grid_tile(entry))
                                                .width(Length::Fill)
                                                .height(Length::Fill)
                                                .padding(spacing.sm)
                                                .style(
                                                    move |_theme: &Theme, status: ButtonStatus| {
                                                        let mut style =
                                                            iced::widget::button::Style {
                                                                text_color: colors.text_primary,
                                                                ..Default::default()
                                                            };

                                                        if is_selected {
                                                            style.background = Some(
                                                                Background::Color(colors.selection),
                                                            );
                                                            style.border = border::rounded(8.0)
                                                                .color(colors.selection_border)
                                                                .width(if is_focused {
                                                                    2.0
                                                                } else {
                                                                    1.0
                                                                });
                                                        }

                                                        if matches!(status, ButtonStatus::Hovered) {
                                                            style.background = Some(
                                                                Background::Color(colors.hover),
                                                            );
                                                        }

                                                        if matches!(status, ButtonStatus::Pressed) {
                                                            style.background = Some(
                                                                Background::Color(colors.pressed),
                                                            );
                                                        }

                                                        style
                                                    },
                                                )
                                                .on_press(message),
                                        )
                                        .width(Length::FillPortion(1))
                                        .height(Length::Fixed(tile_height)),
                                    )
                                    .on_press(UiMessage::EntryPressed(pressed_path))
                                    .on_right_press(UiMessage::OpenContextMenuForEntry(
                                        context_path,
                                    ))
                                    .into()
                                }
                                None => container(row![])
                                    .width(Length::FillPortion(1))
                                    .height(Length::Fixed(tile_height))
                                    .into(),
                            };
                            tile_row = tile_row.push(tile_element);
                        }
                        list = list.push(tile_row);
                    }

                    if grid.window.padding_bottom > 0.0 {
                        list = list.push(
                            vertical_space().height(Length::Fixed(grid.window.padding_bottom)),
                        );
                    }

                    list
                }
            }
        };

        let list_header_visible =
            self.list_header_visible(view_mode, display_entries, filtered_indices.as_ref());
        let list_header: Element<'_, UiMessage> = if list_header_visible {
            let mut header_row = row![].spacing(spacing.md).align_y(Alignment::Center);
            for spec in &column_specs {
                let is_active_sort = spec
                    .sort_key
                    .is_some_and(|key| key == self.state.config.list.sort_key);
                let sort_indicator = if is_active_sort {
                    match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => "↑",
                        SortOrderConfig::Desc => "↓",
                    }
                } else {
                    ""
                };
                let label = if sort_indicator.is_empty() {
                    spec.label.to_string()
                } else {
                    format!("{} {}", spec.label, sort_indicator)
                };
                let header_text = text(label)
                    .size(typography.caption)
                    .font(typography.caption_font);
                let cell: Element<'_, UiMessage> = if let Some(sort_key) = spec.sort_key {
                    button(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                            }

                            style
                        })
                        .on_press(UiMessage::ChangeSort(sort_key))
                        .into()
                } else {
                    container(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .into()
                };
                header_row = header_row.push(container(cell).width(spec.width).align_x(spec.align));
                // Feature H: resize handle between columns
                let col_name = spec.label.to_string();
                let is_resizing = self.column_resize_state.as_ref().is_some_and(|r| r.column == col_name);
                let handle_color = if is_resizing { colors.accent } else { colors.border };
                header_row = header_row.push(
                    mouse_area(
                        container(row![])
                            .width(Length::Fixed(4.0))
                            .height(Length::Fill)
                            .style(move |_| iced::widget::container::Style {
                                background: Some(Background::Color(handle_color)),
                                ..Default::default()
                            })
                    )
                    .on_press(UiMessage::ColumnResizeStart(col_name))
                    .on_release(UiMessage::ColumnResizeEnd)
                    .interaction(mouse::Interaction::ResizingHorizontally)
                );
            }

            container(header_row)
                .padding([spacing.xs, spacing.sm])
                .height(Length::Fixed(row_height))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.chrome_background)),
                    border: border::rounded(6.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            container(row![]).into()
        };

        let rename_prompt = if let Some(dialog) = &self.rename_dialog {
            let input = text_input("Nouveau nom…", &dialog.input)
                .on_input(UiMessage::RenameInputChanged)
                .on_submit(UiMessage::RenameSubmit)
                .size(typography.body)
                .font(typography.body_font)
                .padding([spacing.xs, spacing.md]);
            container(
                row![
                    text("Renommer :")
                        .size(typography.body)
                        .font(typography.body_font),
                    input,
                    toolbar_button("Valider".to_string()).on_press(UiMessage::RenameSubmit),
                    toolbar_button("Annuler".to_string()).on_press(UiMessage::RenameCancel)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
            )
            .padding([spacing.sm, spacing.md])
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(8.0).color(colors.border).width(1.0),
                ..Default::default()
            })
        } else {
            container(row![])
        };

        let mut list_column = column![].spacing(spacing.xl);
        if self.rename_dialog.is_some() {
            list_column = list_column.push(rename_prompt);
        }
        if list_header_visible {
            list_column = list_column.push(list_header);
        }
        list_column = list_column.push(list_content);

        // Add a clickable spacer below entries so rubber band selection can start
        // from empty space.  Without this, clicks below the last entry land on the
        // scrollable's dead zone and never reach the outer mouse_area.
        let bg_spacer_height = if self.scroll.height > 1.0 {
            self.scroll.height
        } else {
            500.0
        };
        list_column = list_column.push(
            mouse_area(
                container(row![])
                    .width(Length::Fill)
                    .height(Length::Fixed(bg_spacer_height)),
            )
            .on_press(UiMessage::ListBackgroundPressed)
            .on_right_press(UiMessage::BackgroundContextMenu),
        );

        let list = mouse_area(
            scrollable(container(list_column).padding(spacing.md))
                .id(iced::widget::Id::new(super::LIST_SCROLLABLE_ID))
                .on_scroll(|viewport| {
                    UiMessage::Scroll(ScrollViewport {
                        offset_y: viewport.absolute_offset().y,
                        viewport_height: viewport.bounds().height,
                        content_height: viewport.content_bounds().height,
                        bounds: viewport.bounds(),
                    })
                }),
        )
        .on_press(UiMessage::ListBackgroundPressed)
        .on_right_press(UiMessage::BackgroundContextMenu);

        // Accordion collapsed state per section name
        let sec_tree      = self.sidebar_collapsed.contains("Arborescence");
        let sec_access    = self.sidebar_collapsed.contains("Accès rapide");
        let sec_favorites = self.sidebar_collapsed.contains("Favoris");
        let sec_drives    = self.sidebar_collapsed.contains("Lecteurs");

        let tree_nodes = &self.cached_tree_nodes;

        let mut tree_section = column![].spacing(spacing.xs);
        if tree_nodes.is_empty() {
            tree_section = tree_section.push(
                text("Arborescence indisponible")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    }),
            );
        } else {
            let window = self.tree_virtual_window(tree_nodes.len());
            if window.padding_top > 0.0 {
                tree_section =
                    tree_section.push(vertical_space().height(Length::Fixed(window.padding_top)));
            }
            for node in tree_nodes.iter().skip(window.start).take(window.len()) {
                let label = node.label.clone();
                let path = node.path.clone();
                let selected = node.selected;
                let depth = node.depth;
                let expanded = node.expanded;
                let icon = if depth == 0 { icons::PC } else { icons::FOLDER };
                let chevron = if expanded { "▾" } else { "▸" };
                let indent =
                    horizontal_space().width(Length::Fixed(depth as f32 * (spacing.sm + 2.0)));
                let content: Element<'_, UiMessage> = row![
                    indent,
                    text(chevron)
                        .size(typography.caption)
                        .font(typography.caption_font),
                    text(icon).size(typography.body).font(typography.body_font),
                    text(label).size(typography.body).font(typography.body_font)
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center)
                .into();

                let drop_path = path.clone();
                let button = mouse_area(
                    button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            if selected {
                                style.background = Some(Background::Color(colors.selection));
                                style.border = border::rounded(6.0)
                                    .color(colors.selection_border)
                                    .width(1.0);
                            }

                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                            }

                            if matches!(status, ButtonStatus::Pressed) {
                                style.background = Some(Background::Color(colors.pressed));
                            }

                            style
                        })
                        .on_press(UiMessage::NavigateTo(path)),
                )
                .on_release(UiMessage::DropOnPath(drop_path));

                tree_section = tree_section.push(button);
            }
            if window.padding_bottom > 0.0 {
                tree_section = tree_section
                    .push(vertical_space().height(Length::Fixed(window.padding_bottom)));
            }
        }
        let tree_panel = if sec_tree {
            column![section_header("Arborescence", true)].spacing(spacing.xs)
        } else {
            column![
                section_header("Arborescence", false),
                scrollable(tree_section)
                    .height(Length::Fixed(self.pane_resize.tree_height))
                    .width(Length::Fill)
                    .on_scroll(|viewport| UiMessage::TreeScroll(ScrollViewport {
                        offset_y: viewport.absolute_offset().y,
                        viewport_height: viewport.bounds().height,
                        content_height: viewport.content_bounds().height,
                        bounds: viewport.bounds(),
                    }))
            ]
            .spacing(spacing.xs)
        };

        let tree_resize_bar: Element<'_, UiMessage> = if sec_tree {
            row![].into()
        } else {
            mouse_area(
                container(row![])
                    .width(Length::Fill)
                    .height(Length::Fixed(TREE_RESIZE_BAR_HEIGHT))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.hover)),
                        border: border::rounded(6.0).color(colors.border).width(1.0),
                        ..Default::default()
                    }),
            )
            .on_press(UiMessage::TreeResizeStart)
            .on_release(UiMessage::TreeResizeEnd)
            .interaction(mouse::Interaction::ResizingVertically)
            .into()
        };

        // ── Quick Access with user folders ──────────────────────────────
        let mut quick_access =
            column![section_header("Accès rapide", sec_access)].spacing(spacing.xs);

        // Add specific user folders with proper icons
        let user_folder_entries: Vec<(&str, &str, Option<PathBuf>)> = {
            let favorites = self.favorites.list();
            let find_favorite = |name: &str| -> Option<PathBuf> {
                favorites.iter().find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n == name)
                }).cloned()
            };
            vec![
                (icons::HOME, "Accueil", home_dir.clone()),
                (icons::DESKTOP, "Bureau", find_favorite("Desktop")),
                (icons::DOWNLOAD, "Téléchargements", find_favorite("Downloads")),
                (icons::DOCUMENTS, "Documents", find_favorite("Documents")),
                (icons::GALLERY, "Images", find_favorite("Pictures")),
                (icons::MUSIC, "Musique", find_favorite("Music")),
                (icons::VIDEO, "Vidéos", find_favorite("Videos")),
            ]
        };

        if !sec_access {
            for (icon, label, path) in &user_folder_entries {
                if path.is_some() {
                    quick_access = quick_access.push(sidebar_button(icon, label, path.clone()));
                }
            }
        }

        // Custom favorites (user-added, not duplicating system folders)
        let system_paths: std::collections::HashSet<PathBuf> = user_folder_entries
            .iter()
            .filter_map(|(_, _, p)| p.clone())
            .collect();
        let custom_favorites: Vec<_> = self
            .favorites
            .list()
            .iter()
            .filter(|p| !system_paths.contains(*p))
            .collect();

        let mut favorites_section =
            column![section_header("Favoris", sec_favorites)].spacing(spacing.xs);
        if !sec_favorites {
            if custom_favorites.is_empty() {
                favorites_section = favorites_section.push(
                    text("Aucun favori")
                        .size(typography.caption)
                        .font(typography.caption_font),
                );
            } else {
                for favorite in custom_favorites {
                    let label = format_sidebar_label(favorite);
                    let fav_path = favorite.clone();
                    let remove_btn = button(
                        text(icons::CLOSE)
                            .size(typography.caption)
                            .font(typography.caption_font),
                    )
                    .padding(0)
                    .style(move |_theme: &Theme, _status| iced::widget::button::Style {
                        text_color: colors.text_muted,
                        ..Default::default()
                    })
                    .on_press(UiMessage::RemoveFavorite(fav_path));
                    favorites_section = favorites_section.push(
                        row![
                            sidebar_button(
                                icons::FOLDER,
                                &label,
                                Some(favorite.clone()),
                            ),
                            remove_btn,
                        ]
                        .align_y(Alignment::Center),
                    );
                }
            }
        }

        // ── All drives ───────────────────────────────────────────────────
        let mut drive_section = column![section_header("Lecteurs", sec_drives)].spacing(spacing.xs);
        let drives = all_drives();
        if !sec_drives {
        if drives.is_empty() {
            drive_section = drive_section.push(
                text("Aucun lecteur")
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        } else {
            for (mount, usage) in &drives {
                let total_gb = format_gigabytes(usage.total);
                let free_gb = format_gigabytes(usage.available);
                let used_ratio = if usage.total == 0 {
                    0.0
                } else {
                    1.0 - (usage.available as f32 / usage.total as f32)
                };
                let mount_path = mount.clone();
                let content: Element<'_, UiMessage> = column![
                    row![
                        text(icons::DRIVE)
                            .size(typography.caption)
                            .font(typography.caption_font),
                        text(drive_label(mount))
                            .size(typography.caption)
                            .font(typography.caption_font)
                    ]
                    .spacing(spacing.xs)
                    .align_y(Alignment::Center),
                    progress_bar(0.0..=1.0, used_ratio).girth(Length::Fixed(6.0)),
                    text(format!("{} Go libres sur {} Go", free_gb, total_gb))
                        .size(typography.caption)
                        .font(typography.caption_font)
                ]
                .spacing(spacing.xs)
                .into();

                drive_section = drive_section.push(
                    button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };
                            match status {
                                ButtonStatus::Hovered => {
                                    style.background = Some(Background::Color(colors.hover));
                                    style.border =
                                        border::rounded(6.0).color(colors.border).width(1.0);
                                }
                                ButtonStatus::Pressed => {
                                    style.background = Some(Background::Color(colors.pressed));
                                }
                                ButtonStatus::Active | ButtonStatus::Disabled => {}
                            }
                            style
                        })
                        .on_press(UiMessage::NavigateTo(mount_path)),
                );
            }
        }
        drive_section = drive_section.push(sidebar_button(
            icons::NETWORK,
            "Réseau",
            Some(PathBuf::from(NETWORK_ROUTE)),
        ));
        drive_section = drive_section.push(sidebar_button(
            icons::FILE,
            "Récents",
            Some(PathBuf::from(crate::ui::RECENT_ROUTE)),
        ));
        // #21: Trash in sidebar
        drive_section = drive_section.push(
            button(
                row![
                    text(icons::DELETE).size(typography.body).font(typography.body_font),
                    text("Corbeille").size(typography.body).font(typography.body_font),
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(move |_theme: &Theme, status: ButtonStatus| {
                let mut style = iced::widget::button::Style {
                    text_color: colors.text_primary,
                    ..Default::default()
                };
                if matches!(status, ButtonStatus::Hovered) {
                    style.background = Some(Background::Color(colors.hover));
                    style.border = border::rounded(6.0).color(colors.border).width(1.0);
                }
                style
            })
            .on_press(UiMessage::NavigateToTrash),
        );
        } // end if !sec_drives

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

        let preview_row = |label: String, value: String| {
            row![
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                )
                .width(Length::Fixed(90.0)),
                text(value)
                    .size(typography.caption)
                    .font(typography.caption_font)
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center)
        };

        let preview_entry = self.selected_entry(display_entries);
        let preview_body: Element<'_, UiMessage> = match preview_entry {
            Some(entry) => {
                let icon = match entry.entry_type {
                    FsEntryType::Directory => icons::FOLDER,
                    FsEntryType::File => entry.path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(icons::icon_for_extension)
                        .unwrap_or(icons::FILE),
                    FsEntryType::Symlink => icons::SYMLINK,
                    FsEntryType::Other => icons::UNKNOWN,
                };
                let preview_media_size =
                    (self.state.config.view.thumbnail_size as f32 * 3.0).clamp(120.0, 220.0);
                let preview_media: Element<'_, UiMessage> = match entry.entry_type {
                    FsEntryType::File => {
                        if let Some(animated) = self
                            .media.animated
                            .as_ref()
                            .filter(|animated| animated.path == entry.path)
                        {
                            image(animated.handle.clone())
                                .width(Length::Fixed(preview_media_size))
                                .height(Length::Fixed(preview_media_size))
                                .into()
                        } else {
                            self.media.preview_handles
                                .get(&entry.path)
                                .map(|handle| {
                                    image(handle.clone())
                                        .width(Length::Fixed(preview_media_size))
                                        .height(Length::Fixed(preview_media_size))
                                        .into()
                                })
                                .unwrap_or_else(|| {
                                    text(icons::LOADING)
                                        .size(typography.title)
                                        .font(typography.title_font)
                                        .into()
                                })
                        }
                    }
                    _ => text(icon)
                        .size(typography.title)
                        .font(typography.title_font)
                        .into(),
                };

                let metadata = column![
                    preview_row(
                        "Type".to_string(),
                        entry_type_label(entry.entry_type).to_string()
                    ),
                    preview_row("Taille".to_string(), format_entry_size(entry)),
                    preview_row(
                        "Modifié".to_string(),
                        format_modified(entry.metadata.modified),
                    ),
                    preview_row("Créé".to_string(), format_modified(entry.metadata.created)),
                    preview_row(
                        "Accès".to_string(),
                        format_modified(entry.metadata.accessed)
                    ),
                    preview_row(
                        "Lecture seule".to_string(),
                        if entry.metadata.readonly {
                            "Oui".to_string()
                        } else {
                            "Non".to_string()
                        },
                    ),
                ]
                .spacing(spacing.xs);

                // Feature 6: Text preview with syntax highlighting
                let text_preview_section: Element<'_, UiMessage> = self
                    .cached_text_preview
                    .as_ref()
                    .filter(|(p, _)| p == &entry.path)
                    .map(|(_, content)| {
                        // Check if we have highlighted lines for this path
                        let is_pdf = entry.path.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("pdf"))
                            .unwrap_or(false);
                        let has_highlight = self.cached_highlighted_preview
                            .as_ref()
                            .is_some_and(|(p, _)| p == &entry.path);

                        let inner: Element<'_, UiMessage> = if has_highlight {
                            if let Some((_, lines)) = &self.cached_highlighted_preview {
                                let mut lines_col = column![].spacing(0);
                                for line in lines {
                                    let mut line_row = row![].spacing(0);
                                    for (rgba, span_text) in &line.spans {
                                        let r = (rgba >> 24) as u8;
                                        let g = (rgba >> 16) as u8;
                                        let b = (rgba >> 8) as u8;
                                        let a = *rgba as u8;
                                        let color = Color::from_rgba8(r, g, b, a as f32 / 255.0);
                                        line_row = line_row.push(
                                            text(span_text.as_str())
                                                .size(typography.caption)
                                                .font(typography.body_font)
                                                .color(color)
                                        );
                                    }
                                    lines_col = lines_col.push(line_row);
                                }
                                lines_col.into()
                            } else {
                                text(content.as_str()).size(typography.caption).font(typography.body_font).into()
                            }
                        } else {
                            let label = if is_pdf {
                                format!("(PDF - texte extrait)\n{}", content)
                            } else {
                                content.clone()
                            };
                            text(label).size(typography.caption).font(typography.body_font).into()
                        };

                        let preview_text = container(
                            scrollable(
                                container(inner)
                                    .padding(spacing.sm)
                                    .width(Length::Fill),
                            )
                            .height(Length::Fixed(200.0)),
                        )
                        .style(move |_| iced::widget::container::Style {
                            background: Some(Background::Color(colors.chrome_background)),
                            border: border::rounded(6.0).color(colors.border).width(1.0),
                            ..Default::default()
                        })
                        .width(Length::Fill);
                        let el: Element<'_, UiMessage> = preview_text.into();
                        el
                    })
                    .unwrap_or_else(|| container(row![]).into());

                // Feature L: encoding badge
                let encoding_badge: Element<'_, UiMessage> = if let Some(enc) = &self.preview_encoding {
                    text(enc.as_str())
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style { color: Some(colors.text_muted) })
                        .into()
                } else {
                    row![].into()
                };

                column![
                    preview_media,
                    row![
                        text(&entry.name).size(typography.body).font(typography.body_font),
                        horizontal_space(),
                        encoding_badge,
                    ].align_y(Alignment::Center).spacing(spacing.xs),
                    text(entry.path.display().to_string())
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                    text_preview_section,
                    metadata
                ]
                .spacing(spacing.sm)
                .align_x(Alignment::Center)
                .into()
            }
            None if self.state.navigation.selection.selected.is_empty() => column![
                text("Sélectionnez un élément")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
            None => column![
                text("Aperçu en cours de chargement…")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
        };

        let preview_progress = self.preview_anim_progress;
        let preview_visible = preview_progress > 0.001;

        // Feature 10: Archive browser overlay
        // Feature G: Recent files list when route is Recent
        let main_list_element: Element<'_, UiMessage> = if self.state.route.is_recent() {
            let recents = self.recents.list();
            let recent_rows: Vec<Element<'_, UiMessage>> = if recents.is_empty() {
                vec![
                    text("Aucun fichier récent")
                        .size(typography.body)
                        .font(typography.body_font)
                        .into()
                ]
            } else {
                recents.iter().map(|entry| {
                    let icon = entry.path.extension()
                        .and_then(|e| e.to_str())
                        .map(icons::icon_for_extension)
                        .unwrap_or(icons::FILE);
                    let name = entry.path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let parent = entry.path.parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let accessed_str = {
                        let dt: chrono::DateTime<chrono::Local> = entry.accessed.into();
                        dt.format("%d/%m/%Y %H:%M").to_string()
                    };
                    let path = entry.path.clone();
                    button(
                        row![
                            text(icon).size(typography.body).font(typography.body_font),
                            column![
                                text(name).size(typography.body).font(typography.body_font),
                                row![
                                    text(parent).size(typography.caption).font(typography.caption_font)
                                        .style(move |_| iced::widget::text::Style { color: Some(colors.text_muted) }),
                                    text(accessed_str).size(typography.caption).font(typography.caption_font)
                                        .style(move |_| iced::widget::text::Style { color: Some(colors.text_muted) }),
                                ].spacing(spacing.md),
                            ].spacing(2),
                        ]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center)
                    )
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fill)
                    .style(move |_: &Theme, status: ButtonStatus| iced::widget::button::Style {
                        text_color: colors.text_primary,
                        background: if matches!(status, ButtonStatus::Hovered) {
                            Some(Background::Color(colors.hover))
                        } else { None },
                        ..Default::default()
                    })
                    .on_press(UiMessage::ActivateEntry(path))
                    .into()
                }).collect()
            };
            let header = container(
                row![
                    text("Fichiers récents").size(typography.body).font(typography.body_font),
                    horizontal_space(),
                    button(text("Effacer").size(typography.caption).font(typography.caption_font))
                        .on_press(UiMessage::ClearRecents)
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        }),
                ]
                .align_y(Alignment::Center)
                .spacing(spacing.sm)
            )
            .padding([spacing.xs, spacing.sm])
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            });
            container(
                column![header, scrollable(column(recent_rows).spacing(0)).height(Length::Fill)]
                    .spacing(spacing.xs)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(10.0).color(colors.border).width(1.0),
                ..Default::default()
            })
            .into()
        } else if let Some(archive) = &self.archive_browser {
            let archive_path_str = archive.archive_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Archive")
                .to_string();
            let inner_str = if archive.inner_path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", archive.inner_path)
            };

            let archive_entries: Vec<Element<'_, UiMessage>> = archive.entries.iter()
                .filter(|e| {
                    // Show only entries directly inside the current inner_path folder.
                    // Normalize backslashes to forward slashes for consistent handling.
                    let path = e.inner_path.replace('\\', "/");
                    if archive.inner_path.is_empty() {
                        // Root level: no '/' (files) or exactly one trailing '/' (dirs)
                        !path.contains('/') || (path.ends_with('/') && path.matches('/').count() == 1)
                    } else {
                        let inner = archive.inner_path.replace('\\', "/");
                        // Must start with "<inner_path>/" to avoid matching siblings
                        let prefix = format!("{}/", inner.trim_end_matches('/'));
                        path.starts_with(&prefix)
                    }
                })
                .map(|entry| {
                    let icon = if entry.is_dir { icons::FOLDER } else {
                        entry.inner_path.rsplit('.').next()
                            .map(icons::icon_for_extension)
                            .unwrap_or(icons::FILE)
                    };
                    let size_str = if entry.is_dir { "—".to_string() } else {
                        super::helpers::format_bytes(entry.size)
                    };
                    let entry_inner = entry.inner_path.clone();
                    let archive_path_clone = archive.archive_path.clone();
                    let dest_dir = std::env::temp_dir();
                    let on_press = if entry.is_dir {
                        UiMessage::ArchiveFolderOpen { inner_path: entry_inner }
                    } else {
                        UiMessage::ExtractArchiveEntry {
                            archive: archive_path_clone,
                            inner_path: entry_inner,
                            dest_dir,
                        }
                    };
                    button(
                        row![
                            text(icon).size(typography.body).font(typography.body_font),
                            text(entry.name.clone()).size(typography.body).font(typography.body_font),
                            horizontal_space(),
                            text(size_str).size(typography.caption).font(typography.caption_font),
                        ]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center)
                    )
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fill)
                    .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    })
                    .on_press(on_press)
                    .into()
                })
                .collect();

            // Build parent inner_path for the "go up" button
            let parent_inner_path: Option<String> = if archive.inner_path.is_empty() {
                None
            } else {
                let parts: Vec<&str> = archive.inner_path.trim_end_matches('/').split('/').collect();
                if parts.len() <= 1 {
                    Some(String::new())
                } else {
                    Some(parts[..parts.len() - 1].join("/"))
                }
            };
            let mut archive_header_row = row![]
                .spacing(spacing.sm)
                .align_y(Alignment::Center);
            if let Some(parent) = parent_inner_path {
                archive_header_row = archive_header_row.push(
                    button(text("↑").size(typography.body).font(typography.body_font))
                        .on_press(UiMessage::ArchiveFolderOpen { inner_path: parent })
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        }),
                );
            }
            archive_header_row = archive_header_row
                .push(
                    text(format!("{} {} [{}]{}", icons::FILE_ARCHIVE, archive_path_str, match archive.archive_type {
                        super::types::ArchiveType::Zip => "ZIP",
                        super::types::ArchiveType::TarGz => "TAR.GZ",
                        super::types::ArchiveType::SevenZ => "7Z",
                    }, inner_str))
                        .size(typography.body)
                        .font(typography.body_font),
                )
                .push(horizontal_space())
                .push(
                    button(text(icons::CLOSE).size(typography.body).font(typography.body_font))
                        .on_press(UiMessage::CloseArchiveBrowser)
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        }),
                );
            let archive_header = container(archive_header_row)
            .padding([spacing.xs, spacing.sm])
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            });

            container(
                column![
                    archive_header,
                    scrollable(column(archive_entries).spacing(0)).height(Length::Fill),
                ]
                .spacing(spacing.xs)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(10.0).color(colors.border).width(1.0),
                ..Default::default()
            })
            .into()
        } else {
            container(list)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(10.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        };

        // ── Quick Filter bar (Feature E) ──────────────────────────────────────
        let mut list_col = if self.quick_filter_active {
            let filter_bar: Element<'_, UiMessage> = container(
                row![
                    text("Filtrer :").size(typography.caption).font(typography.caption_font),
                    text_input("", &self.quick_filter)
                        .on_input(UiMessage::QuickFilterChanged)
                        .padding(spacing.xs)
                        .width(Length::Fill),
                    button(text("✕").size(typography.caption).font(typography.body_font))
                        .on_press(UiMessage::QuickFilterClear)
                        .padding([spacing.xs, spacing.sm]),
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center)
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.hover)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            })
            .into();
            column![filter_bar, main_list_element]
        } else {
            column![main_list_element]
        };

        // ── Terminal panel (slides up below the file list only) ──────────────
        let term_progress = self.terminal_anim_progress;

        if term_progress > 0.001 {
            let animated_height = TERMINAL_DEFAULT_HEIGHT * term_progress;

            let active_tab = self.terminal.active_ref();
            let output_text: Element<'_, UiMessage> = if active_tab.lines.is_empty() {
                text("Entrez une commande…")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_: &Theme| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
                    .into()
            } else {
                let combined = active_tab.cached_output.as_deref().unwrap_or("");
                text(combined.to_owned())
                    .size(typography.caption)
                    .font(typography.caption_font)
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
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("C:\\")));
            let cwd_display = self
                .terminal
                .effective_cwd(&fallback_cwd)
                .display()
                .to_string();

            let prompt_label = text(format!("{cwd_display} >"))
                .size(typography.caption)
                .font(typography.body_font)
                .style(move |_: &Theme| iced::widget::text::Style {
                    color: Some(colors.accent),
                });

            let input_field = text_input("commande…", &self.terminal.active_ref().input)
                .id(iced::widget::Id::new("terminal_input"))
                .size(typography.caption)
                .font(typography.caption_font)
                .on_input(UiMessage::TerminalInputChanged)
                .on_submit(UiMessage::TerminalInputSubmitted)
                .style(move |_theme: &Theme, _status| iced::widget::text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: border::rounded(0.0).color(Color::TRANSPARENT).width(0.0),
                    icon: colors.text_muted,
                    placeholder: colors.text_muted,
                    value: colors.text_primary,
                    selection: colors.selection,
                });

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
                let tab_btn = button(
                    text(tab_label.clone()).size(typography.caption).font(typography.caption_font)
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                    text_color: if is_active { colors.accent } else { colors.text_muted },
                    background: if is_active { Some(Background::Color(colors.hover)) } else { None },
                    ..Default::default()
                });
                let tab_btn = if is_active {
                    tab_btn
                } else {
                    tab_btn.on_press(UiMessage::TerminalSwitchTab(i))
                };
                tab_bar = tab_bar.push(tab_btn);
                if self.terminal.tabs.len() > 1 {
                    let muted = colors.text_muted;
                    tab_bar = tab_bar.push(
                        button(text("✕").size(typography.caption).font(typography.body_font))
                            .padding([spacing.xs, spacing.xs])
                            .on_press(UiMessage::TerminalCloseTab(i))
                            .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                                text_color: muted,
                                ..Default::default()
                            })
                    );
                }
            }
            let muted_color = colors.text_muted;
            // Feature J: Shell switcher buttons
            use crate::core::ShellConfig;
            let current_shell = &self.state.config.terminal_shell;
            let shells: &[(&str, ShellConfig)] = &[
                ("CMD", ShellConfig::Cmd),
                ("PS", ShellConfig::PowerShell),
                ("Bash", ShellConfig::GitBash),
            ];
            for (label, shell_cfg) in shells {
                let is_active = current_shell == shell_cfg;
                let shell_cfg_clone = shell_cfg.clone();
                tab_bar = tab_bar.push(
                    button(text(*label).size(typography.caption).font(typography.caption_font))
                        .padding([spacing.xs, spacing.xs])
                        .on_press(UiMessage::SetShell(shell_cfg_clone))
                        .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                            text_color: if is_active { colors.accent } else { muted_color },
                            background: if is_active { Some(Background::Color(colors.hover)) } else { None },
                            border: if is_active { border::rounded(4.0).color(colors.accent).width(1.0) } else { border::rounded(4.0).width(0.0) },
                            ..Default::default()
                        })
                );
            }
            tab_bar = tab_bar.push(
                button(text("+").size(typography.caption).font(typography.body_font))
                    .padding([spacing.xs, spacing.sm])
                    .on_press(UiMessage::TerminalAddTab)
                    .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                        text_color: muted_color,
                        ..Default::default()
                    })
            );
            let tab_bar_element: Element<'_, UiMessage> = container(tab_bar.spacing(spacing.xs))
                .width(Length::Fill)
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.sidebar_background)),
                    border: border::rounded(0.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into();

            let term_panel: Element<'_, UiMessage> = container(
                column![tab_bar_element, output_area, input_row].spacing(0),
            )
            .width(Length::Fill)
            .height(Length::Fixed(animated_height))
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(8.0).color(colors.border).width(1.0),
                ..Default::default()
            })
            .into();

            list_col = list_col.push(term_panel);
        }

        let list_col = list_col.width(Length::FillPortion(4)).height(Length::Fill).spacing(spacing.xs);

        let mut body = row![
            sidebar.width(Length::FillPortion(1)),
            list_col,
        ];

        // Feature 11: Dual pane
        if self.dual_pane.enabled {
            if let Some(pane_b) = &self.dual_pane.pane_b {
                let pane_b_path = pane_b.path.display().to_string();
                // Limit rendered entries to avoid lag with large directories (no virtual windowing in pane B)
                let pane_b_entries: Vec<Element<'_, UiMessage>> = pane_b.entries.iter().take(200).map(|entry| {
                    let icon = match entry.entry_type {
                        FsEntryType::Directory => icons::FOLDER,
                        FsEntryType::File => entry.path.extension()
                            .and_then(|e| e.to_str())
                            .map(icons::icon_for_extension)
                            .unwrap_or(icons::FILE),
                        FsEntryType::Symlink => icons::SYMLINK,
                        FsEntryType::Other => icons::UNKNOWN,
                    };
                    let activate_path = entry.path.clone();
                    button(
                        row![
                            text(icon).size(typography.body).font(typography.body_font),
                            text(entry.name.clone()).size(typography.body).font(typography.body_font),
                        ]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center)
                    )
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fill)
                    .style(move |_: &Theme, status: ButtonStatus| iced::widget::button::Style {
                        text_color: colors.text_primary,
                        background: if matches!(status, ButtonStatus::Hovered) {
                            Some(Background::Color(colors.hover))
                        } else { None },
                        ..Default::default()
                    })
                    .on_press(UiMessage::PaneBActivate(activate_path))
                    .into()
                }).collect();

                let pane_b_header = container(
                    row![
                        text(icons::FOLDER).size(typography.caption).font(typography.body_font),
                        text(pane_b_path).size(typography.caption).font(typography.caption_font),
                        horizontal_space(),
                        button(text("✕").size(typography.caption).font(typography.body_font))
                            .on_press(UiMessage::ToggleDualPane)
                            .padding([spacing.xs, spacing.sm])
                            .style(move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
                                text_color: colors.text_muted,
                                ..Default::default()
                            }),
                    ]
                    .spacing(spacing.sm)
                    .align_y(Alignment::Center)
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.chrome_background)),
                    border: border::rounded(6.0).color(colors.border).width(1.0),
                    ..Default::default()
                });

                let pane_b_list: Element<'_, UiMessage> = if pane_b.is_loading {
                    container(
                        text("Chargement…").size(typography.caption).font(typography.caption_font)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    scrollable(column(pane_b_entries).spacing(0)).height(Length::Fill).into()
                };
                let pane_b_panel = container(
                    column![pane_b_header, pane_b_list].spacing(spacing.xs)
                )
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(10.0).color(colors.border).width(1.0),
                    ..Default::default()
                });

                // Divider between panes
                let pane_divider = container(row![])
                    .width(Length::Fixed(2.0))
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.border)),
                        ..Default::default()
                    });

                body = body.push(pane_divider);
                body = body.push(pane_b_panel);
            }
        }

        if preview_visible {
            let animated_width = self.pane_resize.preview_width * preview_progress;

            let preview_resize_bar: Element<'_, UiMessage> = mouse_area(
                container(row![])
                    .width(Length::Fixed(PREVIEW_RESIZE_BAR_WIDTH))
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: if self.pane_resize.preview_resizing {
                            Some(Background::Color(colors.hover))
                        } else {
                            None
                        },
                        border: if self.pane_resize.preview_resizing {
                            border::rounded(6.0).color(colors.border).width(1.0)
                        } else {
                            border::rounded(6.0).width(0.0)
                        },
                        ..Default::default()
                    }),
            )
            .on_press(UiMessage::PreviewResizeStart)
            .on_release(UiMessage::PreviewResizeEnd)
            .interaction(mouse::Interaction::ResizingHorizontally)
            .into();

            let preview_header = mouse_area(
                text("Prévisualisation")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style { color: Some(colors.text_muted) }),
            )
            .on_press(UiMessage::Noop) // absorb single click
            .on_double_click(UiMessage::PreviewDoubleClick);

            let preview_panel = container(
                column![
                    preview_header,
                    preview_body
                ]
                .spacing(spacing.md),
            )
            .padding(spacing.md)
            .width(Length::Fixed(animated_width))
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(10.0).color(colors.border).width(1.0),
                ..Default::default()
            });

            body = body.push(preview_resize_bar);
            body = body.push(preview_panel);
        }


        let body = body.height(Length::Fill).spacing(spacing.xs);

        let selection = &self.state.navigation.selection;

        // Count folders and files separately for status bar
        let (dir_count, file_count) = {
            let items_iter: Box<dyn Iterator<Item = &FsEntry>> = if let Some(indices) = &filtered_indices {
                Box::new(indices.iter().filter_map(|&i| display_entries.get(i)))
            } else {
                Box::new(display_entries.items.iter().flatten())
            };
            let mut dirs = 0usize;
            let mut files = 0usize;
            for entry in items_iter {
                match entry.entry_type {
                    FsEntryType::Directory => dirs += 1,
                    _ => files += 1,
                }
            }
            (dirs, files)
        };
        let entry_count_label = {
            let base = match (dir_count, file_count) {
                (0, 0) => "aucun élément".to_string(),
                (d, 0) => if d == 1 { "1 dossier".to_string() } else { format!("{d} dossiers") },
                (0, f) => if f == 1 { "1 fichier".to_string() } else { format!("{f} fichiers") },
                (d, f) => {
                    let d_label = if d == 1 { "1 dossier".to_string() } else { format!("{d} dossiers") };
                    let f_label = if f == 1 { "1 fichier".to_string() } else { format!("{f} fichiers") };
                    format!("{d_label}, {f_label}")
                }
            };
            if self.quick_filter_active && !self.quick_filter.is_empty() {
                format!("{base} (filtre: \"{}\")", self.quick_filter)
            } else {
                base
            }
        };

        let selection_part = if selection.selected.is_empty() {
            String::new()
        } else {
            let selected_size: u64 = selection
                .selected
                .iter()
                .filter_map(|path| {
                    display_entries
                        .items
                        .iter()
                        .flatten()
                        .find(|e| &e.path == path)
                })
                .filter(|e| e.entry_type != FsEntryType::Directory)
                .map(|e| e.metadata.size)
                .sum();

            let size_suffix = if selected_size > 0 {
                format!(" ({})", super::helpers::format_bytes(selected_size))
            } else {
                String::new()
            };

            if selection.selected.len() == 1 {
                let name = selection
                    .selected
                    .iter()
                    .next()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "—".to_string());
                format!("Sélection : {}{} — ", name, size_suffix)
            } else {
                format!(
                    "Sélection : {} fichiers{} — ",
                    selection.selected.len(),
                    size_suffix
                )
            }
        };

        let index_status = if self.is_loading {
            "Chargement…".to_string()
        } else if self.search.indexing {
            "Indexation…".to_string()
        } else if let Some(count) = self.search.matches {
            format!("Résultats indexés : {count}")
        } else {
            entry_count_label.clone()
        };

        let status_text = format!("{}{}", selection_part, index_status);

        let status_left = row![
            text(status_text)
                .size(typography.caption)
                .font(typography.caption_font)
        ]
        .spacing(spacing.md)
        .align_y(Alignment::Center);

        let is_list = matches!(view_mode, ViewMode::List);
        let is_grid = matches!(view_mode, ViewMode::Grid);

        let view_button = |icon: String, active: bool| {
            button(
                text(icon)
                    .size(typography.caption)
                    .font(typography.body_font),
            )
            .padding([spacing.xs, spacing.xs])
            .style(move |_theme: &Theme, status: ButtonStatus| {
                let mut style = iced::widget::button::Style {
                    text_color: if active { colors.accent } else { colors.text_muted },
                    ..Default::default()
                };
                if active {
                    style.background = Some(Background::Color(colors.selection));
                    style.border = border::rounded(4.0).color(colors.selection_border).width(1.0);
                }
                if matches!(status, ButtonStatus::Hovered) {
                    style.background = Some(Background::Color(colors.hover));
                }
                style
            })
            .on_press(UiMessage::ToggleViewMode)
        };

        let terminal_active = self.terminal_anim_target > 0.5;
        let terminal_btn = button(
            text(icons::TERMINAL.to_string())
                .size(typography.caption)
                .font(typography.body_font),
        )
        .padding([spacing.xs, spacing.xs])
        .style(move |_theme: &Theme, status: ButtonStatus| {
            let mut style = iced::widget::button::Style {
                text_color: if terminal_active { colors.accent } else { colors.text_muted },
                ..Default::default()
            };
            if terminal_active {
                style.background = Some(Background::Color(colors.selection));
                style.border = border::rounded(4.0).color(colors.selection_border).width(1.0);
            }
            if matches!(status, ButtonStatus::Hovered) {
                style.background = Some(Background::Color(colors.hover));
            }
            style
        })
        .on_press(UiMessage::ToggleTerminal);

        let mut status_right = row![
            terminal_btn,
            view_button(icons::VIEW_LIST.to_string(), is_list),
            view_button(icons::VIEW_GRID.to_string(), is_grid),
        ]
        .spacing(spacing.xs)
        .align_y(Alignment::Center);

        if let Some(status) = self.last_action.clone() {
            status_right = status_right.push(
                text(status)
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        }

        let status_bar = container(
            row![status_left, horizontal_space(), status_right].align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(8.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let content = column![header, body, status_bar]
            .spacing(spacing.md)
            .padding(spacing.lg)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        let base: Element<'_, UiMessage> = container(content)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                ..Default::default()
            })
            .into();

        let drag_overlay: Option<Element<'_, UiMessage>> = self
            .drag_state
            .as_ref()
            .and_then(|drag_state| self.cursor_position.map(|position| (drag_state, position)))
            .map(|(drag_state, position)| {
                let count = drag_state.items.len();
                let action = if self.modifiers.control {
                    "Copier"
                } else {
                    "Déplacer"
                };
                let label = if count == 1 {
                    format!("{action} 1 élément")
                } else {
                    format!("{action} {count} éléments")
                };
                let overlay = container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font),
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(999.0).color(colors.border).width(1.0),
                    ..Default::default()
                });
                let position_x = (position.x + spacing.md).max(0.0);
                let position_y = (position.y + spacing.md).max(0.0);
                let layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(overlay)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                layer
            });

        let selection_overlay: Option<Element<'_, UiMessage>> = self
            .selection_box_rect()
            .and_then(|rect| self.list_viewport_bounds.map(|bounds| (rect, bounds)))
            .map(|(rect, bounds)| {
                let selection_x = (bounds.x + rect.x).max(0.0);
                let selection_y = (bounds.y + rect.y - self.scroll.offset).max(0.0);
                let selection_width = rect.width.max(0.0);
                let selection_height = rect.height.max(0.0);
                if selection_width == 0.0 || selection_height == 0.0 {
                    return container(row![]).into();
                }
                let fill = Color {
                    a: 0.2,
                    ..colors.accent
                };
                let outline = container(row![])
                    .width(Length::Fixed(selection_width))
                    .height(Length::Fixed(selection_height))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(fill)),
                        border: border::rounded(2.0).color(colors.accent).width(1.0),
                        ..Default::default()
                    });
                container(
                    column![
                        vertical_space().height(Length::Fixed(selection_y)),
                        row![
                            horizontal_space().width(Length::Fixed(selection_x)),
                            opaque(outline)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            });

        let selection_layer: Element<'_, UiMessage> =
            selection_overlay.unwrap_or_else(|| container(row![]).into());
        let drag_layer: Element<'_, UiMessage> =
            drag_overlay.unwrap_or_else(|| container(row![]).into());
        let history_layer: Element<'_, UiMessage> =
            history_menu.unwrap_or_else(|| container(row![]).into());
        let context_layer: Element<'_, UiMessage> =
            context_menu.unwrap_or_else(|| container(row![]).into());

        // Feature R: Breadcrumb dropdown
        let breadcrumb_dropdown_layer: Element<'_, UiMessage> =
            if self.breadcrumb_dropdown.is_some() {
                let subdirs = &self.breadcrumb_dropdown_items;
                if subdirs.is_empty() {
                    container(row![]).into()
                } else {
                    let mut items_col = column![].spacing(spacing.xs);
                    for dir in subdirs {
                        let label = dir
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let dir_clone = dir.clone();
                        items_col = items_col.push(
                            button(
                                text(label)
                                    .size(typography.body)
                                    .font(typography.body_font),
                            )
                            .width(Length::Fill)
                            .padding([spacing.xs, spacing.sm])
                            .style(move |_theme: &Theme, status: ButtonStatus| {
                                let mut style = iced::widget::button::Style {
                                    text_color: colors.text_primary,
                                    ..Default::default()
                                };
                                if matches!(status, ButtonStatus::Hovered) {
                                    style.background =
                                        Some(Background::Color(colors.hover));
                                }
                                style
                            })
                            .on_press(UiMessage::NavigateTo(dir_clone)),
                        );
                    }
                    if self.breadcrumb_dropdown_has_more {
                        items_col = items_col.push(
                            text("\u{2026}")  // "…"
                                .size(typography.caption)
                                .font(typography.caption_font)
                                .color(colors.text_muted),
                        );
                    }
                    let menu = container(
                        scrollable(items_col).height(Length::Shrink),
                    )
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fixed(280.0))
                    .max_height(400.0)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    });
                    // Position below the header area
                    let menu_layer: Element<'_, UiMessage> = container(
                        column![
                            vertical_space().height(Length::Fixed(90.0)),
                            row![
                                horizontal_space().width(Length::Fixed(120.0)),
                                opaque(menu)
                            ]
                        ]
                        .spacing(0),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
                    let dismiss_layer: Element<'_, UiMessage> =
                        mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                            .on_press(UiMessage::CloseBreadcrumbDropdown)
                            .into();
                    stack![dismiss_layer, menu_layer].into()
                }
            } else {
                container(row![]).into()
            };

        // Feature 3, 5, C, K, O, P: overlay modals (see view/overlays.rs)
        let properties_layer  = self.render_properties_layer(colors, spacing, typography);
        let bulk_rename_layer = self.render_bulk_rename_layer(colors, spacing, typography);
        let diff_layer        = self.render_diff_layer(colors, spacing, typography);
        let hex_layer         = self.render_hex_layer(colors, spacing, typography);
        let grep_layer        = self.render_grep_layer(colors, spacing, typography);
        let permissions_layer = self.render_permissions_layer(colors, spacing, typography);

        stack![base, selection_layer, drag_layer, breadcrumb_dropdown_layer, history_layer, context_layer, properties_layer, bulk_rename_layer, diff_layer, hex_layer, grep_layer, permissions_layer].into()
    }
}
