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

mod file_operations {
    use super::*;
    use xion::filesystem::LocalFileOperations;

    #[test]
    fn copy_single_file() {
        let src_dir = create_temp_dir("copy_src");
        let dst_dir = create_temp_dir("copy_dst");
        let file = src_dir.join("hello.txt");
        fs::write(&file, b"hello").unwrap();

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(&[file.clone()], &dst_dir, None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(report.failed.is_empty());
        assert!(dst_dir.join("hello.txt").exists());
        // Original still exists
        assert!(file.exists());

        cleanup_temp_dir(&src_dir);
        cleanup_temp_dir(&dst_dir);
    }

    #[test]
    fn move_single_file() {
        let src_dir = create_temp_dir("move_src");
        let dst_dir = create_temp_dir("move_dst");
        let file = src_dir.join("doc.txt");
        fs::write(&file, b"content").unwrap();

        let ops = LocalFileOperations::new();
        let report = ops.move_items(&[file.clone()], &dst_dir, None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(report.failed.is_empty());
        assert!(dst_dir.join("doc.txt").exists());
        // Original is gone
        assert!(!file.exists());

        cleanup_temp_dir(&src_dir);
        cleanup_temp_dir(&dst_dir);
    }

    #[test]
    fn delete_files() {
        let dir = create_temp_dir("delete_test");
        let file1 = dir.join("a.txt");
        let file2 = dir.join("b.txt");
        fs::write(&file1, b"a").unwrap();
        fs::write(&file2, b"b").unwrap();

        let ops = LocalFileOperations::new();
        let report = ops.delete_items(&[file1.clone(), file2.clone()]);

        assert_eq!(report.succeeded.len(), 2);
        assert!(report.failed.is_empty());
        assert!(!file1.exists());
        assert!(!file2.exists());

        cleanup_temp_dir(&dir);
    }

    #[test]
    fn copy_reports_progress_via_counter() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

        let src_dir = create_temp_dir("progress_src");
        let dst_dir = create_temp_dir("progress_dst");
        for i in 0..5 {
            fs::write(src_dir.join(format!("file_{i}.txt")), b"x").unwrap();
        }

        let items: Vec<PathBuf> = (0..5)
            .map(|i| src_dir.join(format!("file_{i}.txt")))
            .collect();
        let counter = Arc::new(AtomicUsize::new(0));
        let ops = LocalFileOperations::new();
        ops.copy_items(&items, &dst_dir, Some(counter.clone()));

        assert_eq!(counter.load(Ordering::Relaxed), 5);

        cleanup_temp_dir(&src_dir);
        cleanup_temp_dir(&dst_dir);
    }

    #[test]
    fn copy_fails_gracefully_on_nonexistent_source() {
        let dst_dir = create_temp_dir("fail_dst");
        let nonexistent = PathBuf::from("/nonexistent_xion_test_path/file.txt");

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(&[nonexistent], &dst_dir, None);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed.len(), 1);

        cleanup_temp_dir(&dst_dir);
    }

    #[test]
    fn copy_directory_recursive() {
        let src_dir = create_temp_dir("copy_dir_src");
        let dst_dir = create_temp_dir("copy_dir_dst");
        let sub = src_dir.join("subdir");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("nested.txt"), b"nested").unwrap();

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(&[src_dir.join("subdir")], &dst_dir, None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(dst_dir.join("subdir").join("nested.txt").exists());

        cleanup_temp_dir(&src_dir);
        cleanup_temp_dir(&dst_dir);
    }
}

mod config_save_load {
    use super::*;
    use xion::core::{AppConfig, SortKeyConfig, SortOrderConfig};

    #[test]
    fn dark_mode_roundtrip() {
        let tmp = create_temp_dir("config_save");
        // We can't easily test ConfigManager's path without exposing it,
        // but we can test that default config parses dark_mode correctly.
        let config = AppConfig::default();
        assert!(!config.dark_mode, "default should be light mode");
        cleanup_temp_dir(&tmp);
    }

    #[test]
    fn default_sort_config() {
        let config = AppConfig::default();
        assert_eq!(config.list.sort_key, SortKeyConfig::Name);
        assert_eq!(config.list.sort_order, SortOrderConfig::Asc);
        assert!(config.list.directories_first);
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

/// Tests for `XionApp::update` state transitions.
///
/// These tests exercise the message-handling logic without an Iced runtime.
/// `new_for_test()` creates a minimal app (no window, no GPU, no real file watcher).
/// `update_for_test()` calls `update()` and returns the Task; tests ignore Tasks
/// and inspect only the resulting state changes.
mod ui_update {
    use std::path::PathBuf;
    use xion::ui::app::XionApp;
    use xion::ui::UiMessage;

    // ── Dark mode ──────────────────────────────────────────────────────────────

    #[test]
    fn toggle_dark_mode_cycles_themes() {
        use xion::core::ThemeConfig;
        let mut app = XionApp::new_for_test();
        // Default is Light (dark_mode = false)
        assert_eq!(app.state_for_test().config.theme, ThemeConfig::Light);
        assert!(!app.state_for_test().config.dark_mode);
        // Light → Dark
        let _ = app.update_for_test(UiMessage::ToggleDarkMode);
        assert_eq!(app.state_for_test().config.theme, ThemeConfig::Dark);
        assert!(app.state_for_test().config.dark_mode);
        // 4 more toggles complete the cycle back to Light
        let _ = app.update_for_test(UiMessage::ToggleDarkMode); // Nord
        let _ = app.update_for_test(UiMessage::ToggleDarkMode); // Solarized
        let _ = app.update_for_test(UiMessage::ToggleDarkMode); // HighContrast
        let _ = app.update_for_test(UiMessage::ToggleDarkMode); // Light
        assert_eq!(app.state_for_test().config.theme, ThemeConfig::Light);
        assert!(!app.state_for_test().config.dark_mode);
    }

    // ── Gitignore ──────────────────────────────────────────────────────────────

    #[test]
    fn toggle_gitignore_flips_flag() {
        let mut app = XionApp::new_for_test();
        let initial = app.state_for_test().config.respect_gitignore;
        app.update_for_test(UiMessage::ToggleGitignore);
        assert_eq!(app.state_for_test().config.respect_gitignore, !initial);
        app.update_for_test(UiMessage::ToggleGitignore);
        assert_eq!(app.state_for_test().config.respect_gitignore, initial);
    }

    // ── Dual pane ─────────────────────────────────────────────────────────────

    #[test]
    fn toggle_dual_pane_enables_and_disables() {
        let mut app = XionApp::new_for_test();
        assert!(!app.dual_pane_for_test());
        app.update_for_test(UiMessage::ToggleDualPane);
        assert!(app.dual_pane_for_test());
        app.update_for_test(UiMessage::ToggleDualPane);
        assert!(!app.dual_pane_for_test());
    }

    // ── Quick filter ───────────────────────────────────────────────────────────

    #[test]
    fn quick_filter_sets_and_clears() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::QuickFilterChanged("rust".to_string()));
        assert_eq!(app.quick_filter_for_test(), "rust");
        assert!(app.quick_filter_active_for_test());
        app.update_for_test(UiMessage::QuickFilterClear);
        assert!(app.quick_filter_for_test().is_empty());
        assert!(!app.quick_filter_active_for_test());
    }

    // ── Tabs ──────────────────────────────────────────────────────────────────

    #[test]
    fn add_tab_increments_count() {
        let mut app = XionApp::new_for_test();
        assert_eq!(app.tab_count_for_test(), 1);
        app.update_for_test(UiMessage::AddTab);
        assert_eq!(app.tab_count_for_test(), 2);
    }

    #[test]
    fn close_tab_decrements_count() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::AddTab);
        assert_eq!(app.tab_count_for_test(), 2);
        app.update_for_test(UiMessage::CloseTab(1));
        assert_eq!(app.tab_count_for_test(), 1);
    }

    #[test]
    fn switch_tab_changes_active() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::AddTab);
        app.update_for_test(UiMessage::SwitchTab(1));
        assert_eq!(app.active_tab_for_test(), 1);
        app.update_for_test(UiMessage::SwitchTab(0));
        assert_eq!(app.active_tab_for_test(), 0);
    }

    // ── Context menu ─────────────────────────────────────────────────────────

    #[test]
    fn toggle_context_menu_opens_and_closes() {
        let mut app = XionApp::new_for_test();
        assert!(!app.context_menu_open_for_test());
        app.update_for_test(UiMessage::ToggleContextMenu(true));
        assert!(app.context_menu_open_for_test());
        app.update_for_test(UiMessage::ToggleContextMenu(false));
        assert!(!app.context_menu_open_for_test());
    }

    // ── Rename dialog ─────────────────────────────────────────────────────────

    #[test]
    fn rename_cancel_clears_dialog() {
        let mut app = XionApp::new_for_test();
        // Simulate opening rename for a path that doesn't need to exist for cancel
        app.update_for_test(UiMessage::RenameCancel);
        assert!(!app.rename_dialog_for_test());
    }

    // ── Properties dialog ─────────────────────────────────────────────────────

    #[test]
    fn close_properties_clears_dialog() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::CloseProperties);
        assert!(!app.properties_dialog_for_test());
    }

    // ── Hex viewer ────────────────────────────────────────────────────────────

    #[test]
    fn close_hex_view_clears_state() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::CloseHexView);
        assert!(!app.hex_view_for_test());
    }

    // ── Diff viewer ───────────────────────────────────────────────────────────

    #[test]
    fn close_diff_clears_state() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::CloseDiff);
        assert!(!app.diff_view_for_test());
    }

    // ── Grep ──────────────────────────────────────────────────────────────────

    #[test]
    fn close_grep_clears_state() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::CloseGrep);
        assert!(!app.grep_state_for_test());
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    #[test]
    fn navigate_to_recent_sets_recent_route() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::NavigateToRecent);
        assert!(app.state_for_test().route.is_recent());
    }

    #[test]
    fn navigate_to_local_path_updates_route() {
        let mut app = XionApp::new_for_test();
        let target = std::env::temp_dir();
        app.update_for_test(UiMessage::NavigateTo(target.clone()));
        assert_eq!(app.state_for_test().route.local_path(), Some(&target));
    }

    // ── Recents ───────────────────────────────────────────────────────────────

    #[test]
    fn clear_recents_empties_service() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::ClearRecents);
        // After clearing, navigating to Recent should show 0 recents
        assert!(app.recents_is_empty_for_test());
    }

    // ── Bulk rename ───────────────────────────────────────────────────────────

    #[test]
    fn bulk_rename_cancel_clears_state() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::BulkRenameCancel);
        assert!(!app.bulk_rename_for_test());
    }

    // ── Address bar ───────────────────────────────────────────────────────────

    #[test]
    fn address_input_changed_updates_field() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::AddressInputChanged("C:\\Windows".to_string()));
        assert_eq!(app.address_input_for_test(), "C:\\Windows");
    }

    #[test]
    fn address_edit_start_and_cancel() {
        let mut app = XionApp::new_for_test();
        app.update_for_test(UiMessage::AddressEditStart);
        assert!(app.address_editing_for_test());
        app.update_for_test(UiMessage::AddressEditCancel);
        assert!(!app.address_editing_for_test());
    }

    // ── Network discovery ─────────────────────────────────────────────────────

    #[test]
    fn network_scan_completed_updates_discovery_cache() {
        use xion::services::NetworkResource;
        let mut app = XionApp::new_for_test();
        let resources = vec![
            NetworkResource { name: "NAS".to_string(), path: "smb://NAS".to_string(), status: xion::services::network::NetworkStatus::Online },
        ];
        app.update_for_test(UiMessage::NetworkScanCompleted(resources));
        // Cache is no longer stale after update
        assert!(!app.network_needs_scan_for_test());
    }
}
