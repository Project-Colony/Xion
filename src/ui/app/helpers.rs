//! Pure utility functions for the Xion UI.
//!
//! This module contains stateless helper functions used by the main application
//! for formatting, layout calculations, tree building, and keyboard mapping.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use iced::{Length, Rectangle};

use crate::core::{
    EntryFilterConfig, KeyInput, KeyKind, NamedKey, ShortcutBindings,
    SortKeyConfig, SortOrderConfig, ViewColumn,
};
use crate::filesystem::{EntryFilter, FsEntry, FsEntryType, ListOptions, SortKey, SortOrder};
use crate::ui::KeyboardCommand;
use crate::services::Thumbnail;
use crate::ui::theme::layout::TREE_MAX_CHILDREN;
use iced::keyboard;

use super::{AnimatedFrame, AnimatedPreview, ColumnSpec};

// ── Formatting ────────────────────────────────────────────────────────────────

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

pub fn list_options_from_config(list_config: crate::core::ListConfig) -> ListOptions {
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
    }
}

// ── Column spec ───────────────────────────────────────────────────────────────

pub fn column_specs(columns: &[ViewColumn]) -> Vec<ColumnSpec> {
    let mut specs: Vec<ColumnSpec> = columns.iter().map(ColumnSpec::from_column).collect();

    let has_fill = specs
        .iter()
        .any(|spec| matches!(spec.width, Length::Fill | Length::FillPortion(_)));
    if !has_fill {
        if let Some(first) = specs.first_mut() {
            first.width = Length::Fill;
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

pub fn build_animated_preview(
    path: PathBuf,
    preview: &Thumbnail,
) -> Option<AnimatedPreview> {
    use std::io::Cursor;
    use ::image::codecs::gif::GifDecoder;
    use ::image::AnimationDecoder;

    let decoder = GifDecoder::new(Cursor::new(preview.bytes.as_slice())).ok()?;
    let frames = decoder.into_frames().collect_frames().ok()?;
    if frames.is_empty() {
        return None;
    }

    let animated_frames: Vec<AnimatedFrame> = frames
        .into_iter()
        .map(|frame| {
            let delay = gif_frame_delay(&frame);
            let buffer = frame.into_buffer();
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
        0
    } else {
        (numer as u64) / (denom as u64)
    };
    Duration::from_millis(ms.max(20))
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
    directories.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    for (dir_path, _) in directories.into_iter().take(TREE_MAX_CHILDREN) {
        collect_tree_nodes(&dir_path, current_path, depth + 1, max_depth, options, nodes);
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

    if shortcuts.move_up.matches(&input, true) {
        return Some(KeyboardCommand::MoveUp { extend });
    }
    if shortcuts.move_down.matches(&input, true) {
        return Some(KeyboardCommand::MoveDown { extend });
    }
    if shortcuts.move_home.matches(&input, true) {
        return Some(KeyboardCommand::MoveHome { extend });
    }
    if shortcuts.move_end.matches(&input, true) {
        return Some(KeyboardCommand::MoveEnd { extend });
    }
    if shortcuts.activate.matches(&input, false) {
        return Some(KeyboardCommand::Activate);
    }
    if shortcuts.clear_selection.matches(&input, false) {
        return Some(KeyboardCommand::ClearSelection);
    }
    if shortcuts.cycle_pane_focus.matches(&input, false) {
        return Some(KeyboardCommand::CyclePaneFocus);
    }
    if shortcuts.back.matches(&input, false) {
        return Some(KeyboardCommand::Back);
    }
    if shortcuts.forward.matches(&input, false) {
        return Some(KeyboardCommand::Forward);
    }
    if shortcuts.refresh.matches(&input, false) {
        return Some(KeyboardCommand::Refresh);
    }
    if shortcuts.select_all.matches(&input, false) {
        return Some(KeyboardCommand::SelectAll);
    }
    if shortcuts.toggle_context_menu.matches(&input, false) {
        return Some(KeyboardCommand::ToggleContextMenu);
    }
    if shortcuts.rename.matches(&input, false) {
        return Some(KeyboardCommand::Rename);
    }
    if shortcuts.delete.matches(&input, false) {
        return Some(KeyboardCommand::Delete);
    }
    if shortcuts.new_folder.matches(&input, false) {
        return Some(KeyboardCommand::NewFolder);
    }
    if shortcuts.focus_search.matches(&input, false) {
        return Some(KeyboardCommand::FocusSearch);
    }
    // F5 as alternate refresh (standard Windows shortcut)
    if matches!(input.key, KeyKind::Named(NamedKey::F5)) && !input.ctrl && !input.alt {
        return Some(KeyboardCommand::Refresh);
    }

    None
}

pub fn key_input_from_event(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<KeyInput> {
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
            keyboard::key::Named::F2 => KeyKind::Named(NamedKey::F2),
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
