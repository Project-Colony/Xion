//! Keyboard shortcut types: key representations, chord matching, and default bindings.

/// Logical named keys used in shortcut bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Enter,
    Escape,
    Tab,
    Space,
    F2,
    F3,
    F5,
    Delete,
}

/// A key, either a named key or a single character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyKind {
    Named(NamedKey),
    Character(String),
}

/// A keyboard shortcut: a key plus optional modifier keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChord {
    pub key: KeyKind,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyChord {
    /// Parses a shortcut string like `"Ctrl+Shift+N"` or `"Alt+ArrowLeft"`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut key_token = None;

        for token in raw
            .split('+')
            .map(|token| token.trim())
            .filter(|t| !t.is_empty())
        {
            let lower = token.to_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                _ => {
                    key_token = Some(token.to_string());
                }
            }
        }

        let key_token = key_token.ok_or_else(|| "Touche manquante".to_string())?;
        let key = match key_token.to_lowercase().as_str() {
            "arrowup" => KeyKind::Named(NamedKey::ArrowUp),
            "arrowdown" => KeyKind::Named(NamedKey::ArrowDown),
            "arrowleft" => KeyKind::Named(NamedKey::ArrowLeft),
            "arrowright" => KeyKind::Named(NamedKey::ArrowRight),
            "home" => KeyKind::Named(NamedKey::Home),
            "end" => KeyKind::Named(NamedKey::End),
            "enter" => KeyKind::Named(NamedKey::Enter),
            "escape" | "esc" => KeyKind::Named(NamedKey::Escape),
            "tab" => KeyKind::Named(NamedKey::Tab),
            "space" => KeyKind::Named(NamedKey::Space),
            "f2" => KeyKind::Named(NamedKey::F2),
            "f3" => KeyKind::Named(NamedKey::F3),
            "f5" => KeyKind::Named(NamedKey::F5),
            "delete" | "del" => KeyKind::Named(NamedKey::Delete),
            other => {
                if other.chars().count() == 1 {
                    KeyKind::Character(other.to_string())
                } else {
                    return Err(format!("Touche inconnue: {other}"));
                }
            }
        };

        Ok(Self { key, ctrl, alt, shift })
    }

    /// Returns `true` if this chord matches the given key input.
    ///
    /// `allow_shift_override` — when `true`, a chord without `shift` will match
    /// even if the input has shift held (useful for uppercase letter shortcuts).
    pub fn matches(&self, input: &KeyInput, allow_shift_override: bool) -> bool {
        let shift_matches = if allow_shift_override && !self.shift {
            true
        } else {
            self.shift == input.shift
        };
        self.ctrl == input.ctrl && self.alt == input.alt && shift_matches && self.key == input.key
    }
}

/// A key input event (from the keyboard driver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    pub key: KeyKind,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Default keyboard shortcut bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBindings {
    pub move_up: KeyChord,
    pub move_down: KeyChord,
    pub move_home: KeyChord,
    pub move_end: KeyChord,
    pub activate: KeyChord,
    pub clear_selection: KeyChord,
    pub cycle_pane_focus: KeyChord,
    pub back: KeyChord,
    pub forward: KeyChord,
    pub refresh: KeyChord,
    pub select_all: KeyChord,
    pub toggle_context_menu: KeyChord,
    pub rename: KeyChord,
    pub delete: KeyChord,
    pub new_folder: KeyChord,
    pub focus_search: KeyChord,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            move_up: KeyChord {
                key: KeyKind::Named(NamedKey::ArrowUp),
                ctrl: false, alt: false, shift: false,
            },
            move_down: KeyChord {
                key: KeyKind::Named(NamedKey::ArrowDown),
                ctrl: false, alt: false, shift: false,
            },
            move_home: KeyChord {
                key: KeyKind::Named(NamedKey::Home),
                ctrl: false, alt: false, shift: false,
            },
            move_end: KeyChord {
                key: KeyKind::Named(NamedKey::End),
                ctrl: false, alt: false, shift: false,
            },
            activate: KeyChord {
                key: KeyKind::Named(NamedKey::Enter),
                ctrl: false, alt: false, shift: false,
            },
            clear_selection: KeyChord {
                key: KeyKind::Named(NamedKey::Escape),
                ctrl: false, alt: false, shift: false,
            },
            cycle_pane_focus: KeyChord {
                key: KeyKind::Named(NamedKey::Tab),
                ctrl: false, alt: false, shift: false,
            },
            back: KeyChord {
                key: KeyKind::Named(NamedKey::ArrowLeft),
                ctrl: false, alt: true, shift: false,
            },
            forward: KeyChord {
                key: KeyKind::Named(NamedKey::ArrowRight),
                ctrl: false, alt: true, shift: false,
            },
            refresh: KeyChord {
                key: KeyKind::Character("r".to_string()),
                ctrl: true, alt: false, shift: false,
            },
            select_all: KeyChord {
                key: KeyKind::Character("a".to_string()),
                ctrl: true, alt: false, shift: false,
            },
            toggle_context_menu: KeyChord {
                key: KeyKind::Character("m".to_string()),
                ctrl: true, alt: false, shift: false,
            },
            rename: KeyChord {
                key: KeyKind::Named(NamedKey::F2),
                ctrl: false, alt: false, shift: false,
            },
            delete: KeyChord {
                key: KeyKind::Named(NamedKey::Delete),
                ctrl: false, alt: false, shift: false,
            },
            new_folder: KeyChord {
                key: KeyKind::Character("n".to_string()),
                ctrl: true, alt: false, shift: true,
            },
            focus_search: KeyChord {
                key: KeyKind::Character("e".to_string()),
                ctrl: true, alt: false, shift: false,
            },
        }
    }
}
