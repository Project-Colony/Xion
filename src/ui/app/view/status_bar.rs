//! The status bar: selection summary, entry counts, and the right-hand
//! indicators.
//!
//! Moved out of `view()` unchanged.

use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Element, Theme, border};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::core::ViewMode;
use crate::filesystem::{FsEntry, FsEntryType};
use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;
use crate::ui::app::types::*;

use super::widgets::RADIUS;

impl XionApp {
    /// `display_entries` and `filtered_indices` are resolved once in `view()`
    /// and passed in rather than recomputed: both are O(n) over the directory.
    pub(super) fn render_status_bar<'a>(
        &'a self,
        ctx: ViewCtx,
        display_entries: &'a PagedEntries,
        filtered_indices: &Option<Vec<usize>>,
    ) -> Element<'a, UiMessage> {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let view_mode = self.state.config.view.mode;

        let selection = &self.state.navigation.selection;

        // Folder/file counts and the size of the selection, in a single pass.
        // These used to be two independent walks of every loaded entry — plus a
        // boxed trait-object iterator whose per-item dynamic dispatch defeated
        // any chance of the counting loop being vectorised — repeated on every
        // rebuild of the widget tree.
        let want_selection_size = !selection.selected.is_empty();
        // The counts describe the filtered view, but the selection may hold
        // entries a filter is currently hiding and its reported size has to
        // keep covering all of them — so the two only share a pass when no
        // filter is active, which is the usual case.
        let fuse_selection = want_selection_size && filtered_indices.is_none();
        let (dir_count, file_count, mut selected_size) = {
            let mut dirs = 0usize;
            let mut files = 0usize;
            let mut size = 0u64;
            {
                let mut tally = |entry: &FsEntry| {
                    if entry.entry_type == FsEntryType::Directory {
                        dirs += 1;
                    } else {
                        files += 1;
                        if fuse_selection && selection.selected.contains(&entry.path) {
                            size += entry.metadata.size;
                        }
                    }
                };
                match &filtered_indices {
                    Some(indices) => indices
                        .iter()
                        .filter_map(|&i| display_entries.get(i))
                        .for_each(&mut tally),
                    None => display_entries.items.iter().flatten().for_each(&mut tally),
                }
            }
            (dirs, files, size)
        };
        if want_selection_size && !fuse_selection {
            selected_size = display_entries
                .items
                .iter()
                .flatten()
                .filter(|entry| entry.entry_type != FsEntryType::Directory)
                .filter(|entry| selection.selected.contains(&entry.path))
                .map(|entry| entry.metadata.size)
                .sum();
        }
        let entry_count_label = {
            let base = match (dir_count, file_count) {
                (0, 0) => "aucun élément".to_string(),
                (d, 0) => {
                    if d == 1 {
                        "1 dossier".to_string()
                    } else {
                        format!("{d} dossiers")
                    }
                }
                (0, f) => {
                    if f == 1 {
                        "1 fichier".to_string()
                    } else {
                        format!("{f} fichiers")
                    }
                }
                (d, f) => {
                    let d_label = if d == 1 {
                        "1 dossier".to_string()
                    } else {
                        format!("{d} dossiers")
                    };
                    let f_label = if f == 1 {
                        "1 fichier".to_string()
                    } else {
                        format!("{f} fichiers")
                    };
                    format!("{d_label}, {f_label}")
                }
            };
            if self.quick_filter_active && !self.quick_filter.is_empty() {
                format!("{base} (filtre: \"{}\")", self.quick_filter)
            } else {
                base
            }
        };

        let selection_part = if !want_selection_size {
            String::new()
        } else {
            let size_suffix = if selected_size > 0 {
                format!(
                    " ({})",
                    crate::ui::app::helpers::format_bytes(selected_size)
                )
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
                    text_color: if active {
                        colors.accent
                    } else {
                        colors.text_muted
                    },
                    ..Default::default()
                };
                if active {
                    style.background = Some(Background::Color(colors.selection));
                    style.border = border::rounded(RADIUS.sm)
                        .color(colors.selection_border)
                        .width(1.0);
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
                text_color: if terminal_active {
                    colors.accent
                } else {
                    colors.text_muted
                },
                ..Default::default()
            };
            if terminal_active {
                style.background = Some(Background::Color(colors.selection));
                style.border = border::rounded(RADIUS.sm)
                    .color(colors.selection_border)
                    .width(1.0);
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
            border: border::rounded(RADIUS.lg).color(colors.border).width(1.0),
            ..Default::default()
        });

        status_bar.into()
    }
}
