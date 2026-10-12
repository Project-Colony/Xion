//! Theme and design tokens for the Xion UI.
//!
//! This module defines the visual design system including colors, spacing,
//! and typography constants used throughout the application.

use colony_ui::ColorExt;
use colony_ui::preferences::StandardPreferences;
use iced::{Color, Font};

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
///
/// Every field is derived from a Colony palette by [`UiColors::from_colony`];
/// nothing here is a colour of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Background for text selections and toggles that are on.
    pub selection: Color,
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

/// Whether the chosen theme resolves to a dark palette.
///
/// Derived from the background's luminance rather than from the catalogue's
/// `mode` field: the measurement is what the syntax highlighter actually cares
/// about, and it keeps working for the fallback palette an unknown family
/// resolves to.
pub fn resolves_dark(preferences: &StandardPreferences) -> bool {
    !colony_ui::resolve(&preferences.selected_theme, &preferences.selected_variant)
        .bg_primary
        .is_light()
}

/// Nudges a colour away from `surface` until the two can be told apart.
///
/// Colony's palettes separate their surfaces by fill, the way a card-based
/// layout does. Xion separates them by line. Measured across the fifty-seven
/// variants of the catalogue, `border_subtle` equals `bg_card` in twenty-nine
/// of them, and `divider` in twenty-nine as well, so taking either as given
/// would draw an invisible border in more than half the themes.
///
/// The nudge goes towards `towards`, which the caller picks as the most
/// contrasting thing the palette has against that surface.
fn separated_from(candidate: Color, surface: Color, towards: Color) -> Color {
    const MIN_SEPARATION: f32 = 0.06;
    const BLEND: f32 = 0.22;

    if (candidate.luma() - surface.luma()).abs() >= MIN_SEPARATION {
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
    /// The palette arrives as an argument for the same reason. `for_preferences` runs
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
            // `bg_selected` equals `bg_card` in twelve variants out of
            // fifty-seven: the selected row would vanish into the list
            // background there.
            selection: separated_from(palette.bg_selected, palette.bg_card, accent),
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

/// The palette Colony falls back to, with its own accent: what a fresh
/// configuration resolves to.
impl Default for UiColors {
    fn default() -> Self {
        let palette = colony_ui::FALLBACK_PALETTE;
        Self::from_colony(palette, palette.accent_blue)
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
        // If someone adds an extension longer than the buffer, it would
        // silently fall back to the generic icon. This test says so.
        assert_ne!(icon_for_extension("gitattributes"), FILE);
    }

    #[test]
    fn anything_too_long_or_non_ascii_falls_back_like_the_match_arm() {
        assert_eq!(icon_for_extension("anextensionthatisfartoolong"), FILE);
        assert_eq!(icon_for_extension("é"), FILE);
        assert_eq!(icon_for_extension(""), FILE);
        assert_eq!(icon_for_extension("unknown"), FILE);
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

/// Xion's three text roles, sized and set by the user's preferences.
///
/// Built from colony-ui's [`Typography`](colony_ui::Typography), so the
/// Appearance size and the Accessibility text size reach every role through
/// `sz(base)`, and the dyslexia font replaces the interface font everywhere at
/// once. The interface font ships in Regular, Medium and Bold only: titles are
/// Bold, body text and captions Regular, and captions differ by size and by
/// the muted colour their call sites give them.
#[derive(Debug, Clone, Copy)]
pub struct UiTypography {
    /// Title font size: `sz(16)`.
    pub title: f32,
    /// Body text font size: `sz(14)`.
    pub body: f32,
    /// Caption/small text font size: `sz(12)`.
    pub caption: f32,
    /// Font for titles.
    pub title_font: Font,
    /// Font for body text.
    pub body_font: Font,
    /// Font for captions.
    pub caption_font: Font,
    /// Font for Nerd Font glyphs, which OpenDyslexic does not carry: always
    /// JetBrains Mono, whatever the dyslexia toggle says.
    pub glyph_font: Font,
    /// What colony-ui's own widgets take.
    kit: colony_ui::Typography,
}

impl UiTypography {
    /// The user's current text settings.
    ///
    /// Reads colony-ui's globals, which `StandardPreferences::apply` sets at
    /// startup and after every change: call it once per view.
    pub fn current() -> Self {
        Self::from_kit(colony_ui::Typography::current())
    }

    fn from_kit(kit: colony_ui::Typography) -> Self {
        Self {
            title: kit.sz(16),
            body: kit.sz(14),
            caption: kit.sz(12),
            title_font: kit.bold,
            body_font: kit.regular,
            caption_font: kit.regular,
            glyph_font: colony_ui::fonts::glyph_font(),
            kit,
        }
    }

    /// What colony-ui's shared widgets expect.
    pub fn colony(&self) -> colony_ui::Typography {
        self.kit
    }
}

/// Complete design token set for the UI.
#[derive(Debug, Clone, Copy)]
pub struct UiTokens {
    /// Color palette.
    pub colors: UiColors,
    /// Spacing values.
    pub spacing: UiSpacing,
    /// Typography settings.
    pub typography: UiTypography,
}

impl UiTokens {
    /// Returns tokens for the user's preferences.
    ///
    /// The colours are pure: `resolve` and `with_high_contrast` both are, and
    /// this runs once per rebuild of the widget tree. The typography reads
    /// colony-ui's text globals, as every Colony program's view does.
    pub fn for_preferences(preferences: &StandardPreferences) -> Self {
        let palette =
            colony_ui::resolve(&preferences.selected_theme, &preferences.selected_variant);
        let palette = if preferences.high_contrast {
            palette.with_high_contrast()
        } else {
            palette
        };
        // `None` while auto accent is on or no accent was picked: the theme's
        // own accent then applies.
        let accent = preferences.accent_override().unwrap_or(palette.accent_blue);
        Self {
            colors: UiColors::from_colony(palette, accent),
            spacing: UiSpacing::default(),
            typography: UiTypography::current(),
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
    /// How long the "Theme applied." toast stays before it hides itself.
    pub const TOAST_DURATION: Duration = Duration::from_secs(4);
}

/// UI layout constants.
///
/// Heights that hold a line of text are written at a text scale of 1.0 and go
/// through [`layout::scaled`] where they are used, so they grow with the two
/// size preferences instead of clipping 1.68x text in a row drawn for 1.0x.
pub mod layout {
    /// `height` at the user's text scale.
    pub fn scaled(height: f32) -> f32 {
        scaled_by(height, colony_ui::font_scale())
    }

    /// `height` at `scale`, rounded to a whole pixel like `sz()`.
    pub fn scaled_by(height: f32, scale: f32) -> f32 {
        (height * scale).round()
    }

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
    /// Maximum height of the terminal panel.
    ///
    /// Generous on purpose: a full-screen program (`claude`, `htop`, `lazygit`)
    /// needs lines, and the default of 240 pixels only offered eleven once the
    /// tab strip was taken off. The bound exists so there is always room left
    /// to see the files behind it.
    pub const TERMINAL_MAX_HEIGHT: f32 = 900.0;
    /// Height of the strip carrying the tabs, at the top of the window as in
    /// the terminal panel.
    ///
    /// Shared on purpose: the active tab must touch the bottom of its strip with
    /// no gap, and that contact is what makes a row of buttons read as tabs. Two
    /// numbers for the same thing drift apart as soon as the thing changes.
    pub const TAB_STRIP_HEIGHT: f32 = 30.0;
    /// Thickness of the terminal panel's resize handle.
    ///
    /// Thinner than the preview pane's, which is six pixels: this one is
    /// horizontal and adds to the chrome height, already taken by the tab
    /// strip. Four pixels stay easy to grab and give two pixels back to the
    /// grid.
    pub const TERMINAL_RESIZE_BAR_HEIGHT: f32 = 4.0;
}

#[cfg(test)]
mod colony_palette_tests {
    use super::UiColors;
    use colony_ui::ColorExt;

    /// What Xion requires of every palette, checked across the whole catalogue.
    ///
    /// Colony offers twenty-five families and fifty-seven variants. None is
    /// reviewed by hand; this test does it instead, and it will fail the day the
    /// foundation adds one that does not separate what Xion separates.
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

                let apart = |a: iced::Color, b: iced::Color| (a.luma() - b.luma()).abs();

                assert!(
                    apart(colors.border, colors.panel_background) >= MIN,
                    "{at}: the border is invisible on the list"
                );
                assert!(
                    apart(colors.selection, colors.panel_background) >= MIN,
                    "{at}: the selected row blends into the background"
                );
                assert!(
                    apart(colors.text_primary, colors.panel_background) >= 0.2,
                    "{at}: the text is unreadable on the list"
                );
                assert!(
                    apart(colors.text_muted, colors.panel_background) >= 0.08,
                    "{at}: the muted text is unreadable on the list"
                );
            }
        }

        assert!(checked >= 50, "catalogue too short: {checked} variants");
    }

    /// High contrast is a modifier, not a theme: it applies to any palette. That
    /// is what fixes the flaw of the old home-made palette, where the header and
    /// the list were both black and only a border told them apart.
    #[test]
    fn high_contrast_applies_to_any_family() {
        let palette = colony_ui::resolve("gruvbox", "dark");
        let plain = UiColors::from_colony(palette, palette.accent_blue);
        let boosted_palette = palette.with_high_contrast();
        let boosted = UiColors::from_colony(boosted_palette, boosted_palette.accent_blue);

        let contrast = |c: &UiColors| (c.text_primary.luma() - c.panel_background.luma()).abs();
        assert!(
            contrast(&boosted) >= contrast(&plain),
            "high contrast must raise the contrast, not lower it"
        );
    }

    /// An unknown family (an old configuration, a family removed from the
    /// foundation) must degrade to the fallback, not prevent startup.
    #[test]
    fn an_unknown_family_falls_back_instead_of_failing() {
        let palette = colony_ui::resolve("no-such-family", "made-up-variant");
        let colors = UiColors::from_colony(palette, palette.accent_blue);
        assert!(
            (colors.text_primary.luma() - colors.panel_background.luma()).abs() >= 0.2,
            "the fallback must stay readable"
        );
    }

    /// The defaults are what a fresh configuration resolves to, not a palette
    /// of their own: Colony's fallback with its own accent.
    #[test]
    fn the_default_colours_are_the_default_theme() {
        let resolved = super::UiTokens::for_preferences(
            &colony_ui::preferences::StandardPreferences::default(),
        )
        .colors;
        assert_eq!(UiColors::default(), resolved);
    }

    /// With auto accent on, the picked accent is remembered but the theme's
    /// own applies.
    #[test]
    fn auto_accent_hands_the_accent_back_to_the_theme() {
        let mut preferences = colony_ui::preferences::StandardPreferences {
            selected_accent: Some("violet".to_string()),
            ..Default::default()
        };
        let picked = super::UiTokens::for_preferences(&preferences).colors.accent;
        preferences.auto_accent = true;
        let auto = super::UiTokens::for_preferences(&preferences).colors.accent;

        assert_ne!(picked, auto);
        assert_eq!(auto, UiColors::default().accent);
    }
}

#[cfg(test)]
mod typography_tests {
    use super::UiTypography;

    fn at(scale: f32) -> UiTypography {
        UiTypography::from_kit(colony_ui::Typography {
            scale,
            ..colony_ui::Typography::default()
        })
    }

    /// Both size preferences multiply, from 0.85 x 0.85 to 1.2 x 1.4. Every
    /// role is a whole pixel size and the three stay distinct and ordered at
    /// both ends, so a caption never grows past the body text it annotates.
    #[test]
    fn the_roles_stay_ordered_at_the_extreme_scales() {
        for (scale, title, body, caption) in [(0.7225, 12.0, 10.0, 9.0), (1.68, 27.0, 24.0, 20.0)] {
            let typography = at(scale);
            assert_eq!(
                (typography.title, typography.body, typography.caption),
                (title, body, caption),
                "at {scale}"
            );
        }
    }

    /// A compact list row still holds its text at the largest scale, and a
    /// normal one at the smallest still shows it whole.
    #[test]
    fn rows_grow_with_the_text_they_hold() {
        for scale in [0.7225, 1.0, 1.68] {
            let body = at(scale).body;
            for row in [22.0, 32.0] {
                let height = super::layout::scaled_by(row, scale);
                assert!(height > body, "{row} px row at {scale}: {height} <= {body}");
            }
        }
    }

    #[test]
    fn the_default_scale_keeps_the_historical_sizes() {
        let typography = at(1.0);
        assert_eq!(
            (typography.title, typography.body, typography.caption),
            (16.0, 14.0, 12.0)
        );
    }
}
