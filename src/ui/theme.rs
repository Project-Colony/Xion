//! Theme and design tokens for the Xion UI.
//!
//! This module defines the visual design system including colors, spacing,
//! and typography constants used throughout the application.

use iced::font::{Family, Style, Weight};
use iced::{Color, Font};

/// Font name used throughout the application.
pub const FONT_NAME: &str = "JetBrainsMono Nerd Font";

/// Embedded font data for JetBrains Mono Nerd Font variants.
pub mod fonts {
    pub const REGULAR: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Regular.ttf");
    pub const ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Italic.ttf");
    pub const THIN: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Thin.ttf");
    pub const THIN_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ThinItalic.ttf");
    pub const EXTRA_LIGHT: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraLight.ttf");
    pub const EXTRA_LIGHT_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraLightItalic.ttf");
    pub const LIGHT: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Light.ttf");
    pub const LIGHT_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-LightItalic.ttf");
    pub const MEDIUM: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Medium.ttf");
    pub const MEDIUM_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-MediumItalic.ttf");
    pub const SEMI_BOLD: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-SemiBold.ttf");
    pub const SEMI_BOLD_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-SemiBoldItalic.ttf");
    pub const BOLD: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Bold.ttf");
    pub const BOLD_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-BoldItalic.ttf");
    pub const EXTRA_BOLD: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraBold.ttf");
    pub const EXTRA_BOLD_ITALIC: &[u8] =
        include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraBoldItalic.ttf");
}

/// Nerd Font icons used in the UI.
pub mod icons {
    pub const DEVICE: &str = "\u{f0379}";   // 󰍹 nf-md-monitor
    pub const LOADING: &str = "\u{f1356}";  // 󱍖 nf-md-loading
    pub const FOLDER: &str = "\u{f024b}";   // 󰉋 nf-md-folder
    pub const FILE: &str = "\u{f0214}";     // 󰈔 nf-md-file
    pub const SYMLINK: &str = "\u{f0337}";  // 󰌷 nf-md-link_variant
    pub const UNKNOWN: &str = "\u{f02d7}";  // 󰋗 nf-md-help_circle
    pub const HOME: &str = "\u{f02dc}";     // 󰋜 nf-md-home
    pub const GALLERY: &str = "\u{f024f}";  // 󰉏 nf-md-folder_image
    pub const DESKTOP: &str = "\u{f0391}";  // 󰎑 nf-md-monitor_dashboard (approximation)
    pub const DOWNLOAD: &str = "\u{f01da}"; // 󰇚 nf-md-download
    pub const DOCUMENTS: &str = "\u{f0219}";// 󰈙 nf-md-file_document
    pub const MUSIC: &str = "\u{f0388}";    // 󰎈 nf-md-music_box
    pub const VIDEO: &str = "\u{f057e}";    // 󰕾 nf-md-video
    pub const PC: &str = "\u{f0379}";       // 󰍹 nf-md-monitor
    pub const DRIVE: &str = "\u{f02ca}";    // 󰋊 nf-md-harddisk
    pub const NETWORK: &str = "\u{f0317}";  // 󰌗 nf-md-lan
    pub const BACK: &str = "\u{f004d}";     // 󰁍 nf-md-arrow_left
    pub const FORWARD: &str = "\u{f0054}";  // 󰁔 nf-md-arrow_right
    pub const REFRESH: &str = "\u{f0450}";  // 󰑐 nf-md-refresh
    pub const SEARCH: &str = "\u{f0349}";   // 󰍉 nf-md-magnify
    pub const NEW: &str = "\u{f0415}";      // 󰐕 nf-md-plus
    pub const CUT: &str = "\u{f0190}";      // 󰆐 nf-md-content_cut
    pub const COPY: &str = "\u{f018f}";     // 󰆏 nf-md-content_copy
    pub const PASTE: &str = "\u{f0192}";    // 󰆒 nf-md-content_paste
    pub const SORT: &str = "\u{f04ba}";     // 󰒺 nf-md-sort
    pub const VIEW_GRID: &str = "\u{f0574}"; // 󰕴 nf-md-view_grid
    pub const VIEW_LIST: &str = "\u{f0575}"; // 󰕵 nf-md-view_list
    pub const MORE: &str = "\u{f01d8}";     // 󰇘 nf-md-dots_horizontal
    pub const ACTIONS: &str = "\u{f035b}";  // 󰍛 nf-md-menu
    pub const OPEN: &str = "\u{f0256}";     // 󰉖 nf-md-folder_open
    pub const RENAME: &str = "\u{f0ea8}";   // 󰺨 nf-md-rename_box
    pub const DELETE: &str = "\u{f01b4}";   // 󰆴 nf-md-delete
    pub const CLOSE: &str = "\u{f0156}";    // 󰅖 nf-md-close
    pub const THEME: &str = "\u{f05df}";    // 󰗟 nf-md-weather_night (dark mode toggle)

    // ── File type icons ────────────────────────────────────────────
    pub const FILE_CODE: &str = "\u{f0217}";    // 󰈗 nf-md-file_code
    pub const FILE_IMAGE: &str = "\u{f021f}";   // 󰈟 nf-md-file_image
    pub const FILE_MUSIC: &str = "\u{f0223}";   // 󰈣 nf-md-file_music
    pub const FILE_VIDEO: &str = "\u{f022b}";   // 󰈫 nf-md-file_video
    pub const FILE_PDF: &str = "\u{f0226}";     // 󰈦 nf-md-file_pdf_box
    pub const FILE_ARCHIVE: &str = "\u{f06fb}"; // 󰛻 nf-md-zip_box
    pub const FILE_TEXT: &str = "\u{f0219}";    // 󰈙 nf-md-file_document
    pub const FILE_TABLE: &str = "\u{f021b}";   // 󰈛 nf-md-file_excel
    pub const FILE_CONFIG: &str = "\u{f0493}";  // 󰒓 nf-md-settings
    pub const FILE_GIT: &str = "\u{f02a2}";     // 󰊢 nf-md-git
    pub const FILE_LOCK: &str = "\u{f033e}";    // 󰌾 nf-md-lock
    pub const FILE_FONT: &str = "\u{f031a}";    // 󰌚 nf-md-format_font
    pub const FILE_EXE: &str = "\u{f0214}";     // 󰈔 nf-md-file (kept same as default)
    pub const FILE_DB: &str = "\u{f01bc}";      // 󰆼 nf-md-database

    /// Returns the Nerd Font icon for a file based on its extension.
    pub fn icon_for_extension(ext: &str) -> &'static str {
        match ext.to_ascii_lowercase().as_str() {
            // Code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp"
            | "cs" | "java" | "go" | "rb" | "php" | "swift" | "kt" | "lua" | "zig"
            | "asm" | "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "r" | "dart"
            | "scala" | "html" | "htm" | "css" | "scss" | "sass" | "less" | "vue"
            | "svelte" => FILE_CODE,
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff"
            | "tif" | "psd" | "ai" | "raw" | "cr2" | "nef" | "heic" | "avif" => FILE_IMAGE,
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "aac" | "wma" | "m4a" | "opus" | "mid"
            | "midi" => FILE_MUSIC,
            // Video
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg"
            | "mpeg" | "3gp" => FILE_VIDEO,
            // Documents
            "pdf" => FILE_PDF,
            "txt" | "md" | "rtf" | "log" | "nfo" | "readme" => FILE_TEXT,
            "csv" | "xls" | "xlsx" | "ods" => FILE_TABLE,
            "doc" | "docx" | "odt" => FILE_TEXT,
            // Archives
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4"
            | "cab" | "iso" | "dmg" => FILE_ARCHIVE,
            // Config
            "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "cfg" | "conf"
            | "env" | "properties" => FILE_CONFIG,
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
        }
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
    pub title: u16,
    /// Body text font size.
    pub body: u16,
    /// Caption/small text font size.
    pub caption: u16,
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
            title: 16,
            body: 14,
            caption: 12,
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

impl UiColors {
    /// Dark theme color palette.
    pub fn dark() -> Self {
        Self {
            chrome_background: Color::from_rgb8(30, 30, 34),
            panel_background: Color::from_rgb8(36, 36, 42),
            border: Color::from_rgb8(58, 58, 68),
            sidebar_background: Color::from_rgb8(28, 28, 32),
            accent: Color::from_rgb8(78, 154, 240),
            text_primary: Color::from_rgb8(220, 222, 228),
            text_muted: Color::from_rgb8(160, 164, 174),
            selection: Color::from_rgb8(40, 56, 80),
            selection_border: Color::from_rgb8(60, 90, 130),
            hover: Color::from_rgb8(44, 44, 52),
            pressed: Color::from_rgb8(50, 50, 60),
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
    /// Returns tokens with the dark color palette.
    pub fn dark() -> Self {
        Self {
            colors: UiColors::dark(),
            ..Default::default()
        }
    }

    /// Returns tokens matching the dark_mode flag.
    pub fn for_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::dark()
        } else {
            Self::default()
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
}
