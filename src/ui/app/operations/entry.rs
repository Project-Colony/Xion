//! Opening an entry, creating files and folders, properties, and preview eligibility.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::{Path, PathBuf};

use iced::Task;

use crate::filesystem::FsEntryType;
use crate::ui::UiMessage;

use crate::ui::app::XionApp;
use crate::ui::app::types::*;

impl XionApp {
    pub(in crate::ui::app) fn activate_focused_entry(&mut self) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone();
        if let Some(path) = focused {
            return self.activate_entry(path);
        }
        Task::none()
    }

    pub(in crate::ui::app) fn activate_entry(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(entry) = self
            .entries
            .items
            .iter()
            .flatten()
            .find(|entry| entry.path == path)
        {
            if entry.entry_type == FsEntryType::Directory {
                if self.state.route.is_network() {
                    self.last_action =
                        Some(format!("Connexion au partage : {}", entry.path.display()));
                    return Task::none();
                }
                return self.navigate_to(entry.path.clone());
            }
            // Feature 10 / M: Open archives in archive browser
            let ext = entry
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let path_str = entry.path.to_string_lossy().to_lowercase();
            if ext == "zip" || ext == "7z" || ext == "tgz" || path_str.ends_with(".tar.gz") {
                // Record in recents
                self.recents.record(entry.path.clone());
                return self.open_archive(entry.path.clone());
            }
            // Record opened file in recents
            self.recents.record(entry.path.clone());
            // Open file with system default application
            let file_path = entry.path.clone();
            self.last_action = Some(match Self::shell_open(&file_path) {
                Ok(()) => format!(
                    "Ouverture : {}",
                    file_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ),
                Err(error) => error,
            });
        }
        Task::none()
    }

    pub(in crate::ui::app) fn create_new_folder(&mut self) -> Task<UiMessage> {
        let Some(current_dir) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Création impossible en vue réseau".to_string());
            return Task::none();
        };
        Task::perform(
            async move {
                let base_name = "Nouveau dossier";
                let mut target = current_dir.join(base_name);
                let mut counter = 1u32;
                while target.exists() && counter < 1000 {
                    counter += 1;
                    target = current_dir.join(format!("{base_name} ({counter})"));
                }
                if target.exists() {
                    return Err("Impossible de trouver un nom disponible".to_string());
                }
                match std::fs::create_dir(&target) {
                    Ok(()) => Ok(target),
                    Err(error) => Err(error.to_string()),
                }
            },
            UiMessage::NewFolderCreated,
        )
    }

    pub(in crate::ui::app) fn create_new_file(&mut self) -> Task<UiMessage> {
        let Some(current_dir) = self.state.route.local_path().cloned() else {
            self.last_action = Some("Création impossible en vue réseau".to_string());
            return Task::none();
        };
        Task::perform(
            async move {
                let base_name = "Nouveau fichier";
                let ext = ".txt";
                let mut target = current_dir.join(format!("{base_name}{ext}"));
                let mut counter = 1u32;
                while target.exists() && counter < 1000 {
                    counter += 1;
                    target = current_dir.join(format!("{base_name} ({counter}){ext}"));
                }
                if target.exists() {
                    return Err("Impossible de trouver un nom disponible".to_string());
                }
                match std::fs::File::create(&target) {
                    Ok(_) => Ok(target),
                    Err(error) => Err(error.to_string()),
                }
            },
            UiMessage::NewFileCreated,
        )
    }

    pub(in crate::ui::app) fn open_properties(&mut self, path: PathBuf) -> Task<UiMessage> {
        use chrono::{DateTime, Local};
        use std::fs;

        let meta = fs::metadata(&path).ok();
        let size_bytes = meta.as_ref().map(|m| m.len());
        let readonly = meta
            .as_ref()
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);
        let created = meta.as_ref().and_then(|m| m.created().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.format("%d/%m/%Y %H:%M:%S").to_string()
        });

        self.properties_dialog = Some(PropertiesDialog {
            path: path.clone(),
            size_bytes,
            created,
            modified,
            readonly,
            sha256: None,
            computing_hash: true,
            selection_count: None,
        });

        // Spawn async SHA-256 computation (only for files)
        if path.is_file() {
            let hash_path = path.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        use sha2::{Digest, Sha256};
                        use std::io::Read;
                        let mut file = std::fs::File::open(&hash_path).ok()?;
                        let mut hasher = Sha256::new();
                        let mut buf = vec![0u8; 65536];
                        loop {
                            let n = file.read(&mut buf).ok()?;
                            if n == 0 {
                                break;
                            }
                            hasher.update(&buf[..n]);
                        }
                        let result = hasher.finalize();
                        Some((hash_path, format!("{:x}", result)))
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |result| match result {
                    Some((path, hash)) => UiMessage::PropertiesHashComputed { path, hash },
                    None => UiMessage::Noop,
                },
            )
        } else {
            // For directories, mark hash as not applicable
            if let Some(dialog) = &mut self.properties_dialog {
                dialog.computing_hash = false;
                dialog.sha256 = Some("N/A (dossier)".to_string());
            }
            Task::none()
        }
    }

    // ── Feature 5: Bulk rename ────────────────────────────────────────────────

    /// Returns true if the file extension suggests a text/code file suitable for preview.
    pub(in crate::ui::app) fn is_text_previewable(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "txt"
                | "md"
                | "log"
                | "nfo"
                | "readme"
                | "rs"
                | "py"
                | "js"
                | "ts"
                | "jsx"
                | "tsx"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "cs"
                | "java"
                | "go"
                | "rb"
                | "php"
                | "swift"
                | "kt"
                | "lua"
                | "zig"
                | "sh"
                | "bash"
                | "zsh"
                | "ps1"
                | "bat"
                | "cmd"
                | "html"
                | "htm"
                | "css"
                | "scss"
                | "sass"
                | "less"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "xml"
                | "ini"
                | "cfg"
                | "conf"
                | "env"
                | "properties"
                | "csv"
                | "sql"
                | "gitignore"
                | "gitmodules"
                | "gitattributes"
                | "dockerfile"
                | "makefile"
                | "cmake"
                | "r"
                | "dart"
                | "scala"
                | "vue"
                | "svelte"
                | "pdf"
        )
    }

    /// Returns true if any modal dialog is currently open.
    pub(in crate::ui::app) fn has_open_modal(&self) -> bool {
        self.rename_dialog.is_some()
            || self.properties_dialog.is_some()
            || self.bulk_rename.is_some()
            || self.diff_view.is_some()
            || self.hex_view.is_some()
            || self.permissions_view.is_some()
            || self.archive_browser.is_some()
            || self.grep_state.is_some()
    }

    /// Open a file or folder with the OS default handler.
    ///
    /// Delegates to [`crate::platform::shell_open`], which uses `ShellExecuteW`
    /// on Windows. The previous implementation built a `cmd /C start "" "<path>"`
    /// command line; `std` escapes an embedded quote as `\"`, which cmd.exe does
    /// not honour, so a file named `note"&calc&".txt` broke out of the quoting
    /// and ran an arbitrary command. Passing the path as one argument removes
    /// the shell from the picture entirely.
    pub(in crate::ui::app) fn shell_open(path: &std::path::Path) -> Result<(), String> {
        crate::platform::shell_open(path)
            .map_err(|error| format!("Ouverture impossible de {} : {error}", path.display()))
    }

    /// Show a path in the OS file manager without running it.
    pub(in crate::ui::app) fn shell_reveal(path: &std::path::Path) -> Result<(), String> {
        crate::platform::reveal_in_file_manager(path)
            .map_err(|error| format!("Affichage impossible de {} : {error}", path.display()))
    }
}
