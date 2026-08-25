//! Unified sorting and comparison utilities for filesystem entries.
//!
//! This module provides consistent sorting behavior across local and network
//! file listings, ensuring predictable ordering regardless of entry source.

use std::cmp::Ordering;

use crate::filesystem::{EntryFilter, FsEntry, FsEntryType, ListOptions, SortKey, SortOrder};

/// Compare two strings using natural ordering: numeric segments
/// are compared by value so "file2" sorts before "file10".
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&ac), Some(&bc)) if ac.is_ascii_digit() && bc.is_ascii_digit() => {
                let mut a_num: u64 = 0;
                while let Some(&c) = a_chars.peek() {
                    if c.is_ascii_digit() {
                        a_num = a_num
                            .saturating_mul(10)
                            .saturating_add(c.to_digit(10).unwrap_or(0) as u64);
                        a_chars.next();
                    } else {
                        break;
                    }
                }
                let mut b_num: u64 = 0;
                while let Some(&c) = b_chars.peek() {
                    if c.is_ascii_digit() {
                        b_num = b_num
                            .saturating_mul(10)
                            .saturating_add(c.to_digit(10).unwrap_or(0) as u64);
                        b_chars.next();
                    } else {
                        break;
                    }
                }
                match a_num.cmp(&b_num) {
                    std::cmp::Ordering::Equal => continue,
                    ord => return ord,
                }
            }
            _ => {
                let ac = match a_chars.next() {
                    Some(c) => c,
                    None => return std::cmp::Ordering::Less,
                };
                let bc = match b_chars.next() {
                    Some(c) => c,
                    None => return std::cmp::Ordering::Greater,
                };
                // ASCII fast path: `char::to_lowercase` builds a three-slot
                // state machine per character and compares it through an
                // iterator, which is pure overhead for the file names that make
                // up almost every listing. `to_ascii_lowercase` is used rather
                // than the usual `| 0x20` trick because the mask also rewrites
                // punctuation ('_' would sort after 'a') and would silently
                // change the ordering of non-alphabetic names.
                let ordering = if ac.is_ascii() && bc.is_ascii() {
                    ac.to_ascii_lowercase().cmp(&bc.to_ascii_lowercase())
                } else {
                    let mut a_lower = ac.to_lowercase();
                    let mut b_lower = bc.to_lowercase();
                    a_lower.by_ref().cmp(b_lower.by_ref())
                };
                match ordering {
                    std::cmp::Ordering::Equal => continue,
                    ord => return ord,
                }
            }
        }
    }
}

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
/// What ordering needs to know about an entry.
///
/// Two types get sorted by exactly the same rules: `FsEntry`, and the lighter
/// stub the local listing builds before it has read any metadata. The
/// comparator existed twice, once for each, differing only in how it reached
/// the size and the date — which is to say, in nothing that ordering is about.
pub trait Sortable {
    fn entry_type(&self) -> FsEntryType;
    fn name(&self) -> &str;
    /// `None` when the metadata has not been read yet.
    fn modified(&self) -> Option<std::time::SystemTime>;
    /// `None` when the metadata has not been read yet.
    fn size(&self) -> Option<u64>;
}

impl Sortable for FsEntry {
    fn entry_type(&self) -> FsEntryType {
        self.entry_type
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn modified(&self) -> Option<std::time::SystemTime> {
        self.metadata.modified
    }
    fn size(&self) -> Option<u64> {
        Some(self.metadata.size)
    }
}

/// Orders two entries by the rules in `options`.
pub fn compare<T: Sortable + ?Sized>(left: &T, right: &T, options: &ListOptions) -> Ordering {
    // Les dossiers d'abord — mais seulement quand exactement l'un des deux en
    // est un.
    //
    // La condition portait sur `entry_type() != entry_type()`, donc elle se
    // déclenchait aussi entre un fichier et un lien symbolique, où le `match`
    // retombait sur `Equal` sans jamais regarder la clé de tri. La transitivité
    // y passait : `a` == lien `b`, lien `b` == `c`, mais `a` < `c`. Rust le
    // détecte et fait paniquer le tri — rencontré à l'ouverture d'un dossier
    // contenant un lien à côté de fichiers ordinaires.
    if options.directories_first {
        let left_is_dir = left.entry_type() == FsEntryType::Directory;
        let right_is_dir = right.entry_type() == FsEntryType::Directory;
        if left_is_dir != right_is_dir {
            return if left_is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
    }

    // Sort by the specified key
    let ordering = match options.sort_by {
        SortKey::Name => natural_cmp(left.name(), right.name()),
        SortKey::Modified => left.modified().cmp(&right.modified()),
        SortKey::Size => left.size().cmp(&right.size()),
    };

    // Apply sort order
    match options.sort_order {
        SortOrder::Asc => ordering,
        SortOrder::Desc => ordering.reverse(),
    }
}

pub fn compare_entries(left: &FsEntry, right: &FsEntry, options: &ListOptions) -> Ordering {
    compare(left, right, options)
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
        let dirs_first = options.directories_first;
        let desc = matches!(options.sort_order, SortOrder::Desc);

        // Sort using indices with natural ordering (numeric-aware)
        let mut indices: Vec<usize> = (0..entries.len()).collect();
        indices.sort_by(|&a, &b| {
            if dirs_first && entries[a].entry_type != entries[b].entry_type {
                return match (entries[a].entry_type, entries[b].entry_type) {
                    (FsEntryType::Directory, _) => Ordering::Less,
                    (_, FsEntryType::Directory) => Ordering::Greater,
                    _ => Ordering::Equal,
                };
            }
            let ord = natural_cmp(&entries[a].name, &entries[b].name);
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
            if target >= data.len() {
                tracing::warn!(
                    "apply_permutation: index corrompu {target} >= {}",
                    data.len()
                );
                break;
            }
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
    /// Reproduit la panique « comparison function does not correctly implement
    /// a total order », rencontrée à l'ouverture d'un dossier contenant un lien
    /// symbolique à côté de fichiers ordinaires.
    ///
    /// Avec « dossiers d'abord », le court-circuit se déclenchait dès que les
    /// deux types différaient — donc aussi entre un fichier et un lien, où il
    /// renvoyait `Equal` sans jamais regarder le nom. La transitivité y passe :
    /// `a` == lien `b`, lien `b` == `c`, mais `a` < `c`.
    #[test]
    fn mixing_files_and_symlinks_keeps_a_total_order() {
        use crate::filesystem::FsMetadata;

        let make = |name: &str, entry_type: FsEntryType| FsEntry {
            path: std::path::PathBuf::from(name),
            name: name.to_string(),
            entry_type,
            metadata: FsMetadata::default(),
        };
        let options = ListOptions {
            sort_by: SortKey::Name,
            directories_first: true,
            ..ListOptions::default()
        };

        let a = make("a.txt", FsEntryType::File);
        let b = make("b.txt", FsEntryType::Symlink);
        let c = make("c.txt", FsEntryType::File);

        // Deux entrées qui ne sont pas des dossiers se départagent par leur clé
        // de tri, quel que soit leur type.
        assert_eq!(super::compare(&a, &b, &options), Ordering::Less);
        assert_eq!(super::compare(&b, &c, &options), Ordering::Less);
        assert_eq!(super::compare(&a, &c, &options), Ordering::Less);

        // Et un vrai tri sur les quatre types ne doit pas paniquer.
        let mut entries: Vec<FsEntry> = Vec::new();
        for (index, entry_type) in [
            FsEntryType::File,
            FsEntryType::Symlink,
            FsEntryType::Other,
            FsEntryType::Directory,
        ]
        .into_iter()
        .cycle()
        .take(64)
        .enumerate()
        {
            entries.push(make(&format!("entree-{index:02}"), entry_type));
        }
        entries.sort_by(|left, right| super::compare(left, right, &options));

        // Les dossiers restent groupés en tête.
        let first_non_dir = entries
            .iter()
            .position(|e| e.entry_type != FsEntryType::Directory)
            .unwrap_or(entries.len());
        assert!(
            entries[first_non_dir..]
                .iter()
                .all(|e| e.entry_type != FsEntryType::Directory),
            "un dossier s'est retrouvé après un non-dossier"
        );
    }

    /// Le comparateur existait en double, un par type. Rien ne garantissait
    /// que les deux copies restent d'accord ; maintenant il n'y en a qu'un, et
    /// ce test dit pourquoi c'était le bon choix.
    #[test]
    fn both_shapes_of_entry_order_the_same_way() {
        use crate::filesystem::{FsMetadata, SortKey};
        use std::time::{Duration, SystemTime};

        /// Un `Sortable` minimal, dans l'esprit du stub que le listage local
        /// construit avant d'avoir lu la moindre métadonnée.
        struct Stub {
            name: String,
            entry_type: FsEntryType,
            metadata: Option<FsMetadata>,
        }
        impl super::Sortable for Stub {
            fn entry_type(&self) -> FsEntryType {
                self.entry_type
            }
            fn name(&self) -> &str {
                &self.name
            }
            fn modified(&self) -> Option<SystemTime> {
                self.metadata.as_ref().and_then(|m| m.modified)
            }
            fn size(&self) -> Option<u64> {
                self.metadata.as_ref().map(|m| m.size)
            }
        }

        let base = SystemTime::UNIX_EPOCH;
        let make = |name: &str, dir: bool, size: u64, secs: u64| {
            let metadata = FsMetadata {
                size,
                modified: Some(base + Duration::from_secs(secs)),
                ..FsMetadata::default()
            };
            let entry_type = if dir {
                FsEntryType::Directory
            } else {
                FsEntryType::File
            };
            (
                FsEntry {
                    path: std::path::PathBuf::from(name),
                    name: name.to_string(),
                    entry_type,
                    metadata: metadata.clone(),
                },
                Stub {
                    name: name.to_string(),
                    entry_type,
                    metadata: Some(metadata),
                },
            )
        };

        let cases = [
            make("fichier2.txt", false, 300, 20),
            make("fichier10.txt", false, 100, 30),
            make("dossier", true, 200, 10),
            make("Archive.zip", false, 200, 10),
        ];

        for key in [SortKey::Name, SortKey::Size, SortKey::Modified] {
            for order in [SortOrder::Asc, SortOrder::Desc] {
                for directories_first in [true, false] {
                    let options = ListOptions {
                        sort_by: key,
                        sort_order: order,
                        directories_first,
                        ..ListOptions::default()
                    };
                    for (left_entry, left_stub) in &cases {
                        for (right_entry, right_stub) in &cases {
                            assert_eq!(
                                super::compare(left_entry, right_entry, &options),
                                super::compare(left_stub, right_stub, &options),
                                "{} vs {} ({key:?}, {order:?}, dossiers d'abord={directories_first})",
                                left_entry.name,
                                right_entry.name,
                            );
                        }
                    }
                }
            }
        }
    }

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
    fn natural_cmp_ascii_fast_path_matches_char_to_lowercase() {
        for left in 33u8..127 {
            for right in 33u8..127 {
                let (left, right) = (left as char, right as char);
                if left.is_ascii_digit() && right.is_ascii_digit() {
                    continue; // numeric branch, compared by value not by char
                }
                assert_eq!(
                    natural_cmp(&left.to_string(), &right.to_string()),
                    left.to_lowercase().cmp(right.to_lowercase()),
                    "désaccord sur {left:?} vs {right:?}"
                );
            }
        }
    }

    #[test]
    fn natural_cmp_keeps_punctuation_before_letters() {
        // `| 0x20` would map '_' (0x5F) to 0x7F and sort it after every letter.
        assert_eq!(natural_cmp("_alpha", "alpha"), Ordering::Less);
        assert_eq!(natural_cmp("[a]", "aa"), Ordering::Less);
    }

    #[test]
    fn natural_cmp_still_folds_non_ascii_case() {
        assert_eq!(natural_cmp("École", "école"), Ordering::Equal);
        assert_eq!(natural_cmp("Ärger", "ärger"), Ordering::Equal);
    }

    #[test]
    fn natural_cmp_orders_numbers_by_value() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("file10", "file2"), Ordering::Greater);
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
