use std::path::Path;

use crate::core::AppResult;
use crate::filesystem::{EntryFilter, FileSystem, FsEntry, ListOptions, SortKey};

#[derive(Debug, Default)]
pub struct SearchService;

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
}
