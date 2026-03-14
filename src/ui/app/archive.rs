//! Archive listing and creation helpers.
//!
//! ZIP listing is handled by the zip crate elsewhere; this module covers
//! TAR.GZ and 7Z listing, plus creation for all three formats.

use std::path::Path;
use crate::ui::ArchiveEntry;

/// List entries inside a `.tar.gz` / `.tgz` file.
pub(super) fn list_tar_gz(path: &std::path::Path) -> Option<Vec<ArchiveEntry>> {
    use std::io::BufReader;
    let file = std::fs::File::open(path).ok()?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut archive = tar::Archive::new(gz);
    let mut entries = Vec::new();
    for e in archive.entries().ok()?.flatten() {
        let inner_path = e.path().ok()?.to_string_lossy().to_string();
        let name = inner_path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&inner_path)
            .to_string();
        let is_dir = e.header().entry_type().is_dir();
        let size = e.header().size().unwrap_or(0);
        entries.push(ArchiveEntry {
            name,
            inner_path,
            is_dir,
            size,
            compressed_size: 0,
        });
    }
    Some(entries)
}

/// List entries inside a `.7z` file.
pub(super) fn list_7z(path: &std::path::Path) -> Option<Vec<ArchiveEntry>> {
    let mut archive =
        sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty()).ok()?;
    let mut entries = Vec::new();
    archive
        .for_each_entries(|entry, _reader| {
            let inner_path = entry.name().to_string();
            let name = inner_path
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or(&inner_path)
                .to_string();
            let is_dir = entry.is_directory();
            let size = entry.size();
            entries.push(ArchiveEntry {
                name,
                inner_path,
                is_dir,
                size,
                compressed_size: 0,
            });
            Ok(true)
        })
        .ok()?;
    Some(entries)
}

// ── Archive creation ────────────────────────────────────────────────────────

/// Create a `.tar.gz` archive from the given list of files/directories.
/// Returns the path of the created archive.
pub(super) fn create_tar_gz(
    sources: &[std::path::PathBuf],
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs::File;

    let out_path = dest.to_path_buf();
    let file = File::create(&out_path).map_err(|e| format!("Création tar.gz: {e}"))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(enc);

    for source in sources {
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        if source.is_dir() {
            builder
                .append_dir_all(&name, source)
                .map_err(|e| format!("tar.gz: ajout dossier '{}': {e}", name))?;
        } else {
            builder
                .append_path_with_name(source, &name)
                .map_err(|e| format!("tar.gz: ajout fichier '{}': {e}", name))?;
        }
    }

    builder
        .finish()
        .map_err(|e| format!("tar.gz: finalisation: {e}"))?;

    Ok(out_path)
}

/// Create a `.7z` archive from the given list of files/directories.
/// Returns the path of the created archive.
pub(super) fn create_7z(
    sources: &[std::path::PathBuf],
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    let out_path = dest.to_path_buf();

    // sevenz-rust expects a single source directory or file
    // For multiple items, we compress one by one
    for source in sources {
        sevenz_rust::compress_to_path(source, &out_path)
            .map_err(|e| format!("Compression 7z '{}': {e}", source.display()))?;
    }

    Ok(out_path)
}

/// Extract a single entry from a ZIP archive.
pub(super) fn extract_zip_entry(
    archive_path: &Path,
    inner_path: &str,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    use std::io::{Read, Write};
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Ouverture ZIP: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Lecture ZIP: {e}"))?;

    let mut entry = archive
        .by_name(inner_path)
        .map_err(|e| format!("Entrée ZIP '{}': {e}", inner_path))?;

    let name = entry.name().rsplit('/').find(|s| !s.is_empty())
        .unwrap_or(entry.name())
        .to_string();
    let out_path = dest_dir.join(&name);

    // Security: ensure the output path stays within dest_dir
    let canonical_dest = dest_dir.canonicalize().unwrap_or_else(|_| dest_dir.to_path_buf());
    if !out_path.starts_with(&canonical_dest) && !out_path.starts_with(dest_dir) {
        return Err(format!("Path traversal détecté: {}", name));
    }

    if entry.is_dir() {
        std::fs::create_dir_all(&out_path)
            .map_err(|e| format!("Création dossier: {e}"))?;
    } else {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Création dossier parent: {e}"))?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)
            .map_err(|e| format!("Lecture entrée ZIP: {e}"))?;
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Écriture fichier: {e}"))?;
        out_file.write_all(&buf)
            .map_err(|e| format!("Écriture fichier: {e}"))?;
    }

    Ok(out_path)
}

/// Extract a single entry from a TAR.GZ archive.
pub(super) fn extract_tar_gz_entry(
    archive_path: &Path,
    inner_path: &str,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    use std::io::BufReader;
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Ouverture tar.gz: {e}"))?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut archive = tar::Archive::new(gz);

    let canonical_dest = dest_dir.canonicalize().unwrap_or_else(|_| dest_dir.to_path_buf());

    for entry in archive.entries().map_err(|e| format!("Lecture tar.gz: {e}"))?.flatten() {
        let path = entry.path().map_err(|e| format!("Lecture chemin: {e}"))?;
        let path_str = path.to_string_lossy();
        if path_str.trim_end_matches('/') == inner_path.trim_end_matches('/') {
            let name = path_str.rsplit('/').find(|s| !s.is_empty())
                .unwrap_or(&path_str)
                .to_string();
            let out_path = dest_dir.join(&name);

            if !out_path.starts_with(&canonical_dest) && !out_path.starts_with(dest_dir) {
                return Err(format!("Path traversal détecté: {}", name));
            }

            let mut entry = entry;
            entry.unpack(&out_path)
                .map_err(|e| format!("Extraction tar.gz: {e}"))?;
            return Ok(out_path);
        }
    }

    Err(format!("Entrée '{}' non trouvée dans l'archive", inner_path))
}
