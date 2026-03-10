use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::XionError;

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

    pub fn copy_items(&self, items: &[PathBuf], dest_dir: &Path) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Copy);
        for item in items {
            match destination_for(item, dest_dir) {
                Ok(destination) => match copy_entry(item, &destination) {
                    Ok(()) => report.succeeded.push(destination),
                    Err(error) => report.push_failure(item.clone(), error),
                },
                Err(error) => report.push_failure(item.clone(), error),
            }
        }
        report
    }

    pub fn move_items(&self, items: &[PathBuf], dest_dir: &Path) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Move);
        for item in items {
            match destination_for(item, dest_dir) {
                Ok(destination) => match move_entry(item, &destination) {
                    Ok(()) => report.succeeded.push(destination),
                    Err(error) => report.push_failure(item.clone(), error),
                },
                Err(error) => report.push_failure(item.clone(), error),
            }
        }
        report
    }

    pub fn rename_item(&self, from: &Path, to: &Path) -> OperationReport {
        let mut report = OperationReport::new(FileOperationKind::Rename);
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

fn destination_for(source: &Path, dest_dir: &Path) -> Result<PathBuf, XionError> {
    let name = source
        .file_name()
        .ok_or_else(|| XionError::InvalidPath(source.to_path_buf()))?;
    Ok(dest_dir.join(name))
}

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
    ensure_destination_absent(destination)?;
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_dir() {
        copy_dir_recursive(source, destination)?;
    } else {
        fs::copy(source, destination)?;
        copy_file_times(source, destination);
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), XionError> {
    ensure_destination_absent(destination)?;
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
            copy_file_times(&path, &target);
        }
    }
    // Set directory times after all children are processed.
    copy_file_times(source, destination);
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
    ensure_destination_absent(destination)?;
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            copy_entry(source, destination)?;
            delete_entry(source)?;
            Ok(())
        }
        Err(error) => Err(XionError::Io(error)),
    }
}

fn delete_entry(path: &Path) -> Result<(), XionError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
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
