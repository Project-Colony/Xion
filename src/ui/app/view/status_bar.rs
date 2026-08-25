//! The status bar: selection summary, entry counts, and the right-hand
//! indicators.
//!
//! Moved out of `view()` unchanged.

use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, container, row};
use iced::{Alignment, Element, Length};

use crate::core::ViewMode;
use crate::filesystem::{FsEntry, FsEntryType};
use crate::ui::UiMessage;
use crate::ui::theme::icons;

use super::XionApp;
use super::widgets::ViewCtx;
use crate::ui::app::types::*;

use super::widgets::RADIUS;
use super::widgets::caption_text;
use super::widgets::glyph_text;
use super::widgets::surface_style;
use super::widgets::toggle_button_style;

/// Côté des bascules de la barre d'état.
///
/// Carrées à dessein : trois icônes côte à côte de largeurs différentes se
/// lisent comme trois objets sans rapport. La taille laisse de la marge autour
/// du glyphe le plus large sans que le bouton ne domine la barre.
const TOGGLE_SIZE: f32 = 26.0;

/// De combien l'icône est ramenée vers la gauche dans sa bascule.
///
/// Compense le dessin de la police, pas un défaut de mise en page : les glyphes
/// de la fonte d'icônes sont posés à droite de leur chasse, donc une boîte
/// parfaitement centrée montre quand même une icône décalée. Si Xion change de
/// fonte, ce nombre n'a plus de raison d'être — et se verra.
const ICON_LEFT_SHIFT: f32 = 2.0;

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

        let status_left = row![caption_text(typography, status_text)]
            .spacing(spacing.md)
            .align_y(Alignment::Center);

        let is_list = matches!(view_mode, ViewMode::List);
        let is_grid = matches!(view_mode, ViewMode::Grid);

        // Un carré, et le glyphe centré dedans.
        //
        // Le bouton se dimensionnait sur la boîte du glyphe, et les glyphes Nerd
        // Font ont une chasse large dont l'encre n'occupe pas le centre : les
        // boîtes sortaient rectangulaires et les icônes décalées, chacune
        // différemment selon le dessin de son glyphe.
        let toggle_face = |icon: String| {
            let glyph = glyph_text(typography, icon)
                // La hauteur de ligne vaut celle de la boîte : c'est là que se
                // joue le centrage vertical.
                //
                // Par défaut elle vaut 1,3 fois la taille du texte, donc la
                // boîte réserve une ascendante et une descendante que l'icône
                // n'occupe pas — centrer cette boîte pousse le glyphe vers le
                // bas. Sans interligne à répartir, il ne reste que le glyphe.
                .line_height(iced::widget::text::LineHeight::Absolute(TOGGLE_SIZE.into()))
                .center();

            // Et une compensation horizontale, parce que centrer ne suffit pas.
            //
            // Ces glyphes sont dessinés à droite de leur chasse : la boîte est
            // bien centrée, l'encre ne l'est pas. Le rembourrage à droite
            // rétrécit l'aire dans laquelle le glyphe se centre, ce qui le
            // ramène vers la gauche de la moitié de sa valeur — sans changer la
            // taille du bouton, qui reste carré.
            container(glyph)
                .width(Length::Fixed(TOGGLE_SIZE))
                .height(Length::Fixed(TOGGLE_SIZE))
                .padding(iced::Padding {
                    right: ICON_LEFT_SHIFT * 2.0,
                    ..iced::Padding::ZERO
                })
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
        };

        let view_button = |icon: String, active: bool| {
            button(toggle_face(icon))
                .padding(0)
                .style(toggle_button_style(colors, active))
                .on_press(UiMessage::ToggleViewMode)
        };

        let terminal_active = self.terminal_anim_target > 0.5;
        let terminal_btn = button(toggle_face(icons::TERMINAL.to_string()))
            .padding(0)
            .style(toggle_button_style(colors, terminal_active))
            .on_press(UiMessage::ToggleTerminal);

        let mut status_right = row![
            terminal_btn,
            view_button(icons::VIEW_LIST.to_string(), is_list),
            view_button(icons::VIEW_GRID.to_string(), is_grid),
        ]
        .spacing(spacing.xs)
        .align_y(Alignment::Center);

        if let Some(status) = self.last_action.clone() {
            status_right = status_right.push(caption_text(typography, status));
        }

        let status_bar = container(
            row![status_left, horizontal_space(), status_right].align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .style(surface_style(colors, RADIUS.lg));

        status_bar.into()
    }
}
