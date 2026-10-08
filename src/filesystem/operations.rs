use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::core::XionError;

#[cfg(windows)]
fn ensure_long_path(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    let s = path.to_string_lossy();
    if s.len() > 240 && !s.starts_with("\\\\?\\") && path.is_absolute() {
        if s.starts_with("\\\\") {
            // UNC path: \\server\share -> \\?\UNC\server\share
            std::borrow::Cow::Owned(std::path::PathBuf::from(format!(
                "\\\\?\\UNC\\{}",
                s.strip_prefix("\\\\").unwrap_or(&s)
            )))
        } else {
            std::borrow::Cow::Owned(std::path::PathBuf::from(format!("\\\\?\\{}", s)))
        }
    } else {
        std::borrow::Cow::Borrowed(path)
    }
}

#[cfg(not(windows))]
fn ensure_long_path(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    std::borrow::Cow::Borrowed(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperationKind {
    Copy,
    Move,
    Rename,
    Delete,
}

#[derive(Debug, Clone)]
pub struct OperationFailure {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct OperationReport {
    pub action: FileOperationKind,
    pub succeeded: Vec<PathBuf>,
    pub failed: Vec<OperationFailure>,
}

impl OperationReport {
    fn new(action: FileOperationKind) -> Self {
        Self {
            action,
            succeeded: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// An empty report, used when a worker task could not run at all.
    pub fn empty(action: FileOperationKind) -> Self {
        Self::new(action)
    }

    fn push_failure(&mut self, path: PathBuf, error: XionError) {
        self.failed.push(OperationFailure {
            path,
            error: error.to_string(),
        });
    }
}

#[derive(Debug, Default)]
pub struct LocalFileOperations;

impl LocalFileOperations {
    pub fn new() -> Self {
        Self
    }

    pub fn copy_items(
        &self,
        items: &[PathBuf],
        dest_dir: &Path,
        progress: Option<Arc<AtomicUsize>>,
    ) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Copy);
        for item in items {
            match destination_for(item, dest_dir) {
                Ok(destination) => match copy_entry(item, &destination) {
                    Ok(()) => report.succeeded.push(destination),
                    Err(error) => report.push_failure(item.clone(), error),
                },
                Err(error) => report.push_failure(item.clone(), error),
            }
            if let Some(ref counter) = progress {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
        report
    }

    pub fn move_items(
        &self,
        items: &[PathBuf],
        dest_dir: &Path,
        progress: Option<Arc<AtomicUsize>>,
    ) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Move);
        for item in items {
            match destination_for(item, dest_dir) {
                Ok(destination) => match move_entry(item, &destination) {
                    Ok(()) => report.succeeded.push(destination),
                    Err(error) => report.push_failure(item.clone(), error),
                },
                Err(error) => report.push_failure(item.clone(), error),
            }
            if let Some(ref counter) = progress {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
        report
    }

    pub fn rename_item(&self, from: &Path, to: &Path) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Rename);
        if let Some(name) = to.file_name().and_then(|n| n.to_str())
            && is_reserved_windows_name(name)
        {
            report.push_failure(
                from.to_path_buf(),
                XionError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("'{}' is a reserved Windows filename", name),
                )),
            );
            return report;
        }
        match move_entry(from, to) {
            Ok(()) => report.succeeded.push(to.to_path_buf()),
            Err(error) => report.push_failure(from.to_path_buf(), error),
        }
        report
    }

    pub fn delete_items(&self, items: &[PathBuf]) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Delete);
        for item in items {
            match delete_entry(item) {
                Ok(()) => report.succeeded.push(item.clone()),
                Err(error) => report.push_failure(item.clone(), error),
            }
        }
        report
    }
}

#[cfg(windows)]
fn is_reserved_windows_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(not(windows))]
fn is_reserved_windows_name(_name: &str) -> bool {
    false
}

fn destination_for(source: &Path, dest_dir: &Path) -> Result<PathBuf, XionError> {
    let name = source
        .file_name()
        .ok_or_else(|| XionError::InvalidPath(source.to_path_buf()))?;
    Ok(dest_dir.join(name))
}

/// Depth backstop for `copy_tree`.
///
/// `guard_copy_target` already rejects a destination nested inside its source,
/// which is the only way to build an unbounded recursion out of a real
/// filesystem. This is the belt to that pair of braces: a directory tree deeper
/// than this is pathological, and aborting beats a stack overflow.
const MAX_COPY_DEPTH: u32 = 256;

/// Best-effort pre-check for destination existence.
///
/// This is subject to TOCTOU races: another process may create the path
/// between this check and the actual operation. Callers should also handle
/// `AlreadyExists` errors from the underlying fs operations.
///
/// Uses `symlink_metadata` rather than `exists()` so a dangling symlink at the
/// destination counts as occupied instead of reporting "absent" and then
/// failing further down with a confusing error.
fn ensure_destination_absent(destination: &Path) -> Result<(), XionError> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err(XionError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Destination already exists: {}", destination.display()),
        )));
    }
    Ok(())
}

/// Reject a copy or move that would consume its own output.
///
/// Copying `/projects` into `/projects/build` used to work like this:
/// `create_dir_all` made `/projects/build/projects` *before* `read_dir` listed
/// `/projects`, so the listing returned the directory that had just been
/// created and the recursion never terminated — it filled the disk at write
/// speed. Both paths are canonicalised first so symlinks, `..` segments and
/// `\\?\` prefixes cannot be used to disguise the nesting.
///
/// The destination does not exist yet, so it is anchored on its parent.
fn guard_copy_target(source: &Path, destination: &Path) -> Result<(), XionError> {
    let source_real = fs::canonicalize(source)?;

    let parent = destination
        .parent()
        .ok_or_else(|| XionError::InvalidPath(destination.to_path_buf()))?;
    let name = destination
        .file_name()
        .ok_or_else(|| XionError::InvalidPath(destination.to_path_buf()))?;
    let destination_real = fs::canonicalize(parent)?.join(name);

    if destination_real == source_real {
        return Err(XionError::Rejected(format!(
            "la source et la destination sont identiques : {}",
            source_real.display()
        )));
    }

    // `Path::starts_with` compares whole components, so `/a/bc` is correctly
    // NOT considered nested in `/a/b`.
    if destination_real.starts_with(&source_real) {
        return Err(XionError::Rejected(format!(
            "impossible de copier {} dans son propre sous-dossier {}",
            source_real.display(),
            destination_real.display()
        )));
    }

    ensure_destination_absent(destination)
}

/// Remove a destination that was only partially written.
///
/// Safe because `guard_copy_target` proved the destination did not exist before
/// the operation started: there is nothing of the user's to destroy.
fn discard_partial(destination: &Path) {
    if fs::symlink_metadata(destination).is_err() {
        return;
    }
    if let Err(error) = delete_entry(destination) {
        tracing::warn!(
            "Copie interrompue : nettoyage de {} impossible : {error}",
            destination.display()
        );
    }
}

fn copy_entry(source: &Path, destination: &Path) -> Result<(), XionError> {
    let source = ensure_long_path(source);
    let destination = ensure_long_path(destination);
    guard_copy_target(&source, &destination)?;
    copy_tree_checked(&source, &destination)
}

/// Copy a tree that has already passed `guard_copy_target`, discarding a
/// partial result if anything fails.
fn copy_tree_checked(source: &Path, destination: &Path) -> Result<(), XionError> {
    match copy_tree(source, destination, 0) {
        Ok(()) => Ok(()),
        Err(error) => {
            discard_partial(destination);
            Err(error)
        }
    }
}

fn copy_tree(source: &Path, destination: &Path, depth: u32) -> Result<(), XionError> {
    if depth > MAX_COPY_DEPTH {
        return Err(XionError::Rejected(format!(
            "profondeur maximale de {MAX_COPY_DEPTH} niveaux dépassée sous {}",
            source.display()
        )));
    }

    let metadata = fs::symlink_metadata(source)?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        return copy_symlink(source, destination).map_err(XionError::Io);
    }

    if file_type.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
                depth + 1,
            )?;
        }
        // Directory timestamps are deliberately not copied: `copy_file_times`
        // opens the destination for writing, which fails on a directory on
        // every supported platform, so calling it here was always a no-op.
        return Ok(());
    }

    fs::copy(source, destination)?;
    copy_file_times(source, destination);
    Ok(())
}

/// Recreate a symbolic link at the destination, preserving its target verbatim.
///
/// The previous implementation skipped symlinks with a `tracing::warn!` and
/// still reported success. A cross-device *move* then deleted the source, so
/// every link and junction in the tree was destroyed while the status bar said
/// the move had succeeded. Failing loudly is the point.
#[cfg(windows)]
fn copy_symlink(source: &Path, destination: &Path) -> io::Result<()> {
    let target = fs::read_link(source)?;
    // Windows fixes whether a link is a file or a directory link at creation
    // time. `metadata` follows the link; a broken link is treated as a file
    // link, which is the only thing we can do without the original flag.
    if fs::metadata(source).is_ok_and(|meta| meta.is_dir()) {
        std::os::windows::fs::symlink_dir(&target, destination)
    } else {
        std::os::windows::fs::symlink_file(&target, destination)
    }
}

#[cfg(not(windows))]
fn copy_symlink(source: &Path, destination: &Path) -> io::Result<()> {
    let target = fs::read_link(source)?;
    std::os::unix::fs::symlink(&target, destination)
}

fn copy_file_times(source: &Path, destination: &Path) {
    let Ok(source_meta) = fs::metadata(source) else {
        return;
    };
    let Ok(dst_file) = fs::OpenOptions::new().write(true).open(destination) else {
        return;
    };
    let mut times = fs::FileTimes::new();
    if let Ok(modified) = source_meta.modified() {
        times = times.set_modified(modified);
    }
    if let Ok(accessed) = source_meta.accessed() {
        times = times.set_accessed(accessed);
    }
    let _ = dst_file.set_times(times);
}

fn move_entry(source: &Path, destination: &Path) -> Result<(), XionError> {
    let source = ensure_long_path(source);
    let destination = ensure_long_path(destination);
    guard_copy_target(&source, &destination)?;

    match fs::rename(source.as_ref(), destination.as_ref()) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            // The copy half must be complete before the source is touched:
            // `copy_tree_checked` propagates any failure and clears the partial
            // destination, so a tree we could not fully reproduce never results
            // in the original being deleted.
            copy_tree_checked(&source, &destination)?;

            if let Err(del_err) = delete_entry(&source) {
                tracing::warn!(
                    "Déplacement inter-volumes : suppression de la source impossible, \
                     retrait de la copie : {del_err}"
                );
                discard_partial(&destination);
                return Err(del_err);
            }
            Ok(())
        }
        Err(error) => Err(XionError::Io(error)),
    }
}

fn delete_entry(path: &Path) -> Result<(), XionError> {
    let path = ensure_long_path(path);
    let metadata = fs::symlink_metadata(path.as_ref())?;
    let ft = metadata.file_type();
    if ft.is_dir() && !ft.is_symlink() {
        fs::remove_dir_all(path.as_ref())?;
    } else {
        fs::remove_file(path.as_ref())?;
    }
    Ok(())
}

fn is_cross_device_error(error: &io::Error) -> bool {
    match error.raw_os_error() {
        #[cfg(unix)]
        Some(18) => true, // EXDEV
        #[cfg(windows)]
        Some(17) => true, // ERROR_NOT_SAME_DEVICE
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    /// Regression: copying a folder into one of its own descendants used to
    /// recurse forever and fill the disk.
    #[test]
    fn copy_into_own_subdirectory_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("projects");
        write(&source.join("a.txt"), "a");
        let nested = source.join("build");
        fs::create_dir_all(&nested).unwrap();

        let report =
            LocalFileOperations::new().copy_items(std::slice::from_ref(&source), &nested, None);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(
            report.failed[0].error.contains("sous-dossier"),
            "message inattendu : {}",
            report.failed[0].error
        );
        assert!(
            !nested.join("projects").exists(),
            "rien ne doit avoir été écrit dans la destination"
        );
    }

    /// A destination nested one level deeper must be rejected too: the guard
    /// compares whole components, not string prefixes.
    #[test]
    fn copy_into_deep_own_subdirectory_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("projects");
        let nested = source.join("build").join("out").join("dist");
        fs::create_dir_all(&nested).unwrap();

        let report = LocalFileOperations::new().copy_items(&[source], &nested, None);

        assert_eq!(report.failed.len(), 1);
        assert!(report.succeeded.is_empty());
    }

    /// A sibling whose name merely shares a prefix is NOT nested.
    #[test]
    fn copy_into_sibling_with_shared_prefix_is_allowed() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("proj");
        write(&source.join("a.txt"), "a");
        let sibling = root.path().join("projects");
        fs::create_dir_all(&sibling).unwrap();

        let report = LocalFileOperations::new().copy_items(&[source], &sibling, None);

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(sibling.join("proj").join("a.txt").exists());
    }

    #[test]
    fn copy_onto_itself_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("data");
        write(&source.join("a.txt"), "a");

        let report = LocalFileOperations::new().copy_items(&[source], root.path(), None);

        assert_eq!(report.failed.len(), 1);
        assert!(
            report.failed[0].error.contains("identiques"),
            "message inattendu : {}",
            report.failed[0].error
        );
    }

    /// Regression: symlinks were silently skipped, and a cross-device move then
    /// deleted the source anyway. They must now be recreated.
    #[cfg(unix)]
    #[test]
    fn copy_recreates_symlinks_instead_of_skipping_them() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("tree");
        write(&source.join("real.txt"), "payload");
        std::os::unix::fs::symlink("real.txt", source.join("link.txt")).unwrap();
        let dest_dir = root.path().join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let report = LocalFileOperations::new().copy_items(&[source], &dest_dir, None);

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        let copied_link = dest_dir.join("tree").join("link.txt");
        let meta = fs::symlink_metadata(&copied_link).unwrap();
        assert!(meta.file_type().is_symlink(), "le lien doit rester un lien");
        assert_eq!(fs::read_link(&copied_link).unwrap(), Path::new("real.txt"));
    }

    /// A dangling symlink must still be reproduced rather than dereferenced.
    #[cfg(unix)]
    #[test]
    fn copy_reproduces_dangling_symlink() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("tree");
        fs::create_dir_all(&source).unwrap();
        std::os::unix::fs::symlink("nowhere", source.join("broken")).unwrap();
        let dest_dir = root.path().join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let report = LocalFileOperations::new().copy_items(&[source], &dest_dir, None);

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        let copied = dest_dir.join("tree").join("broken");
        assert!(
            fs::symlink_metadata(&copied)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn copy_rejects_an_occupied_destination() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("a.txt");
        write(&source, "one");
        let dest_dir = root.path().join("dest");
        write(&dest_dir.join("a.txt"), "already here");

        let report = LocalFileOperations::new().copy_items(&[source], &dest_dir, None);

        assert_eq!(report.failed.len(), 1);
        assert_eq!(
            fs::read_to_string(dest_dir.join("a.txt")).unwrap(),
            "already here"
        );
    }

    #[test]
    fn rename_still_works_after_the_guard() {
        let root = tempfile::tempdir().unwrap();
        let from = root.path().join("before.txt");
        write(&from, "x");
        let to = root.path().join("after.txt");

        let report = LocalFileOperations::new().rename_item(&from, &to);

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(to.exists() && !from.exists());
    }

    #[test]
    fn move_within_the_same_volume_uses_rename() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("tree");
        write(&source.join("a.txt"), "payload");
        let dest_dir = root.path().join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let report =
            LocalFileOperations::new().move_items(std::slice::from_ref(&source), &dest_dir, None);

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(!source.exists());
        assert_eq!(
            fs::read_to_string(dest_dir.join("tree").join("a.txt")).unwrap(),
            "payload"
        );
    }
}
