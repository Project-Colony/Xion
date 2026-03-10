//! Integration tests for Xion file explorer.
//!
//! These tests verify the correct interaction between modules
//! and end-to-end functionality.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use xion::core::{AppConfig, ConfigManager};
use xion::filesystem::{
    EntryFilter, FileSystem, ListOptions, LocalFileSystem, PageRequest, SortKey, SortOrder,
};
use xion::services::{DirectoryLoader, HistoryService, SearchService};

/// Creates a temporary directory with a unique name for testing.
fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    path.push(format!("xion_test_{}_{}", prefix, stamp));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

/// Cleans up a temporary directory.
fn cleanup_temp_dir(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

mod filesystem_integration {
    use super::*;

    #[test]
    fn list_and_sort_by_name() {
        let root = create_temp_dir("list_sort");
        fs::create_dir_all(root.join("zebra")).unwrap();
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("middle.txt"), "content").unwrap();

        let fs = LocalFileSystem::new();
        let options = ListOptions {
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            ..Default::default()
        };

        let entries = fs.list_dir(&root, options).expect("list dir");
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        // Directories first, then sorted alphabetically
        assert_eq!(names, vec!["alpha", "zebra", "middle.txt"]);

        cleanup_temp_dir(&root);
    }

    #[test]
    fn list_with_pagination() {
        let root = create_temp_dir("pagination");
        for i in 0..10 {
            fs::write(root.join(format!("file_{:02}.txt", i)), "content").unwrap();
        }

        let fs = LocalFileSystem::new();
        let options = ListOptions::default();

        // First page
        let page1 = fs
            .list_dir_paged(&root, options.clone(), PageRequest::new(0, 3))
            .expect("page 1");
        assert_eq!(page1.total, 10);
        assert_eq!(page1.items.len(), 3);
        assert_eq!(page1.offset, 0);

        // Second page
        let page2 = fs
            .list_dir_paged(&root, options.clone(), PageRequest::new(3, 3))
            .expect("page 2");
        assert_eq!(page2.items.len(), 3);
        assert_eq!(page2.offset, 3);

        // Last page (partial)
        let page4 = fs
            .list_dir_paged(&root, options, PageRequest::new(9, 3))
            .expect("page 4");
        assert_eq!(page4.items.len(), 1);

        cleanup_temp_dir(&root);
    }

    #[test]
    fn filter_files_only() {
        let root = create_temp_dir("filter");
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("file.txt"), "content").unwrap();

        let fs = LocalFileSystem::new();
        let options = ListOptions {
            filter: EntryFilter::OnlyFiles,
            ..Default::default()
        };

        let entries = fs.list_dir(&root, options).expect("list files");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file.txt");

        cleanup_temp_dir(&root);
    }

    #[test]
    fn filter_directories_only() {
        let root = create_temp_dir("filter_dirs");
        fs::create_dir_all(root.join("folder1")).unwrap();
        fs::create_dir_all(root.join("folder2")).unwrap();
        fs::write(root.join("file.txt"), "content").unwrap();

        let fs = LocalFileSystem::new();
        let options = ListOptions {
            filter: EntryFilter::OnlyDirectories,
            ..Default::default()
        };

        let entries = fs.list_dir(&root, options).expect("list dirs");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.name.starts_with("folder")));

        cleanup_temp_dir(&root);
    }

    #[test]
    fn hidden_files_filtering() {
        let root = create_temp_dir("hidden");
        fs::write(root.join(".hidden"), "content").unwrap();
        fs::write(root.join("visible.txt"), "content").unwrap();

        let fs = LocalFileSystem::new();

        // Without hidden files
        let options = ListOptions {
            show_hidden: false,
            ..Default::default()
        };
        let entries = fs.list_dir(&root, options).expect("no hidden");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "visible.txt");

        // With hidden files
        let options = ListOptions {
            show_hidden: true,
            ..Default::default()
        };
        let entries = fs.list_dir(&root, options).expect("with hidden");
        assert_eq!(entries.len(), 2);

        cleanup_temp_dir(&root);
    }
}

mod services_integration {
    use super::*;

    #[test]
    fn history_navigation() {
        let mut history = HistoryService::default();

        let path1 = PathBuf::from("/home/user");
        let path2 = PathBuf::from("/home/user/documents");
        let path3 = PathBuf::from("/home/user/downloads");

        history.record(path1.clone());
        history.record(path2.clone());
        history.record(path3.clone());

        // Go back
        assert!(history.can_back());
        let back1 = history.back();
        assert_eq!(back1, Some(path2.clone()));

        let back2 = history.back();
        assert_eq!(back2, Some(path1.clone()));

        // Go forward
        assert!(history.can_forward());
        let fwd1 = history.forward();
        assert_eq!(fwd1, Some(path2.clone()));
    }

    #[test]
    fn directory_loader_caching() {
        let root = create_temp_dir("loader_cache");
        for i in 0..5 {
            fs::write(root.join(format!("file_{}.txt", i)), "content").unwrap();
        }

        let mut loader = DirectoryLoader::new(
            100,                       // cache entries
            Duration::from_secs(300),  // TTL
            10,                        // page size
        );

        let fs = LocalFileSystem::new();
        let options = ListOptions::default();

        // First load - should hit filesystem
        let page1 = loader
            .load_page(&fs, &root, options.clone(), PageRequest::new(0, 10))
            .expect("first load");
        assert_eq!(page1.total, 5);

        // Second load - should use cache
        let page2 = loader
            .load_page(&fs, &root, options, PageRequest::new(0, 10))
            .expect("cached load");
        assert_eq!(page2.total, 5);

        cleanup_temp_dir(&root);
    }

    #[test]
    fn search_service_text_filter() {
        let root = create_temp_dir("search");
        fs::write(root.join("document.pdf"), "content").unwrap();
        fs::write(root.join("photo.jpg"), "content").unwrap();
        fs::write(root.join("document_backup.pdf"), "content").unwrap();

        let fs = LocalFileSystem::new();
        let search = SearchService::default();

        let results = search
            .search_in_dir(&fs, &root, "document")
            .expect("search");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.name.contains("document")));

        cleanup_temp_dir(&root);
    }
}

mod config_integration {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = AppConfig::default();

        // Check sensible defaults
        assert!(!config.list.show_hidden);
        assert!(config.list.directories_first);
        assert!(config.cache.thumbnail_entries >= 32);
        assert!(config.paging.page_size >= 24);
    }

    #[test]
    fn config_manager_loads_defaults_on_missing_file() {
        let manager = ConfigManager::new();
        let load = manager.load();

        // Should load default config without errors
        assert!(load.config.paging.page_size > 0);
    }
}

mod sorting_integration {
    use super::*;
    use xion::filesystem::{compare_entries, FsEntry, FsEntryType, FsMetadata};

    fn make_entry(name: &str, is_dir: bool, size: u64) -> FsEntry {
        FsEntry {
            path: PathBuf::from(name),
            name: name.to_string(),
            entry_type: if is_dir {
                FsEntryType::Directory
            } else {
                FsEntryType::File
            },
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
    fn unified_sorting_directories_first() {
        let file = make_entry("aaa.txt", false, 100);
        let dir = make_entry("zzz", true, 0);

        let options = ListOptions {
            directories_first: true,
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            ..Default::default()
        };

        let ordering = compare_entries(&dir, &file, &options);
        assert_eq!(ordering, std::cmp::Ordering::Less);
    }

    #[test]
    fn unified_sorting_by_size() {
        let small = make_entry("small.txt", false, 100);
        let large = make_entry("large.txt", false, 10000);

        let options = ListOptions {
            directories_first: false,
            sort_by: SortKey::Size,
            sort_order: SortOrder::Desc,
            ..Default::default()
        };

        let ordering = compare_entries(&large, &small, &options);
        assert_eq!(ordering, std::cmp::Ordering::Less); // Desc: larger comes first
    }
}
