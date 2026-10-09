//! Integration tests for Xion file explorer.
//!
//! These tests verify the correct interaction between modules
//! and end-to-end functionality.
//!
//! Every test that touches the disk works inside a `tempfile::TempDir`, which
//! removes itself on `Drop`. The previous helpers built a path by hand and
//! relied on a `cleanup_temp_dir(...)` call placed as the last statement of the
//! test body, so any assertion that panicked before it left the whole tree
//! behind in the system temp directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;

use xion::core::{AppConfig, ConfigManager};
use xion::filesystem::{
    EntryFilter, FileSystem, FsEntry, ListOptions, LocalFileSystem, PageRequest, SortKey, SortOrder,
};
use xion::services::{DirectoryLoader, HistoryService, SearchService};

/// Creates a self-cleaning temporary directory for one test.
///
/// Keep the returned handle alive for the whole test: dropping it deletes the
/// directory.
fn temp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("xion-test-")
        .tempdir()
        .expect("créer un répertoire temporaire")
}

/// Writes a small file inside `dir` and returns its path.
fn write_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"contenu").expect("écrire un fichier de test");
    path
}

/// Entry names in listing order, for order-sensitive assertions.
fn names(entries: &[FsEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name.as_str()).collect()
}

mod filesystem_integration {
    use super::*;

    #[test]
    fn list_and_sort_by_name() {
        let temp = temp_dir();
        let root = temp.path();
        fs::create_dir_all(root.join("zebra")).expect("créer zebra");
        fs::create_dir_all(root.join("alpha")).expect("créer alpha");
        write_file(root, "middle.txt");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            ..Default::default()
        };

        let entries = filesystem.list_dir(root, options).expect("list dir");

        // Directories first, then sorted alphabetically
        assert_eq!(names(&entries), vec!["alpha", "zebra", "middle.txt"]);
    }

    #[test]
    fn list_with_pagination() {
        let temp = temp_dir();
        let root = temp.path();
        for index in 0..10 {
            write_file(root, &format!("file_{index:02}.txt"));
        }

        let filesystem = LocalFileSystem::new();
        let options = ListOptions::default();

        // First page
        let page1 = filesystem
            .list_dir_paged(root, options.clone(), PageRequest::new(0, 3))
            .expect("page 1");
        assert_eq!(page1.total, 10);
        assert_eq!(page1.items.len(), 3);
        assert_eq!(page1.offset, 0);

        // Second page
        let page2 = filesystem
            .list_dir_paged(root, options.clone(), PageRequest::new(3, 3))
            .expect("page 2");
        assert_eq!(page2.items.len(), 3);
        assert_eq!(page2.offset, 3);

        // Last page (partial)
        let page4 = filesystem
            .list_dir_paged(root, options, PageRequest::new(9, 3))
            .expect("page 4");
        assert_eq!(page4.items.len(), 1);
    }

    #[test]
    fn filter_files_only() {
        let temp = temp_dir();
        let root = temp.path();
        fs::create_dir_all(root.join("folder")).expect("créer folder");
        write_file(root, "file.txt");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            filter: EntryFilter::OnlyFiles,
            ..Default::default()
        };

        let entries = filesystem.list_dir(root, options).expect("list files");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file.txt");
    }

    #[test]
    fn filter_directories_only() {
        let temp = temp_dir();
        let root = temp.path();
        fs::create_dir_all(root.join("folder1")).expect("créer folder1");
        fs::create_dir_all(root.join("folder2")).expect("créer folder2");
        write_file(root, "file.txt");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            filter: EntryFilter::OnlyDirectories,
            ..Default::default()
        };

        let entries = filesystem.list_dir(root, options).expect("list dirs");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry.name.starts_with("folder")));
    }

    #[test]
    fn show_hidden_lists_every_entry() {
        let temp = temp_dir();
        let root = temp.path();
        write_file(root, ".hidden");
        write_file(root, "visible.txt");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            show_hidden: true,
            ..Default::default()
        };

        let entries = filesystem.list_dir(root, options).expect("with hidden");
        assert_eq!(entries.len(), 2);
    }

    /// Split out of the former `hidden_files_filtering`, which asserted the
    /// dot convention unconditionally and therefore failed on Windows: there
    /// visibility comes from the hidden file attribute, which cannot be set
    /// from portable `std` code, so `.hidden` is a perfectly visible name.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn dot_files_are_hidden_by_default_on_unix() {
        let temp = temp_dir();
        let root = temp.path();
        write_file(root, ".hidden");
        write_file(root, "visible.txt");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            show_hidden: false,
            ..Default::default()
        };

        let entries = filesystem.list_dir(root, options).expect("no hidden");
        assert_eq!(names(&entries), vec!["visible.txt"]);
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
        let temp = temp_dir();
        let root = temp.path();
        for index in 0..5 {
            write_file(root, &format!("file_{index}.txt"));
        }

        let mut loader = DirectoryLoader::new(
            100,                      // cache entries
            Duration::from_secs(300), // TTL
            10,                       // page size
        );

        let filesystem = LocalFileSystem::new();
        let options = ListOptions::default();

        // First load - should hit filesystem
        let page1 = loader
            .load_page(&filesystem, root, options.clone(), PageRequest::new(0, 10))
            .expect("first load");
        assert_eq!(page1.total, 5);

        // Second load - should use cache
        let page2 = loader
            .load_page(&filesystem, root, options, PageRequest::new(0, 10))
            .expect("cached load");
        assert_eq!(page2.total, 5);
    }

    /// Regression: the listing cache was keyed on the directory path alone, so
    /// the same folder listed with a new sort hit the cached entry and served
    /// the previous ordering for the rest of the TTL. The key now carries the
    /// list options, so a sort change is visible on the very next load.
    #[test]
    fn changing_the_sort_order_is_not_masked_by_the_cache() {
        let temp = temp_dir();
        let root = temp.path();
        for name in ["a.txt", "b.txt", "c.txt"] {
            write_file(root, name);
        }

        let mut loader = DirectoryLoader::new(100, Duration::from_secs(300), 10);
        let filesystem = LocalFileSystem::new();
        let ascending = ListOptions {
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            ..Default::default()
        };
        let descending = ListOptions {
            sort_order: SortOrder::Desc,
            ..ascending.clone()
        };

        let first = loader
            .load_page(
                &filesystem,
                root,
                ascending.clone(),
                PageRequest::new(0, 10),
            )
            .expect("tri croissant");
        assert_eq!(names(&first.items), vec!["a.txt", "b.txt", "c.txt"]);

        let flipped = loader
            .load_page(&filesystem, root, descending, PageRequest::new(0, 10))
            .expect("tri décroissant");
        assert_eq!(names(&flipped.items), vec!["c.txt", "b.txt", "a.txt"]);

        // The first ordering is still cached under its own key, and still right.
        let again = loader
            .load_page(&filesystem, root, ascending, PageRequest::new(0, 10))
            .expect("retour au tri croissant");
        assert_eq!(names(&again.items), vec!["a.txt", "b.txt", "c.txt"]);
    }

    #[test]
    fn search_service_text_filter() {
        let temp = temp_dir();
        let root = temp.path();
        write_file(root, "document.pdf");
        write_file(root, "photo.jpg");
        write_file(root, "document_backup.pdf");

        let filesystem = LocalFileSystem::new();
        let search = SearchService;

        let results = search
            .search_in_dir(&filesystem, root, "document")
            .expect("search");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|entry| entry.name.contains("document")));
    }
}

mod config_integration {
    use super::*;
    use xion::core::{
        ConfigSource, ListConfig, PagingConfig, SortKeyConfig, SortOrderConfig, ThemeChoice,
        ViewConfig,
    };

    #[test]
    fn default_config_values() {
        let config = AppConfig::default();

        // Check sensible defaults
        assert!(!config.list.show_hidden);
        assert!(config.list.directories_first);
        assert!(config.cache.thumbnail_entries >= 32);
        assert!(config.paging.page_size >= 24);
    }

    /// The loader clamps every numeric field on read (`validated_*` in
    /// `src/core/config/manager.rs`). A default that drifted outside those
    /// bounds would be silently rewritten on the first load, so the shipped
    /// defaults and the accepted ranges have to agree.
    #[test]
    fn default_config_stays_inside_the_validated_ranges() {
        let config = AppConfig::default();

        assert!((24..=2048).contains(&config.paging.page_size));
        assert!((32..=8192).contains(&config.cache.thumbnail_entries));
        assert!((32..=8192).contains(&config.cache.directory_entries));
        assert!((30..=86_400).contains(&config.cache.thumbnail_ttl_seconds));
        assert!((30..=86_400).contains(&config.cache.directory_ttl_seconds));
        assert!((24..=256).contains(&config.view.thumbnail_size));
        assert!((20.0..=72.0).contains(&config.view.row_height));
        assert!((1..=12).contains(&config.view.grid_columns));
        assert!((72.0..=240.0).contains(&config.view.grid_row_height));
        assert!(config.view.overscan <= 128);
        assert!((16..=4096).contains(&config.filesystem.metadata_batch_size));
        assert!((1..=32).contains(&config.filesystem.metadata_parallelism));
    }

    /// Replaces `config_manager_loads_defaults_on_missing_file`, which built
    /// its manager with `ConfigManager::new()` and therefore read the
    /// developer's own file: if Xion had ever run, it loaded the user's
    /// settings, not the defaults its name promised. Its single assertion
    /// (`page_size > 0`) could not fail either. `with_path` keeps the whole
    /// exchange inside a temporary directory.
    #[test]
    fn missing_file_loads_the_defaults() {
        let temp = temp_dir();
        let manager = ConfigManager::with_path(temp.path().join("config.toml"));

        let load = manager.load();

        assert!(matches!(load.source, ConfigSource::Default));
        assert!(
            load.warnings.is_empty(),
            "un fichier absent n'est pas une anomalie : {:?}",
            load.warnings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(load.config, AppConfig::default());
        assert!(!manager.path().exists(), "un load ne doit rien écrire");
    }

    /// The round-trip the former `dark_mode_roundtrip` claimed but never did:
    /// it opened a temporary directory it ignored, wrote nothing, read nothing
    /// back, and only asserted a default flag.
    #[test]
    fn saving_then_loading_restores_the_configuration() {
        let temp = temp_dir();
        let manager = ConfigManager::with_path(temp.path().join("config.toml"));

        let defaults = AppConfig::default();
        let saved = AppConfig {
            dark_mode: true,
            theme: ThemeChoice {
                family: "nord".to_string(),
                variant: "dark".to_string(),
                high_contrast: false,
                accent: None,
            },
            compact_mode: true,
            respect_gitignore: true,
            list: ListConfig {
                show_hidden: true,
                sort_key: SortKeyConfig::Size,
                sort_order: SortOrderConfig::Desc,
                ..defaults.list.clone()
            },
            // Compact mode owns the row height, so the pair has to stay
            // consistent for the comparison below to be about persistence.
            view: ViewConfig {
                row_height: 22.0,
                ..defaults.view.clone()
            },
            paging: PagingConfig { page_size: 240 },
            ..defaults
        };

        manager.save(&saved);
        assert!(manager.path().exists(), "save doit créer le fichier");

        let load = manager.load();

        assert!(matches!(load.source, ConfigSource::File(_)));
        assert!(
            load.warnings.is_empty(),
            "relecture bruyante : {:?}",
            load.warnings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(load.config, saved);
    }

    /// V0 files have no `version` key. The migration that rebuilds the config
    /// of every existing user had no test at all, in either direction.
    #[test]
    fn a_v0_file_is_migrated_to_the_current_format() {
        let temp = temp_dir();
        let path = temp.path().join("config.toml");
        fs::write(&path, "show_hidden = true\nthumbnail_size = 64\n")
            .expect("écrire une config V0");

        let load = ConfigManager::with_path(path).load();

        assert!(
            matches!(load.source, ConfigSource::Migrated(_)),
            "une config sans version doit être signalée comme migrée"
        );
        assert!(load.config.list.show_hidden);
        assert_eq!(load.config.view.thumbnail_size, 64);
        // Everything V0 never knew about falls back to the current defaults.
        assert_eq!(
            load.config.paging.page_size,
            AppConfig::default().paging.page_size
        );
    }

    /// `compact_mode` owns the row height. A file that persisted the flag but
    /// kept the normal height used to come back un-compacted on the next
    /// launch, undoing the user's choice.
    #[test]
    fn a_persisted_compact_mode_restores_the_compact_row_height() {
        let temp = temp_dir();
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            "version = 1\ncompact_mode = true\n\n[view]\nrow_height = 32.0\n",
        )
        .expect("écrire une config V1");

        let load = ConfigManager::with_path(path).load();

        assert!(load.config.compact_mode);
        assert!(
            (load.config.view.row_height - 22.0).abs() < f32::EPSILON,
            "hauteur de ligne non compacte : {}",
            load.config.view.row_height
        );
    }

    /// A value outside the accepted range must fall back to the default and
    /// say so, instead of being trusted into the running app.
    #[test]
    fn an_out_of_range_page_size_falls_back_with_a_warning() {
        let temp = temp_dir();
        let path = temp.path().join("config.toml");
        fs::write(&path, "version = 1\n\n[paging]\npage_size = 100000\n")
            .expect("écrire une config V1");

        let load = ConfigManager::with_path(path).load();

        assert!(matches!(load.source, ConfigSource::File(_)));
        assert_eq!(
            load.config.paging.page_size,
            AppConfig::default().paging.page_size
        );
        assert!(
            !load.warnings.is_empty(),
            "le repli doit être signalé à l'utilisateur"
        );
    }
}

mod file_operations {
    use super::*;
    use xion::filesystem::LocalFileOperations;

    #[test]
    fn copy_single_file() {
        let src = temp_dir();
        let dst = temp_dir();
        let file = write_file(src.path(), "hello.txt");

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(std::slice::from_ref(&file), dst.path(), None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(report.failed.is_empty());
        assert!(dst.path().join("hello.txt").exists());
        // Original still exists
        assert!(file.exists());
    }

    #[test]
    fn move_single_file() {
        let src = temp_dir();
        let dst = temp_dir();
        let file = write_file(src.path(), "doc.txt");

        let ops = LocalFileOperations::new();
        let report = ops.move_items(std::slice::from_ref(&file), dst.path(), None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(report.failed.is_empty());
        assert!(dst.path().join("doc.txt").exists());
        // Original is gone
        assert!(!file.exists());
    }

    #[test]
    fn delete_files() {
        let temp = temp_dir();
        let file1 = write_file(temp.path(), "a.txt");
        let file2 = write_file(temp.path(), "b.txt");

        let ops = LocalFileOperations::new();
        let report = ops.delete_items(&[file1.clone(), file2.clone()]);

        assert_eq!(report.succeeded.len(), 2);
        assert!(report.failed.is_empty());
        assert!(!file1.exists());
        assert!(!file2.exists());
    }

    /// `delete_items` is the primitive every irreversible path ends on: the
    /// confirmed permanent delete, and the undo of a copy or of a creation.
    /// It has to erase the whole tree it was handed and stop there.
    #[test]
    fn delete_items_removes_a_tree_and_leaves_its_siblings_alone() {
        let temp = temp_dir();
        let doomed = temp.path().join("doomed");
        fs::create_dir_all(doomed.join("nested")).expect("créer l'arborescence");
        write_file(&doomed, "top.txt");
        write_file(&doomed.join("nested"), "deep.txt");
        let survivor = write_file(temp.path(), "survivor.txt");

        let ops = LocalFileOperations::new();
        let report = ops.delete_items(std::slice::from_ref(&doomed));

        assert!(report.failed.is_empty());
        assert_eq!(report.succeeded, vec![doomed.clone()]);
        assert!(!doomed.exists());
        assert!(survivor.exists(), "un frère ne doit jamais être emporté");
    }

    /// `rename_item` is what undo replays backwards for a move or a rename.
    #[test]
    fn rename_item_moves_a_file_back_to_its_original_path() {
        let temp = temp_dir();
        let original = write_file(temp.path(), "original.txt");
        let renamed = temp.path().join("renamed.txt");

        let ops = LocalFileOperations::new();
        let forward = ops.rename_item(&original, &renamed);
        assert!(forward.failed.is_empty());
        assert!(renamed.exists());
        assert!(!original.exists());

        let backward = ops.rename_item(&renamed, &original);
        assert!(backward.failed.is_empty());
        assert!(original.exists());
        assert!(!renamed.exists());
        assert_eq!(
            fs::read(&original).expect("relire le fichier"),
            b"contenu".to_vec()
        );
    }

    #[test]
    fn copy_reports_progress_via_counter() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let src = temp_dir();
        let dst = temp_dir();
        let items: Vec<PathBuf> = (0..5)
            .map(|index| write_file(src.path(), &format!("file_{index}.txt")))
            .collect();

        let counter = Arc::new(AtomicUsize::new(0));
        let ops = LocalFileOperations::new();
        ops.copy_items(&items, dst.path(), Some(counter.clone()));

        assert_eq!(counter.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn copy_fails_gracefully_on_nonexistent_source() {
        let src = temp_dir();
        let dst = temp_dir();
        // Built inside a real temp directory rather than from a hard-coded
        // absolute path, which was Unix-shaped and meant nothing on Windows.
        let nonexistent = src.path().join("jamais_créé.txt");

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(&[nonexistent], dst.path(), None);

        assert!(report.succeeded.is_empty());
        assert_eq!(report.failed.len(), 1);
    }

    #[test]
    fn copy_directory_recursive() {
        let src = temp_dir();
        let dst = temp_dir();
        let sub = src.path().join("subdir");
        fs::create_dir_all(&sub).expect("créer subdir");
        write_file(&sub, "nested.txt");

        let ops = LocalFileOperations::new();
        let report = ops.copy_items(&[sub], dst.path(), None);

        assert_eq!(report.succeeded.len(), 1);
        assert!(dst.path().join("subdir").join("nested.txt").exists());
    }
}

mod config_defaults {
    use super::*;
    use xion::core::{SortKeyConfig, SortOrderConfig};

    /// What was left of `dark_mode_roundtrip` once the name stopped lying: a
    /// default-value check. The actual round-trip now lives in
    /// `config_integration::saving_then_loading_restores_the_configuration`.
    /// Le défaut suit celui de l'écosystème.
    ///
    /// Xion démarrait en clair sur ses palettes maison. La palette de repli du
    /// catalogue Colony est `gruvbox/dark`, donc une installation neuve démarre
    /// désormais en sombre — un changement visible, assumé pour que Xion
    /// ressemble à ses voisins dès le premier lancement. Les configurations
    /// existantes conservent leur choix, qui est migré.
    #[test]
    fn the_default_theme_follows_the_ecosystem_fallback() {
        let config = AppConfig::default();
        assert_eq!(config.theme.family, "gruvbox");
        assert_eq!(config.theme.variant, "dark");
        assert!(!config.theme.high_contrast);
        assert_eq!(config.theme.accent, None);
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
    use xion::filesystem::{FsEntryType, FsMetadata, compare_entries};

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
///
/// The `*_for_test` harness only exists under the `test-util` feature, which
/// the self dev-dependency in `Cargo.toml` turns on for this target. The gate
/// makes that requirement explicit: should the feature ever stop reaching the
/// test target, these modules disappear instead of breaking the build.
#[cfg(feature = "test-util")]
mod ui_update {
    use xion::ui::UiMessage;
    use xion::ui::app::XionApp;

    // ── Dark mode ──────────────────────────────────────────────────────────────

    /// `ToggleDarkMode` ne fait plus tourner cinq thèmes en dur : il bascule la
    /// variante claire ou sombre de la famille courante. Le catalogue en compte
    /// cinquante-sept ; les parcourir un par un n'aurait plus de sens.
    #[test]
    fn toggle_dark_mode_flips_the_variant_of_the_current_family() {
        let mut app = XionApp::new_for_test();
        assert_eq!(app.state_for_test().config.theme.variant, "dark");
        assert!(app.state_for_test().config.dark_mode);

        let _ = app.update_for_test(UiMessage::ToggleDarkMode);
        assert_eq!(app.state_for_test().config.theme.variant, "light");
        assert_eq!(
            app.state_for_test().config.theme.family,
            "gruvbox",
            "la famille ne doit pas changer"
        );
        assert!(!app.state_for_test().config.dark_mode);

        let _ = app.update_for_test(UiMessage::ToggleDarkMode);
        assert_eq!(app.state_for_test().config.theme.variant, "dark");
        assert!(app.state_for_test().config.dark_mode);
    }

    // ── Gitignore ──────────────────────────────────────────────────────────────

    #[test]
    fn toggle_gitignore_flips_flag() {
        let mut app = XionApp::new_for_test();
        let initial = app.state_for_test().config.respect_gitignore;
        let _ = app.update_for_test(UiMessage::ToggleGitignore);
        assert_eq!(app.state_for_test().config.respect_gitignore, !initial);
        let _ = app.update_for_test(UiMessage::ToggleGitignore);
        assert_eq!(app.state_for_test().config.respect_gitignore, initial);
    }

    // ── Dual pane ─────────────────────────────────────────────────────────────

    #[test]
    fn toggle_dual_pane_enables_and_disables() {
        let mut app = XionApp::new_for_test();
        assert!(!app.dual_pane_for_test());
        let _ = app.update_for_test(UiMessage::ToggleDualPane);
        assert!(app.dual_pane_for_test());
        let _ = app.update_for_test(UiMessage::ToggleDualPane);
        assert!(!app.dual_pane_for_test());
    }

    // ── Quick filter ───────────────────────────────────────────────────────────

    #[test]
    fn quick_filter_sets_and_clears() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::QuickFilterChanged("rust".to_string()));
        assert_eq!(app.quick_filter_for_test(), "rust");
        assert!(app.quick_filter_active_for_test());
        let _ = app.update_for_test(UiMessage::QuickFilterClear);
        assert!(app.quick_filter_for_test().is_empty());
        assert!(!app.quick_filter_active_for_test());
    }

    // ── Tabs ──────────────────────────────────────────────────────────────────

    #[test]
    fn add_tab_increments_count() {
        let mut app = XionApp::new_for_test();
        assert_eq!(app.tab_count_for_test(), 1);
        let _ = app.update_for_test(UiMessage::AddTab);
        assert_eq!(app.tab_count_for_test(), 2);
    }

    #[test]
    fn close_tab_decrements_count() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::AddTab);
        assert_eq!(app.tab_count_for_test(), 2);
        let _ = app.update_for_test(UiMessage::CloseTab(1));
        assert_eq!(app.tab_count_for_test(), 1);
    }

    #[test]
    fn switch_tab_changes_active() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::AddTab);
        let _ = app.update_for_test(UiMessage::SwitchTab(1));
        assert_eq!(app.active_tab_for_test(), 1);
        let _ = app.update_for_test(UiMessage::SwitchTab(0));
        assert_eq!(app.active_tab_for_test(), 0);
    }

    // ── Context menu ─────────────────────────────────────────────────────────

    #[test]
    fn toggle_context_menu_opens_and_closes() {
        let mut app = XionApp::new_for_test();
        assert!(!app.context_menu_open_for_test());
        let _ = app.update_for_test(UiMessage::ToggleContextMenu(true));
        assert!(app.context_menu_open_for_test());
        let _ = app.update_for_test(UiMessage::ToggleContextMenu(false));
        assert!(!app.context_menu_open_for_test());
    }

    // ── Rename dialog ─────────────────────────────────────────────────────────

    #[test]
    fn rename_cancel_clears_dialog() {
        let mut app = XionApp::new_for_test();
        // Simulate opening rename for a path that doesn't need to exist for cancel
        let _ = app.update_for_test(UiMessage::RenameCancel);
        assert!(!app.rename_dialog_for_test());
    }

    // ── Properties dialog ─────────────────────────────────────────────────────

    #[test]
    fn close_properties_clears_dialog() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::CloseProperties);
        assert!(!app.properties_dialog_for_test());
    }

    // ── Hex viewer ────────────────────────────────────────────────────────────

    #[test]
    fn close_hex_view_clears_state() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::CloseHexView);
        assert!(!app.hex_view_for_test());
    }

    // ── Diff viewer ───────────────────────────────────────────────────────────

    #[test]
    fn close_diff_clears_state() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::CloseDiff);
        assert!(!app.diff_view_for_test());
    }

    // ── Grep ──────────────────────────────────────────────────────────────────

    #[test]
    fn close_grep_clears_state() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::CloseGrep);
        assert!(!app.grep_state_for_test());
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    #[test]
    fn navigate_to_recent_sets_recent_route() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::NavigateToRecent);
        assert!(app.state_for_test().route.is_recent());
    }

    #[test]
    fn navigate_to_local_path_updates_route() {
        let mut app = XionApp::new_for_test();
        // Deliberately not a `TempDir`: `NavigateTo` persists the tab list to
        // the config file, and a path that vanishes at the end of the test
        // would leave a dead tab behind.
        let target = std::env::temp_dir();
        let _ = app.update_for_test(UiMessage::NavigateTo(target.clone()));
        assert_eq!(app.state_for_test().route.local_path(), Some(&target));
    }

    // ── Recents ───────────────────────────────────────────────────────────────

    #[test]
    fn clear_recents_empties_service() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::ClearRecents);
        // After clearing, navigating to Recent should show 0 recents
        assert!(app.recents_is_empty_for_test());
    }

    // ── Bulk rename ───────────────────────────────────────────────────────────

    #[test]
    fn bulk_rename_cancel_clears_state() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::BulkRenameCancel);
        assert!(!app.bulk_rename_for_test());
    }

    // ── Address bar ───────────────────────────────────────────────────────────

    #[test]
    fn address_input_changed_updates_field() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::AddressInputChanged("C:\\Windows".to_string()));
        assert_eq!(app.address_input_for_test(), "C:\\Windows");
    }

    #[test]
    fn address_edit_start_and_cancel() {
        let mut app = XionApp::new_for_test();
        let _ = app.update_for_test(UiMessage::AddressEditStart);
        assert!(app.address_editing_for_test());
        let _ = app.update_for_test(UiMessage::AddressEditCancel);
        assert!(!app.address_editing_for_test());
    }

    // ── Network discovery ─────────────────────────────────────────────────────

    #[test]
    fn network_scan_completed_updates_discovery_cache() {
        use xion::services::NetworkResource;
        let mut app = XionApp::new_for_test();
        let resources = vec![NetworkResource {
            name: "NAS".to_string(),
            path: "smb://NAS".to_string(),
            status: xion::services::network::NetworkStatus::Online,
        }];
        let _ = app.update_for_test(UiMessage::NetworkScanCompleted(resources));
        // Cache is no longer stale after update
        assert!(!app.network_needs_scan_for_test());
    }
}

/// Message-level regression tests for the two delete paths.
///
/// Delete goes to the trash without asking; Shift+Delete is irreversible and
/// always waits for a confirmation. Both stage their real work in the returned
/// `Task`, which no runtime drives here — so the observable difference is the
/// selection: the trash path clears it at once, the permanent path only after
/// `ConfirmAccept` consumes the pending dialog.
#[cfg(feature = "test-util")]
mod deletion_flow {
    use super::*;
    use xion::ui::app::XionApp;
    use xion::ui::{KeyboardCommand, SelectionKind, UiMessage};

    /// Sorted, so assertions never depend on `HashSet` iteration order.
    fn selection(app: &XionApp) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = app
            .state_for_test()
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect();
        paths.sort();
        paths
    }

    fn select(app: &mut XionApp, path: &Path) {
        let _ = app.update_for_test(UiMessage::SelectEntry {
            path: path.to_path_buf(),
            kind: SelectionKind::Toggle,
        });
    }

    /// Regression: Delete used to call `remove_file`/`remove_dir_all` outright.
    /// It must hand the selection to the trash, erase nothing on the spot, and
    /// raise no confirmation — a recoverable action has nothing to confirm.
    #[test]
    fn delete_key_goes_to_the_trash_without_asking() {
        let temp = temp_dir();
        let victim = write_file(temp.path(), "victime.txt");
        let mut app = XionApp::new_for_test();

        select(&mut app, &victim);
        assert_eq!(selection(&app), vec![victim.clone()]);

        let _ = app.update_for_test(UiMessage::KeyboardCommand(KeyboardCommand::Delete));

        assert!(victim.exists(), "Suppr ne doit rien effacer directement");
        assert!(
            selection(&app).is_empty(),
            "la sélection est partie à la corbeille"
        );

        // Nothing is pending, so accepting a confirmation is a no-op and the
        // fresh selection survives. Had Delete opened the permanent-delete
        // dialog, `ConfirmAccept` would have cleared it.
        select(&mut app, &victim);
        let _ = app.update_for_test(UiMessage::ConfirmAccept);
        assert_eq!(selection(&app), vec![victim.clone()]);
        assert!(victim.exists());
    }

    #[test]
    fn shift_delete_waits_for_the_confirmation() {
        let temp = temp_dir();
        let victim = write_file(temp.path(), "victime.txt");
        let mut app = XionApp::new_for_test();

        select(&mut app, &victim);
        let _ = app.update_for_test(UiMessage::KeyboardCommand(
            KeyboardCommand::DeletePermanently,
        ));

        assert!(
            victim.exists(),
            "rien ne doit disparaître avant la confirmation"
        );
        assert_eq!(
            selection(&app),
            vec![victim.clone()],
            "la sélection reste intacte tant que le dialogue est ouvert"
        );

        // Accepting consumes the pending action, which clears the selection.
        let _ = app.update_for_test(UiMessage::ConfirmAccept);
        assert!(selection(&app).is_empty());
    }

    #[test]
    fn cancelling_shift_delete_discards_the_pending_deletion() {
        let temp = temp_dir();
        let victim = write_file(temp.path(), "victime.txt");
        let mut app = XionApp::new_for_test();

        select(&mut app, &victim);
        let _ = app.update_for_test(UiMessage::KeyboardCommand(
            KeyboardCommand::DeletePermanently,
        ));
        let _ = app.update_for_test(UiMessage::ConfirmCancel);

        // The dialog was dropped, not just hidden: a later accept finds nothing.
        let _ = app.update_for_test(UiMessage::ConfirmAccept);
        assert_eq!(selection(&app), vec![victim.clone()]);
        assert!(victim.exists());
    }

    #[test]
    fn delete_without_selection_changes_nothing() {
        let mut app = XionApp::new_for_test();
        assert!(selection(&app).is_empty());
        let _ = app.update_for_test(UiMessage::KeyboardCommand(KeyboardCommand::Delete));
        assert!(selection(&app).is_empty());
    }
}

/// `ChangeSort` must update the sort state on the spot, so the refresh it
/// triggers reads the new key and order.
#[cfg(feature = "test-util")]
mod sort_messages {
    use xion::core::{SortKeyConfig, SortOrderConfig};
    use xion::ui::UiMessage;
    use xion::ui::app::XionApp;

    #[test]
    fn change_sort_on_the_active_key_flips_the_order() {
        let mut app = XionApp::new_for_test();
        assert_eq!(
            app.state_for_test().config.list.sort_key,
            SortKeyConfig::Name
        );
        assert_eq!(
            app.state_for_test().config.list.sort_order,
            SortOrderConfig::Asc
        );

        let _ = app.update_for_test(UiMessage::ChangeSort(SortKeyConfig::Name));
        assert_eq!(
            app.state_for_test().config.list.sort_order,
            SortOrderConfig::Desc
        );

        let _ = app.update_for_test(UiMessage::ChangeSort(SortKeyConfig::Name));
        assert_eq!(
            app.state_for_test().config.list.sort_order,
            SortOrderConfig::Asc
        );
    }

    #[test]
    fn change_sort_on_another_key_restarts_ascending() {
        let mut app = XionApp::new_for_test();
        // Leaves the Name column descending.
        let _ = app.update_for_test(UiMessage::ChangeSort(SortKeyConfig::Name));

        let _ = app.update_for_test(UiMessage::ChangeSort(SortKeyConfig::Size));
        assert_eq!(
            app.state_for_test().config.list.sort_key,
            SortKeyConfig::Size
        );
        assert_eq!(
            app.state_for_test().config.list.sort_order,
            SortOrderConfig::Asc
        );
    }
}
