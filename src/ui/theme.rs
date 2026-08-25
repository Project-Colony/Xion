//! Theme and design tokens for the Xion UI.
//!
//! This module defines the visual design system including colors, spacing,
//! and typography constants used throughout the application.

use iced::font::{Family, Style, Weight};
use iced::{Color, Font};

/// Font name used throughout the application.
pub const FONT_NAME: &str = "JetBrainsMono Nerd Font";

/// Embedded font data.
///
/// Only the three variants `UiTypography` actually selects are shipped. The
/// other thirteen Nerd Font weights used to be embedded and registered too:
/// 13 x 2.36 Mo of binary that iced parsed into its font database at startup
/// and no widget ever asked for. Adding a weight here means adding it to
/// `UiTypography` as well, or it is dead again.
pub mod fonts {
    /// `UiTypography::body_font` — Weight::Normal.
    pub const REGULAR: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Regular.ttf");
    /// `UiTypography::title_font` — Weight::Semibold.
    pub const SEMI_BOLD: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-SemiBold.ttf");
    /// `UiTypography::caption_font` — Weight::Light + Style::Italic.
    pub const LIGHT_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-LightItalic.ttf");
}

/// Nerd Font icons used in the UI.
pub mod icons {
    pub const DEVICE: &str = "\u{f0379}"; // 󰍹 nf-md-monitor
    pub const LOADING: &str = "\u{f1356}"; // 󱍖 nf-md-loading
    pub const FOLDER: &str = "\u{f024b}"; // 󰉋 nf-md-folder
    pub const FILE: &str = "\u{f0214}"; // 󰈔 nf-md-file
    pub const SYMLINK: &str = "\u{f0337}"; // 󰌷 nf-md-link_variant
    pub const UNKNOWN: &str = "\u{f02d7}"; // 󰋗 nf-md-help_circle
    pub const HOME: &str = "\u{f02dc}"; // 󰋜 nf-md-home
    pub const GALLERY: &str = "\u{f024f}"; // 󰉏 nf-md-folder_image
    pub const DESKTOP: &str = "\u{f0391}"; // 󰎑 nf-md-monitor_dashboard (approximation)
    pub const DOWNLOAD: &str = "\u{f01da}"; // 󰇚 nf-md-download
    pub const DOCUMENTS: &str = "\u{f0219}"; // 󰈙 nf-md-file_document
    pub const MUSIC: &str = "\u{f0388}"; // 󰎈 nf-md-music_box
    pub const VIDEO: &str = "\u{f057e}"; // 󰕾 nf-md-video
    pub const PC: &str = "\u{f0379}"; // 󰍹 nf-md-monitor
    pub const DRIVE: &str = "\u{f02ca}"; // 󰋊 nf-md-harddisk
    pub const NETWORK: &str = "\u{f0317}"; // 󰌗 nf-md-lan
    pub const BACK: &str = "\u{f004d}"; // 󰁍 nf-md-arrow_left
    pub const FORWARD: &str = "\u{f0054}"; // 󰁔 nf-md-arrow_right
    pub const REFRESH: &str = "\u{f0450}"; // 󰑐 nf-md-refresh
    pub const SEARCH: &str = "\u{f0349}"; // 󰍉 nf-md-magnify
    pub const NEW: &str = "\u{f0415}"; // 󰐕 nf-md-plus
    pub const CUT: &str = "\u{f0190}"; // 󰆐 nf-md-content_cut
    pub const COPY: &str = "\u{f018f}"; // 󰆏 nf-md-content_copy
    pub const PASTE: &str = "\u{f0192}"; // 󰆒 nf-md-content_paste
    pub const SORT: &str = "\u{f04ba}"; // 󰒺 nf-md-sort
    pub const VIEW_GRID: &str = "\u{f0574}"; // 󰕴 nf-md-view_grid
    pub const VIEW_LIST: &str = "\u{f0575}"; // 󰕵 nf-md-view_list
    pub const MORE: &str = "\u{f01d8}"; // 󰇘 nf-md-dots_horizontal
    pub const ACTIONS: &str = "\u{f035b}"; // 󰍛 nf-md-menu
    pub const OPEN: &str = "\u{f0256}"; // 󰉖 nf-md-folder_open
    pub const RENAME: &str = "\u{f0ea8}"; // 󰺨 nf-md-rename_box
    pub const DELETE: &str = "\u{f01b4}"; // 󰆴 nf-md-delete
    pub const CLOSE: &str = "\u{f0156}"; // 󰅖 nf-md-close
    pub const THEME: &str = "\u{f05df}"; // 󰗟 nf-md-weather_night (dark mode toggle)
    pub const TERMINAL: &str = "\u{f0489}"; // 󰒉 nf-md-console

    // ── File type icons ────────────────────────────────────────────
    pub const FILE_CODE: &str = "\u{f0217}"; // 󰈗 nf-md-file_code
    pub const FILE_IMAGE: &str = "\u{f021f}"; // 󰈟 nf-md-file_image
    pub const FILE_MUSIC: &str = "\u{f0223}"; // 󰈣 nf-md-file_music
    pub const FILE_VIDEO: &str = "\u{f022b}"; // 󰈫 nf-md-file_video
    pub const FILE_PDF: &str = "\u{f0226}"; // 󰈦 nf-md-file_pdf_box
    pub const FILE_ARCHIVE: &str = "\u{f06fb}"; // 󰛻 nf-md-zip_box
    pub const FILE_TEXT: &str = "\u{f0219}"; // 󰈙 nf-md-file_document
    pub const FILE_TABLE: &str = "\u{f021b}"; // 󰈛 nf-md-file_excel
    pub const FILE_CONFIG: &str = "\u{f0493}"; // 󰒓 nf-md-settings
    pub const FILE_GIT: &str = "\u{f02a2}"; // 󰊢 nf-md-git
    pub const FILE_LOCK: &str = "\u{f033e}"; // 󰌾 nf-md-lock
    pub const FILE_FONT: &str = "\u{f031a}"; // 󰌚 nf-md-format_font
    pub const FILE_EXE: &str = "\u{f0214}"; // 󰈔 nf-md-file (kept same as default)
    pub const FILE_DB: &str = "\u{f01bc}"; // 󰆼 nf-md-database

    /// The icon for an entry: its type, or its extension when it is a file.
    ///
    /// Five call sites across three modules wrote this same match by hand.
    pub fn icon_for_entry(entry: &crate::filesystem::FsEntry) -> &'static str {
        use crate::filesystem::FsEntryType;
        match entry.entry_type {
            FsEntryType::Directory => FOLDER,
            FsEntryType::File => entry
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(icon_for_extension)
                .unwrap_or(FILE),
            FsEntryType::Symlink => SYMLINK,
            FsEntryType::Other => UNKNOWN,
        }
    }

    /// Returns the Nerd Font icon for a file based on its extension.
    ///
    /// The lowercase copy this used to allocate was made once per visible row,
    /// on every rebuild of the widget tree, to look up a table of constants.
    /// Extensions are ASCII in practice; a stack buffer covers every entry in
    /// the table below (longest is `gitattributes`, 13 bytes) and anything
    /// longer cannot match, so it takes the same `FILE` the match arm would.
    pub fn icon_for_extension(ext: &str) -> &'static str {
        const MAX_EXTENSION: usize = 16;
        if ext.len() > MAX_EXTENSION || !ext.is_ascii() {
            return FILE;
        }
        let mut buffer = [0u8; MAX_EXTENSION];
        buffer[..ext.len()].copy_from_slice(ext.as_bytes());
        buffer[..ext.len()].make_ascii_lowercase();
        // SAFETY-free: the bytes came from a `&str` and lowercasing ASCII keeps
        // it valid UTF-8, so this cannot fail.
        let lowered = std::str::from_utf8(&buffer[..ext.len()]).unwrap_or("");
        match lowered {
            // Code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp" | "cs"
            | "java" | "go" | "rb" | "php" | "swift" | "kt" | "lua" | "zig" | "asm" | "sh"
            | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "r" | "dart" | "scala" | "html" | "htm"
            | "css" | "scss" | "sass" | "less" | "vue" | "svelte" => FILE_CODE,
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "tif"
            | "psd" | "ai" | "raw" | "cr2" | "nef" | "heic" | "avif" => FILE_IMAGE,
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "aac" | "wma" | "m4a" | "opus" | "mid" | "midi" => {
                FILE_MUSIC
            }
            // Video
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
            | "3gp" => FILE_VIDEO,
            // Documents
            "pdf" => FILE_PDF,
            "txt" | "md" | "rtf" | "log" | "nfo" | "readme" => FILE_TEXT,
            "csv" | "xls" | "xlsx" | "ods" => FILE_TABLE,
            "doc" | "docx" | "odt" => FILE_TEXT,
            // Archives
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4" | "cab" | "iso"
            | "dmg" => FILE_ARCHIVE,
            // Config
            "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "cfg" | "conf" | "env"
            | "properties" => FILE_CONFIG,
            // Git
            "gitignore" | "gitmodules" | "gitattributes" => FILE_GIT,
            // Fonts
            "ttf" | "otf" | "woff" | "woff2" | "eot" => FILE_FONT,
            // Database
            "db" | "sqlite" | "sqlite3" | "sql" | "mdb" => FILE_DB,
            // Executables
            "exe" | "msi" | "dll" | "so" | "dylib" => FILE_EXE,
            // Lock files
            "lock" => FILE_LOCK,
            _ => FILE,
        }
    }
}

/// Color palette for the UI.
#[derive(Debug, Clone, Copy)]
pub struct UiColors {
    /// Background color for chrome elements (toolbar, status bar).
    pub chrome_background: Color,
    /// Background color for main content panels.
    pub panel_background: Color,
    /// Border color for separators and outlines.
    pub border: Color,
    /// Background color for sidebar elements.
    pub sidebar_background: Color,
    /// Accent color for interactive elements.
    pub accent: Color,
    /// Primary text color.
    pub text_primary: Color,
    /// Muted/secondary text color.
    pub text_muted: Color,
    /// Background color for selected items.
    pub selection: Color,
    /// Border color for selected items.
    pub selection_border: Color,
    /// Background color for hovered items.
    pub hover: Color,
    /// Background color for pressed items.
    pub pressed: Color,
    /// Color for diff/rename "removed" (old) lines.
    pub diff_removed: Color,
    /// Color for diff/rename "added" (new) lines.
    pub diff_added: Color,
    /// Color for git staged files.
    pub git_staged: Color,
    /// Color for git conflict files.
    pub git_conflict: Color,
    /// Color for git deleted files.
    pub git_deleted: Color,
    /// Color for address bar "directory" validation badge.
    pub address_directory: Color,
    /// Color for address bar "file" validation badge.
    pub address_file: Color,
    /// Color for address bar "not found" validation badge.
    pub address_not_found: Color,
}

/// Perceived brightness, for deciding whether two colours read as different.
fn luminance(color: Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

/// Nudges a colour away from `surface` until the two can be told apart.
///
/// Colony's palettes separate their surfaces by fill, the way a card-based
/// layout does. Xion separates them by line. Measured across the fifty-seven
/// variants of the catalogue, `border_subtle` equals `bg_card` in twenty-nine
/// of them, and `divider` in twenty-nine as well — so taking either as given
/// would draw an invisible border in more than half the themes.
///
/// The nudge goes towards `towards`, which the caller picks as the most
/// contrasting thing the palette has against that surface.
fn separated_from(candidate: Color, surface: Color, towards: Color) -> Color {
    const MIN_SEPARATION: f32 = 0.06;
    const BLEND: f32 = 0.22;

    if (luminance(candidate) - luminance(surface)).abs() >= MIN_SEPARATION {
        return candidate;
    }

    Color {
        r: surface.r + (towards.r - surface.r) * BLEND,
        g: surface.g + (towards.g - surface.g) * BLEND,
        b: surface.b + (towards.b - surface.b) * BLEND,
        a: 1.0,
    }
}

impl UiColors {
    /// Derives Xion's nineteen surface names from a Colony palette.
    ///
    /// Xion keeps its own names rather than passing `ThemePalette` around: the
    /// view threads colours explicitly through `ViewCtx`, and a hidden
    /// dependency on Colony's global theme state would scatter through hundreds
    /// of call sites.
    ///
    /// The palette arrives as an argument for the same reason. `for_theme` runs
    /// inside `view`, so it runs on every rebuild of the widget tree; reaching
    /// for `active_palette()` there would take a lock per frame, and setting it
    /// would be the view mutating global state. `colony_ui::resolve` is pure,
    /// so neither is needed.
    pub fn from_colony(palette: colony_ui::ThemePalette, accent: Color) -> Self {
        Self {
            chrome_background: palette.bg_primary,
            panel_background: palette.bg_card,
            sidebar_background: palette.bg_sidebar,
            border: separated_from(palette.border_subtle, palette.bg_card, palette.text_primary),
            accent,
            text_primary: palette.text_primary,
            text_muted: palette.text_muted,
            // `bg_selected` égale `bg_card` dans douze variantes sur
            // cinquante-sept : la ligne sélectionnée y disparaîtrait dans le
            // fond de la liste.
            selection: separated_from(palette.bg_selected, palette.bg_card, accent),
            selection_border: accent,
            hover: palette.bg_card_hover,
            pressed: palette.bg_card_pressed,
            diff_removed: palette.error,
            diff_added: palette.success,
            git_staged: palette.success,
            git_conflict: palette.error,
            git_deleted: palette.error_light,
            address_directory: palette.success,
            address_file: palette.warning,
            address_not_found: palette.error,
        }
    }
}

impl Default for UiColors {
    fn default() -> Self {
        Self {
            chrome_background: Color::from_rgb8(247, 247, 250),
            panel_background: Color::from_rgb8(255, 255, 255),
            border: Color::from_rgb8(223, 226, 232),
            sidebar_background: Color::from_rgb8(242, 244, 248),
            accent: Color::from_rgb8(0, 120, 215),
            text_primary: Color::from_rgb8(32, 34, 38),
            text_muted: Color::from_rgb8(110, 114, 122),
            selection: Color::from_rgb8(214, 230, 248),
            selection_border: Color::from_rgb8(178, 206, 236),
            hover: Color::from_rgb8(233, 239, 247),
            pressed: Color::from_rgb8(220, 230, 244),
            diff_removed: Color::from_rgb(0.9, 0.4, 0.4),
            diff_added: Color::from_rgb(0.4, 0.9, 0.4),
            git_staged: Color::from_rgb8(80, 200, 80),
            git_conflict: Color::from_rgb8(220, 50, 50),
            git_deleted: Color::from_rgb8(200, 80, 80),
            address_directory: Color::from_rgb8(55, 125, 60),
            address_file: Color::from_rgb8(186, 120, 40),
            address_not_found: Color::from_rgb8(176, 72, 72),
        }
    }
}

#[cfg(test)]
mod icon_tests {
    use super::icons::*;

    #[test]
    fn case_is_ignored() {
        assert_eq!(icon_for_extension("RS"), icon_for_extension("rs"));
        assert_eq!(icon_for_extension("PnG"), icon_for_extension("png"));
    }

    #[test]
    fn the_longest_entry_in_the_table_still_fits_the_buffer() {
        // Si quelqu'un ajoute une extension plus longue que le tampon, elle
        // tomberait silencieusement sur l'icône générique. Ce test le dit.
        assert_ne!(icon_for_extension("gitattributes"), FILE);
    }

    #[test]
    fn anything_too_long_or_non_ascii_falls_back_like_the_match_arm() {
        assert_eq!(icon_for_extension("uneextensionbeaucouptroplongue"), FILE);
        assert_eq!(icon_for_extension("é"), FILE);
        assert_eq!(icon_for_extension(""), FILE);
        assert_eq!(icon_for_extension("inconnue"), FILE);
    }
}

/// Spacing values for consistent layout.
#[derive(Debug, Clone, Copy)]
pub struct UiSpacing {
    /// Extra small spacing (4px).
    pub xs: f32,
    /// Small spacing (8px).
    pub sm: f32,
    /// Medium spacing (12px).
    pub md: f32,
    /// Large spacing (16px).
    pub lg: f32,
    /// Extra large spacing (20px).
    pub xl: f32,
}

impl Default for UiSpacing {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 20.0,
        }
    }
}

/// Typography settings for text rendering.
#[derive(Debug, Clone, Copy)]
pub struct UiTypography {
    /// Title font size.
    pub title: f32,
    /// Body text font size.
    pub body: f32,
    /// Caption/small text font size.
    pub caption: f32,
    /// Font for titles.
    pub title_font: Font,
    /// Font for body text.
    pub body_font: Font,
    /// Font for captions.
    pub caption_font: Font,
}

impl Default for UiTypography {
    fn default() -> Self {
        Self {
            title: 16.0,
            body: 14.0,
            caption: 12.0,
            title_font: Font {
                family: Family::Name(FONT_NAME),
                weight: Weight::Semibold,
                ..Font::DEFAULT
            },
            body_font: Font {
                family: Family::Name(FONT_NAME),
                weight: Weight::Normal,
                ..Font::DEFAULT
            },
            caption_font: Font {
                family: Family::Name(FONT_NAME),
                weight: Weight::Light,
                style: Style::Italic,
                ..Font::DEFAULT
            },
        }
    }
}

/// Complete design token set for the UI.
#[derive(Debug, Clone, Copy, Default)]
pub struct UiTokens {
    /// Color palette.
    pub colors: UiColors,
    /// Spacing values.
    pub spacing: UiSpacing,
    /// Typography settings.
    pub typography: UiTypography,
}

impl UiTokens {
    /// Returns tokens for a specific theme config.
    ///
    /// Pure: `resolve` and `with_high_contrast` both are, and this runs once per
    /// rebuild of the widget tree.
    pub fn for_theme(theme: &crate::core::ThemeConfig) -> Self {
        let (family, variant) = theme.colony_keys();
        let palette = colony_ui::resolve(family, variant);
        let palette = if theme.wants_high_contrast() {
            palette.with_high_contrast()
        } else {
            palette
        };
        Self {
            colors: UiColors::from_colony(palette, palette.accent_blue),
            ..Default::default()
        }
    }
}

/// UI timing constants.
pub mod timing {
    use std::time::Duration;

    /// Delay before showing loading indicator.
    pub const LOADING_INDICATOR_DELAY: Duration = Duration::from_millis(75);
    /// Maximum time between clicks for double-click detection.
    pub const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);
    /// Interval for file watcher polling.
    pub const WATCHER_POLL_INTERVAL: Duration = Duration::from_millis(750);
}

/// UI layout constants.
pub mod layout {
    /// Maximum depth for tree view expansion.
    pub const TREE_MAX_DEPTH: usize = 4;
    /// Maximum children to show per tree node.
    pub const TREE_MAX_CHILDREN: usize = 120;
    /// Minimum height for tree pane.
    pub const TREE_MIN_HEIGHT: f32 = 140.0;
    /// Maximum height for tree pane.
    pub const TREE_MAX_HEIGHT: f32 = 420.0;
    /// Height of tree pane resize handle.
    pub const TREE_RESIZE_BAR_HEIGHT: f32 = 10.0;
    /// Row height in tree view.
    pub const TREE_ROW_HEIGHT: f32 = 28.0;
    /// Minimum width for preview pane.
    pub const PREVIEW_MIN_WIDTH: f32 = 220.0;
    /// Maximum width for preview pane.
    pub const PREVIEW_MAX_WIDTH: f32 = 420.0;
    /// Width of preview pane resize handle.
    pub const PREVIEW_RESIZE_BAR_WIDTH: f32 = 6.0;
    /// Minimum drag distance to start drag operation.
    pub const DRAG_START_THRESHOLD: f32 = 6.0;
    /// Default height for terminal panel.
    pub const TERMINAL_DEFAULT_HEIGHT: f32 = 240.0;
    /// Minimum height for terminal panel.
    pub const TERMINAL_MIN_HEIGHT: f32 = 120.0;
}

#[cfg(test)]
mod colony_palette_tests {
    use super::{UiColors, luminance};

    /// Ce que Xion exige de toute palette, vérifié sur le catalogue entier.
    ///
    /// Colony offre vingt-cinq familles et cinquante-sept variantes. Aucune
    /// n'est relue à la main ; ce test l'est à leur place, et il échouera le
    /// jour où le socle en ajoutera une qui ne sépare pas ce que Xion sépare.
    #[test]
    fn every_colony_variant_stays_legible() {
        const MIN: f32 = 0.03;
        let mut checked = 0usize;

        for family in colony_ui::THEME_FAMILIES {
            for variant in family.variants {
                let palette = colony_ui::resolve(family.key, variant.key);
                let colors = UiColors::from_colony(palette, palette.accent_blue);
                let at = format!("{}/{}", family.key, variant.key);
                checked += 1;

                let apart = |a, b| (luminance(a) - luminance(b)).abs();

                assert!(
                    apart(colors.border, colors.panel_background) >= MIN,
                    "{at} : la bordure est invisible sur la liste"
                );
                assert!(
                    apart(colors.selection, colors.panel_background) >= MIN,
                    "{at} : la ligne sélectionnée se confond avec le fond"
                );
                assert!(
                    apart(colors.text_primary, colors.panel_background) >= 0.2,
                    "{at} : le texte est illisible sur la liste"
                );
                assert!(
                    apart(colors.text_muted, colors.panel_background) >= 0.08,
                    "{at} : le texte atténué est illisible sur la liste"
                );
            }
        }

        assert!(checked >= 50, "catalogue trop court : {checked} variantes");
    }

    /// Le contraste élevé est un modificateur, pas un thème : il s'applique à
    /// n'importe quelle palette. C'est ce qui règle le défaut de l'ancienne
    /// palette maison, où l'en-tête et la liste étaient tous deux noirs et où
    /// seule une bordure les distinguait.
    #[test]
    fn high_contrast_applies_to_any_family() {
        let palette = colony_ui::resolve("gruvbox", "dark");
        let plain = UiColors::from_colony(palette, palette.accent_blue);
        let boosted_palette = palette.with_high_contrast();
        let boosted = UiColors::from_colony(boosted_palette, boosted_palette.accent_blue);

        let contrast =
            |c: &UiColors| (luminance(c.text_primary) - luminance(c.panel_background)).abs();
        assert!(
            contrast(&boosted) >= contrast(&plain),
            "le contraste élevé doit augmenter le contraste, pas le réduire"
        );
    }

    /// Une famille inconnue — une configuration ancienne, une famille retirée
    /// du socle — doit dégrader vers le repli, pas empêcher le démarrage.
    #[test]
    fn an_unknown_family_falls_back_instead_of_failing() {
        let palette = colony_ui::resolve("famille-qui-nexiste-pas", "variante-inventee");
        let colors = UiColors::from_colony(palette, palette.accent_blue);
        assert!(
            (luminance(colors.text_primary) - luminance(colors.panel_background)).abs() >= 0.2,
            "le repli doit rester lisible"
        );
    }
}
