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
use crate::filesystem::{FileOperationKind, FileWatcher, FsEntry, Page, WatchEvent};
use crate::services::{
    FavoritesService, PreviewImageService, SearchIndex, ThumbnailService, VirtualWindow,
};
use crate::ui::{AclEntry, DiffLine, GrepResult, NETWORK_ROUTE, RECENT_ROUTE, RouteKind};

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

#[derive(Debug)]
pub(super) struct TabManager {
    pub(super) tabs: Vec<TabState>,
    pub(super) active: usize,
}

impl TabManager {
    pub(super) fn new(tabs: Vec<TabState>, active: usize) -> Self {
        Self {
            active: active.min(tabs.len().saturating_sub(1)),
            tabs,
        }
    }

    pub(super) fn active_path(&self) -> Option<&PathBuf> {
        self.tabs.get(self.active).map(|t| &t.path)
    }

    pub(super) fn set_active_path(&mut self, path: PathBuf) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.path = path;
        }
    }

    pub(super) fn count(&self) -> usize {
        self.tabs.len()
    }
}

// ── Disk info ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
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
    let mut list: Vec<(PathBuf, DiskUsage)> = disks
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
        .collect();
    list.sort_by(|(a, _), (b, _)| a.cmp(b));
    list
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
    pub(super) fn get(
        &self,
        input: &str,
        resolve: impl FnOnce() -> Option<PathBuf>,
    ) -> Option<AddressValidation> {
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

#[derive(Debug, Default)]
pub(super) struct TerminalTab {
    pub(super) title: String,
    pub(super) input: String,
    pub(super) lines: Vec<String>,
    pub(super) process: Option<crate::terminal::TerminalProcess>,
    pub(super) cwd: Option<std::path::PathBuf>,
    /// Cached join of `lines` — rebuilt only when lines change.
    pub(super) cached_output: Option<String>,
}

impl TerminalTab {
    /// Mirror the pty's rendered screen.
    ///
    /// The pty owns the scrollback and its bound, so the tab no longer keeps a
    /// second, shorter buffer that silently truncated what the first one had
    /// already truncated.
    pub(super) fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = lines;
        self.rebuild_cached_output();
    }

    /// Append a line produced by Xion itself, not by the shell.
    pub(super) fn push_notice(&mut self, notice: String) {
        self.lines.push(notice);
        self.rebuild_cached_output();
    }

    fn rebuild_cached_output(&mut self) {
        self.cached_output = Some(self.lines.join("\n"));
    }

    pub(super) fn effective_cwd<'a>(
        &'a self,
        fallback: &'a std::path::Path,
    ) -> &'a std::path::Path {
        self.cwd.as_deref().unwrap_or(fallback)
    }
}

#[derive(Debug)]
pub(super) struct TerminalState {
    pub(super) tabs: Vec<TerminalTab>,
    pub(super) active_tab: usize,
}

impl Default for TerminalState {
    fn default() -> Self {
        let default_tab = TerminalTab {
            title: "Terminal 1".to_string(),
            ..Default::default()
        };
        Self {
            tabs: vec![default_tab],
            active_tab: 0,
        }
    }
}

impl TerminalState {
    /// Ensure active_tab is within bounds.
    fn clamp_active(&mut self) {
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }
    }

    /// Get a mutable reference to the active tab, inserting a default tab if empty.
    pub(super) fn active(&mut self) -> &mut TerminalTab {
        if self.tabs.is_empty() {
            self.tabs.push(TerminalTab::default());
            self.active_tab = 0;
        }
        self.clamp_active();
        &mut self.tabs[self.active_tab]
    }

    /// Get an immutable reference to the active tab, or a static default if empty.
    pub(super) fn active_ref(&self) -> &TerminalTab {
        if self.tabs.is_empty() {
            static DEFAULT_TAB: std::sync::LazyLock<TerminalTab> =
                std::sync::LazyLock::new(TerminalTab::default);
            return &DEFAULT_TAB;
        }
        let idx = self.active_tab.min(self.tabs.len() - 1);
        &self.tabs[idx]
    }

    // Delegate helpers to active tab for backward compat
    pub(super) fn push_notice(&mut self, notice: String) {
        self.active().push_notice(notice);
    }

    pub(super) fn effective_cwd<'a>(
        &'a self,
        fallback: &'a std::path::Path,
    ) -> &'a std::path::Path {
        self.active_ref().effective_cwd(fallback)
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
    pub(super) previews: Vec<(String, String)>, // (original name, new name)
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
    pub(super) inner_path: String, // current folder within archive
    pub(super) entries: Vec<crate::ui::ArchiveEntry>,
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
    pub(super) offset: usize, // scroll offset in rows of 16 bytes
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
    pub(super) error: Option<String>,
}

// ── Recents service ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct RecentEntry {
    pub(super) path: PathBuf,
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

// ── Undo stack ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) enum UndoAction {
    /// Files were copied to destination (undo = delete created copies)
    Copy { created: Vec<PathBuf> },
    /// Files were moved
    Move {
        original_paths: Vec<(PathBuf, PathBuf)>,
    }, // (source, destination) pairs
    /// A file was created
    FileCreated { path: PathBuf },
    /// A folder was created
    FolderCreated { path: PathBuf },
    /// A rename was done
    Renamed {
        old_path: PathBuf,
        new_path: PathBuf,
    },
}

#[derive(Debug, Default)]
pub(super) struct UndoStack {
    pub(super) actions: Vec<UndoAction>,
}

impl UndoStack {
    pub(super) fn push(&mut self, action: UndoAction) {
        if self.actions.len() >= 20 {
            self.actions.remove(0);
        }
        self.actions.push(action);
    }

    pub(super) fn pop(&mut self) -> Option<UndoAction> {
        self.actions.pop()
    }
}

/// Context stashed before an async file operation, to build undo actions on completion.
#[derive(Debug, Clone)]
pub(super) enum PendingUndoContext {
    /// Clipboard copy: sources and destination directory
    Copy {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    /// Clipboard move or cut: sources and destination directory
    Move {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    /// Rename: old path (new path comes from the report)
    Rename { old_path: PathBuf },
}

// ── Routing helpers ───────────────────────────────────────────────────────────

pub(super) fn build_default_favorites(persisted: &[PathBuf]) -> FavoritesService {
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
    // Restore user-added favorites from config
    for path in persisted {
        if path.exists() {
            favorites.add(path.clone());
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

// ── Menu state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContextSubmenu {
    Compress,
    Label,
}

#[derive(Debug, Default)]
pub(super) struct MenuState {
    pub(super) context_open: bool,
    pub(super) context_position: Option<iced::Point>,
    /// Background context menu (right-click on empty space, no selection)
    pub(super) background_context_open: bool,
    /// Currently open submenu (Compress / Label)
    pub(super) context_submenu: Option<ContextSubmenu>,
    pub(super) history_open: bool,
    pub(super) history_position: Option<iced::Point>,
}

// ── Dual-pane state ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub(super) struct DualPaneState {
    pub(super) enabled: bool,
    pub(super) pane_b: Option<PaneB>,
    pub(super) active: usize,
}

// ── Confirmation dialog ──────────────────────────────────────────────────────

/// What a confirmation dialog will carry out if the user accepts.
///
/// Only irreversible operations go through here. Anything recoverable — moving
/// to the trash, for instance — must not ask, or the prompt becomes noise the
/// user clicks through.
#[derive(Debug, Clone)]
pub(super) enum ConfirmedAction {
    /// Permanent deletion, bypassing the trash.
    DeletePermanently(Vec<std::path::PathBuf>),
}

#[derive(Debug, Clone)]
pub(super) struct ConfirmDialog {
    pub(super) title: String,
    pub(super) message: String,
    pub(super) confirm_label: String,
    pub(super) action: ConfirmedAction,
}
