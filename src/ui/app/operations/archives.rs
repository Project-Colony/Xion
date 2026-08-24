//! Opening an archive for browsing and extracting a single entry from it.
//!
//! Moved verbatim out of the single 1342-line `impl XionApp` block.

use std::path::PathBuf;

use iced::Task;

use crate::ui::{ArchiveEntry, UiMessage};

use crate::ui::app::XionApp;
use crate::ui::app::archive;

impl XionApp {
    pub(in crate::ui::app) fn open_archive(&mut self, path: PathBuf) -> Task<UiMessage> {
        let archive_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let ext = archive_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let path_str = archive_path.to_string_lossy().to_lowercase();
                    let entries = if ext == "7z" {
                        archive::list_7z(&archive_path)
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        archive::list_tar_gz(&archive_path)
                    } else {
                        // ZIP (default)
                        let file = std::fs::File::open(&archive_path).ok()?;
                        let mut zip = zip::ZipArchive::new(file).ok()?;
                        let mut entries = Vec::new();
                        for i in 0..zip.len() {
                            if let Ok(entry) = zip.by_index(i) {
                                let inner_path = entry.name().to_string();
                                let name = inner_path
                                    .rsplit('/')
                                    .find(|s| !s.is_empty())
                                    .unwrap_or(&inner_path)
                                    .to_string();
                                let is_dir = entry.is_dir();
                                let size = entry.size();
                                let compressed_size = entry.compressed_size();
                                entries.push(ArchiveEntry {
                                    name,
                                    inner_path,
                                    is_dir,
                                    size,
                                    compressed_size,
                                });
                            }
                        }
                        return Some((archive_path, entries));
                    };
                    let entries = entries?;
                    Some((archive_path, entries))
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((archive_path, entries)) => UiMessage::ArchiveListLoaded {
                    archive_path,
                    inner_path: String::new(),
                    entries,
                },
                None => UiMessage::Noop,
            },
        )
    }

    pub(in crate::ui::app) fn extract_archive_entry(
        &self,
        archive: PathBuf,
        inner_path: String,
        dest_dir: PathBuf,
    ) -> Task<UiMessage> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                    let ext = archive
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let path_str = archive.to_string_lossy().to_lowercase();

                    if ext == "7z" {
                        // 7Z: inline because sevenz-rust has no single-entry extract.
                        // The guard here used to be `inner_path.contains("..")`
                        // alone, which let an ABSOLUTE entry name through:
                        // `dest_dir.join("C:\\Windows\\...")` discards the base and
                        // writes wherever the archive says. Same `safe_join` as
                        // the ZIP and TAR paths now.
                        let out_path = archive::safe_join(&dest_dir, &inner_path)?;
                        archive::create_parent_within(&dest_dir, &out_path)?;
                        archive::ensure_absent(&out_path)?;
                        let mut found = false;
                        let mut arch = sevenz_rust::SevenZReader::open(
                            &archive,
                            sevenz_rust::Password::empty(),
                        )
                        .map_err(|e| e.to_string())?;
                        arch.for_each_entries(|entry, reader| {
                            if entry.name() == inner_path {
                                let mut out_file = std::fs::File::create(&out_path)
                                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                                std::io::copy(reader, &mut out_file)?;
                                found = true;
                            }
                            Ok(true)
                        })
                        .map_err(|e| e.to_string())?;
                        if found {
                            Ok(out_path)
                        } else {
                            Err("Fichier non trouvé dans l'archive".to_string())
                        }
                    } else if ext == "tgz" || path_str.ends_with(".tar.gz") {
                        archive::extract_tar_gz_entry(&archive, &inner_path, &dest_dir)
                    } else {
                        archive::extract_zip_entry(&archive, &inner_path, &dest_dir)
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            UiMessage::ExtractComplete,
        )
    }

    // ── Feature 11: Dual pane ─────────────────────────────────────────────────
}
