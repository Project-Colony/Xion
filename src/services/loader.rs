use std::path::Path;
use std::time::Duration;

use crate::core::AppResult;
use crate::filesystem::{
    DirectoryCache, DirectoryKey, FileSystem, FsEntry, ListOptions, Page, PageRequest,
};

#[derive(Debug)]
pub struct DirectoryLoader {
    directory_cache: DirectoryCache,
    page_size: usize,
}

impl DirectoryLoader {
    pub fn new(cache_size: usize, ttl: Duration, page_size: usize) -> Self {
        Self {
            directory_cache: DirectoryCache::new(cache_size, ttl),
            page_size: page_size.max(1),
        }
    }

    pub fn load_page(
        &mut self,
        filesystem: &dyn FileSystem,
        path: &Path,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        let key = DirectoryKey {
            path: path.to_path_buf(),
            options,
        };

        if let Some(entries) = self.directory_cache.get(&key) {
            return Ok(Self::page_from(entries, page));
        }

        // Read and sort the directory ONCE, then cache the whole listing.
        //
        // The old code called `list_dir_paged` per page and only cached when
        // `items.len() == total` — a condition `page_size = 120` makes false
        // for any directory with more than 120 entries. Scrolling a folder of
        // 100 000 files therefore re-ran a full `read_dir` plus a full sort for
        // every page: 834 complete listings for one pass down the list.
        let entries = filesystem.list_dir(path, key.options.clone())?;
        let result = Self::page_from(&entries, page);
        self.directory_cache.insert(key, entries);

        Ok(result)
    }

    fn page_from(entries: &[FsEntry], page: PageRequest) -> Page<FsEntry> {
        Page {
            items: page.slice(entries).to_vec(),
            total: entries.len(),
            offset: page.offset.min(entries.len()),
            limit: page.limit,
        }
    }

    pub fn load_next_page(
        &mut self,
        filesystem: &dyn FileSystem,
        path: &Path,
        options: ListOptions,
        current_page: &Page<FsEntry>,
    ) -> AppResult<Page<FsEntry>> {
        let next_offset = current_page.next_offset();
        let page = PageRequest::new(next_offset, current_page.limit.max(self.page_size));
        self.load_page(filesystem, path, options, page)
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn clear(&mut self) {
        self.directory_cache.clear();
    }

    /// Invalidate every cached listing for a directory.
    ///
    /// The same directory is cached once per set of list options, so a change
    /// on disk has to drop all of its variants, not just the current one.
    pub fn invalidate(&mut self, path: &Path) {
        self.directory_cache.remove_path(path);
    }
}

// `MetadataCache` used to live here, populated on every listing and cleared on
// every invalidation — and never read once: `MetadataCache::get` had no caller
// anywhere in the crate. A 10 000-entry listing paid 20 000 allocations to
// clone a `PathBuf` and an `FsMetadata` that the `FsEntry` in the directory
// cache already held, then threw them away. Removed rather than wired up: the
// metadata is already one dereference away.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::{FsEntry, FsEntryType, FsMetadata, SortKey, SortOrder};
    use std::cell::Cell;

    /// A filesystem that counts how many times it was asked to list.
    struct CountingFs {
        calls: Cell<usize>,
        names: Vec<&'static str>,
    }

    impl CountingFs {
        fn new(names: Vec<&'static str>) -> Self {
            Self {
                calls: Cell::new(0),
                names,
            }
        }
    }

    impl FileSystem for CountingFs {
        fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>> {
            self.calls.set(self.calls.get() + 1);
            let mut names = self.names.clone();
            if options.sort_order == SortOrder::Desc {
                names.reverse();
            }
            Ok(names
                .into_iter()
                .map(|name| FsEntry {
                    path: path.join(name),
                    name: name.to_string(),
                    entry_type: FsEntryType::File,
                    metadata: FsMetadata::default(),
                })
                .collect())
        }

        fn metadata(&self, _path: &Path) -> AppResult<FsMetadata> {
            Ok(FsMetadata::default())
        }
    }

    fn options(order: SortOrder) -> ListOptions {
        ListOptions {
            sort_by: SortKey::Name,
            sort_order: order,
            ..ListOptions::default()
        }
    }

    /// Regression: the cache was only populated when a page happened to hold
    /// the entire directory, so every page re-read and re-sorted everything.
    #[test]
    fn a_second_page_is_served_from_cache() {
        let fs = CountingFs::new(vec!["a", "b", "c", "d", "e"]);
        let mut loader = DirectoryLoader::new(8, Duration::from_secs(60), 2);
        let path = Path::new("/tmp/xion-loader");

        let first = loader
            .load_page(&fs, path, options(SortOrder::Asc), PageRequest::new(0, 2))
            .unwrap();
        let second = loader
            .load_page(&fs, path, options(SortOrder::Asc), PageRequest::new(2, 2))
            .unwrap();

        assert_eq!(first.total, 5);
        assert_eq!(
            first
                .items
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            second
                .items
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            vec!["c", "d"]
        );
        assert_eq!(fs.calls.get(), 1, "le dossier ne doit être lu qu'une fois");
    }

    /// Regression: the cache key was the path alone, so changing the sort
    /// order kept serving the previous ordering for the whole TTL.
    #[test]
    fn changing_the_sort_order_bypasses_the_cache() {
        let fs = CountingFs::new(vec!["a", "b", "c"]);
        let mut loader = DirectoryLoader::new(8, Duration::from_secs(60), 10);
        let path = Path::new("/tmp/xion-loader");

        let ascending = loader
            .load_page(&fs, path, options(SortOrder::Asc), PageRequest::new(0, 10))
            .unwrap();
        let descending = loader
            .load_page(&fs, path, options(SortOrder::Desc), PageRequest::new(0, 10))
            .unwrap();

        assert_eq!(ascending.items[0].name, "a");
        assert_eq!(descending.items[0].name, "c");
        assert_eq!(fs.calls.get(), 2, "chaque tri doit relire le dossier");
    }

    #[test]
    fn invalidating_a_path_drops_every_sort_variant() {
        let fs = CountingFs::new(vec!["a", "b"]);
        let mut loader = DirectoryLoader::new(8, Duration::from_secs(60), 10);
        let path = Path::new("/tmp/xion-loader");

        loader
            .load_page(&fs, path, options(SortOrder::Asc), PageRequest::new(0, 10))
            .unwrap();
        loader
            .load_page(&fs, path, options(SortOrder::Desc), PageRequest::new(0, 10))
            .unwrap();
        assert_eq!(fs.calls.get(), 2);

        loader.invalidate(path);

        loader
            .load_page(&fs, path, options(SortOrder::Asc), PageRequest::new(0, 10))
            .unwrap();
        loader
            .load_page(&fs, path, options(SortOrder::Desc), PageRequest::new(0, 10))
            .unwrap();
        assert_eq!(fs.calls.get(), 4, "les deux variantes doivent être purgées");
    }
}
