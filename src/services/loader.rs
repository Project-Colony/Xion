use std::path::PathBuf;
use std::time::Duration;

use crate::core::AppResult;
use crate::filesystem::{
    DirectoryCache, FileSystem, FsEntry, ListOptions, MetadataCache, Page, PageRequest,
};

#[derive(Debug)]
pub struct DirectoryLoader {
    directory_cache: DirectoryCache,
    metadata_cache: MetadataCache,
    page_size: usize,
}

impl DirectoryLoader {
    pub fn new(cache_size: usize, ttl: Duration, page_size: usize) -> Self {
        Self {
            directory_cache: DirectoryCache::new(cache_size, ttl),
            metadata_cache: MetadataCache::new(cache_size * 4, ttl),
            page_size: page_size.max(1),
        }
    }

    pub fn load_page(
        &mut self,
        filesystem: &dyn FileSystem,
        path: &PathBuf,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        if let Some(entries) = self.directory_cache.get(path) {
            let page_entries = page.slice(entries).iter().cloned().collect::<Vec<_>>();
            return Ok(Page {
                items: page_entries,
                total: entries.len(),
                offset: page.offset.min(entries.len()),
                limit: page.limit,
            });
        }

        let page_data = filesystem.list_dir_paged(path, options, page)?;
        self.cache_metadata(&page_data.items);

        if page_data.offset == 0 && page_data.items.len() == page_data.total {
            self.directory_cache
                .insert(path.clone(), page_data.items.clone());
        }

        Ok(page_data)
    }

    pub fn load_next_page(
        &mut self,
        filesystem: &dyn FileSystem,
        path: &PathBuf,
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
        self.metadata_cache.clear();
    }

    pub fn prefetch_metadata(
        &mut self,
        filesystem: &dyn FileSystem,
        paths: &[PathBuf],
    ) -> AppResult<()> {
        let metadata = filesystem.metadata_batch(paths)?;
        for (path, metadata) in paths.iter().cloned().zip(metadata) {
            self.metadata_cache.insert(path, metadata);
        }
        Ok(())
    }

    fn cache_metadata(&mut self, entries: &[FsEntry]) {
        for entry in entries {
            self.metadata_cache
                .insert(entry.path.clone(), entry.metadata.clone());
        }
    }
}
