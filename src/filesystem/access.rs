//! Filesystem access abstraction and local filesystem implementation.
//!
//! This module provides the [`FileSystem`] trait for abstracting filesystem
//! operations and [`LocalFileSystem`] as the concrete implementation for
//! local disk access.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use tracing::warn;

use crate::core::{AppResult, FilesystemConfig, XionError};
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

#[derive(Debug, Clone)]
pub struct LocalFileSystem {
    metadata_batch_size: usize,
    metadata_parallelism: usize,
}

impl LocalFileSystem {
    pub fn new() -> Self {
        Self::from_config(FilesystemConfig::default())
    }

    pub fn from_config(config: FilesystemConfig) -> Self {
        Self {
            metadata_batch_size: config.metadata_batch_size.max(1),
            metadata_parallelism: config.metadata_parallelism.max(1),
        }
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

    fn compare_entry_stubs(options: &ListOptions, left: &EntryStub, right: &EntryStub) -> Ordering {
        if options.directories_first && left.entry_type != right.entry_type {
            return match (left.entry_type, right.entry_type) {
                (FsEntryType::Directory, _) => Ordering::Less,
                (_, FsEntryType::Directory) => Ordering::Greater,
                _ => Ordering::Equal,
            };
        }

        let ordering = match options.sort_by {
            SortKey::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortKey::Modified => left
                .metadata
                .as_ref()
                .map(|metadata| metadata.modified)
                .cmp(&right.metadata.as_ref().map(|metadata| metadata.modified)),
            SortKey::Size => left
                .metadata
                .as_ref()
                .map(|metadata| metadata.size)
                .cmp(&right.metadata.as_ref().map(|metadata| metadata.size)),
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

            entries.push(EntryStub {
                path: entry.path(),
                name,
                entry_type,
                metadata: None,
            });
        }

        let metadata = if entries.is_empty() {
            Vec::new()
        } else {
            let paths = entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            self.metadata_batch(&paths)?
        };

        let mut entries = entries
            .into_iter()
            .zip(metadata)
            .map(|(entry, metadata)| FsEntry {
                path: entry.path,
                name: entry.name,
                entry_type: entry.entry_type,
                metadata,
            })
            .collect::<Vec<_>>();

        entries.sort_by(|left, right| Self::compare_entries(&options, left, right));
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> AppResult<FsMetadata> {
        let metadata = fs::metadata(path)?;
        Ok(FsMetadata::from_metadata(metadata))
    }

    fn list_dir_paged(
        &self,
        path: &Path,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        if !path.exists() {
            return Err(XionError::NotFound(path.to_path_buf()));
        }

        let needs_full_metadata = matches!(options.sort_by, SortKey::Modified | SortKey::Size);
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

            entries.push(EntryStub {
                path: entry.path(),
                name,
                entry_type,
                metadata: None,
            });
        }

        if needs_full_metadata {
            let paths = entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            let mut metadata_iter = if paths.is_empty() {
                Vec::new().into_iter()
            } else {
                self.metadata_batch(&paths)?.into_iter()
            };
            for entry in &mut entries {
                entry.metadata = Some(
                    metadata_iter
                        .next()
                        .ok_or_else(|| XionError::InvalidPath(entry.path.clone()))?,
                );
            }
        }

        entries.sort_by(|left, right| Self::compare_entry_stubs(&options, left, right));

        let total = entries.len();
        let offset = page.offset.min(total);
        let end = offset.saturating_add(page.limit).min(total);
        let slice = &entries[offset..end];

        let missing_paths: Vec<PathBuf> = slice
            .iter()
            .filter(|entry| entry.metadata.is_none())
            .map(|entry| entry.path.clone())
            .collect();

        let mut missing_iter = if missing_paths.is_empty() {
            Vec::new().into_iter()
        } else {
            self.metadata_batch(&missing_paths)?.into_iter()
        };

        let mut items = Vec::with_capacity(slice.len());
        for entry in slice {
            let metadata = match entry.metadata.as_ref() {
                Some(metadata) => metadata.clone(),
                None => missing_iter
                    .next()
                    .ok_or_else(|| XionError::InvalidPath(entry.path.clone()))?,
            };

            items.push(FsEntry {
                path: entry.path.clone(),
                name: entry.name.clone(),
                entry_type: entry.entry_type,
                metadata,
            });
        }

        Ok(Page {
            items,
            total,
            offset,
            limit: page.limit,
        })
    }

    fn metadata_batch(&self, paths: &[PathBuf]) -> AppResult<Vec<FsMetadata>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let mut metadata = Vec::with_capacity(paths.len());
        for chunk in paths.chunks(self.metadata_batch_size.max(1)) {
            let mut batch_metadata = self.metadata_batch_chunk(chunk)?;
            metadata.append(&mut batch_metadata);
        }
        Ok(metadata)
    }
}

impl LocalFileSystem {
    fn metadata_batch_chunk(&self, paths: &[PathBuf]) -> AppResult<Vec<FsMetadata>> {
        let available_threads = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        let thread_count = self
            .metadata_parallelism
            .min(available_threads)
            .max(1);
        let chunk_size = ((paths.len() + thread_count - 1) / thread_count).max(1);
        let mut initial_results = Vec::with_capacity(paths.len());
        initial_results.resize_with(paths.len(), || None);
        let results = Arc::new(Mutex::new(initial_results));

        thread::scope(|scope| {
            for (chunk_index, chunk) in paths.chunks(chunk_size).enumerate() {
                let results = Arc::clone(&results);
                scope.spawn(move || {
                    for (index, path) in chunk.iter().enumerate() {
                        let result = fs::metadata(path)
                            .map(FsMetadata::from_metadata)
                            .map_err(XionError::from);
                        match results.lock() {
                            Ok(mut guard) => {
                                guard[chunk_index * chunk_size + index] = Some(result);
                            }
                            Err(poisoned) => {
                                // Mutex was poisoned by a panic in another thread.
                                // Log warning and recover by accessing the data anyway.
                                warn!(
                                    path = %path.display(),
                                    "Mutex poisoned during metadata batch, recovering"
                                );
                                let mut guard = poisoned.into_inner();
                                guard[chunk_index * chunk_size + index] = Some(result);
                            }
                        }
                    }
                });
            }
        });

        let results = Arc::try_unwrap(results)
            .map_err(|_| XionError::InvalidPath(PathBuf::from("<batch>")))?
            .into_inner()
            .map_err(|_| XionError::InvalidPath(PathBuf::from("<batch>")))?;

        let mut metadata = Vec::with_capacity(paths.len());
        for result in results {
            match result {
                Some(Ok(value)) => metadata.push(value),
                Some(Err(error)) => return Err(error),
                None => return Err(XionError::InvalidPath(PathBuf::from("<batch>"))),
            }
        }

        Ok(metadata)
    }
}

#[derive(Debug)]
struct EntryStub {
    path: PathBuf,
    name: String,
    entry_type: FsEntryType,
    metadata: Option<FsMetadata>,
}

#[cfg(test)]
mod tests {
    use super::{EntryFilter, FileSystem, ListOptions, LocalFileSystem, PageRequest};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("xion_fs_test_{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write file");
    }

    #[test]
    fn list_dir_filters_and_sorts() {
        let root = create_temp_dir();
        let folder = root.join("folder");
        fs::create_dir_all(&folder).expect("create folder");
        write_file(&root.join("alpha.txt"), "alpha");
        write_file(&root.join("beta.log"), "beta");
        write_file(&root.join(".hidden"), "hidden");

        let fs = LocalFileSystem::new();
        let entries = fs
            .list_dir(&root, ListOptions::default())
            .expect("list dir");
        let names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();

        assert_eq!(names, vec!["folder", "alpha.txt", "beta.log"]);

        let files_only = fs
            .list_dir(
                &root,
                ListOptions {
                    filter: EntryFilter::OnlyFiles,
                    ..ListOptions::default()
                },
            )
            .expect("list dir files");
        assert_eq!(files_only.len(), 2);

        let query_only = fs
            .list_dir(
                &root,
                ListOptions::default().with_name_query("alp"),
            )
            .expect("list dir query");
        assert_eq!(query_only.len(), 1);
        assert_eq!(query_only[0].name, "alpha.txt");

        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_dir_paged_returns_expected_slice() {
        let root = create_temp_dir();
        let folder = root.join("folder");
        fs::create_dir_all(&folder).expect("create folder");
        write_file(&root.join("alpha.txt"), "alpha");
        write_file(&root.join("beta.log"), "beta");

        let fs = LocalFileSystem::new();
        let page = fs
            .list_dir_paged(&root, ListOptions::default(), PageRequest::new(1, 1))
            .expect("list dir paged");

        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "alpha.txt");

        fs::remove_dir_all(&root).expect("cleanup");
    }
}
