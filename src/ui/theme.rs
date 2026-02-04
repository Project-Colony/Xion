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
    pub const DEVICE: &str = "";
    pub const LOADING: &str = "";
    pub const FOLDER: &str = "";
    pub const FILE: &str = "";
    pub const SYMLINK: &str = "";
    pub const UNKNOWN: &str = "";
    pub const HOME: &str = "";
    pub const GALLERY: &str = "";
    pub const DESKTOP: &str = "";
    pub const DOWNLOAD: &str = "";
    pub const DOCUMENTS: &str = "";
    pub const MUSIC: &str = "";
    pub const VIDEO: &str = "";
    pub const PC: &str = "";
    pub const DRIVE: &str = "";
    pub const NETWORK: &str = "";
    pub const BACK: &str = "";
    pub const FORWARD: &str = "";
    pub const REFRESH: &str = "";
    pub const SEARCH: &str = "";
    pub const NEW: &str = "";
    pub const CUT: &str = "";
    pub const COPY: &str = "";
    pub const PASTE: &str = "";
    pub const SORT: &str = "";
    pub const VIEW: &str = "";
    pub const MORE: &str = "";
    pub const ACTIONS: &str = "";
    pub const OPEN: &str = "";
    pub const RENAME: &str = "";
    pub const DELETE: &str = "";
    pub const CLOSE: &str = "";
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
