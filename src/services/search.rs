use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::core::AppResult;
use crate::filesystem::{EntryFilter, FileSystem, FsEntry, FsEntryType, ListOptions, SortKey};

/// Maximum directory recursion depth to prevent stack overflow on deeply nested
/// or circular filesystem structures.
const MAX_SEARCH_DEPTH: usize = 32;

#[derive(Debug, Default)]
pub struct SearchService;

#[derive(Debug, Clone)]
pub struct SearchIndexOptions {
    pub include_hidden: bool,
    pub recursive: bool,
    /// Maximum number of entries to index. Prevents unbounded memory growth
    /// when indexing very large directory trees. 0 means unlimited.
    pub max_entries: usize,
}

impl Default for SearchIndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            recursive: true,
            max_entries: 50_000,
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

    pub fn build_index_with_options(
        &self,
        filesystem: &dyn FileSystem,
        root: &Path,
        options: SearchIndexOptions,
        list_options: ListOptions,
    ) -> AppResult<SearchIndex> {
        let mut entries = Vec::new();
        self.index_dir_with_options(
            filesystem,
            root,
            &options,
            &list_options,
            &mut entries,
            MAX_SEARCH_DEPTH,
        )?;
        Ok(SearchIndex {
            root: root.to_path_buf(),
            entries,
        })
    }

    pub fn count_index_matches(&self, index: &SearchIndex, query: &SearchQuery) -> usize {
        Self::matching(index, query).count()
    }

    /// The entries of `index` that satisfy `query`.
    ///
    /// The query is normalised once, here, rather than once per caller: this
    /// same `filter` used to be written out three times, and only two of the
    /// three were ever called.
    fn matching<'a>(
        index: &'a SearchIndex,
        query: &'a SearchQuery,
    ) -> impl Iterator<Item = &'a SearchEntry> + 'a {
        let (normalized_text, normalized_extensions) = Self::normalize_query(query);
        index.entries.iter().filter(move |entry| {
            Self::matches_query(
                entry,
                query,
                normalized_text.as_ref(),
                &normalized_extensions,
            )
        })
    }

    fn normalize_query(query: &SearchQuery) -> (Option<String>, Vec<String>) {
        let text = query.text.as_ref().map(|t| {
            if query.case_sensitive {
                t.clone()
            } else {
                t.to_lowercase()
            }
        });
        let extensions = if query.case_sensitive {
            query.extensions.clone()
        } else {
            query
                .extensions
                .iter()
                .map(|ext| ext.to_lowercase())
                .collect()
        };
        (text, extensions)
    }

    fn index_dir_with_options(
        &self,
        filesystem: &dyn FileSystem,
        path: &Path,
        options: &SearchIndexOptions,
        list_options: &ListOptions,
        output: &mut Vec<SearchEntry>,
        remaining_depth: usize,
    ) -> AppResult<()> {
        if remaining_depth == 0 {
            tracing::warn!(
                "Recherche: profondeur max atteinte à {:?}, résultats incomplets",
                path
            );
            return Ok(());
        }
        if options.max_entries > 0 && output.len() >= options.max_entries {
            return Ok(());
        }

        let mut resolved_options = list_options.clone();
        resolved_options.show_hidden = options.include_hidden;
        resolved_options.name_query = None;

        let entries = match filesystem.list_dir(path, resolved_options.clone()) {
            Ok(entries) => entries,
            Err(_) => {
                // Skip directories we can't access (permission denied, etc.)
                return Ok(());
            }
        };
        for entry in entries {
            if options.max_entries > 0 && output.len() >= options.max_entries {
                break;
            }

            let is_dir = entry.entry_type == FsEntryType::Directory;
            let dir_path = if options.recursive && is_dir {
                Some(entry.path.clone())
            } else {
                None
            };

            output.push(SearchEntry { entry });

            if let Some(dir_path) = dir_path {
                if let Err(e) = self.index_dir_with_options(
                    filesystem,
                    &dir_path,
                    options,
                    &resolved_options,
                    output,
                    remaining_depth - 1,
                ) {
                    tracing::debug!("Recherche: indexation {:?} échouée: {e}", dir_path);
                }
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
            let matches = if query.case_sensitive {
                entry.entry.name.contains(text)
            } else {
                // `normalized_text` is already lowercase; fold the name as we
                // read it rather than storing a lowercase copy of every name.
                contains_lowercased(&entry.entry.name, text)
            };
            if !matches {
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
                entry
                    .entry
                    .path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase())
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

/// One indexed entry.
///
/// It used to carry `name_lower` and `extension_lower`: byte-for-byte lowercase
/// copies of data already in `entry`, 64 extra bytes of heap per entry — 40 % of
/// the index — to spare a case fold at match time. The comparison now folds on
/// the fly, which costs nothing to store.
#[derive(Debug, Clone)]
struct SearchEntry {
    entry: FsEntry,
}

/// Case-insensitive `contains` with no allocation.
///
/// `needle` must already be lowercase. ASCII — nearly every file name — takes a
/// byte scan; anything else folds through `char::to_lowercase` so `ÉCLAIR` still
/// matches `éclair`. Mirrors the comparator the list filter uses.
fn contains_lowercased(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        let hay = haystack.as_bytes();
        let need = needle.as_bytes();
        if need.len() > hay.len() {
            return false;
        }
        return hay
            .windows(need.len())
            .any(|window| window.eq_ignore_ascii_case(need));
    }
    haystack.char_indices().any(|(offset, _)| {
        let mut folded = haystack[offset..].chars().flat_map(char::to_lowercase);
        let mut wanted = needle.chars();
        loop {
            match wanted.next() {
                None => return true,
                Some(expected) => match folded.next() {
                    None => return false,
                    Some(actual) if actual != expected => return false,
                    Some(_) => {}
                },
            }
        }
    })
}
