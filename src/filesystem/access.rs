use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{AppResult, XionError};
use crate::filesystem::metadata::FsMetadata;
use crate::filesystem::paging::{Page, PageRequest};

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub entry_type: FsEntryType,
    pub metadata: FsMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryType {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFilter {
    All,
    OnlyDirectories,
    OnlyFiles,
}

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub sort_by: SortKey,
    pub sort_order: SortOrder,
    pub directories_first: bool,
    pub filter: EntryFilter,
    pub name_query: Option<String>,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            filter: EntryFilter::All,
            name_query: None,
        }
    }
}

impl ListOptions {
    pub fn with_name_query(mut self, query: impl Into<String>) -> Self {
        self.name_query = Some(query.into());
        self
    }
}

pub trait FileSystem {
    fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>>;
    fn metadata(&self, path: &Path) -> AppResult<FsMetadata>;
    fn list_dir_paged(
        &self,
        path: &Path,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        let entries = self.list_dir(path, options)?;
        Ok(page.apply(entries))
    }
    fn metadata_batch(&self, paths: &[PathBuf]) -> AppResult<Vec<FsMetadata>> {
        paths.iter().map(|path| self.metadata(path)).collect()
    }
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
        self.list_dir(path, options.clone())
    }

    fn is_hidden(name: &str) -> bool {
        name.starts_with('.')
    }

    fn matches_filter(entry_type: FsEntryType, filter: EntryFilter) -> bool {
        match filter {
            EntryFilter::All => true,
            EntryFilter::OnlyDirectories => entry_type == FsEntryType::Directory,
            EntryFilter::OnlyFiles => entry_type == FsEntryType::File,
        }
    }

    fn matches_query(name: &str, query: &Option<String>) -> bool {
        match query {
            Some(query) if !query.is_empty() => name.to_lowercase().contains(&query.to_lowercase()),
            _ => true,
        }
    }

    fn compare_entries(options: &ListOptions, left: &FsEntry, right: &FsEntry) -> Ordering {
        if options.directories_first && left.entry_type != right.entry_type {
            return match (left.entry_type, right.entry_type) {
                (FsEntryType::Directory, _) => Ordering::Less,
                (_, FsEntryType::Directory) => Ordering::Greater,
                _ => Ordering::Equal,
            };
        }

        let ordering = match options.sort_by {
            SortKey::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortKey::Modified => left.metadata.modified.cmp(&right.metadata.modified),
            SortKey::Size => left.metadata.size.cmp(&right.metadata.size),
        };

        match options.sort_order {
            SortOrder::Asc => ordering,
            SortOrder::Desc => ordering.reverse(),
        }
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

            if !Self::matches_filter(entry_type, options.filter) {
                continue;
            }

            if !Self::matches_query(&name, &options.name_query) {
                continue;
            }

            let metadata = FsMetadata::from_metadata(entry.metadata()?);
            entries.push(FsEntry {
                path: entry.path(),
                name,
                entry_type,
                metadata,
            });
        }

        entries.sort_by(|left, right| Self::compare_entries(&options, left, right));
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> AppResult<FsMetadata> {
        let metadata = fs::metadata(path)?;
        Ok(FsMetadata::from_metadata(metadata))
    }

    fn metadata_batch(&self, paths: &[PathBuf]) -> AppResult<Vec<FsMetadata>> {
        paths.iter().map(|path| self.metadata(path)).collect()
    }
}
