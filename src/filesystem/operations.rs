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
            std::borrow::Cow::Owned(std::path::PathBuf::from(
                format!("\\\\?\\UNC\\{}", s.strip_prefix("\\\\").unwrap_or(&s))
            ))
        } else {
            std::borrow::Cow::Owned(std::path::PathBuf::from(
                format!("\\\\?\\{}", s)
            ))
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
        if let Some(name) = to.file_name().and_then(|n| n.to_str()) {
            if is_reserved_windows_name(name) {
                report.push_failure(
                    from.to_path_buf(),
                    XionError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("'{}' is a reserved Windows filename", name),
                    )),
                );
                return report;
            }
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
    matches!(stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
        | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
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

/// Best-effort pre-check for destination existence.
///
/// This is subject to TOCTOU races: another process may create the path
/// between this check and the actual operation. Callers should also handle
/// `AlreadyExists` errors from the underlying fs operations.
fn ensure_destination_absent(destination: &Path) -> Result<(), XionError> {
    if destination.exists() {
        return Err(XionError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Destination already exists: {}", destination.display()),
        )));
    }
    Ok(())
}

fn copy_entry(source: &Path, destination: &Path) -> Result<(), XionError> {
    let source = ensure_long_path(source);
    let destination = ensure_long_path(destination);
    ensure_destination_absent(&destination)?;
    let metadata = fs::symlink_metadata(source.as_ref())?;
    if metadata.file_type().is_dir() {
        copy_dir_recursive(source.as_ref(), destination.as_ref())?;
    } else {
        match fs::copy(source.as_ref(), destination.as_ref()) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(XionError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("Destination already exists: {}", destination.display()),
                )));
            }
            Err(e) => return Err(XionError::Io(e)),
        }
        copy_file_times(source.as_ref(), destination.as_ref());
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), XionError> {
    let source = ensure_long_path(source);
    let destination = ensure_long_path(destination);
    ensure_destination_absent(&destination)?;
    match fs::create_dir_all(destination.as_ref()) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(XionError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Destination already exists: {}", destination.display()),
            )));
        }
        Err(e) => return Err(XionError::Io(e)),
    }
    for entry in fs::read_dir(source.as_ref())? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            tracing::warn!("Copie: lien symbolique ignoré: {}", path.display());
            continue;
        }
        if file_type.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
            copy_file_times(&path, &target);
        }
    }
    // Set directory times after all children are processed.
    copy_file_times(source.as_ref(), destination.as_ref());
    Ok(())
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
    ensure_destination_absent(&destination)?;
    match fs::rename(source.as_ref(), destination.as_ref()) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            copy_entry(&source, &destination)?;
            if let Err(del_err) = delete_entry(&source) {
                // Rollback: copy succeeded but delete failed — remove the copy
                tracing::warn!("Move cross-device: suppression source échouée, rollback copie: {del_err}");
                let _ = delete_entry(&destination);
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
