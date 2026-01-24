use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::core::AppResult;
use crate::filesystem::{
    EntryFilter, FileSystem, FsEntry, FsEntryType, ListOptions, Page, PageRequest, SortKey,
};

#[derive(Debug, Default)]
pub struct SearchService;

#[derive(Debug, Clone)]
pub struct SearchIndexOptions {
    pub include_hidden: bool,
    pub recursive: bool,
}

impl Default for SearchIndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            recursive: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub extensions: Vec<String>,
    pub entry_filter: EntryFilter,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub modified_after: Option<SystemTime>,
    pub modified_before: Option<SystemTime>,
    pub case_sensitive: bool,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            extensions: Vec::new(),
            entry_filter: EntryFilter::All,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            case_sensitive: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchIndex {
    pub root: PathBuf,
    entries: Vec<SearchEntry>,
}

impl SearchIndex {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &FsEntry> {
        self.entries.iter().map(|entry| &entry.entry)
    }
}

impl SearchService {
    pub fn search_in_dir(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        query: &str,
    ) -> AppResult<Vec<FsEntry>> {
        let options = ListOptions {
            show_hidden: false,
            sort_by: SortKey::Name,
            ..ListOptions::default()
        }
        .with_name_query(query);

        filesystem.list_dir(path, options)
    }

    pub fn search_in_dir_paged(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        query: &str,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        let options = ListOptions {
            show_hidden: false,
            sort_by: SortKey::Name,
            ..ListOptions::default()
        }
        .with_name_query(query);

        filesystem.list_dir_paged(path, options, page)
    }

    pub fn search_files_only(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        query: &str,
    ) -> AppResult<Vec<FsEntry>> {
        let options = ListOptions {
            filter: EntryFilter::OnlyFiles,
            ..ListOptions::default()
        }
        .with_name_query(query);

        filesystem.list_dir(path, options)
    }

    pub fn build_index(
        &self,
        filesystem: &dyn FileSystem,
        root: &Path,
        options: SearchIndexOptions,
    ) -> AppResult<SearchIndex> {
        let list_options = ListOptions {
            show_hidden: options.include_hidden,
            sort_by: SortKey::Name,
            ..ListOptions::default()
        };

        self.build_index_with_options(filesystem, root, options, list_options)
    }

    pub fn build_index_with_options(
        &self,
        filesystem: &dyn FileSystem,
        root: &Path,
        options: SearchIndexOptions,
        list_options: ListOptions,
    ) -> AppResult<SearchIndex> {
        let mut entries = Vec::new();
        self.index_dir_with_options(filesystem, root, &options, &list_options, &mut entries)?;
        Ok(SearchIndex {
            root: root.to_path_buf(),
            entries,
        })
    }

    pub fn search_index(&self, index: &SearchIndex, query: &SearchQuery) -> Vec<FsEntry> {
        let normalized_text = query.text.as_ref().map(|text| {
            if query.case_sensitive {
                text.clone()
            } else {
                text.to_lowercase()
            }
        });
        let normalized_extensions = if query.case_sensitive {
            query.extensions.clone()
        } else {
            query
                .extensions
                .iter()
                .map(|ext| ext.to_lowercase())
                .collect()
        };

        index
            .entries
            .iter()
            .filter(|entry| {
                Self::matches_query(
                    entry,
                    query,
                    normalized_text.as_ref(),
                    &normalized_extensions,
                )
            })
            .map(|entry| entry.entry.clone())
            .collect()
    }

    pub fn search_index_paged(
        &self,
        index: &SearchIndex,
        query: &SearchQuery,
        page: PageRequest,
    ) -> Page<FsEntry> {
        let results = self.search_index(index, query);
        page.apply(results)
    }

    pub fn count_index_matches(&self, index: &SearchIndex, query: &SearchQuery) -> usize {
        let normalized_text = query.text.as_ref().map(|text| {
            if query.case_sensitive {
                text.clone()
            } else {
                text.to_lowercase()
            }
        });
        let normalized_extensions = if query.case_sensitive {
            query.extensions.clone()
        } else {
            query
                .extensions
                .iter()
                .map(|ext| ext.to_lowercase())
                .collect()
        };

        index
            .entries
            .iter()
            .filter(|entry| {
                Self::matches_query(
                    entry,
                    query,
                    normalized_text.as_ref(),
                    &normalized_extensions,
                )
            })
            .count()
    }

    fn index_dir(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        options: &SearchIndexOptions,
        output: &mut Vec<SearchEntry>,
    ) -> AppResult<()> {
        let list_options = ListOptions {
            show_hidden: options.include_hidden,
            sort_by: SortKey::Name,
            ..ListOptions::default()
        };
        self.index_dir_with_options(filesystem, path, options, &list_options, output)
    }

    fn index_dir_with_options(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        options: &SearchIndexOptions,
        list_options: &ListOptions,
        output: &mut Vec<SearchEntry>,
    ) -> AppResult<()> {
        let mut resolved_options = list_options.clone();
        resolved_options.show_hidden = options.include_hidden;
        resolved_options.name_query = None;

        let entries = filesystem.list_dir(path, resolved_options.clone())?;
        for entry in entries {
            let name_lower = entry.name.to_lowercase();
            let extension_lower = entry
                .path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase());

            output.push(SearchEntry {
                entry: entry.clone(),
                name_lower,
                extension_lower,
            });

            if options.recursive && entry.entry_type == FsEntryType::Directory {
                self.index_dir_with_options(
                    filesystem,
                    &entry.path,
                    options,
                    &resolved_options,
                    output,
                )?;
            }
        }

        Ok(())
    }

    fn matches_query(
        entry: &SearchEntry,
        query: &SearchQuery,
        normalized_text: Option<&String>,
        normalized_extensions: &[String],
    ) -> bool {
        if query.entry_filter != EntryFilter::All {
            let is_match = match query.entry_filter {
                EntryFilter::OnlyDirectories => entry.entry.entry_type == FsEntryType::Directory,
                EntryFilter::OnlyFiles => entry.entry.entry_type == FsEntryType::File,
                EntryFilter::All => true,
            };
            if !is_match {
                return false;
            }
        }

        if let Some(text) = normalized_text {
            let haystack = if query.case_sensitive {
                &entry.entry.name
            } else {
                &entry.name_lower
            };
            if !haystack.contains(text) {
                return false;
            }
        }

        if !normalized_extensions.is_empty() {
            let entry_extension = if query.case_sensitive {
                entry
                    .entry
                    .path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_string())
            } else {
                entry.extension_lower.clone()
            };
            let Some(entry_extension) = entry_extension else {
                return false;
            };
            if !normalized_extensions
                .iter()
                .any(|ext| ext == &entry_extension)
            {
                return false;
            }
        }

        if let Some(min_size) = query.min_size {
            if entry.entry.metadata.size < min_size {
                return false;
            }
        }

        if let Some(max_size) = query.max_size {
            if entry.entry.metadata.size > max_size {
                return false;
            }
        }

        if query.modified_after.is_some() || query.modified_before.is_some() {
            let modified = match entry.entry.metadata.modified {
                Some(modified) => modified,
                None => return false,
            };

            if let Some(after) = query.modified_after {
                if modified < after {
                    return false;
                }
            }

            if let Some(before) = query.modified_before {
                if modified > before {
                    return false;
                }
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
struct SearchEntry {
    entry: FsEntry,
    name_lower: String,
    extension_lower: Option<String>,
}
