//! Archive browsing, extraction and compression.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::{ArchiveFormat, Flow};
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_archive(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::ArchiveListLoaded {
                archive_path,
                inner_path,
                entries,
            } => {
                let Some(archive_type) = ArchiveType::detect(&archive_path) else {
                    self.last_action = Some("Format d'archive non reconnu".to_string());
                    return Ok(Flow::Continue);
                };
                self.archive_browser = Some(ArchiveBrowserState {
                    archive_path,
                    inner_path,
                    entries,
                    archive_type,
                });
            }
            UiMessage::ArchiveFolderOpen { inner_path } => {
                if let Some(browser) = &mut self.archive_browser {
                    browser.inner_path = inner_path;
                }
            }
            UiMessage::CloseArchiveBrowser => {
                self.archive_browser = None;
            }
            UiMessage::ExtractArchiveEntry {
                archive,
                inner_path,
                dest_dir,
            } => {
                tasks.push(self.extract_archive_entry(archive, inner_path, dest_dir));
            }
            UiMessage::ExtractComplete(result) => {
                match result {
                    Ok(path) => {
                        // Deliberately NOT opened: the file name comes straight
                        // from the archive, so auto-running it handed an
                        // attacker execution for the price of one "Extract"
                        // click. Reveal it and let the user decide.
                        self.last_action = Some(format!("Extrait : {}", path.display()));
                        if let Err(error) = Self::shell_reveal(&path) {
                            tracing::warn!("{error}");
                        }
                        // Refresh file list to show extracted files
                        tasks.push(self.refresh_entries_in_place());
                    }
                    Err(e) => {
                        self.last_action = Some(format!("Erreur extraction : {}", e));
                    }
                }
            }
            // Feature 11: Dual pane
            UiMessage::CompressToZip => {
                tasks.push(self.compress_selection(ArchiveFormat::Zip));
            }
            UiMessage::CompressToTarGz => {
                tasks.push(self.compress_selection(ArchiveFormat::TarGz));
            }
            UiMessage::CompressTo7z => {
                tasks.push(self.compress_selection(ArchiveFormat::SevenZ));
            }
            UiMessage::CompressCompleted(result) => match result {
                Ok(path) => {
                    self.last_action = Some(format!(
                        "{} créé",
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("archive")
                    ));
                    tasks.push(self.refresh_entries_in_place());
                }
                Err(e) => {
                    self.last_action = Some(format!("Erreur compression : {}", e));
                }
            },
            // ── Feature B: Open With ─────────────────────────────────────────
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
