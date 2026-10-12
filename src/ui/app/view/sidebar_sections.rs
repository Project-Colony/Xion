//! The four sidebar sections: tree, quick access, favourites and drives.
//!
//! Moved out of `render_sidebar`, which now only stacks what these return.
//! The three shared helpers live here too, since nothing else uses them.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use colony_ui::widgets::selection_style;
use iced::widget::button::Status as ButtonStatus;
use iced::widget::space::{horizontal as horizontal_space, vertical as vertical_space};
use iced::widget::{button, column, container, mouse_area, progress_bar, row, scrollable};
use iced::{Alignment, Element, Length, Theme, mouse};

use crate::i18n::{t, tf};
use crate::ui::theme::icons;
use crate::ui::theme::layout::TREE_RESIZE_BAR_HEIGHT;
use crate::ui::{NETWORK_ROUTE, ScrollViewport, UiMessage};

use super::XionApp;
use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use super::widgets::body_text;
use super::widgets::caption_text;
use super::widgets::filled_style;
use super::widgets::glyph_text;
use super::widgets::icon_text;
use crate::ui::app::types::*;

// ── Shared helpers ──────────────────────────────────────────────────────────
//
// These were closures inside each section, which meant three copies of
// `section_header` and two of `sidebar_button`. None of them touches `self`,
// so they are plain functions.

/// One clickable sidebar entry, filled with the accent while `current` is the
/// place it leads to.
pub(super) fn sidebar_button<'a>(
    ctx: ViewCtx,
    icon: &'a str,
    label: impl Into<Cow<'a, str>>,
    target: Option<PathBuf>,
    current: &Path,
) -> Element<'a, UiMessage> {
    let label: Cow<'a, str> = label.into();
    let ViewCtx {
        colors: _,
        spacing,
        typography,
    } = ctx;
    let content: Element<'_, UiMessage> =
        row![icon_text(typography, icon), body_text(typography, label)]
            .spacing(spacing.sm)
            .align_y(Alignment::Center)
            .into();

    match target {
        Some(path) => button(content)
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(selection_style(path == current))
            .on_press(UiMessage::NavigateTo(path))
            .into(),
        None => container(content)
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .into(),
    }
}

/// A collapsible section header with its chevron.
///
/// `key` is both the section's identity in `sidebar_collapsed` and the i18n
/// key of its title, so switching languages does not reopen every section.
pub(super) fn section_header<'a>(
    ctx: ViewCtx,
    key: &'static str,
    collapsed: bool,
) -> Element<'a, UiMessage> {
    let ViewCtx {
        colors,
        spacing,
        typography,
    } = ctx;
    let chevron = if collapsed { "▸" } else { "▾" };
    button(
        row![
            caption_text(typography, chevron),
            caption_text(typography, t(key)).style(move |_| iced::widget::text::Style {
                color: Some(colors.text_muted)
            }),
        ]
        .spacing(spacing.xs)
        .align_y(Alignment::Center),
    )
    .padding([2.0, 0.0])
    .width(Length::Fill)
    .style(
        move |_: &Theme, _: ButtonStatus| iced::widget::button::Style {
            ..Default::default()
        },
    )
    .on_press(UiMessage::ToggleSidebarSection(key))
    .into()
}

/// Shorten a path for display in the sidebar.
pub(super) fn format_sidebar_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|label| !label.is_empty())
        .map(|label| label.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

impl XionApp {
    /// The navigable folder tree, plus the bar that resizes it.
    pub(super) fn sidebar_tree(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Element<'_, UiMessage>) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;

        // Clickable accordion header: shows ▸/▾ chevron and toggles the section.

        let sec_tree = self.sidebar_collapsed.contains("sidebar_tree");

        let tree_nodes = &self.cached_tree_nodes;

        let mut tree_section = column![].spacing(spacing.xs);
        if tree_nodes.is_empty() {
            tree_section = tree_section.push(
                caption_text(typography, t("sidebar_tree_unavailable")).style(move |_| {
                    iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    }
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
                    caption_text(typography, chevron),
                    icon_text(typography, icon),
                    body_text(typography, label)
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center)
                .into();

                let drop_path = path.clone();
                let button = mouse_area(
                    button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(selection_style(selected))
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
            column![section_header(ctx, "sidebar_tree", true)].spacing(spacing.xs)
        } else {
            column![
                section_header(ctx, "sidebar_tree", false),
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
                    .style(filled_style(colors, colors.hover, RADIUS.md)),
            )
            .on_press(UiMessage::TreeResizeStart)
            .on_release(UiMessage::TreeResizeEnd)
            .interaction(mouse::Interaction::ResizingVertically)
            .into()
        };

        (tree_panel.into(), tree_resize_bar)
    }

    /// Quick access shortcuts and the user's pinned favourites.
    pub(super) fn sidebar_quick_access(
        &self,
        ctx: ViewCtx,
    ) -> (Element<'_, UiMessage>, Element<'_, UiMessage>) {
        let ViewCtx {
            colors,
            spacing,
            typography,
        } = ctx;
        let home_dir = self.cached_home_dir.clone();
        let current = self.state.route.key();

        // Clickable accordion header: shows ▸/▾ chevron and toggles the section.

        let sec_access = self.sidebar_collapsed.contains("sidebar_quick_access");
        let sec_favorites = self.sidebar_collapsed.contains("sidebar_favorites");

        // ── Quick Access with user folders ──────────────────────────────
        let mut quick_access =
            column![section_header(ctx, "sidebar_quick_access", sec_access)].spacing(spacing.xs);

        // Add specific user folders with proper icons
        let user_folder_entries: Vec<(&str, &str, Option<PathBuf>)> = {
            let favorites = self.favorites.list();
            let find_favorite = |name: &str| -> Option<PathBuf> {
                favorites
                    .iter()
                    .find(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n == name)
                    })
                    .cloned()
            };
            vec![
                (icons::HOME, t("sidebar_home"), home_dir.clone()),
                (
                    icons::DESKTOP,
                    t("sidebar_desktop"),
                    find_favorite("Desktop"),
                ),
                (
                    icons::DOWNLOAD,
                    t("sidebar_downloads"),
                    find_favorite("Downloads"),
                ),
                (
                    icons::DOCUMENTS,
                    t("sidebar_documents"),
                    find_favorite("Documents"),
                ),
                (
                    icons::GALLERY,
                    t("sidebar_pictures"),
                    find_favorite("Pictures"),
                ),
                (icons::MUSIC, t("sidebar_music"), find_favorite("Music")),
                (icons::VIDEO, t("sidebar_videos"), find_favorite("Videos")),
            ]
        };

        if !sec_access {
            for (icon, label, path) in &user_folder_entries {
                if path.is_some() {
                    quick_access = quick_access.push(sidebar_button(
                        ctx,
                        icon,
                        *label,
                        path.clone(),
                        &current,
                    ));
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
            column![section_header(ctx, "sidebar_favorites", sec_favorites)].spacing(spacing.xs);
        if !sec_favorites {
            if custom_favorites.is_empty() {
                favorites_section =
                    favorites_section.push(caption_text(typography, t("sidebar_no_favorites")));
            } else {
                for favorite in custom_favorites {
                    let label = format_sidebar_label(favorite);
                    let fav_path = favorite.clone();
                    let remove_btn = button(glyph_text(typography, icons::CLOSE))
                        .padding(0)
                        .style(move |_theme: &Theme, _status| iced::widget::button::Style {
                            text_color: colors.text_muted,
                            ..Default::default()
                        })
                        .on_press(UiMessage::RemoveFavorite(fav_path));
                    favorites_section = favorites_section.push(
                        row![
                            sidebar_button(
                                ctx,
                                icons::FOLDER,
                                label,
                                Some(favorite.clone()),
                                &current
                            ),
                            remove_btn,
                        ]
                        .align_y(Alignment::Center),
                    );
                }
            }
        }

        (quick_access.into(), favorites_section.into())
    }

    /// Mounted drives and their usage bars.
    pub(super) fn sidebar_drives(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing,
            typography,
        } = ctx;

        // Clickable accordion header: shows ▸/▾ chevron and toggles the section.

        let sec_drives = self.sidebar_collapsed.contains("sidebar_drives");
        let current = self.state.route.key();

        // ── All drives ───────────────────────────────────────────────────
        let mut drive_section =
            column![section_header(ctx, "sidebar_drives", sec_drives)].spacing(spacing.xs);
        if !sec_drives {
            // Enumerating disks goes through sysinfo, which blocks for seconds on
            // an unreachable network drive. It used to run even when the section
            // was collapsed and its result thrown away.
            let drives = all_drives();
            if drives.is_empty() {
                drive_section =
                    drive_section.push(caption_text(typography, t("sidebar_no_drives")));
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
                            glyph_text(typography, icons::DRIVE),
                            caption_text(typography, drive_label(mount))
                        ]
                        .spacing(spacing.xs)
                        .align_y(Alignment::Center),
                        progress_bar(0.0..=1.0, used_ratio).girth(Length::Fixed(6.0)),
                        caption_text(
                            typography,
                            tf(
                                "sidebar_drive_free",
                                &[("free", &free_gb), ("total", &total_gb)]
                            )
                        )
                    ]
                    .spacing(spacing.xs)
                    .into();

                    drive_section = drive_section.push(
                        button(content)
                            .padding([spacing.xs, spacing.sm])
                            .width(Length::Fill)
                            .style(selection_style(mount_path == current))
                            .on_press(UiMessage::NavigateTo(mount_path)),
                    );
                }
            }
            // Locations gvfs has mounted: SMB shares, SFTP, phones, cameras,
            // Google Drive, an archive opened as a folder. `gvfsd-fuse` exposes
            // each of them as an ordinary directory, so they navigate through
            // exactly the same code path as a local one.
            for mount in gvfs_mounts() {
                let icon = match mount.scheme.as_str() {
                    "mtp" | "gphoto2" => icons::DEVICE,
                    "archive" => icons::FILE_ARCHIVE,
                    _ => icons::NETWORK,
                };
                drive_section = drive_section.push(sidebar_button(
                    ctx,
                    icon,
                    mount.label,
                    Some(mount.path),
                    &current,
                ));
            }

            drive_section = drive_section.push(sidebar_button(
                ctx,
                icons::NETWORK,
                t("location_network"),
                Some(PathBuf::from(NETWORK_ROUTE)),
                &current,
            ));
            drive_section = drive_section.push(sidebar_button(
                ctx,
                icons::FILE,
                t("location_recent"),
                Some(PathBuf::from(crate::ui::RECENT_ROUTE)),
                &current,
            ));
            // #21: Trash in sidebar
            drive_section = drive_section.push(
                button(
                    row![
                        icon_text(typography, icons::DELETE),
                        body_text(typography, t("location_trash")),
                    ]
                    .spacing(spacing.xs)
                    .align_y(Alignment::Center),
                )
                .padding([spacing.xs, spacing.sm])
                .width(Length::Fill)
                .style(selection_style(false))
                .on_press(UiMessage::NavigateToTrash),
            );
        } // end if !sec_drives

        drive_section.into()
    }
}
