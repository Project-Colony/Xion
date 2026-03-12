//! Private supporting types for XionApp.
//!
//! Defines internal structs used by the application state, such as paginated
//! entry buffers, UI sub-states, and utility helpers.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicUsize};
use std::time::{Duration, Instant};

use directories::UserDirs;
use iced::widget::image;
use sysinfo::Disks;

use crate::core::AppResult;
use crate::filesystem::{FileOperationKind, FsEntry, FileWatcher, Page, WatchEvent};
use crate::services::{
    FavoritesService, PreviewImageService, SearchIndex, ThumbnailService, VirtualWindow,
};
use crate::ui::{NETWORK_ROUTE, RECENT_ROUTE, RouteKind, DiffLine, GrepResult, AclEntry};

// ── Paginated entry buffer ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct PagedEntries {
    pub(super) total: usize,
    pub(super) items: Vec<Option<FsEntry>>,
    pub(super) loaded_pages: HashSet<usize>,
    pub(super) page_size: usize,
}

impl PagedEntries {
    pub(super) fn new(total: usize, page_size: usize) -> Self {
        Self {
            total,
            items: vec![None; total],
            loaded_pages: HashSet::new(),
            page_size: page_size.max(1),
        }
    }

    pub(super) fn reset(&mut self) {
        self.total = 0;
        self.items.clear();
        self.loaded_pages.clear();
    }

    pub(super) fn apply_page(&mut self, page_index: usize, page: Page<FsEntry>) {
        if self.total != page.total || self.items.len() != page.total {
            *self = Self::new(page.total, self.page_size);
        }

        for (index, entry) in page.items.into_iter().enumerate() {
            let target_index = page.offset + index;
            if target_index < self.items.len() {
                self.items[target_index] = Some(entry);
            }
        }

        self.loaded_pages.insert(page_index);
    }

    pub(super) fn is_page_loaded(&self, page_index: usize) -> bool {
        self.loaded_pages.contains(&page_index)
    }

    pub(super) fn get(&self, index: usize) -> Option<&FsEntry> {
        self.items.get(index).and_then(|entry| entry.as_ref())
    }
}

// ── Grid & tab state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(super) struct GridWindow {
    pub(super) window: VirtualWindow,
    pub(super) columns: usize,
    pub(super) total: usize,
}

#[derive(Debug, Clone)]
pub(super) struct TabState {
    pub(super) title: String,
    pub(super) path: PathBuf,
}

// ── Disk info ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(super) struct DiskUsage {
    pub(super) total: u64,
    pub(super) available: u64,
}

/// Cached disk usage to avoid calling Disks::new_with_refreshed_list() on every render frame.
/// Cache TTL: 10 seconds.
const DISK_CACHE_TTL: Duration = Duration::from_secs(10);

type DiskCacheData = Option<(Instant, Vec<(PathBuf, DiskUsage)>)>;

thread_local! {
    static DISK_CACHE: RefCell<DiskCacheData> = const { RefCell::new(None) };
}

fn refresh_disk_list() -> Vec<(PathBuf, DiskUsage)> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .map(|disk| {
            (
                disk.mount_point().to_path_buf(),
                DiskUsage {
                    total: disk.total_space(),
                    available: disk.available_space(),
                },
            )
        })
        .collect()
}

pub(super) fn format_gigabytes(bytes: u64) -> u64 {
    const BYTES_PER_GB: f64 = 1_000_000_000.0;
    ((bytes as f64) / BYTES_PER_GB).round() as u64
}

/// Returns all mounted drives with their usage info, using the shared disk cache.
pub(super) fn all_drives() -> Vec<(PathBuf, DiskUsage)> {
    DISK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let needs_refresh = cache
            .as_ref()
            .is_none_or(|(fetched_at, _)| fetched_at.elapsed() > DISK_CACHE_TTL);
        if needs_refresh {
            *cache = Some((Instant::now(), refresh_disk_list()));
        }
        cache
            .as_ref()
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    })
}

pub(super) fn drive_label(root_path: &Path) -> String {
    let label = root_path.display().to_string();
    if label.len() >= 2 && label.as_bytes().get(1) == Some(&b':') {
        format!("Disque local ({})", &label[..2])
    } else {
        format!("Disque local ({})", label)
    }
}

// ── Clipboard ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClipboardKind {
    Copy,
    Cut,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ClipboardState {
    pub(super) kind: Option<ClipboardKind>,
    pub(super) items: Vec<PathBuf>,
}

// ── Rename dialog ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct RenameDialog {
    pub(super) path: PathBuf,
    pub(super) input: String,
}

// ── Animated GIF preview ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct AnimatedFrame {
    pub(super) handle: image::Handle,
    pub(super) delay: Duration,
}

#[derive(Debug, Clone)]
pub(super) struct AnimatedPreview {
    pub(super) path: PathBuf,
    pub(super) frames: Vec<AnimatedFrame>,
    pub(super) current: usize,
    pub(super) next_frame_at: Instant,
    pub(super) handle: image::Handle,
}

// ── File operation progress ───────────────────────────────────────────────────

#[derive(Debug)]
pub(super) struct FileOpProgress {
    pub(super) counter: Arc<AtomicUsize>,
    pub(super) total: usize,
    pub(super) kind: FileOperationKind,
}

// ── Drag & drop ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct DragState {
    pub(super) items: Vec<PathBuf>,
}

// ── File watcher wrapper ──────────────────────────────────────────────────────

pub(super) struct FileWatcherHandle {
    pub(super) inner: Box<dyn FileWatcher>,
}

impl fmt::Debug for FileWatcherHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("FileWatcherHandle").finish()
    }
}

impl FileWatcherHandle {
    pub(super) fn new(inner: Box<dyn FileWatcher>) -> Self {
        Self { inner }
    }

    pub(super) fn watch(&mut self, path: &Path) -> AppResult<()> {
        self.inner.watch(path)
    }

    pub(super) fn unwatch(&mut self, path: &Path) -> AppResult<()> {
        self.inner.unwatch(path)
    }

    pub(super) fn poll(&mut self) -> AppResult<Vec<WatchEvent>> {
        self.inner.poll()
    }
}

// ── Search sub-state ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub(super) struct SearchState {
    pub(super) input: String,
    pub(super) index: Option<SearchIndex>,
    pub(super) index_path: Option<PathBuf>,
    pub(super) indexing: bool,
    pub(super) matches: Option<usize>,
}

// ── Pane resize state ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub(super) struct PaneResizeState {
    pub(super) tree_height: f32,
    pub(super) tree_resizing: bool,
    pub(super) tree_resize_anchor: Option<(f32, f32)>,
    pub(super) preview_width: f32,
    pub(super) preview_resizing: bool,
    pub(super) preview_resize_anchor: Option<(f32, f32)>,
}

impl Default for PaneResizeState {
    fn default() -> Self {
        Self {
            tree_height: 150.0,
            tree_resizing: false,
            tree_resize_anchor: None,
            preview_width: 280.0,
            preview_resizing: false,
            preview_resize_anchor: None,
        }
    }
}

// ── Media state (thumbnails + previews) ───────────────────────────────────────

#[derive(Debug)]
pub(super) struct MediaState {
    pub(super) thumbnails: ThumbnailService,
    pub(super) thumbnail_handles: HashMap<PathBuf, image::Handle>,
    pub(super) thumbnails_in_flight: HashSet<PathBuf>,
    pub(super) thumbnail_misses: HashSet<PathBuf>,
    pub(super) previews: PreviewImageService,
    pub(super) preview_handles: HashMap<PathBuf, image::Handle>,
    pub(super) previews_in_flight: HashSet<PathBuf>,
    pub(super) preview_misses: HashSet<PathBuf>,
    pub(super) animated: Option<AnimatedPreview>,
}

impl MediaState {
    pub(super) fn new(thumbnails: ThumbnailService, previews: PreviewImageService) -> Self {
        Self {
            thumbnails,
            thumbnail_handles: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            thumbnail_misses: HashSet::new(),
            previews,
            preview_handles: HashMap::new(),
            previews_in_flight: HashSet::new(),
            preview_misses: HashSet::new(),
            animated: None,
        }
    }
}

// ── Address validation cache ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AddressValidation {
    Directory,
    File,
    NotFound,
}

#[derive(Debug, Default)]
pub(super) struct AddressValidationCache {
    inner: RefCell<AddressValidationCacheInner>,
}

#[derive(Debug, Default)]
struct AddressValidationCacheInner {
    input: String,
    result: Option<AddressValidation>,
}

impl AddressValidationCache {
    pub(super) fn get(&self, input: &str, resolve: impl FnOnce() -> Option<PathBuf>) -> Option<AddressValidation> {
        let mut inner = self.inner.borrow_mut();
        if input != inner.input {
            inner.input = input.to_string();
            inner.result = resolve().map(|target| {
                if target.is_dir() {
                    AddressValidation::Directory
                } else if target.exists() {
                    AddressValidation::File
                } else {
                    AddressValidation::NotFound
                }
            });
        }
        inner.result
    }
}

// ── Terminal state ────────────────────────────────────────────────────────────

/// Maximum number of output lines kept in the terminal buffer.
const TERMINAL_MAX_LINES: usize = 500;

#[derive(Debug, Default)]
pub(super) struct TerminalTab {
    pub(super) title: String,
    pub(super) input: String,
    pub(super) lines: Vec<String>,
    pub(super) process: Option<crate::terminal::TerminalProcess>,
    pub(super) cwd: Option<std::path::PathBuf>,
}

impl TerminalTab {
    pub(super) fn push_lines(&mut self, new_lines: Vec<String>) {
        self.lines.extend(new_lines);
        if self.lines.len() > TERMINAL_MAX_LINES {
            let excess = self.lines.len() - TERMINAL_MAX_LINES;
            self.lines.drain(..excess);
        }
    }

    pub(super) fn push_prompt(&mut self, cwd: &std::path::Path, cmd: &str) {
        self.lines.push(format!("{}> {}", cwd.display(), cmd));
    }

    pub(super) fn effective_cwd<'a>(&'a self, fallback: &'a std::path::Path) -> &'a std::path::Path {
        self.cwd.as_deref().unwrap_or(fallback)
    }

    pub(super) fn apply_cd(&mut self, cmd: &str, fallback: &std::path::Path) {
        let trimmed = cmd.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("cd ").or_else(|| trimmed.strip_prefix("CD ")).or_else(|| trimmed.strip_prefix("chdir ")).or_else(|| trimmed.strip_prefix("CHDIR ")) {
            r.trim()
        } else if trimmed.eq_ignore_ascii_case("cd") || trimmed.eq_ignore_ascii_case("chdir") {
            return;
        } else {
            return;
        };

        let base = self.cwd.as_deref().unwrap_or(fallback);
        let new_cwd = if std::path::Path::new(rest).is_absolute() {
            std::path::PathBuf::from(rest)
        } else {
            base.join(rest)
        };
        self.cwd = Some(new_cwd);
    }
}

#[derive(Debug)]
pub(super) struct TerminalState {
    pub(super) tabs: Vec<TerminalTab>,
    pub(super) active_tab: usize,
    #[allow(dead_code)]
    pub(super) visible: bool,
    #[allow(dead_code)]
    pub(super) anim_progress: f32,
}

impl Default for TerminalState {
    fn default() -> Self {
        let mut default_tab = TerminalTab::default();
        default_tab.title = "Terminal 1".to_string();
        Self {
            tabs: vec![default_tab],
            active_tab: 0,
            visible: false,
            anim_progress: 0.0,
        }
    }
}

impl TerminalState {
    /// Get a mutable reference to the active tab.
    pub(super) fn active(&mut self) -> &mut TerminalTab {
        let idx = self.active_tab;
        &mut self.tabs[idx]
    }

    /// Get an immutable reference to the active tab.
    pub(super) fn active_ref(&self) -> &TerminalTab {
        &self.tabs[self.active_tab]
    }

    // Delegate helpers to active tab for backward compat
    pub(super) fn push_lines(&mut self, new_lines: Vec<String>) {
        self.active().push_lines(new_lines);
    }

    pub(super) fn push_prompt(&mut self, cwd: &std::path::Path, cmd: &str) {
        self.active().push_prompt(cwd, cmd);
    }

    pub(super) fn effective_cwd<'a>(&'a self, fallback: &'a std::path::Path) -> &'a std::path::Path {
        self.active_ref().effective_cwd(fallback)
    }

    pub(super) fn apply_cd(&mut self, cmd: &str, fallback: &std::path::Path) {
        let idx = self.active_tab;
        self.tabs[idx].apply_cd(cmd, fallback);
    }
}

// ── Properties dialog ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct PropertiesDialog {
    pub(super) path: PathBuf,
    pub(super) size_bytes: Option<u64>,
    pub(super) created: Option<String>,
    pub(super) modified: Option<String>,
    pub(super) readonly: bool,
    pub(super) sha256: Option<String>,
    pub(super) computing_hash: bool,
    pub(super) selection_count: Option<usize>,
}

// ── Bulk rename ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct BulkRenameState {
    pub(super) paths: Vec<PathBuf>,
    pub(super) find: String,
    pub(super) replace: String,
    pub(super) use_regex: bool,
    pub(super) previews: Vec<(String, String)>,  // (original name, new name)
    pub(super) error: Option<String>,
}

// ── Archive browser ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveType {
    Zip,
    TarGz,
    SevenZ,
}

#[derive(Debug, Clone)]
pub(super) struct ArchiveBrowserState {
    pub(super) archive_path: PathBuf,
    pub(super) inner_path: String,       // current folder within archive
    pub(super) entries: Vec<crate::ui::ArchiveEntry>,
    #[allow(dead_code)]
    pub(super) archive_type: ArchiveType,
}

// ── Diff viewer ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct DiffViewState {
    pub(super) path_a: PathBuf,
    pub(super) path_b: PathBuf,
    pub(super) lines: Vec<DiffLine>,
    pub(super) loading: bool,
}

// ── Hex viewer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct HexViewState {
    pub(super) path: PathBuf,
    pub(super) data: Vec<u8>,
    pub(super) offset: usize,  // scroll offset in rows of 16 bytes
}

// ── Grep ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct GrepState {
    pub(super) query: String,
    pub(super) root: PathBuf,
    pub(super) results: Vec<GrepResult>,
    pub(super) searching: bool,
}

// ── Permissions viewer ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct PermissionsViewState {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<AclEntry>,
    pub(super) loading: bool,
}

// ── Recents service ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct RecentEntry {
    pub(super) path: PathBuf,
    #[allow(dead_code)]
    pub(super) accessed: std::time::SystemTime,
}

#[derive(Debug, Default)]
pub(super) struct RecentsService {
    pub(super) entries: std::collections::VecDeque<RecentEntry>,
}

impl RecentsService {
    const MAX_ENTRIES: usize = 50;

    pub(super) fn record(&mut self, path: PathBuf) {
        // Remove existing entry for this path
        self.entries.retain(|e| e.path != path);
        self.entries.push_front(RecentEntry {
            path,
            accessed: std::time::SystemTime::now(),
        });
        if self.entries.len() > Self::MAX_ENTRIES {
            self.entries.pop_back();
        }
    }

    pub(super) fn list(&self) -> &std::collections::VecDeque<RecentEntry> {
        &self.entries
    }
}

// ── Column resize state ───────────────────────────────────────────────────────

#[derive(Debug)]
pub(super) struct ColumnResizeState {
    pub(super) column: String,
    pub(super) start_x: f32,
    pub(super) start_width: f32,
}

// ── Dual pane ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct PaneB {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<crate::filesystem::FsEntry>,
    #[allow(dead_code)]
    pub(super) selection: std::collections::HashSet<std::path::PathBuf>,
    pub(super) is_loading: bool,
}

// ── Scroll state ──────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub(super) struct ScrollState {
    pub(super) offset: f32,
    pub(super) height: f32,
    pub(super) content_height: f32,
    pub(super) tree_offset: f32,
    pub(super) tree_height: f32,
}

// ── Routing helpers ───────────────────────────────────────────────────────────

pub(super) fn build_default_favorites() -> FavoritesService {
    let mut favorites = FavoritesService::default();
    if let Some(user_dirs) = UserDirs::new() {
        let candidates = [
            Some(user_dirs.home_dir().to_path_buf()),
            user_dirs.desktop_dir().map(|path| path.to_path_buf()),
            user_dirs.download_dir().map(|path| path.to_path_buf()),
            user_dirs.document_dir().map(|path| path.to_path_buf()),
            user_dirs.picture_dir().map(|path| path.to_path_buf()),
            user_dirs.audio_dir().map(|path| path.to_path_buf()),
            user_dirs.video_dir().map(|path| path.to_path_buf()),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                favorites.add(candidate);
            }
        }
    }
    favorites
}

pub(super) fn root_path_for(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        path.ancestors()
            .last()
            .map(|ancestor| ancestor.to_path_buf())
    } else {
        None
    }
}

pub(super) fn is_network_path(path: &Path) -> bool {
    path.to_string_lossy().starts_with(NETWORK_ROUTE)
}

pub(super) fn is_recent_path(path: &Path) -> bool {
    path.to_string_lossy().starts_with(RECENT_ROUTE)
}

pub(super) fn route_kind_from_path(path: &Path) -> RouteKind {
    if is_network_path(path) {
        RouteKind::Network
    } else if is_recent_path(path) {
        RouteKind::Recent
    } else {
        RouteKind::Local(path.to_path_buf())
    }
}
