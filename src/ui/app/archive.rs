//! Archive listing and creation helpers.
//!
//! ZIP listing is handled by the zip crate elsewhere; this module covers
//! TAR.GZ and 7Z listing, plus creation for all three formats.

use crate::ui::ArchiveEntry;
use std::path::Path;

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
        sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).ok()?;
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
    use flate2::Compression;
    use flate2::write::GzEncoder;
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

/// Create a `.zip` archive from the given list of files/directories.
///
/// This algorithm used to live inline inside `update()`, declared at the eighth
/// level of nesting, and read each file fully into memory before writing it —
/// an 8 GB file meant an 8 GB allocation. `io::copy` streams instead.
pub(super) fn create_zip(
    sources: &[std::path::PathBuf],
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    use zip::write::FileOptions;

    let out_path = dest.to_path_buf();
    let file = std::fs::File::create(&out_path).map_err(|e| format!("Création ZIP: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

    for source in sources {
        let name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| format!("Source sans nom: {}", source.display()))?;
        zip_add(&mut zip, source, &name, options, 0)?;
    }

    zip.finish()
        .map_err(|e| format!("ZIP: finalisation: {e}"))?;
    Ok(out_path)
}

/// Depth backstop: a symlink loop inside the tree must not recurse forever.
const MAX_ZIP_DEPTH: u32 = 256;

fn zip_add(
    zip: &mut zip::ZipWriter<std::fs::File>,
    source: &Path,
    name_in_zip: &str,
    options: zip::write::FileOptions<()>,
    depth: u32,
) -> Result<(), String> {
    if depth > MAX_ZIP_DEPTH {
        return Err(format!(
            "profondeur maximale dépassée sous {}",
            source.display()
        ));
    }

    let metadata =
        std::fs::symlink_metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;

    // Symlinks are skipped rather than followed: following them would embed the
    // target's contents under a name the user never selected, and a loop would
    // never terminate.
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    if metadata.is_dir() {
        zip.add_directory(format!("{name_in_zip}/"), options)
            .map_err(|e| format!("ZIP: dossier '{name_in_zip}': {e}"))?;
        let mut children: Vec<_> = std::fs::read_dir(source)
            .map_err(|e| format!("{}: {e}", source.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        // Deterministic order: `read_dir` yields whatever the filesystem gives.
        children.sort_by_key(|child| child.file_name());
        for child in children {
            let child_name = format!("{name_in_zip}/{}", child.file_name().to_string_lossy());
            zip_add(zip, &child.path(), &child_name, options, depth + 1)?;
        }
        return Ok(());
    }

    zip.start_file(name_in_zip, options)
        .map_err(|e| format!("ZIP: entrée '{name_in_zip}': {e}"))?;
    let mut input =
        std::fs::File::open(source).map_err(|e| format!("{}: {e}", source.display()))?;
    std::io::copy(&mut input, zip).map_err(|e| format!("ZIP: écriture '{name_in_zip}': {e}"))?;
    Ok(())
}

/// Create a `.7z` archive from the given list of files/directories.
/// Returns the path of the created archive.
///
/// `compress_to_path` used to be called once per source, and it recreates the
/// archive from scratch every time: with several items selected, only the last
/// one survived while the UI still reported success. A single writer is opened
/// and every source is pushed into it.
pub(super) fn create_7z(
    sources: &[std::path::PathBuf],
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    use sevenz_rust2::ArchiveWriter;

    let out_path = dest.to_path_buf();
    let mut writer = ArchiveWriter::create(&out_path).map_err(|e| format!("Création 7z: {e}"))?;

    for source in sources {
        let root_name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| format!("Source sans nom: {}", source.display()))?;

        if source.is_dir() {
            for entry in walkdir::WalkDir::new(source).follow_links(false) {
                let entry = entry.map_err(|e| format!("Parcours '{}': {e}", source.display()))?;
                // Symlinks are not followed and not stored: 7z cannot express
                // them here, and silently storing their target would duplicate
                // data the user did not select.
                if entry.file_type().is_symlink() {
                    continue;
                }
                let relative = entry
                    .path()
                    .strip_prefix(source)
                    .map_err(|e| format!("Chemin relatif: {e}"))?;
                let name = archive_entry_name(&root_name, relative);
                push_7z_entry(&mut writer, entry.path(), name, entry.file_type().is_dir())?;
            }
        } else {
            push_7z_entry(&mut writer, source, root_name, false)?;
        }
    }

    writer
        .finish()
        .map_err(|e| format!("7z: finalisation: {e}"))?;

    Ok(out_path)
}

/// Build the in-archive name for `relative` under `root_name`, always using
/// `/` as the separator so the archive reads the same on every platform.
fn archive_entry_name(root_name: &str, relative: &Path) -> String {
    if relative.as_os_str().is_empty() {
        return root_name.to_string();
    }
    let tail = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    format!("{root_name}/{tail}")
}

fn push_7z_entry(
    writer: &mut sevenz_rust2::ArchiveWriter<std::fs::File>,
    path: &Path,
    name: String,
    is_dir: bool,
) -> Result<(), String> {
    use sevenz_rust2::ArchiveEntry;

    let entry = ArchiveEntry::from_path(path, name);
    let reader = if is_dir {
        None
    } else {
        Some(
            std::fs::File::open(path)
                .map_err(|e| format!("Ouverture '{}': {e}", path.display()))?,
        )
    };
    writer
        .push_archive_entry(entry, reader)
        .map(|_| ())
        .map_err(|e| format!("7z: ajout '{}': {e}", path.display()))
}

// ── Safe extraction ─────────────────────────────────────────────────────────

/// Turn an archive entry name into a path that cannot leave `dest_dir`.
///
/// The previous guard split on `/` only, so a ZIP entry written with Windows
/// separators (`..\..\Windows\System32\evil.dll`) contained no `/` at all and
/// travelled through intact; the `starts_with` check that followed compares
/// components without normalising `..`, so `dest/../../evil.dll` "starts with"
/// `dest` and passed. Both separators are handled here, and `..` is rejected
/// outright rather than resolved.
pub(super) fn safe_join(dest_dir: &Path, raw_name: &str) -> Result<std::path::PathBuf, String> {
    let mut relative = std::path::PathBuf::new();

    for segment in raw_name.split(['/', '\\']) {
        match segment {
            "" | "." => continue,
            ".." => {
                return Err(format!("entrée refusée (remontée de dossier) : {raw_name}"));
            }
            other => {
                // `C:`, `C:evil` and any drive-relative form must never become
                // a path root once joined.
                let bytes = other.as_bytes();
                if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                    return Err(format!("entrée refusée (préfixe de disque) : {raw_name}"));
                }
                // On Windows a colon also opens an alternate data stream.
                if cfg!(windows) && other.contains(':') {
                    return Err(format!("entrée refusée (flux alternatif) : {raw_name}"));
                }
                relative.push(other);
            }
        }
    }

    if relative.as_os_str().is_empty() {
        return Err(format!("entrée refusée (nom vide) : {raw_name}"));
    }

    Ok(dest_dir.join(relative))
}

/// Confirm that `out_path` really resolves inside `dest_dir`.
///
/// `safe_join` handles the name; this handles the filesystem. A symlink already
/// sitting inside the destination — possibly written by an earlier entry of the
/// same archive — could otherwise redirect the write outside it. The parent is
/// created first so it can be canonicalised.
pub(super) fn create_parent_within(dest_dir: &Path, out_path: &Path) -> Result<(), String> {
    let parent = out_path
        .parent()
        .ok_or_else(|| format!("Chemin sans parent: {}", out_path.display()))?;

    std::fs::create_dir_all(parent).map_err(|e| format!("Création dossier parent: {e}"))?;

    let real_parent = parent
        .canonicalize()
        .map_err(|e| format!("Résolution du dossier cible: {e}"))?;
    let real_dest = dest_dir
        .canonicalize()
        .map_err(|e| format!("Résolution du dossier d'extraction: {e}"))?;

    if !real_parent.starts_with(&real_dest) {
        return Err(format!(
            "entrée refusée (sort du dossier cible) : {}",
            out_path.display()
        ));
    }
    Ok(())
}

/// Refuse to clobber an existing file.
///
/// `File::create` truncates, so extracting an entry over an existing file used
/// to destroy it without a word.
pub(super) fn ensure_absent(out_path: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(out_path).is_ok() {
        return Err(format!(
            "{} existe déjà — extraction annulée",
            out_path.display()
        ));
    }
    Ok(())
}

/// Extract a single entry from a ZIP archive.
pub(super) fn extract_zip_entry(
    archive_path: &Path,
    inner_path: &str,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    use std::io::{Read, Write};
    let file = std::fs::File::open(archive_path).map_err(|e| format!("Ouverture ZIP: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Lecture ZIP: {e}"))?;

    let mut entry = archive
        .by_name(inner_path)
        .map_err(|e| format!("Entrée ZIP '{}': {e}", inner_path))?;

    // The zip crate's own guard: `None` means the name is not safely
    // relative. Checked in addition to `safe_join`, not instead of it.
    if entry.enclosed_name().is_none() {
        return Err(format!("entrée ZIP refusée : {}", entry.name()));
    }
    let out_path = safe_join(dest_dir, entry.name())?;

    if entry.is_dir() {
        create_parent_within(dest_dir, &out_path)?;
        std::fs::create_dir_all(&out_path).map_err(|e| format!("Création dossier: {e}"))?;
        return Ok(out_path);
    }

    create_parent_within(dest_dir, &out_path)?;
    ensure_absent(&out_path)?;

    let mut out_file =
        std::fs::File::create(&out_path).map_err(|e| format!("Écriture fichier: {e}"))?;
    // Streamed rather than read_to_end: an entry declaring a huge size no
    // longer has to fit in RAM before a single byte reaches the disk.
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|e| format!("Lecture entrée ZIP: {e}"))?;
        if read == 0 {
            break;
        }
        out_file
            .write_all(&buffer[..read])
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
    let file = std::fs::File::open(archive_path).map_err(|e| format!("Ouverture tar.gz: {e}"))?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut archive = tar::Archive::new(gz);

    for entry in archive
        .entries()
        .map_err(|e| format!("Lecture tar.gz: {e}"))?
        .flatten()
    {
        let path = entry.path().map_err(|e| format!("Lecture chemin: {e}"))?;
        let path_str = path.to_string_lossy().to_string();
        if path_str.trim_end_matches('/') != inner_path.trim_end_matches('/') {
            continue;
        }

        // tar can carry symlinks and hard links whose target is chosen by the
        // archive. `unpack` would materialise them, giving a write primitive
        // anywhere the link points. Only plain files and directories are
        // extracted.
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(format!(
                "entrée refusée (type {entry_type:?} non extrait) : {path_str}"
            ));
        }

        let out_path = safe_join(dest_dir, &path_str)?;
        create_parent_within(dest_dir, &out_path)?;

        if entry_type.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| format!("Création dossier: {e}"))?;
            return Ok(out_path);
        }

        ensure_absent(&out_path)?;
        let mut entry = entry;
        entry
            .unpack(&out_path)
            .map_err(|e| format!("Extraction tar.gz: {e}"))?;
        return Ok(out_path);
    }

    Err(format!(
        "Entrée '{}' non trouvée dans l'archive",
        inner_path
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dest() -> PathBuf {
        PathBuf::from("/tmp/xion-dest")
    }

    #[test]
    fn safe_join_keeps_a_plain_relative_entry() {
        let joined = safe_join(&dest(), "docs/readme.txt").unwrap();
        assert_eq!(joined, dest().join("docs").join("readme.txt"));
    }

    /// Regression: the old guard split on `/` only, so an entry using Windows
    /// separators carried its `..` segments straight through.
    #[test]
    fn zip_entry_with_backslash_traversal_is_rejected() {
        let error = safe_join(&dest(), r"..\..\..\Windows\System32\evil.dll").unwrap_err();
        assert!(error.contains("remontée"), "message inattendu : {error}");
    }

    #[test]
    fn entry_with_forward_slash_traversal_is_rejected() {
        let error = safe_join(&dest(), "../../evil.sh").unwrap_err();
        assert!(error.contains("remontée"), "message inattendu : {error}");
    }

    #[test]
    fn entry_with_traversal_in_the_middle_is_rejected() {
        assert!(safe_join(&dest(), "docs/../../evil").is_err());
    }

    #[test]
    fn sevenz_entry_with_absolute_path_is_rejected() {
        let error = safe_join(&dest(), r"C:\Windows\System32\evil.dll").unwrap_err();
        assert!(error.contains("disque"), "message inattendu : {error}");
    }

    #[test]
    fn leading_separator_does_not_escape_the_destination() {
        // An absolute-looking name must be treated as relative, not as a root.
        let joined = safe_join(&dest(), "/etc/passwd").unwrap();
        assert_eq!(joined, dest().join("etc").join("passwd"));
    }

    #[test]
    fn empty_entry_name_is_rejected() {
        assert!(safe_join(&dest(), "").is_err());
        assert!(safe_join(&dest(), "./").is_err());
    }

    #[test]
    fn current_directory_segments_are_dropped() {
        let joined = safe_join(&dest(), "./docs/./readme.txt").unwrap();
        assert_eq!(joined, dest().join("docs").join("readme.txt"));
    }

    /// Regression: `compress_to_path` was called once per source and rewrote
    /// the archive each time, so only the last item survived.
    #[test]
    fn sevenz_multi_selection_contains_all_entries() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a.txt");
        let b = root.path().join("b.txt");
        std::fs::write(&a, "alpha").unwrap();
        std::fs::write(&b, "beta").unwrap();
        let out = root.path().join("bundle.7z");

        create_7z(&[a, b], &out).unwrap();

        let listed = list_7z(&out).expect("archive lisible");
        let mut names: Vec<_> = listed.iter().map(|entry| entry.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a.txt".to_string(), "b.txt".to_string()]);
    }

    #[test]
    fn sevenz_stores_a_directory_tree_under_its_root_name() {
        let root = tempfile::tempdir().unwrap();
        let tree = root.path().join("tree");
        std::fs::create_dir_all(tree.join("sub")).unwrap();
        std::fs::write(tree.join("sub").join("deep.txt"), "deep").unwrap();
        let out = root.path().join("tree.7z");

        create_7z(&[tree], &out).unwrap();

        let listed = list_7z(&out).expect("archive lisible");
        assert!(
            listed
                .iter()
                .any(|entry| entry.inner_path == "tree/sub/deep.txt"),
            "entrées : {:?}",
            listed.iter().map(|e| &e.inner_path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn extraction_refuses_to_overwrite_an_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let occupied = root.path().join("keep.txt");
        std::fs::write(&occupied, "précieux").unwrap();

        assert!(ensure_absent(&occupied).is_err());
        assert_eq!(std::fs::read_to_string(&occupied).unwrap(), "précieux");
    }

    /// The bzip2 backend is pure Rust rather than the bundled C library, so
    /// that cross-compiling to Windows needs no MSVC toolchain for this link in
    /// the chain. Nothing else covered that path: every other archive test uses
    /// deflate, tar or 7z.
    #[test]
    fn a_bzip2_compressed_zip_round_trips() {
        use std::io::Write;

        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("compressé.zip");
        let contenu = "vérification du backend bzip2 en Rust pur\n".repeat(64);

        let file = std::fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "note.txt",
                zip::write::FileOptions::<()>::default()
                    .compression_method(zip::CompressionMethod::Bzip2),
            )
            .unwrap();
        writer.write_all(contenu.as_bytes()).unwrap();
        writer.finish().unwrap();

        let dest = root.path().join("sortie");
        std::fs::create_dir_all(&dest).unwrap();
        let extracted = extract_zip_entry(&archive_path, "note.txt", &dest).unwrap();

        assert_eq!(std::fs::read_to_string(&extracted).unwrap(), contenu);
    }

    /// A symlink planted inside the destination must not become a way out.
    #[cfg(unix)]
    #[test]
    fn create_parent_within_rejects_a_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let dest_dir = root.path().join("dest");
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dest_dir.join("escape")).unwrap();

        let out_path = dest_dir.join("escape").join("evil.txt");
        let error = create_parent_within(&dest_dir, &out_path).unwrap_err();

        assert!(error.contains("sort du dossier cible"), "message : {error}");
    }
}
