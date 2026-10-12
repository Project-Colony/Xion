//! The two context menus themselves, the one for a selected entry, the one for
//! empty space, and the submenu panels they open.
//!
//! Moved out of `render_context_menu`, which now only decides which one to show
//! and where to place it.

use iced::widget::{column, container, mouse_area};
use iced::{Element, Length};

use crate::filesystem::FsEntryType;
use crate::i18n::t;
use crate::ui::theme::icons;
use crate::ui::{ContextAction, FileLabel, UiMessage};

use super::XionApp;
use super::menu_items;
use super::widgets::RADIUS;
use super::widgets::ViewCtx;
use super::widgets::surface_style;
use crate::ui::app::types::*;

impl XionApp {
    /// The menu shown when right-clicking a file or folder.
    pub(super) fn context_entry_menu(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing: _,
            typography: _,
        } = ctx;
        let selection_count = self.state.navigation.selection.selected.len();
        let selected_is_archive = self
            .state
            .navigation
            .selection
            .focused
            .as_ref()
            .and_then(|path| ArchiveType::detect(path))
            .is_some();
        // `Path::is_file()`/`is_dir()` are syscalls, and these ran on every
        // rebuild of the widget tree, several times a second, and blocking for
        // seconds at a time on a dead network share. The entry type is already
        // loaded in memory.
        let focused_entry_type = self
            .state
            .navigation
            .selection
            .focused
            .as_ref()
            .and_then(|path| {
                let entries = self.display_entries();
                Self::index_for_path_in(entries, path).and_then(|index| entries.get(index))
            })
            .map(|entry| entry.entry_type);
        let selected_is_file = focused_entry_type == Some(FsEntryType::File);
        let selected_is_dir = focused_entry_type == Some(FsEntryType::Directory);
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        let mut context_col = column![].spacing(2);

        // Group 1: Open
        context_col = context_col
            .push(menu_items::item(
                ctx,
                icons::OPEN,
                t("menu_open"),
                t("shortcut_enter"),
                UiMessage::ContextAction(ContextAction::Open),
            ))
            .push(menu_items::item(
                ctx,
                icons::OPEN,
                t("menu_open_with"),
                "",
                UiMessage::ContextAction(ContextAction::OpenWith),
            ));
        if selected_is_dir {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::TERMINAL,
                t("menu_open_terminal_here"),
                "",
                UiMessage::ToggleTerminal,
            ));
        }
        if selected_is_archive {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::FILE_ARCHIVE,
                t("menu_open_archive"),
                "",
                UiMessage::ActivateEntry(
                    self.state
                        .navigation
                        .selection
                        .focused
                        .clone()
                        .unwrap_or_default(),
                ),
            ));
        }

        // Separator
        context_col = context_col.push(menu_items::separator(ctx));

        // Group 2: Clipboard
        context_col = context_col
            .push(menu_items::item(
                ctx,
                icons::CUT,
                t("menu_cut"),
                "Ctrl+X",
                UiMessage::ClipboardCut,
            ))
            .push(menu_items::item(
                ctx,
                icons::COPY,
                t("menu_copy"),
                "Ctrl+C",
                UiMessage::ClipboardCopy,
            ));
        if has_clipboard {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::PASTE,
                t("menu_paste"),
                "Ctrl+V",
                UiMessage::ClipboardPaste,
            ));
        }

        // Separator
        context_col = context_col.push(menu_items::separator(ctx));

        // Group 3: Edit
        context_col = context_col
            .push(menu_items::item(
                ctx,
                icons::RENAME,
                t("menu_rename"),
                "F2",
                UiMessage::ContextAction(ContextAction::Rename),
            ))
            .push(menu_items::item(
                ctx,
                icons::DELETE,
                t("menu_delete"),
                t("shortcut_delete"),
                UiMessage::ContextAction(ContextAction::MoveToTrash),
            ));
        if selection_count > 1 {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::RENAME,
                t("menu_bulk_rename"),
                "",
                UiMessage::ContextAction(ContextAction::OpenBulkRename),
            ));
        }
        if selection_count == 2 {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::FILE,
                t("menu_compare"),
                "Ctrl+D",
                UiMessage::ContextAction(ContextAction::OpenDiff),
            ));
        }

        // Separator
        context_col = context_col.push(menu_items::separator(ctx));

        // Group 4: Compress submenu trigger (hover to open)
        context_col = context_col.push(
            mouse_area(menu_items::item(
                ctx,
                icons::FILE_ARCHIVE,
                t("menu_compress"),
                "",
                UiMessage::ToggleContextSubmenu(0),
            ))
            .on_enter(UiMessage::ToggleContextSubmenu(0)),
        );

        // Group 5: Label submenu trigger (hover to open)
        context_col = context_col.push(
            mouse_area(menu_items::item(
                ctx,
                "",
                t("menu_label"),
                "",
                UiMessage::ToggleContextSubmenu(1),
            ))
            .on_enter(UiMessage::ToggleContextSubmenu(1)),
        );

        // Separator
        context_col = context_col.push(menu_items::separator(ctx));

        // Group 6: Info
        context_col = context_col
            .push(menu_items::item(
                ctx,
                icons::SYMLINK,
                t("menu_copy_path"),
                t("shortcut_copy_path"),
                UiMessage::ContextAction(ContextAction::CopyPath),
            ))
            .push(menu_items::item(
                ctx,
                icons::HOME,
                t("menu_add_favorite"),
                "",
                UiMessage::ContextAction(ContextAction::AddToFavorites),
            ));
        if selected_is_file {
            context_col = context_col.push(menu_items::item(
                ctx,
                icons::FILE,
                t("menu_view_hex"),
                "",
                UiMessage::ContextAction(ContextAction::OpenHexView),
            ));
        }
        context_col = context_col.push(menu_items::item(
            ctx,
            icons::FILE,
            t("menu_properties"),
            t("shortcut_alt_enter"),
            UiMessage::ContextAction(ContextAction::OpenProperties),
        ));

        context_col.into()
    }

    /// The menu shown when right-clicking empty space.
    pub(super) fn context_background_menu(&self, ctx: ViewCtx) -> Element<'_, UiMessage> {
        let ViewCtx {
            colors: _,
            spacing: _,
            typography: _,
        } = ctx;
        // `Path::is_file()`/`is_dir()` are syscalls, and these ran on every
        // rebuild of the widget tree, several times a second, and blocking for
        // seconds at a time on a dead network share. The entry type is already
        // loaded in memory.
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        let mut bg_context_col = column![].spacing(2);
        if has_clipboard {
            bg_context_col = bg_context_col
                .push(menu_items::item(
                    ctx,
                    icons::PASTE,
                    t("menu_paste"),
                    "Ctrl+V",
                    UiMessage::ClipboardPaste,
                ))
                .push(menu_items::separator(ctx));
        }
        bg_context_col = bg_context_col
            .push(menu_items::item(
                ctx,
                icons::FOLDER,
                t("menu_new_folder"),
                t("shortcut_new_folder"),
                UiMessage::ContextAction(ContextAction::NewFolder),
            ))
            .push(menu_items::item(
                ctx,
                icons::FILE,
                t("menu_new_file"),
                "",
                UiMessage::ContextAction(ContextAction::NewFile),
            ))
            .push(menu_items::separator(ctx))
            .push(menu_items::item(
                ctx,
                icons::TERMINAL,
                t("menu_open_terminal_here"),
                "",
                UiMessage::ToggleTerminal,
            ))
            .push(menu_items::item(
                ctx,
                icons::REFRESH,
                t("menu_refresh"),
                "Ctrl+R",
                UiMessage::Refresh,
            ))
            .push(menu_items::separator(ctx))
            .push(menu_items::item(
                ctx,
                icons::FILE,
                t("menu_folder_properties"),
                "",
                UiMessage::ContextAction(ContextAction::OpenProperties),
            ));

        bg_context_col.into()
    }

    /// The panel of whichever submenu is currently open.
    pub(super) fn context_submenu_panel(&self, ctx: ViewCtx) -> Option<Element<'_, UiMessage>> {
        let ViewCtx {
            colors,
            spacing,
            typography: _,
        } = ctx;
        // `Path::is_file()`/`is_dir()` are syscalls, and these ran on every
        // rebuild of the widget tree, several times a second, and blocking for
        // seconds at a time on a dead network share. The entry type is already
        // loaded in memory.

        let submenu_panel: Option<Element<'_, UiMessage>> = match self.menus.context_submenu {
            Some(ContextSubmenu::Compress) => {
                let sub = column![
                    menu_items::item(
                        ctx,
                        icons::FILE_ARCHIVE,
                        "ZIP",
                        "",
                        UiMessage::ContextAction(ContextAction::CompressToZip)
                    ),
                    menu_items::item(
                        ctx,
                        icons::FILE_ARCHIVE,
                        "TAR.GZ",
                        "",
                        UiMessage::ContextAction(ContextAction::CompressToTarGz)
                    ),
                    menu_items::item(
                        ctx,
                        icons::FILE_ARCHIVE,
                        "7Z",
                        "",
                        UiMessage::ContextAction(ContextAction::CompressTo7z)
                    ),
                ]
                .spacing(2);
                Some(
                    container(sub)
                        .padding([spacing.sm, spacing.md])
                        .width(Length::Fixed(180.0))
                        .style(surface_style(colors, RADIUS.lg))
                        .into(),
                )
            }
            Some(ContextSubmenu::Label) => {
                let sub = column![
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Red.color(&colors),
                        t("label_red").into(),
                        ContextAction::SetLabelRed
                    ),
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Orange.color(&colors),
                        t("label_orange").into(),
                        ContextAction::SetLabelOrange
                    ),
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Yellow.color(&colors),
                        t("label_yellow").into(),
                        ContextAction::SetLabelYellow
                    ),
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Green.color(&colors),
                        t("label_green").into(),
                        ContextAction::SetLabelGreen
                    ),
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Blue.color(&colors),
                        t("label_blue").into(),
                        ContextAction::SetLabelBlue
                    ),
                    menu_items::label_dot(
                        ctx,
                        FileLabel::Purple.color(&colors),
                        t("label_purple").into(),
                        ContextAction::SetLabelPurple
                    ),
                    menu_items::separator(ctx),
                    menu_items::item(
                        ctx,
                        "",
                        t("menu_remove_label"),
                        "",
                        UiMessage::ContextAction(ContextAction::RemoveLabel)
                    ),
                ]
                .spacing(2);
                Some(
                    container(sub)
                        .padding([spacing.sm, spacing.md])
                        .width(Length::Fixed(180.0))
                        .style(surface_style(colors, RADIUS.lg))
                        .into(),
                )
            }
            None => None,
        };

        submenu_panel
    }
}
