use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{AppResult, XionError};

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub entry_type: FsEntryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryType {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct ListOptions {
    pub show_hidden: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self { show_hidden: false }
    }
}

pub trait FileSystem {
    fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>>;
}

#[derive(Debug, Default)]
pub struct LocalFileSystem;

impl LocalFileSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn list_dir_with_options(
        &self,
        path: &Path,
        options: &ListOptions,
    ) -> AppResult<Vec<FsEntry>> {
        self.list_dir(path, *options)
    }

    fn is_hidden(name: &str) -> bool {
        name.starts_with('.')
    }
}

impl FileSystem for LocalFileSystem {
    fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>> {
        if !path.exists() {
            return Err(XionError::NotFound(path.to_path_buf()));
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();

            if !options.show_hidden && Self::is_hidden(&name) {
                continue;
            }

            let file_type = entry.file_type()?;
            let entry_type = if file_type.is_dir() {
                FsEntryType::Directory
            } else if file_type.is_file() {
                FsEntryType::File
            } else if file_type.is_symlink() {
                FsEntryType::Symlink
            } else {
                FsEntryType::Other
            };

            entries.push(FsEntry {
                path: entry.path(),
                name,
                entry_type,
            });
        }

        entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(entries)
    }
}
