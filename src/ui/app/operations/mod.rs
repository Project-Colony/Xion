//! File operations, split by domain.
//!
//! This used to be a single 1340-line `impl XionApp` block. The methods moved
//! verbatim; only their file changed.

mod archives;
mod clipboard;
mod commands;
mod delete;
mod dual_pane;
mod entry;
mod rename;

use super::XionApp;

/// Validate a name typed into the rename dialog.
///
/// Counting path components was not enough: `Path::new("..")` has exactly one
/// component, so `..` and `.` both passed and produced a target of
/// `<parent>/..`, which `rename` then resolved somewhere the user never asked
/// for. The Windows-invalid characters are rejected on every platform so a tree
/// stays portable.
pub(in crate::ui::app) fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("nom vide".to_string());
    }
    if name == "." || name == ".." {
        return Err("« . » et « .. » ne sont pas des noms valides".to_string());
    }
    if name.contains(['/', '\\']) {
        return Err("le nom ne doit pas contenir de séparateur de chemin".to_string());
    }
    if let Some(bad) = name
        .chars()
        .find(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(format!("le caractère « {bad} » est interdit"));
    }
    if name.chars().any(|c| (c as u32) < 0x20) {
        return Err("le nom contient un caractère de contrôle".to_string());
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err("le nom ne doit pas se terminer par un point ou une espace".to_string());
    }
    Ok(())
}

impl XionApp {
    /// Allocate the next operation id.
    pub(super) fn next_operation_id(&mut self) -> u64 {
        self.next_operation_id = self.next_operation_id.wrapping_add(1);
        self.next_operation_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The parent module no longer needs these itself — the methods that used
    // them moved into the submodules — but the tests still do.
    use crate::ui::app::XionApp;
    use crate::ui::app::types::ConfirmedAction;
    use std::path::PathBuf;

    #[test]
    fn rename_rejects_dot_and_dotdot() {
        // Regression: both have exactly one path component, so the old
        // `components().count() > 1` check let them through.
        assert!(validate_file_name(".").is_err());
        assert!(validate_file_name("..").is_err());
    }

    #[test]
    fn rename_rejects_path_separators() {
        assert!(validate_file_name("dossier/fichier.txt").is_err());
        assert!(validate_file_name(r"dossier\fichier.txt").is_err());
    }

    #[test]
    fn rename_rejects_characters_windows_forbids() {
        for name in ["a<b", "a>b", "a:b", "a\"b", "a|b", "a?b", "a*b"] {
            assert!(
                validate_file_name(name).is_err(),
                "{name} devrait être refusé"
            );
        }
    }

    #[test]
    fn rename_rejects_trailing_dot_or_space() {
        assert!(validate_file_name("rapport.").is_err());
        assert!(validate_file_name("rapport ").is_err());
    }

    #[test]
    fn rename_rejects_control_characters() {
        assert!(validate_file_name("a\u{7}b").is_err());
    }

    #[test]
    fn rename_accepts_ordinary_names() {
        for name in [
            "rapport.txt",
            "Déjà vu.md",
            "archive.tar.gz",
            ".gitignore",
            "a b c",
        ] {
            assert!(
                validate_file_name(name).is_ok(),
                "{name} devrait être accepté"
            );
        }
    }

    /// Regression: Delete used to erase immediately. It must now stage a
    /// confirmation and touch nothing until the user accepts.
    #[test]
    fn permanent_delete_asks_before_acting() {
        let mut app = XionApp::new_for_test();
        app.state
            .navigation
            .selection
            .selected
            .insert(PathBuf::from("/tmp/xion-does-not-exist"));

        let _ = app.request_permanent_delete();

        let dialog = app
            .confirm_dialog
            .as_ref()
            .expect("un dialogue doit s'ouvrir");
        assert!(dialog.message.contains("définitivement"));
        assert!(matches!(
            dialog.action,
            ConfirmedAction::DeletePermanently(ref items) if items.len() == 1
        ));
    }

    #[test]
    fn cancelling_the_confirmation_deletes_nothing() {
        let mut app = XionApp::new_for_test();
        app.state
            .navigation
            .selection
            .selected
            .insert(PathBuf::from("/tmp/xion-does-not-exist"));
        let _ = app.request_permanent_delete();

        app.confirm_dialog = None;

        // `accept_confirmation` with no pending dialog must be a no-op.
        let _ = app.accept_confirmation();
        assert!(app.confirm_dialog.is_none());
    }

    #[test]
    fn permanent_delete_without_selection_opens_no_dialog() {
        let mut app = XionApp::new_for_test();
        let _ = app.request_permanent_delete();
        assert!(app.confirm_dialog.is_none());
        assert_eq!(
            app.last_action.as_deref(),
            Some("Aucune sélection à supprimer")
        );
    }
}
