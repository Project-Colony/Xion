//! Filesystem access abstraction and local filesystem implementation.
//!
//! This module provides the [`FileSystem`] trait for abstracting filesystem
//! operations and [`LocalFileSystem`] as the concrete implementation for
//! local disk access.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::core::{AppResult, FilesystemConfig, XionError};
use crate::filesystem::metadata::FsMetadata;
use crate::filesystem::paging::{Page, PageRequest};

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub entry_type: FsEntryType,
    pub metadata: FsMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryType {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortKey {
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryFilter {
    All,
    OnlyDirectories,
    OnlyFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub sort_by: SortKey,
    pub sort_order: SortOrder,
    pub directories_first: bool,
    pub filter: EntryFilter,
    pub name_query: Option<String>,
    pub respect_gitignore: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            filter: EntryFilter::All,
            name_query: None,
            respect_gitignore: false,
        }
    }
}

impl ListOptions {
    pub fn with_name_query(mut self, query: impl Into<String>) -> Self {
        self.name_query = Some(query.into().to_lowercase());
        self
    }
}

pub trait FileSystem {
    fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>>;
    fn metadata(&self, path: &Path) -> AppResult<FsMetadata>;
    fn list_dir_paged(
        &self,
        path: &Path,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        let entries = self.list_dir(path, options)?;
        Ok(page.apply(entries))
    }
    fn metadata_batch(&self, paths: &[&Path]) -> AppResult<Vec<FsMetadata>> {
        paths.iter().map(|path| self.metadata(path)).collect()
    }
}

#[derive(Debug, Clone)]
pub struct LocalFileSystem {
    metadata_batch_size: usize,
    metadata_parallelism: usize,
}

impl Default for LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalFileSystem {
    pub fn new() -> Self {
        Self::from_config(FilesystemConfig::default())
    }

    pub fn from_config(config: FilesystemConfig) -> Self {
        Self {
            metadata_batch_size: config.metadata_batch_size.max(1),
            metadata_parallelism: config.metadata_parallelism.max(1),
        }
    }

    /// Unix convention only: a leading dot hides the entry.
    #[cfg(not(target_os = "windows"))]
    fn is_hidden(name: &str) -> bool {
        name.starts_with('.')
    }

    /// Windows marks hidden entries with an attribute, and the dot convention
    /// does not apply there: Explorer shows `.gitignore` and `.env`, so hiding
    /// them made the default listing differ from every other Windows tool.
    ///
    /// The attribute comes from the directory entry itself. Reading it through
    /// `fs::metadata(path)` reopened every file (one `CreateFileW` per entry,
    /// enough to stall a 20 000-file listing on OneDrive) and followed symlinks,
    /// which reported a broken link as visible whatever its own attributes said.
    #[cfg(target_os = "windows")]
    fn is_hidden_windows(entry: &fs::DirEntry) -> bool {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0002;
        entry
            .metadata()
            .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
            .unwrap_or(false)
    }

    fn matches_filter(entry_type: FsEntryType, filter: EntryFilter) -> bool {
        match filter {
            EntryFilter::All => true,
            EntryFilter::OnlyDirectories => entry_type == FsEntryType::Directory,
            EntryFilter::OnlyFiles => entry_type == FsEntryType::File,
        }
    }

    fn matches_query(name: &str, query: &Option<String>) -> bool {
        match query {
            Some(query) if !query.is_empty() => name.to_lowercase().contains(query),
            _ => true,
        }
    }
}

impl FileSystem for LocalFileSystem {
    fn list_dir(&self, path: &Path, options: ListOptions) -> AppResult<Vec<FsEntry>> {
        let page = self.list_dir_paged(path, options, PageRequest::new(0, usize::MAX))?;
        Ok(page.items)
    }

    fn metadata(&self, path: &Path) -> AppResult<FsMetadata> {
        let metadata = fs::metadata(path)?;
        Ok(FsMetadata::from_metadata(metadata))
    }

    fn list_dir_paged(
        &self,
        path: &Path,
        options: ListOptions,
        page: PageRequest,
    ) -> AppResult<Page<FsEntry>> {
        if !path.exists() {
            return Err(XionError::NotFound(path.to_path_buf()));
        }

        let needs_full_metadata = matches!(options.sort_by, SortKey::Modified | SortKey::Size);
        let mut entries = Vec::new();
        let long_path = ensure_long_path(path);

        // The chain is rooted on `long_path`, like the entries it is matched
        // against: mixing a `\\?\`-prefixed entry with an unprefixed root makes
        // the matcher reject the path outright.
        let gitignore = if options.respect_gitignore {
            build_gitignore_chain(long_path.as_ref())
        } else {
            Vec::new()
        };

        for entry in fs::read_dir(long_path.as_ref())? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // skip inaccessible entries (system files, broken symlinks)
            };
            let file_name = entry.file_name();
            // to_string_lossy is intentional: on Windows, filenames are almost always valid
            // Unicode. The rare U+FFFD replacement is visible and preferable to failing.
            let name = file_name.to_string_lossy().to_string();

            if !options.show_hidden {
                #[cfg(target_os = "windows")]
                let hidden = Self::is_hidden_windows(&entry);
                #[cfg(not(target_os = "windows"))]
                let hidden = Self::is_hidden(&name);
                if hidden {
                    continue;
                }
            }

            // Filter gitignored entries
            if !gitignore.is_empty() {
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                if is_gitignored(&gitignore, &entry.path(), is_dir) {
                    continue;
                }
            }

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue, // skip entries whose type cannot be determined
            };
            let entry_type = if file_type.is_dir() {
                FsEntryType::Directory
            } else if file_type.is_file() {
                FsEntryType::File
            } else if file_type.is_symlink() {
                FsEntryType::Symlink
            } else {
                FsEntryType::Other
            };

            if !Self::matches_filter(entry_type, options.filter) {
                continue;
            }

            if !Self::matches_query(&name, &options.name_query) {
                continue;
            }

            entries.push(EntryStub {
                path: entry.path(),
                name,
                entry_type,
                metadata: None,
            });
        }

        if needs_full_metadata {
            // Borrowed paths, not copies: sorting a 100 000-entry directory by
            // size used to clone every `PathBuf` in it — one heap allocation
            // apiece — purely to hand the batch something it only reads.
            let metadata = {
                let paths: Vec<&Path> = entries.iter().map(|entry| entry.path.as_path()).collect();
                self.metadata_batch(&paths)?
            };
            if metadata.len() != entries.len() {
                let path = entries[metadata.len().min(entries.len() - 1)].path.clone();
                return Err(XionError::InvalidPath(path));
            }
            for (entry, metadata) in entries.iter_mut().zip(metadata) {
                entry.metadata = Some(metadata);
            }
        }

        entries.sort_by(|left, right| crate::filesystem::sorting::compare(left, right, &options));

        let total = entries.len();
        let offset = page.offset.min(total);
        let end = offset.saturating_add(page.limit).min(total);

        let missing = {
            let missing_paths: Vec<&Path> = entries[offset..end]
                .iter()
                .filter(|entry| entry.metadata.is_none())
                .map(|entry| entry.path.as_path())
                .collect();
            self.metadata_batch(&missing_paths)?
        };
        let mut missing_iter = missing.into_iter();

        // Draining the page moves the stubs into the result. Building the page
        // used to clone the path and the name of every row it returned, with
        // the originals dropped on the next line.
        let mut items = Vec::with_capacity(end - offset);
        for entry in entries.drain(offset..end) {
            let metadata = match entry.metadata {
                Some(metadata) => metadata,
                None => missing_iter
                    .next()
                    .ok_or_else(|| XionError::InvalidPath(entry.path.clone()))?,
            };

            items.push(FsEntry {
                path: entry.path,
                name: entry.name,
                entry_type: entry.entry_type,
                metadata,
            });
        }

        Ok(Page {
            items,
            total,
            offset,
            limit: page.limit,
        })
    }

    /// Reads metadata for every path, spreading the `stat` calls over threads.
    ///
    /// The previous version re-entered `thread::scope` for every 256 paths and
    /// asked `available_parallelism()` again each time: sorting 100 000 files by
    /// size spawned and joined ~1 500 OS threads, whose creation cost rivalled
    /// the `stat` calls themselves. Threads now share one scope and write into
    /// disjoint slices, which also removes the `Mutex` (and its unreachable
    /// poison-recovery branch) from the hot path.
    fn metadata_batch(&self, paths: &[&Path]) -> AppResult<Vec<FsMetadata>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = vec![FsMetadata::default(); paths.len()];
        let thread_count = self.batch_thread_count(paths.len());

        if thread_count <= 1 {
            for (slot, path) in results.iter_mut().zip(paths) {
                *slot = read_metadata_or_default(path);
            }
            return Ok(results);
        }

        let chunk_size = paths.len().div_ceil(thread_count).max(1);
        thread::scope(|scope| {
            for (slots, chunk) in results.chunks_mut(chunk_size).zip(paths.chunks(chunk_size)) {
                scope.spawn(move || {
                    for (slot, path) in slots.iter_mut().zip(chunk) {
                        *slot = read_metadata_or_default(path);
                    }
                });
            }
        });

        Ok(results)
    }
}

impl LocalFileSystem {
    /// `metadata_batch_size` is now the minimum amount of work worth handing to
    /// a thread rather than a resubmission boundary, so small listings stay on
    /// the calling thread and never pay for a spawn.
    fn batch_thread_count(&self, path_count: usize) -> usize {
        let available_threads = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        let useful_threads = path_count.div_ceil(self.metadata_batch_size.max(1)).max(1);
        self.metadata_parallelism
            .min(available_threads)
            .min(useful_threads)
            .max(1)
    }
}

/// Inaccessible entries (system files, locked files, broken links) keep the
/// listing going with fallback metadata instead of failing the whole page.
fn read_metadata_or_default(path: &Path) -> FsMetadata {
    match fs::metadata(path) {
        Ok(metadata) => FsMetadata::from_metadata(metadata),
        Err(_) => FsMetadata::default(),
    }
}

/// Builds the gitignore matchers that apply to `directory`, closest first.
///
/// Only `directory/.gitignore` used to be loaded, so turning the option on and
/// walking into `repo/src` ignored nothing: the rules live in the repository
/// root, which was never consulted, and neither was `.git/info/exclude`.
fn build_gitignore_chain(directory: &Path) -> Vec<ignore::gitignore::Gitignore> {
    let mut chain = Vec::new();
    for ancestor in directory.ancestors() {
        if ancestor.as_os_str().is_empty() {
            break;
        }
        let mut builder = ignore::gitignore::GitignoreBuilder::new(ancestor);

        let ignore_file = ancestor.join(".gitignore");
        if ignore_file.is_file()
            && let Some(error) = builder.add(&ignore_file)
        {
            tracing::warn!("Gitignore: {} illisible: {error}", ignore_file.display());
        }

        // `.git` is a directory in a plain clone and a file in a worktree or a
        // submodule, so `exists()` rather than `is_dir()` marks the repo root.
        let repository_root = ancestor.join(".git").exists();
        if repository_root {
            let exclude = ancestor.join(".git").join("info").join("exclude");
            if exclude.is_file()
                && let Some(error) = builder.add(&exclude)
            {
                tracing::warn!("Gitignore: {} illisible: {error}", exclude.display());
            }
        }

        match builder.build() {
            // An empty matcher costs a lookup per entry and can never match.
            Ok(matcher) if !matcher.is_empty() => chain.push(matcher),
            Ok(_) => {}
            Err(error) => tracing::warn!(
                "Gitignore: erreur de parsing dans {}: {error}",
                ancestor.display()
            ),
        }

        if repository_root {
            break;
        }
    }
    chain
}

/// Applies the chain closest first, as git does: a nested `.gitignore` — and
/// its negated (`!`) patterns — overrides what its ancestors decided.
fn is_gitignored(chain: &[ignore::gitignore::Gitignore], path: &Path, is_dir: bool) -> bool {
    for matcher in chain {
        let matched = matcher.matched_path_or_any_parents(path, is_dir);
        if matched.is_ignore() {
            return true;
        }
        if matched.is_whitelist() {
            return false;
        }
    }
    false
}

/// Number of UTF-16 units above which a path gets the `\\?\` prefix.
///
/// `MAX_PATH` counts UTF-16 units, but the threshold used to be applied to the
/// UTF-8 length of the path: an accented path was converted hundreds of bytes
/// too early while a plain ASCII one could still overflow. The margin below 260
/// also covers the child names appended to a directory path while listing it —
/// the old check looked only at the directory, so a 230-character folder never
/// got the prefix even though every file inside it was over the limit.
#[cfg(windows)]
const LONG_PATH_THRESHOLD: usize = 200;

#[cfg(windows)]
fn ensure_long_path(path: &Path) -> std::borrow::Cow<'_, Path> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Component, Prefix};

    let wide_len = path.as_os_str().encode_wide().count();
    if wide_len <= LONG_PATH_THRESHOLD {
        return std::borrow::Cow::Borrowed(path);
    }

    // `\\?\` turns off Win32 path normalisation, so `.`, `..` and `/` have to be
    // resolved first or the converted path stops referring to the same file.
    let absolute = match std::path::absolute(path) {
        Ok(absolute) => absolute,
        Err(error) => {
            tracing::warn!(
                "Chemin long: normalisation de {} impossible: {error}",
                path.display()
            );
            return std::borrow::Cow::Borrowed(path);
        }
    };

    let prefix = match absolute.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        // Relative or device-relative: no verbatim form applies.
        _ => return std::borrow::Cow::Borrowed(path),
    };

    // Building through UTF-16 rather than `to_string_lossy` keeps unpaired
    // surrogates intact instead of replacing them with U+FFFD.
    let wide: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    let converted: Vec<u16> = match prefix {
        Prefix::Disk(_) => r"\\?\".encode_utf16().chain(wide).collect(),
        // \\server\share -> \\?\UNC\server\share
        Prefix::UNC(_, _) => r"\\?\UNC\"
            .encode_utf16()
            .chain(wide.iter().copied().skip(2))
            .collect(),
        // Already verbatim, or a device path that must not be rewritten.
        _ => return std::borrow::Cow::Borrowed(path),
    };

    std::borrow::Cow::Owned(std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &converted,
    )))
}

#[cfg(not(windows))]
fn ensure_long_path(path: &Path) -> std::borrow::Cow<'_, Path> {
    std::borrow::Cow::Borrowed(path)
}

/// An entry as `read_dir` gives it, before any `stat`.
///
/// `metadata` is `None` until the listing decides it needs it — which it only
/// does for the page it is about to return, or for everything when the sort key
/// requires it.
#[derive(Debug)]
struct EntryStub {
    path: PathBuf,
    name: String,
    entry_type: FsEntryType,
    metadata: Option<FsMetadata>,
}

impl crate::filesystem::sorting::Sortable for EntryStub {
    fn entry_type(&self) -> FsEntryType {
        self.entry_type
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn modified(&self) -> Option<std::time::SystemTime> {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.modified)
    }
    fn size(&self) -> Option<u64> {
        self.metadata.as_ref().map(|metadata| metadata.size)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EntryFilter, FileSystem, FilesystemConfig, ListOptions, LocalFileSystem, PageRequest,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("xion_fs_test_{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write file");
    }

    /// Hides `path` the way its platform does. On Unix the leading dot already
    /// does; Windows ignores the dot and reads the hidden attribute, which Git
    /// for Windows sets on `.git` itself.
    fn hide(path: &Path) {
        #[cfg(windows)]
        assert!(
            std::process::Command::new(crate::platform::system_binary("attrib.exe"))
                .arg("+h")
                .arg(path)
                .status()
                .expect("run attrib")
                .success()
        );
        #[cfg(not(windows))]
        let _ = path;
    }

    #[test]
    fn list_dir_filters_and_sorts() {
        let root = create_temp_dir();
        let folder = root.join("folder");
        fs::create_dir_all(&folder).expect("create folder");
        write_file(&root.join("alpha.txt"), "alpha");
        write_file(&root.join("beta.log"), "beta");
        write_file(&root.join(".hidden"), "hidden");
        hide(&root.join(".hidden"));

        let fs = LocalFileSystem::new();
        let entries = fs
            .list_dir(&root, ListOptions::default())
            .expect("list dir");
        let names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();

        assert_eq!(names, vec!["folder", "alpha.txt", "beta.log"]);

        let files_only = fs
            .list_dir(
                &root,
                ListOptions {
                    filter: EntryFilter::OnlyFiles,
                    ..ListOptions::default()
                },
            )
            .expect("list dir files");
        assert_eq!(files_only.len(), 2);

        let query_only = fs
            .list_dir(&root, ListOptions::default().with_name_query("alp"))
            .expect("list dir query");
        assert_eq!(query_only.len(), 1);
        assert_eq!(query_only[0].name, "alpha.txt");

        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn gitignore_rules_come_from_the_repository_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let repo = root.path();
        fs::create_dir_all(repo.join(".git")).expect("create .git");
        fs::write(repo.join(".gitignore"), "*.log\ntarget/\n").expect("write .gitignore");
        let src = repo.join("src");
        fs::create_dir_all(&src).expect("create src");
        write_file(&src.join("main.rs"), "fn main() {}");
        write_file(&src.join("debug.log"), "noise");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            respect_gitignore: true,
            ..ListOptions::default()
        };
        let names: Vec<String> = filesystem
            .list_dir(&src, options)
            .expect("list src")
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        // Before, only src/.gitignore was read, so the root rules were ignored
        // as soon as the user walked one level down.
        assert_eq!(names, vec!["main.rs"]);
    }

    #[test]
    fn gitignore_honours_git_info_exclude() {
        let root = tempfile::tempdir().expect("tempdir");
        let repo = root.path();
        fs::create_dir_all(repo.join(".git").join("info")).expect("create .git/info");
        hide(&repo.join(".git"));
        fs::write(
            repo.join(".git").join("info").join("exclude"),
            "secret.txt\n",
        )
        .expect("write exclude");
        write_file(&repo.join("secret.txt"), "hidden by exclude");
        write_file(&repo.join("public.txt"), "visible");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            respect_gitignore: true,
            ..ListOptions::default()
        };
        let names: Vec<String> = filesystem
            .list_dir(repo, options)
            .expect("list repo")
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        assert_eq!(names, vec!["public.txt"]);
    }

    #[test]
    fn nested_gitignore_can_whitelist_a_parent_rule() {
        let root = tempfile::tempdir().expect("tempdir");
        let repo = root.path();
        fs::create_dir_all(repo.join(".git")).expect("create .git");
        fs::write(repo.join(".gitignore"), "*.log\n").expect("write .gitignore");
        let src = repo.join("src");
        fs::create_dir_all(&src).expect("create src");
        fs::write(src.join(".gitignore"), "!debug.log\n").expect("write nested .gitignore");
        write_file(&src.join("debug.log"), "kept");
        write_file(&src.join("other.log"), "dropped");

        let filesystem = LocalFileSystem::new();
        let options = ListOptions {
            respect_gitignore: true,
            show_hidden: true,
            ..ListOptions::default()
        };
        let names: Vec<String> = filesystem
            .list_dir(&src, options)
            .expect("list src")
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        assert!(
            names.contains(&"debug.log".to_string()),
            "obtenu: {names:?}"
        );
        assert!(
            !names.contains(&"other.log".to_string()),
            "obtenu: {names:?}"
        );
    }

    #[test]
    fn gitignore_disabled_keeps_every_entry() {
        let root = tempfile::tempdir().expect("tempdir");
        let repo = root.path();
        fs::create_dir_all(repo.join(".git")).expect("create .git");
        hide(&repo.join(".git"));
        fs::write(repo.join(".gitignore"), "*.log\n").expect("write .gitignore");
        write_file(&repo.join("debug.log"), "noise");

        let filesystem = LocalFileSystem::new();
        let names: Vec<String> = filesystem
            .list_dir(repo, ListOptions::default())
            .expect("list repo")
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        // Windows shows `.gitignore`, as Explorer does: only the attribute hides.
        let expected = if cfg!(windows) {
            vec![".gitignore", "debug.log"]
        } else {
            vec!["debug.log"]
        };
        assert_eq!(names, expected);
    }

    #[test]
    fn metadata_batch_keeps_input_order_across_threads() {
        let root = tempfile::tempdir().expect("tempdir");
        // More paths than one batch, so the parallel path is exercised.
        let paths: Vec<PathBuf> = (0..64usize)
            .map(|index| {
                let path = root.path().join(format!("file_{index}.bin"));
                write_file(&path, &"x".repeat(index));
                path
            })
            .collect();

        let filesystem = LocalFileSystem::from_config(FilesystemConfig {
            metadata_batch_size: 8,
            metadata_parallelism: 4,
        });
        let borrowed: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        let metadata = filesystem
            .metadata_batch(&borrowed)
            .expect("metadata batch");

        assert_eq!(metadata.len(), paths.len());
        for (index, entry) in metadata.iter().enumerate() {
            assert_eq!(entry.size, index as u64, "décalage à l'index {index}");
        }
    }

    #[test]
    fn metadata_batch_falls_back_on_unreadable_paths() {
        let root = tempfile::tempdir().expect("tempdir");
        let present = root.path().join("present.txt");
        write_file(&present, "abc");
        let missing = root.path().join("missing.txt");

        let filesystem = LocalFileSystem::new();
        let metadata = filesystem
            .metadata_batch(&[missing.as_path(), present.as_path()])
            .expect("metadata batch");

        assert_eq!(metadata.len(), 2);
        assert_eq!(
            metadata[0].size, 0,
            "chemin absent -> métadonnées par défaut"
        );
        assert_eq!(metadata[1].size, 3);
    }

    #[test]
    fn metadata_batch_on_empty_input_spawns_nothing() {
        let filesystem = LocalFileSystem::new();
        assert!(
            filesystem
                .metadata_batch(&[])
                .expect("empty batch")
                .is_empty()
        );
    }

    #[test]
    fn list_dir_paged_returns_expected_slice() {
        let root = create_temp_dir();
        let folder = root.join("folder");
        fs::create_dir_all(&folder).expect("create folder");
        write_file(&root.join("alpha.txt"), "alpha");
        write_file(&root.join("beta.log"), "beta");

        let fs = LocalFileSystem::new();
        let page = fs
            .list_dir_paged(&root, ListOptions::default(), PageRequest::new(1, 1))
            .expect("list dir paged");

        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "alpha.txt");

        fs::remove_dir_all(&root).expect("cleanup");
    }
}
