//! Parsing and rendering key chords in the config file.

use crate::core::config::shortcuts::{KeyChord, KeyKind, NamedKey, ShortcutBindings};
use crate::core::config::types::ConfigWarning;

use super::*;
// ── Shortcut merge / parse ────────────────────────────────────────────────────

pub(super) fn merge_shortcuts(
    shortcuts: ShortcutBindingsFile,
    mut fallback: ShortcutBindings,
    warnings: &mut Vec<ConfigWarning>,
) -> ShortcutBindings {
    macro_rules! merge_field {
        ($field:ident, $label:expr) => {
            if let Some(value) = shortcuts.$field {
                fallback.$field = parse_shortcut(value, fallback.$field, $label, warnings);
            }
        };
    }
    merge_field!(move_up, "shortcuts.move_up");
    merge_field!(move_down, "shortcuts.move_down");
    merge_field!(move_home, "shortcuts.move_home");
    merge_field!(move_end, "shortcuts.move_end");
    merge_field!(activate, "shortcuts.activate");
    merge_field!(clear_selection, "shortcuts.clear_selection");
    merge_field!(cycle_pane_focus, "shortcuts.cycle_pane_focus");
    merge_field!(back, "shortcuts.back");
    merge_field!(forward, "shortcuts.forward");
    merge_field!(refresh, "shortcuts.refresh");
    merge_field!(select_all, "shortcuts.select_all");
    merge_field!(toggle_context_menu, "shortcuts.toggle_context_menu");
    merge_field!(rename, "shortcuts.rename");
    merge_field!(delete, "shortcuts.delete");
    merge_field!(new_folder, "shortcuts.new_folder");
    merge_field!(focus_search, "shortcuts.focus_search");
    fallback
}

pub(super) fn parse_shortcut(
    raw: String,
    fallback: KeyChord,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> KeyChord {
    match KeyChord::parse(&raw) {
        Ok(chord) => chord,
        Err(error) => {
            warnings.push(ConfigWarning {
                message: format!("{label} invalide ({error}), fallback sur valeur par défaut"),
            });
            fallback
        }
    }
}

// ── Serialisation ─────────────────────────────────────────────────────────────

pub(super) fn chord_to_string(chord: &KeyChord) -> String {
    let mut parts: Vec<String> = Vec::new();
    if chord.ctrl {
        parts.push("Ctrl".to_string());
    }
    if chord.alt {
        parts.push("Alt".to_string());
    }
    if chord.shift {
        parts.push("Shift".to_string());
    }
    let key_str = match &chord.key {
        KeyKind::Named(named) => match named {
            NamedKey::ArrowUp => "ArrowUp",
            NamedKey::ArrowDown => "ArrowDown",
            NamedKey::ArrowLeft => "ArrowLeft",
            NamedKey::ArrowRight => "ArrowRight",
            NamedKey::Home => "Home",
            NamedKey::End => "End",
            NamedKey::Enter => "Enter",
            NamedKey::Escape => "Escape",
            NamedKey::Tab => "Tab",
            NamedKey::Space => "Space",
            NamedKey::F2 => "F2",
            NamedKey::F3 => "F3",
            NamedKey::F5 => "F5",
            NamedKey::Delete => "Delete",
        }
        .to_string(),
        KeyKind::Character(c) => c.clone(),
    };
    parts.push(key_str);
    parts.join("+")
}
