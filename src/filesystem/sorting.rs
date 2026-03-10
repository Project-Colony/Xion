//! Unified sorting and comparison utilities for filesystem entries.
//!
//! This module provides consistent sorting behavior across local and network
//! file listings, ensuring predictable ordering regardless of entry source.

use std::cmp::Ordering;

use crate::filesystem::{EntryFilter, FsEntry, FsEntryType, ListOptions, SortKey, SortOrder};

/// Compares two filesystem entries according to the specified options.
///
/// # Ordering Rules
///
/// 1. If `directories_first` is enabled, directories always sort before files.
/// 2. Entries are then sorted by the specified key (name, modified, size).
/// 3. Sort order (ascending/descending) is applied last.
///
/// # Example
///
/// ```ignore
/// let options = ListOptions::default();
/// let ordering = compare_entries(&entry_a, &entry_b, &options);
/// ```
pub fn compare_entries(left: &FsEntry, right: &FsEntry, options: &ListOptions) -> Ordering {
    // Handle directories-first sorting
    if options.directories_first && left.entry_type != right.entry_type {
        return match (left.entry_type, right.entry_type) {
            (FsEntryType::Directory, _) => Ordering::Less,
            (_, FsEntryType::Directory) => Ordering::Greater,
            _ => Ordering::Equal,
        };
    }

    // Sort by the specified key
    let ordering = match options.sort_by {
        SortKey::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        SortKey::Modified => left.metadata.modified.cmp(&right.metadata.modified),
        SortKey::Size => left.metadata.size.cmp(&right.metadata.size),
    };

    // Apply sort order
    match options.sort_order {
        SortOrder::Asc => ordering,
        SortOrder::Desc => ordering.reverse(),
    }
}

/// Checks if an entry passes the filter criteria.
///
/// # Arguments
///
/// * `entry` - The entry to check.
/// * `options` - Options containing filter and query settings.
///
/// # Returns
///
/// `true` if the entry matches all filter criteria, `false` otherwise.
pub fn matches_filter(entry: &FsEntry, options: &ListOptions) -> bool {
    // Check entry type filter
    let passes_type_filter = match options.filter {
        EntryFilter::All => true,
        EntryFilter::OnlyDirectories => entry.entry_type == FsEntryType::Directory,
        EntryFilter::OnlyFiles => entry.entry_type == FsEntryType::File,
    };

    if !passes_type_filter {
        return false;
    }

    // Check name query filter (query is already lowercased by with_name_query)
    match &options.name_query {
        Some(query) if !query.is_empty() => entry.name.to_lowercase().contains(query.as_str()),
        _ => true,
    }
}

/// Sorts a vector of entries in place according to the specified options.
///
/// # Arguments
///
/// * `entries` - Mutable reference to the entries to sort.
/// * `options` - Sorting options.
pub fn sort_entries(entries: &mut [FsEntry], options: &ListOptions) {
    if matches!(options.sort_by, SortKey::Name) {
        // Pre-compute lowercase names to avoid O(2n log n) transient String allocations
        let lowercase: Vec<String> = entries.iter().map(|e| e.name.to_lowercase()).collect();
        let dirs_first = options.directories_first;
        let desc = matches!(options.sort_order, SortOrder::Desc);

        // Sort using indices to reference the cached lowercase names
        let mut indices: Vec<usize> = (0..entries.len()).collect();
        indices.sort_by(|&a, &b| {
            if dirs_first && entries[a].entry_type != entries[b].entry_type {
                return match (entries[a].entry_type, entries[b].entry_type) {
                    (FsEntryType::Directory, _) => Ordering::Less,
                    (_, FsEntryType::Directory) => Ordering::Greater,
                    _ => Ordering::Equal,
                };
            }
            let ord = lowercase[a].cmp(&lowercase[b]);
            if desc { ord.reverse() } else { ord }
        });

        // Apply the permutation in-place
        apply_permutation(entries, &mut indices);
    } else {
        entries.sort_by(|left, right| compare_entries(left, right, options));
    }
}

/// Reorders `data` according to the permutation in `indices` (in-place, O(n) swaps).
fn apply_permutation<T>(data: &mut [T], indices: &mut [usize]) {
    for i in 0..indices.len() {
        while indices[i] != i {
            let target = indices[i];
            data.swap(i, target);
            indices.swap(i, target);
        }
    }
}

/// Filters and sorts entries, returning a new vector.
///
/// # Arguments
///
/// * `entries` - The entries to process.
/// * `options` - Filter and sort options.
///
/// # Returns
///
/// A new vector containing only entries that match the filter, sorted according
/// to the options.
pub fn filter_and_sort(entries: Vec<FsEntry>, options: &ListOptions) -> Vec<FsEntry> {
    let mut filtered: Vec<FsEntry> = entries
        .into_iter()
        .filter(|entry| matches_filter(entry, options))
        .collect();
    sort_entries(&mut filtered, options);
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::FsMetadata;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_entry(name: &str, entry_type: FsEntryType, size: u64) -> FsEntry {
        FsEntry {
            path: PathBuf::from(name),
            name: name.to_string(),
            entry_type,
            metadata: FsMetadata {
                size,
                modified: Some(SystemTime::now()),
                accessed: None,
                created: None,
                readonly: false,
            },
        }
    }

    #[test]
    fn directories_first_sorting() {
        let file = make_entry("alpha.txt", FsEntryType::File, 100);
        let dir = make_entry("beta", FsEntryType::Directory, 0);

        let options = ListOptions {
            directories_first: true,
            ..Default::default()
        };

        assert_eq!(compare_entries(&dir, &file, &options), Ordering::Less);
        assert_eq!(compare_entries(&file, &dir, &options), Ordering::Greater);
    }

    #[test]
    fn name_sorting_case_insensitive() {
        let alpha = make_entry("Alpha.txt", FsEntryType::File, 100);
        let beta = make_entry("beta.txt", FsEntryType::File, 100);

        let options = ListOptions {
            directories_first: false,
            ..Default::default()
        };

        assert_eq!(compare_entries(&alpha, &beta, &options), Ordering::Less);
    }

    #[test]
    fn size_sorting() {
        let small = make_entry("small.txt", FsEntryType::File, 100);
        let large = make_entry("large.txt", FsEntryType::File, 1000);

        let options = ListOptions {
            directories_first: false,
            sort_by: SortKey::Size,
            ..Default::default()
        };

        assert_eq!(compare_entries(&small, &large, &options), Ordering::Less);
    }

    #[test]
    fn descending_order() {
        let alpha = make_entry("alpha.txt", FsEntryType::File, 100);
        let beta = make_entry("beta.txt", FsEntryType::File, 100);

        let options = ListOptions {
            directories_first: false,
            sort_order: SortOrder::Desc,
            ..Default::default()
        };

        assert_eq!(compare_entries(&alpha, &beta, &options), Ordering::Greater);
    }

    #[test]
    fn filter_directories_only() {
        let file = make_entry("file.txt", FsEntryType::File, 100);
        let dir = make_entry("folder", FsEntryType::Directory, 0);

        let options = ListOptions {
            filter: EntryFilter::OnlyDirectories,
            ..Default::default()
        };

        assert!(!matches_filter(&file, &options));
        assert!(matches_filter(&dir, &options));
    }

    #[test]
    fn filter_by_name_query() {
        let entry = make_entry("document.pdf", FsEntryType::File, 100);

        let matching = ListOptions {
            name_query: Some("doc".to_string()),
            ..Default::default()
        };
        let non_matching = ListOptions {
            name_query: Some("image".to_string()),
            ..Default::default()
        };

        assert!(matches_filter(&entry, &matching));
        assert!(!matches_filter(&entry, &non_matching));
    }
}
