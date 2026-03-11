//! View rendering for XionApp.
//!
//! Contains the [`XionApp::view`] method which builds the entire widget tree.

use std::path::PathBuf;

use iced::widget::button::Status as ButtonStatus;
#[allow(unused_imports)]
use tracing::{debug, info, warn};
use iced::widget::{
    button, column, container, image, mouse_area, opaque, progress_bar, row,
    scrollable, stack, text, text_input,
};
use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Theme,
    border,
};

use crate::core::{SortOrderConfig, ViewColumn, ViewMode};
use crate::filesystem::{FsEntry, FsEntryType};
use crate::ui::{
    ContextAction, NETWORK_ROUTE, ScrollViewport, UiMessage,
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
    format_entry_size, format_modified,
};

impl XionApp {
    pub(super) fn view(&self) -> Element<'_, UiMessage> {
        let tokens = UiTokens::for_mode(self.state.config.dark_mode);
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

        let section_title = |label: String| {
            text(label)
                .size(typography.caption)
                .font(typography.caption_font)
                .style(move |_| iced::widget::text::Style {
                    color: Some(colors.text_muted),
                })
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

        let back_button = if self.history.can_back() {
            toolbar_button(icons::BACK.to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button(icons::BACK.to_string())
        };

        let forward_button = if self.history.can_forward() {
            toolbar_button(icons::FORWARD.to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button(icons::FORWARD.to_string())
        };

        let refresh_button = toolbar_button(icons::REFRESH.to_string()).on_press(UiMessage::Refresh);

        let navigation = row![back_button, forward_button, refresh_button].spacing(spacing.sm);

        let mut tabs = row![];
        for (index, tab) in self.tabs.iter().enumerate() {
            let label = format!("{} {}", icons::PC, tab.title);
            let mut button = tab_button(label, index == self.active_tab);
            if index != self.active_tab {
                button = button.on_press(UiMessage::SwitchTab(index));
            }
            let mut tab_row = row![button].spacing(spacing.xs).align_y(Alignment::Center);
            if index != 0 {
                tab_row = tab_row.push(
                    tab_button(icons::CLOSE.to_string(), false).on_press(UiMessage::CloseTab(index)),
                );
            }
            tabs = tabs.push(tab_row);
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
                .padding([spacing.xs, spacing.md]);

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
                if i == 0 && segment.ends_with(':') {
                    // Drive root, e.g. "C:"
                    accumulated = format!("{}\\", segment);
                } else {
                    accumulated = format!("{}{}\\", accumulated, segment);
                }

                if i > 0 {
                    breadcrumb_row = breadcrumb_row.push(
                        text(" ❯ ")
                            .size(typography.caption)
                            .font(typography.caption_font)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(colors.text_muted),
                            }),
                    );
                }

                let target = PathBuf::from(&accumulated);
                let is_last = i == segments.len() - 1;
                let label = segment.to_string();

                if is_last {
                    breadcrumb_row = breadcrumb_row.push(
                        text(label)
                            .size(typography.body)
                            .font(typography.body_font),
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
                        AddressValidation::Directory => ("Dossier".to_string(), Color::from_rgb8(55, 125, 60)),
                        AddressValidation::File => ("Fichier".to_string(), Color::from_rgb8(186, 120, 40)),
                        AddressValidation::NotFound => ("Introuvable".to_string(), Color::from_rgb8(176, 72, 72)),
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
            .on_press(UiMessage::ToggleHistoryMenu(!self.history_menu_open));

        let history_menu: Option<Element<'_, UiMessage>> =
            if self.history_menu_open && !address_suggestions.is_empty() {
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
                let position = self.history_menu_position.unwrap_or(Point::ORIGIN);
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
            .tabs
            .get(self.active_tab)
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

        let command_bar = row![
            toolbar_button(format!("{} Nouveau", icons::NEW)).on_press(UiMessage::NewFolder),
            cut_button,
            copy_button,
            paste_button,
            toolbar_button(dark_mode_label).on_press(UiMessage::ToggleDarkMode),
            toolbar_button(format!("{} Actions", icons::ACTIONS))
                .on_press(UiMessage::ToggleContextMenu(!self.context_menu_open))
        ]
        .spacing(spacing.sm);

        let loading_badge: Element<'_, UiMessage> = if self.show_loading_indicator {
            let content: Element<'_, UiMessage> = row![
                text("Chargement…")
                    .size(typography.caption)
                    .font(typography.caption_font),
                progress_bar(0.0..=1.0, 0.5)
                    .girth(Length::Fixed(4.0))
                    .length(Length::Fixed(64.0)),
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

        let context_actions = column![
            toolbar_button(format!("{} Ouvrir", icons::OPEN))
                .on_press(UiMessage::ContextAction(ContextAction::Open,)),
            toolbar_button(format!("{} Renommer", icons::RENAME))
                .on_press(UiMessage::ContextAction(ContextAction::Rename,)),
            toolbar_button(format!("{} Supprimer", icons::DELETE))
                .on_press(UiMessage::ContextAction(ContextAction::Delete,)),
            toolbar_button(format!("{} Copier le chemin", icons::SYMLINK))
                .on_press(UiMessage::ContextAction(ContextAction::CopyPath),)
        ]
        .spacing(spacing.sm);

        let context_menu: Option<Element<'_, UiMessage>> =
            if self.context_menu_open && !self.state.navigation.selection.selected.is_empty() {
                let position = self.context_menu_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);
                let menu = container(context_actions)
                    .padding([spacing.sm, spacing.md])
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
                FsEntryType::File => self
                    .media.thumbnail_handles
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
                    }),
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
                        let name_row = row![
                            entry_leading(entry),
                            text(entry.name.clone())
                                .size(typography.body)
                                .font(typography.body_font),
                            horizontal_space()
                        ]
                        .spacing(spacing.sm)
                        .align_y(Alignment::Center);
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
                    ViewColumn::Size => container(
                        text(format_entry_size(entry))
                            .size(typography.caption)
                            .font(typography.caption_font),
                    )
                    .width(spec.width)
                    .align_x(spec.align)
                    .into(),
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
            let tile_content = column![
                entry_leading(entry),
                text(entry.name.clone())
                    .size(typography.caption)
                    .font(typography.caption_font),
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

        let list = mouse_area(
            scrollable(container(list_column).padding(spacing.md)).on_scroll(|viewport| {
                UiMessage::Scroll(ScrollViewport {
                    offset_y: viewport.absolute_offset().y,
                    viewport_height: viewport.bounds().height,
                    content_height: viewport.content_bounds().height,
                    bounds: viewport.bounds(),
                })
            }),
        )
        .on_press(UiMessage::ListBackgroundPressed);

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
        let tree_panel = column![
            section_title("Arborescence".to_string()),
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
        .spacing(spacing.xs);

        let tree_resize_bar: Element<'_, UiMessage> = mouse_area(
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
        .into();

        // ── Quick Access with user folders ──────────────────────────────
        let mut quick_access =
            column![section_title("Accès rapide".to_string())].spacing(spacing.xs);

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

        for (icon, label, path) in &user_folder_entries {
            if path.is_some() {
                quick_access = quick_access.push(sidebar_button(icon, label, path.clone()));
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
            column![section_title("Favoris".to_string())].spacing(spacing.xs);
        if custom_favorites.is_empty() {
            favorites_section = favorites_section.push(
                text("Aucun favori")
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        } else {
            for favorite in custom_favorites {
                let label = format_sidebar_label(favorite);
                favorites_section = favorites_section.push(sidebar_button(
                    icons::FOLDER,
                    &label,
                    Some(favorite.clone()),
                ));
            }
        }

        // ── All drives ───────────────────────────────────────────────────
        let mut drive_section = column![section_title("Lecteurs".to_string())].spacing(spacing.xs);
        let drives = all_drives();
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

        let sidebar = container(
            column![
                tree_panel,
                tree_resize_bar,
                quick_access,
                favorites_section,
                drive_section,
                section_title("Raccourcis".to_string())
            ]
            .spacing(spacing.sm),
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

                // Text preview for code/text files
                let text_preview_section: Element<'_, UiMessage> = self
                    .cached_text_preview
                    .as_ref()
                    .filter(|(p, _)| p == &entry.path)
                    .map(|(_, content)| {
                        let preview_text = container(
                            scrollable(
                                container(
                                    text(content)
                                        .size(typography.caption)
                                        .font(typography.body_font),
                                )
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

                column![
                    preview_media,
                    text(&entry.name)
                        .size(typography.body)
                        .font(typography.body_font),
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

        // ── Terminal panel (slides up below the file list only) ──────────────
        let term_progress = self.terminal_anim_progress;
        let mut list_col = column![
            container(list)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(10.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
        ];

        if term_progress > 0.001 {
            let animated_height = TERMINAL_DEFAULT_HEIGHT * term_progress;

            let output_text: Element<'_, UiMessage> = if self.terminal.lines.is_empty() {
                text("Entrez une commande…")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_: &Theme| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
                    .into()
            } else {
                let combined = self.terminal.lines.join("\n");
                text(combined)
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

            let fallback_path = std::path::PathBuf::from(".");
            let fallback_cwd = self
                .state
                .route
                .local_path()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| fallback_path.clone());
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

            let input_field = text_input("commande…", &self.terminal.input)
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

            let term_panel: Element<'_, UiMessage> = container(
                column![output_area, input_row].spacing(0),
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

        let list_col = list_col.width(Length::Fill).height(Length::Fill).spacing(spacing.xs);

        let mut body = row![
            sidebar.width(Length::Fixed(220.0)),
            list_col,
        ];

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
            .into();

            let preview_panel = container(
                column![section_title("Prévisualisation".to_string()), preview_body]
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
        let entry_count_label = match (dir_count, file_count) {
            (0, 0) => "aucun élément".to_string(),
            (d, 0) => if d == 1 { "1 dossier".to_string() } else { format!("{d} dossiers") },
            (0, f) => if f == 1 { "1 fichier".to_string() } else { format!("{f} fichiers") },
            (d, f) => {
                let d_label = if d == 1 { "1 dossier".to_string() } else { format!("{d} dossiers") };
                let f_label = if f == 1 { "1 fichier".to_string() } else { format!("{f} fichiers") };
                format!("{d_label}, {f_label}")
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

        stack![base, selection_layer, drag_layer, history_layer, context_layer].into()
    }
}
