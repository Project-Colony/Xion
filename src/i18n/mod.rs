//! The interface text, in English and French.
//!
//! English (`en.json`) is canonical and French (`fr.json`) is a complete
//! translation; a test fails when their keys differ. The shared Preferences
//! labels (theme names, accents, the standard categories) are not copied here:
//! [`t`] falls through to `colony_ui::i18n`, which owns them.
//!
//! There is one active locale for the whole process, colony-ui's, so Xion's
//! strings and the shared widgets can never disagree on the language.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

use colony_ui::i18n::Locale;

use crate::core::Language;

const EN: &str = include_str!("en.json");
const FR: &str = include_str!("fr.json");

fn table(locale: Locale) -> &'static HashMap<String, String> {
    static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    static FR_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();

    let (cell, source) = match locale {
        Locale::En => (&EN_TABLE, EN),
        Locale::Fr => (&FR_TABLE, FR),
    };
    // Both files are compiled in and parsed by the tests below, so this can
    // only fail on a build that skipped them.
    cell.get_or_init(|| serde_json::from_str(source).expect("embedded locale table is valid JSON"))
}

/// The text for `key` in the active locale.
///
/// Falls back to English, then to colony-ui's shared labels, then to the key
/// itself: a missing string shows up as its key instead of an empty widget.
pub fn t(key: &str) -> &'static str {
    let locale = colony_ui::i18n::locale();
    table(locale)
        .get(key)
        .or_else(|| table(Locale::En).get(key))
        .map(String::as_str)
        .unwrap_or_else(|| colony_ui::i18n::t(key))
}

/// [`t`] with every `{name}` placeholder replaced by its value.
pub fn tf(key: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Make `language` the active locale, for Xion and for the shared widgets.
pub fn apply(language: Language) {
    colony_ui::i18n::set_locale(resolve(language));
}

/// The locale a language preference stands for.
pub fn resolve(language: Language) -> Locale {
    match language {
        Language::Auto => system_locale(),
        Language::English => Locale::En,
        Language::French => Locale::Fr,
    }
}

/// The session's language, read the way the C library reads it.
#[cfg(not(windows))]
fn system_locale() -> Locale {
    locale_from_env(|name| std::env::var(name).ok())
}

/// The user's display language, from Windows rather than from variables
/// nothing sets there.
#[cfg(windows)]
fn system_locale() -> Locale {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    // LOCALE_NAME_MAX_LENGTH.
    let mut buffer = [0u16; 85];
    // SAFETY: the pointer and length describe `buffer`, which outlives the call;
    // the API writes at most that many UTF-16 units, NUL included.
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    let length = usize::try_from(written).unwrap_or(0).saturating_sub(1);
    Locale::from_tag(&String::from_utf16_lossy(&buffer[..length]))
}

/// `LC_ALL`, then `LC_MESSAGES`, then `LANG`: the first one set and not empty.
///
/// An empty `LC_ALL` is common (some session managers export it blank) and
/// means "not set", not "English".
#[cfg_attr(windows, allow(dead_code))]
fn locale_from_env(var: impl Fn(&str) -> Option<String>) -> Locale {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(var)
        .find(|value| !value.is_empty())
        .map_or(Locale::En, |tag| Locale::from_tag(&tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(locale: Locale) -> Vec<&'static str> {
        let mut keys: Vec<&str> = table(locale).keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn fr_and_en_have_identical_key_sets() {
        assert_eq!(keys(Locale::En), keys(Locale::Fr));
    }

    /// No string left empty, in either language: an empty value renders as an
    /// empty widget, which is worse than the key.
    #[test]
    fn no_string_is_empty() {
        for locale in [Locale::En, Locale::Fr] {
            for (key, value) in table(locale) {
                assert!(!value.trim().is_empty(), "{key} is empty in {locale:?}");
            }
        }
    }

    /// The shared labels belong to colony-ui; a copy here would drift.
    #[test]
    fn no_key_shadows_a_shared_label() {
        let shared: Vec<&str> = colony_ui::i18n::all(Locale::En)
            .map(|(key, _)| key)
            .collect();
        for key in keys(Locale::En) {
            assert!(!shared.contains(&key), "{key} is a colony-ui label");
        }
    }

    /// Every literal key passed to `t` or `tf` in the sources exists, either
    /// here or in colony-ui's shared table.
    #[test]
    fn every_key_used_in_the_sources_exists() {
        let shared: Vec<&str> = colony_ui::i18n::all(Locale::En)
            .map(|(key, _)| key)
            .collect();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for path in rust_files(&root) {
            // This file's own tests call `t` with keys that must not exist.
            if path.ends_with("i18n/mod.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for key in literal_keys(&source) {
                if !table(Locale::En).contains_key(key) && !shared.contains(&key) {
                    missing.push(format!("{}: {key}", path.display()));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "keys missing from en.json: {missing:#?}"
        );
    }

    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
        files
    }

    /// The `"key"` in every `t("key")` and `tf("key", ...)` call.
    fn literal_keys(source: &str) -> Vec<&str> {
        let mut found = Vec::new();
        for call in ["t(\"", "tf(\""] {
            for (start, _) in source.match_indices(call) {
                let preceded_by_identifier = source[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if preceded_by_identifier {
                    continue;
                }
                let rest = &source[start + call.len()..];
                if let Some(end) = rest.find('"') {
                    found.push(&rest[..end]);
                }
            }
        }
        found
    }

    #[test]
    fn the_scanner_finds_calls_and_skips_lookalikes() {
        let source = r#"t("a_key"); i18n::tf("b_key", &[]); set("no"); format!("no")"#;
        assert_eq!(literal_keys(source), ["a_key", "b_key"]);
    }

    #[test]
    fn placeholders_are_filled() {
        // `cli_unknown_option` takes one placeholder in both languages.
        let text = tf("cli_unknown_option", &[("option", &"--nope")]);
        assert!(text.contains("--nope"), "{text}");
        assert!(!text.contains('{'), "{text}");
    }

    #[test]
    fn an_unknown_key_renders_as_itself() {
        assert_eq!(t("no_such_key_anywhere"), "no_such_key_anywhere");
    }

    #[test]
    fn the_first_non_empty_variable_wins() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.to_string())
            }
        };
        assert_eq!(locale_from_env(env(&[("LANG", "fr_FR.UTF-8")])), Locale::Fr);
        assert_eq!(
            locale_from_env(env(&[("LC_ALL", ""), ("LANG", "fr_FR.UTF-8")])),
            Locale::Fr,
            "an empty LC_ALL means unset"
        );
        assert_eq!(
            locale_from_env(env(&[("LC_MESSAGES", "en_GB.UTF-8"), ("LANG", "fr_FR")])),
            Locale::En
        );
        assert_eq!(locale_from_env(env(&[])), Locale::En);
        assert_eq!(locale_from_env(env(&[("LANG", "C")])), Locale::En);
    }

    #[test]
    fn an_explicit_language_ignores_the_session() {
        assert_eq!(resolve(Language::French), Locale::Fr);
        assert_eq!(resolve(Language::English), Locale::En);
    }
}
