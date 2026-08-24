//! Pure utility functions for the Xion UI.
//!
//! This module contains stateless helper functions used by the main application
//! for formatting, layout calculations, tree building, and keyboard mapping.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use iced::{Length, Rectangle};

use crate::core::{
    EntryFilterConfig, KeyInput, KeyKind, NamedKey, ShortcutBindings, SortKeyConfig,
    SortOrderConfig, ViewColumn,
};
use crate::filesystem::{EntryFilter, FsEntry, FsEntryType, ListOptions, SortKey, SortOrder};
use crate::services::Thumbnail;
use crate::ui::KeyboardCommand;
use crate::ui::theme::layout::TREE_MAX_CHILDREN;
use iced::keyboard;

use super::{AnimatedFrame, AnimatedPreview, ColumnSpec};

// ── Formatting ────────────────────────────────────────────────────────────────

/// Truncate a filename for display, preserving the extension.
/// Returns the original name if it fits within `max_chars`.
pub fn truncate_name(name: &str, max_chars: usize) -> std::borrow::Cow<'_, str> {
    if name.chars().count() <= max_chars {
        return std::borrow::Cow::Borrowed(name);
    }
    if max_chars < 2 {
        return std::borrow::Cow::Borrowed(name);
    }
    // Preserve extension: "very_long_name.txt" -> "very_lon...txt"
    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos..]; // includes the dot
        let available = max_chars.saturating_sub(ext.len()).saturating_sub(1); // 1 for ellipsis
        if available > 0 {
            let stem: String = name.chars().take(available).collect();
            return std::borrow::Cow::Owned(format!("{stem}\u{2026}{ext}"));
        }
    }
    let truncated: String = name.chars().take(max_chars - 1).collect();
    std::borrow::Cow::Owned(format!("{truncated}\u{2026}"))
}

pub fn entry_type_label(entry_type: FsEntryType) -> &'static str {
    match entry_type {
        FsEntryType::Directory => "Dossier",
        FsEntryType::File => "Fichier",
        FsEntryType::Symlink => "Lien",
        FsEntryType::Other => "Autre",
    }
}

pub fn format_entry_size(entry: &FsEntry) -> String {
    match entry.entry_type {
        FsEntryType::Directory => "—".to_string(),
        _ => format_bytes(entry.metadata.size),
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let units = ["o", "Ko", "Mo", "Go", "To"];
    let mut size = bytes as f64;
    let mut index = 0;
    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} {}", units[index])
    } else {
        format!("{:.1} {}", size, units[index])
    }
}

pub fn format_modified(modified: Option<std::time::SystemTime>) -> String {
    modified
        .map(|time| {
            let datetime: DateTime<Local> = time.into();
            datetime.format("%d/%m/%Y %H:%M").to_string()
        })
        .unwrap_or_else(|| "—".to_string())
}

// ── Geometry ──────────────────────────────────────────────────────────────────

pub fn rectangles_intersect(a: Rectangle, b: Rectangle) -> bool {
    let epsilon = 0.01;
    let a_right = a.x + a.width;
    let a_bottom = a.y + a.height;
    let b_right = b.x + b.width;
    let b_bottom = b.y + b.height;
    a.x <= b_right + epsilon
        && a_right + epsilon >= b.x
        && a.y <= b_bottom + epsilon
        && a_bottom + epsilon >= b.y
}

// ── Config mapping ────────────────────────────────────────────────────────────

pub fn list_options_from_config(
    list_config: crate::core::ListConfig,
    respect_gitignore: bool,
) -> ListOptions {
    ListOptions {
        show_hidden: list_config.show_hidden,
        sort_by: match list_config.sort_key {
            SortKeyConfig::Name => SortKey::Name,
            SortKeyConfig::Modified => SortKey::Modified,
            SortKeyConfig::Size => SortKey::Size,
        },
        sort_order: match list_config.sort_order {
            SortOrderConfig::Asc => SortOrder::Asc,
            SortOrderConfig::Desc => SortOrder::Desc,
        },
        directories_first: list_config.directories_first,
        filter: match list_config.filter {
            EntryFilterConfig::All => EntryFilter::All,
            EntryFilterConfig::OnlyDirectories => EntryFilter::OnlyDirectories,
            EntryFilterConfig::OnlyFiles => EntryFilter::OnlyFiles,
        },
        name_query: None,
        respect_gitignore,
    }
}

// ── Column spec ───────────────────────────────────────────────────────────────

/// Builds the column layout, applying any width the user dragged a handle to.
///
/// `widths` used to be write-only: dragging a resize handle stored a number in
/// the config that nothing ever read back, so the column never moved.
pub fn column_specs(columns: &[ViewColumn], widths: &HashMap<String, f32>) -> Vec<ColumnSpec> {
    let mut specs: Vec<ColumnSpec> = columns.iter().map(ColumnSpec::from_column).collect();

    for spec in &mut specs {
        if let Some(&width) = widths.get(spec.column.key()) {
            spec.width = Length::Fixed(width);
        }
    }

    let has_fill = specs
        .iter()
        .any(|spec| matches!(spec.width, Length::Fill | Length::FillPortion(_)));
    if !has_fill {
        // Something has to absorb the leftover space or the table stops short of
        // the pane edge. The last column takes it, so a width the user chose for
        // an earlier column survives.
        if let Some(last) = specs.last_mut() {
            last.width = Length::Fill;
        }
    }

    specs
}

// ── GIF / animated preview ────────────────────────────────────────────────────

pub fn is_gif_preview(preview: &Thumbnail) -> bool {
    preview.mime.as_deref() == Some("image/gif")
        || preview.bytes.starts_with(b"GIF87a")
        || preview.bytes.starts_with(b"GIF89a")
}

/// Longest animation kept in memory.
///
/// Every retained frame is an uncompressed RGBA buffer. Without a cap, a
/// 500x500 GIF of 100 frames materialised 95 MiB of handles for one preview.
const MAX_ANIMATED_FRAMES: usize = 60;

/// Largest edge an animation frame is scaled to before becoming a handle.
///
/// Frames used to be converted at their source resolution even though the
/// preview panel draws them a few hundred pixels wide.
const MAX_ANIMATED_EDGE: u32 = 256;

pub fn build_animated_preview(path: PathBuf, preview: &Thumbnail) -> Option<AnimatedPreview> {
    use ::image::AnimationDecoder;
    use ::image::codecs::gif::GifDecoder;
    use ::image::imageops::FilterType;
    use std::io::Cursor;

    let decoder = GifDecoder::new(Cursor::new(preview.bytes.as_ref())).ok()?;
    // `collect_frames` would decode the whole animation before we could refuse
    // any of it; taking from the iterator stops at the cap instead.
    let frames: Vec<_> = decoder
        .into_frames()
        .take(MAX_ANIMATED_FRAMES)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if frames.is_empty() {
        return None;
    }

    let animated_frames: Vec<AnimatedFrame> = frames
        .into_iter()
        .map(|frame| {
            let delay = gif_frame_delay(&frame);
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            // Scale down before allocating the handle, not after: the preview
            // panel never draws these larger than a few hundred pixels.
            let buffer = if width.max(height) > MAX_ANIMATED_EDGE {
                ::image::imageops::resize(
                    &buffer,
                    (width * MAX_ANIMATED_EDGE / width.max(height)).max(1),
                    (height * MAX_ANIMATED_EDGE / width.max(height)).max(1),
                    FilterType::Triangle,
                )
            } else {
                buffer
            };
            let (width, height) = buffer.dimensions();
            let handle = iced::widget::image::Handle::from_rgba(width, height, buffer.into_raw());
            AnimatedFrame { handle, delay }
        })
        .collect();

    let first = animated_frames.first()?;
    let first_handle = first.handle.clone();
    let first_delay = first.delay;
    Some(AnimatedPreview {
        path,
        frames: animated_frames,
        current: 0,
        next_frame_at: Instant::now() + first_delay,
        handle: first_handle,
    })
}

pub fn gif_frame_delay(frame: &::image::Frame) -> Duration {
    let delay = frame.delay();
    let (numer, denom) = delay.numer_denom_ms();
    let ms = if denom == 0 {
        0.0_f64
    } else {
        (numer as f64) / (denom as f64)
    };
    Duration::from_millis(ms.round().max(20.0) as u64)
}

// ── Tree view ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TreeNode {
    pub path: PathBuf,
    pub label: String,
    pub depth: usize,
    pub expanded: bool,
    pub selected: bool,
}

pub fn tree_label_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|label| !label.is_empty())
        .map(|label| label.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn build_tree_nodes(
    root: &Path,
    current_path: &Path,
    max_depth: usize,
    options: &ListOptions,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    if !root.exists() {
        return nodes;
    }
    collect_tree_nodes(root, current_path, 0, max_depth, options, &mut nodes);
    nodes
}

fn collect_tree_nodes(
    path: &Path,
    current_path: &Path,
    depth: usize,
    max_depth: usize,
    options: &ListOptions,
    nodes: &mut Vec<TreeNode>,
) {
    let expanded = current_path.starts_with(path) && depth < max_depth;
    nodes.push(TreeNode {
        path: path.to_path_buf(),
        label: tree_label_for_path(path),
        depth,
        expanded,
        selected: current_path == path,
    });

    if !expanded {
        return;
    }

    // Read directory entries directly — we only need names and types,
    // not metadata, so we skip the full list_dir + metadata_batch pipeline.
    let read_dir = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut directories: Vec<(PathBuf, String)> = read_dir
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let ft = entry.file_type().ok()?;
            if !ft.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !options.show_hidden && name.starts_with('.') {
                return None;
            }
            Some((entry.path(), name))
        })
        .collect();
    directories.sort_by_key(|a| a.1.to_lowercase());

    for (dir_path, _) in directories.into_iter().take(TREE_MAX_CHILDREN) {
        collect_tree_nodes(
            &dir_path,
            current_path,
            depth + 1,
            max_depth,
            options,
            nodes,
        );
    }
}

// ── Keyboard ──────────────────────────────────────────────────────────────────

pub fn command_from_key_press_with_shortcuts(
    shortcuts: &ShortcutBindings,
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<KeyboardCommand> {
    let input = key_input_from_event(key, modifiers)?;
    let extend = modifiers.shift();

    macro_rules! check_shortcut {
        ($binding:expr, $command:expr, $ignore_shift:expr) => {
            if $binding.matches(&input, $ignore_shift) {
                return Some($command);
            }
        };
    }
    check_shortcut!(shortcuts.move_up, KeyboardCommand::MoveUp { extend }, true);
    check_shortcut!(
        shortcuts.move_down,
        KeyboardCommand::MoveDown { extend },
        true
    );
    check_shortcut!(
        shortcuts.move_home,
        KeyboardCommand::MoveHome { extend },
        true
    );
    check_shortcut!(
        shortcuts.move_end,
        KeyboardCommand::MoveEnd { extend },
        true
    );
    check_shortcut!(shortcuts.activate, KeyboardCommand::Activate, false);
    check_shortcut!(
        shortcuts.clear_selection,
        KeyboardCommand::ClearSelection,
        false
    );
    check_shortcut!(
        shortcuts.cycle_pane_focus,
        KeyboardCommand::CyclePaneFocus,
        false
    );
    check_shortcut!(shortcuts.back, KeyboardCommand::Back, false);
    check_shortcut!(shortcuts.forward, KeyboardCommand::Forward, false);
    check_shortcut!(shortcuts.refresh, KeyboardCommand::Refresh, false);
    check_shortcut!(shortcuts.select_all, KeyboardCommand::SelectAll, false);
    check_shortcut!(
        shortcuts.toggle_context_menu,
        KeyboardCommand::ToggleContextMenu,
        false
    );
    check_shortcut!(shortcuts.rename, KeyboardCommand::Rename, false);
    // Shift+Delete is the Explorer convention for "skip the recycle bin".
    // Checked before the plain binding so the modifier is not swallowed.
    if matches!(input.key, KeyKind::Named(NamedKey::Delete))
        && input.shift
        && !input.ctrl
        && !input.alt
    {
        return Some(KeyboardCommand::DeletePermanently);
    }
    check_shortcut!(shortcuts.delete, KeyboardCommand::Delete, false);
    check_shortcut!(shortcuts.new_folder, KeyboardCommand::NewFolder, false);
    check_shortcut!(shortcuts.focus_search, KeyboardCommand::FocusSearch, false);
    // F5 as alternate refresh (standard Windows shortcut)
    if matches!(input.key, KeyKind::Named(NamedKey::F5)) && !input.ctrl && !input.alt {
        return Some(KeyboardCommand::Refresh);
    }

    // Space → QuickLook (no modifiers)
    if matches!(&input.key, KeyKind::Named(NamedKey::Space))
        && !input.ctrl
        && !input.alt
        && !input.shift
    {
        return Some(KeyboardCommand::QuickLook);
    }

    // Shift+F2 → BulkRename
    if matches!(&input.key, KeyKind::Named(NamedKey::F2))
        && input.shift
        && !input.ctrl
        && !input.alt
    {
        return Some(KeyboardCommand::BulkRename);
    }

    // F3 → ToggleDualPane
    if matches!(&input.key, KeyKind::Named(NamedKey::F3))
        && !input.ctrl
        && !input.alt
        && !input.shift
    {
        return Some(KeyboardCommand::ToggleDualPane);
    }

    // Ctrl+L → FocusAddress
    if input.ctrl && !input.alt && !input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c == "l" {
                return Some(KeyboardCommand::FocusAddress);
            }
        }
    }

    // Alt+↑ → GoToParent
    if !input.ctrl
        && input.alt
        && !input.shift
        && matches!(&input.key, KeyKind::Named(NamedKey::ArrowUp))
    {
        return Some(KeyboardCommand::GoToParent);
    }

    // Ctrl+Z → Undo
    if input.ctrl && !input.alt && !input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c == "z" {
                return Some(KeyboardCommand::Undo);
            }
        }
    }

    // Tab management shortcuts (hardcoded, not user-configurable)
    if input.ctrl && !input.alt {
        match &input.key {
            KeyKind::Character(c) if c == "t" && !input.shift => {
                return Some(KeyboardCommand::NewTab);
            }
            KeyKind::Character(c) if c == "w" && !input.shift => {
                return Some(KeyboardCommand::CloseCurrentTab);
            }
            KeyKind::Named(NamedKey::Tab) => {
                return Some(if input.shift {
                    KeyboardCommand::PrevTab
                } else {
                    KeyboardCommand::NextTab
                });
            }
            _ => {}
        }
    }

    // Tab without Ctrl → SwitchActivePane (only handled in app based on dual_pane state)
    if matches!(&input.key, KeyKind::Named(NamedKey::Tab))
        && !input.ctrl
        && !input.alt
        && !input.shift
    {
        // CyclePaneFocus handled via shortcut binding; this fallback for SwitchActivePane
        // is handled via the CyclePaneFocus binding in the default config (Tab = CyclePaneFocus)
        // so we don't emit SwitchActivePane here; the app handles it contextually
    }

    // Ctrl+D → OpenDiff (when 2 files selected)
    if input.ctrl && !input.alt && !input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c == "d" {
                return Some(KeyboardCommand::OpenDiff);
            }
        }
    }

    // Ctrl+Shift+F → OpenGrep
    if input.ctrl && !input.alt && input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c == "f" {
                return Some(KeyboardCommand::OpenGrep);
            }
        }
    }

    // Ctrl+Shift+C → CopyPath
    if input.ctrl && !input.alt && input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c == "c" {
                return Some(KeyboardCommand::CopyPath);
            }
        }
    }

    // Ctrl+1..9 → GoToBookmark (jump to numbered favorite)
    if input.ctrl && !input.alt && !input.shift {
        if let KeyKind::Character(c) = &input.key {
            if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10)) {
                if (1..=9).contains(&digit) {
                    return Some(KeyboardCommand::GoToBookmark(digit as usize - 1));
                }
            }
        }
    }

    // Quick filter: printable single character, no modifiers
    if !input.ctrl && !input.alt && !input.shift {
        if let KeyKind::Character(c) = &input.key {
            if c.len() == 1
                && c.chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
            {
                return Some(KeyboardCommand::QuickFilterChanged(c.clone()));
            }
        }
    }

    // Escape clears quick filter (handled in mod.rs via ClearSelection pattern)

    None
}

pub fn key_input_from_event(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<KeyInput> {
    let key_kind = match key {
        keyboard::Key::Named(named) => match named {
            keyboard::key::Named::ArrowUp => KeyKind::Named(NamedKey::ArrowUp),
            keyboard::key::Named::ArrowDown => KeyKind::Named(NamedKey::ArrowDown),
            keyboard::key::Named::ArrowLeft => KeyKind::Named(NamedKey::ArrowLeft),
            keyboard::key::Named::ArrowRight => KeyKind::Named(NamedKey::ArrowRight),
            keyboard::key::Named::Home => KeyKind::Named(NamedKey::Home),
            keyboard::key::Named::End => KeyKind::Named(NamedKey::End),
            keyboard::key::Named::Enter => KeyKind::Named(NamedKey::Enter),
            keyboard::key::Named::Escape => KeyKind::Named(NamedKey::Escape),
            keyboard::key::Named::Tab => KeyKind::Named(NamedKey::Tab),
            keyboard::key::Named::Space => KeyKind::Named(NamedKey::Space),
            keyboard::key::Named::F2 => KeyKind::Named(NamedKey::F2),
            keyboard::key::Named::F3 => KeyKind::Named(NamedKey::F3),
            keyboard::key::Named::F5 => KeyKind::Named(NamedKey::F5),
            keyboard::key::Named::Delete => KeyKind::Named(NamedKey::Delete),
            _ => return None,
        },
        keyboard::Key::Character(character) => {
            let normalized = character.to_lowercase();
            if normalized.is_empty() {
                return None;
            }
            KeyKind::Character(normalized)
        }
        _ => return None,
    };

    Some(KeyInput {
        key: key_kind,
        ctrl: modifiers.control(),
        alt: modifiers.alt(),
        shift: modifiers.shift(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_name_unchanged() {
        assert_eq!(truncate_name("hello.txt", 20), "hello.txt");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate_name("abc.txt", 7), "abc.txt");
    }

    #[test]
    fn truncate_long_name_preserves_extension() {
        let result = truncate_name("very_long_filename.txt", 12);
        assert!(result.ends_with(".txt"), "Should preserve .txt: {result}");
        assert!(
            result.chars().count() <= 12,
            "Should be <= 12 chars: {result}"
        );
        assert!(
            result.contains('\u{2026}'),
            "Should contain ellipsis: {result}"
        );
    }

    #[test]
    fn truncate_tiny_max() {
        assert_eq!(truncate_name("hello.txt", 1), "hello.txt");
    }

    #[test]
    fn truncate_no_extension() {
        let result = truncate_name("a_very_long_name_without_extension", 10);
        assert!(result.chars().count() <= 10);
        assert!(result.contains('\u{2026}'));
    }

    #[test]
    fn format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 o");
    }

    #[test]
    fn format_bytes_small() {
        assert_eq!(format_bytes(512), "512 o");
    }

    #[test]
    fn format_bytes_kilobytes() {
        assert_eq!(format_bytes(1024), "1.0 Ko");
    }

    #[test]
    fn format_bytes_megabytes() {
        assert_eq!(format_bytes(1_048_576), "1.0 Mo");
    }

    #[test]
    fn format_bytes_gigabytes() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 Go");
    }

    #[test]
    fn format_modified_none() {
        assert_eq!(format_modified(None), "—");
    }

    #[test]
    fn format_modified_some() {
        let result = format_modified(Some(std::time::SystemTime::now()));
        assert!(result.contains('/'));
        assert!(result.contains(':'));
    }

    #[test]
    fn rect_intersect_overlapping() {
        let a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let b = Rectangle {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        };
        assert!(rectangles_intersect(a, b));
    }

    #[test]
    fn rect_no_intersect() {
        let a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let b = Rectangle {
            x: 20.0,
            y: 20.0,
            width: 10.0,
            height: 10.0,
        };
        assert!(!rectangles_intersect(a, b));
    }

    #[test]
    fn entry_type_labels() {
        assert_eq!(entry_type_label(FsEntryType::Directory), "Dossier");
        assert_eq!(entry_type_label(FsEntryType::File), "Fichier");
    }

    #[test]
    fn tree_label_normal() {
        assert_eq!(tree_label_for_path(Path::new("/home/user/docs")), "docs");
    }

    #[test]
    fn column_specs_ensures_fill() {
        let specs = column_specs(&[ViewColumn::Name, ViewColumn::Size], &HashMap::new());
        assert!(matches!(
            specs[0].width,
            Length::Fill | Length::FillPortion(_)
        ));
    }

    #[test]
    fn column_specs_empty() {
        assert!(column_specs(&[], &HashMap::new()).is_empty());
    }

    #[test]
    fn column_specs_apply_stored_width() {
        let mut widths = HashMap::new();
        widths.insert("Name".to_string(), 220.0);
        let specs = column_specs(&[ViewColumn::Name, ViewColumn::Size], &widths);
        assert!(matches!(specs[0].width, Length::Fixed(w) if w == 220.0));
    }

    #[test]
    fn column_specs_fill_falls_on_the_last_column() {
        // Resizing the only flexible column must not be undone by the fallback.
        let mut widths = HashMap::new();
        widths.insert("Name".to_string(), 220.0);
        let specs = column_specs(&[ViewColumn::Name, ViewColumn::Size], &widths);
        assert!(matches!(specs[0].width, Length::Fixed(_)));
        assert!(matches!(specs[1].width, Length::Fill));
    }
}

/// Largest text file the preview, diff and hex views will load.
///
/// Reads here used to be unbounded: opening an 8 GB log allocated 8 GB before
/// the UI could refuse it.
pub(super) const MAX_TEXT_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;

/// Largest file the animated-image preview will load.
pub(super) const MAX_PREVIEW_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Read at most `max_bytes` of a file, refusing outright anything larger.
///
/// The size is checked before allocating, so a huge file costs one `stat` and
/// not a multi-gigabyte buffer. `Read::take` still bounds the copy in case the
/// file grows between the check and the read.
pub(super) fn read_file_capped(path: &std::path::Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "fichier trop volumineux ({}) — limite {}",
            format_bytes(metadata.len()),
            format_bytes(max_bytes)
        ));
    }

    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut buffer = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes)
        .read_to_end(&mut buffer)
        .map_err(|error| error.to_string())?;
    Ok(buffer)
}

/// Same as [`read_file_capped`], decoding the result as UTF-8 lossily.
pub(super) fn read_text_capped(path: &std::path::Path, max_bytes: u64) -> Result<String, String> {
    let bytes = read_file_capped(path, max_bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod animated_preview_tests {
    use super::*;

    /// Encode a GIF of `frames` frames at `size` x `size`.
    fn gif(frames: usize, size: u32) -> Vec<u8> {
        use ::image::codecs::gif::GifEncoder;
        use ::image::{Delay, Frame, RgbaImage};
        use std::time::Duration;

        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            let made: Vec<Frame> = (0..frames)
                .map(|index| {
                    let shade = (index % 256) as u8;
                    let image =
                        RgbaImage::from_pixel(size, size, ::image::Rgba([shade, 0, 0, 255]));
                    Frame::from_parts(
                        image,
                        0,
                        0,
                        Delay::from_saturating_duration(Duration::from_millis(40)),
                    )
                })
                .collect();
            encoder.encode_frames(made).unwrap();
        }
        bytes
    }

    /// Regression: every frame used to become an RGBA handle at source
    /// resolution, with no cap. A 500x500 animation of 100 frames materialised
    /// 95 MiB for one preview.
    #[test]
    fn frame_count_is_capped() {
        let preview = Thumbnail::new(
            gif(MAX_ANIMATED_FRAMES + 25, 8),
            Some("image/gif".to_string()),
        );
        let animated = build_animated_preview(std::path::PathBuf::from("/tmp/a.gif"), &preview)
            .expect("l'animation doit être construite");

        assert_eq!(animated.frames.len(), MAX_ANIMATED_FRAMES);
    }

    #[test]
    fn a_short_animation_keeps_all_its_frames() {
        let preview = Thumbnail::new(gif(5, 8), Some("image/gif".to_string()));
        let animated = build_animated_preview(std::path::PathBuf::from("/tmp/a.gif"), &preview)
            .expect("l'animation doit être construite");

        assert_eq!(animated.frames.len(), 5);
    }

    /// Frames larger than the preview panel are scaled before allocation.
    #[test]
    fn oversized_frames_are_scaled_down() {
        let big = MAX_ANIMATED_EDGE * 2;
        let preview = Thumbnail::new(gif(2, big), Some("image/gif".to_string()));
        let animated = build_animated_preview(std::path::PathBuf::from("/tmp/a.gif"), &preview)
            .expect("l'animation doit être construite");

        // The handle carries its dimensions; a scaled frame is at most the cap.
        for frame in &animated.frames {
            if let iced::widget::image::Handle::Rgba { width, height, .. } = &frame.handle {
                assert!(
                    *width <= MAX_ANIMATED_EDGE && *height <= MAX_ANIMATED_EDGE,
                    "trame non redimensionnée : {width}x{height}"
                );
            }
        }
    }
}

#[cfg(test)]
mod capped_read_tests {
    use super::*;

    /// Regression: preview and diff used to `read` the whole file, so an 8 GB
    /// log allocated 8 GB before anything could refuse it.
    #[test]
    fn a_file_over_the_cap_is_refused_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.log");
        std::fs::write(&path, vec![b'x'; 4096]).unwrap();

        let error = read_file_capped(&path, 1024).unwrap_err();

        assert!(error.contains("trop volumineux"), "message : {error}");
    }

    #[test]
    fn a_file_under_the_cap_is_read_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.txt");
        std::fs::write(&path, b"bonjour").unwrap();

        assert_eq!(read_file_capped(&path, 1024).unwrap(), b"bonjour");
        assert_eq!(read_text_capped(&path, 1024).unwrap(), "bonjour");
    }

    #[test]
    fn a_missing_file_reports_an_error_rather_than_an_empty_result() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_text_capped(&dir.path().join("absent"), 1024).is_err());
    }

    #[test]
    fn invalid_utf8_is_decoded_lossily_not_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latin1.txt");
        std::fs::write(&path, [0x61, 0xFF, 0x62]).unwrap();

        let text = read_text_capped(&path, 1024).unwrap();

        assert!(text.starts_with('a') && text.ends_with('b'));
    }
}
