use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use directories::UserDirs;
use iced::alignment::Horizontal;
use iced::font::{Family, Style, Weight};
use iced::widget::button::Status as ButtonStatus;
use iced::widget::{
    button, column, container, horizontal_space, image, mouse_area, opaque, progress_bar, row,
    scrollable, stack, text, text_input, vertical_space,
};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Point, Subscription, Task, Theme,
    border, keyboard, mouse,
};

use crate::core::{
    ConfigManager, EntryFilterConfig, KeyInput, KeyKind, NamedKey, SortKeyConfig, SortOrderConfig,
    ViewColumn,
};
use crate::filesystem::{
    EntryFilter, FileOperationKind, FileSystem, FsEntry, FsEntryType, ListOptions,
    LocalFileOperations, LocalFileSystem, OperationReport, Page, PageRequest, SortKey, SortOrder,
};
use crate::services::{
    DirectoryLoader, FavoritesService, HistoryService, ThumbnailService, VirtualList,
    VirtualWindow, generate_thumbnail,
};
use crate::ui::{
    AppState, ContextAction, KeyboardCommand, ModifiersState, ScrollViewport, SelectionKind,
    UiMessage,
};
use sysinfo::Disks;

const FONT_NAME: &str = "JetBrainsMono Nerd Font";
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Regular.ttf");
const JETBRAINS_MONO_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Italic.ttf");
const JETBRAINS_MONO_THIN: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Thin.ttf");
const JETBRAINS_MONO_THIN_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ThinItalic.ttf");
const JETBRAINS_MONO_EXTRA_LIGHT: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraLight.ttf");
const JETBRAINS_MONO_EXTRA_LIGHT_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraLightItalic.ttf");
const JETBRAINS_MONO_LIGHT: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Light.ttf");
const JETBRAINS_MONO_LIGHT_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-LightItalic.ttf");
const JETBRAINS_MONO_MEDIUM: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Medium.ttf");
const JETBRAINS_MONO_MEDIUM_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-MediumItalic.ttf");
const JETBRAINS_MONO_SEMI_BOLD: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-SemiBold.ttf");
const JETBRAINS_MONO_SEMI_BOLD_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-SemiBoldItalic.ttf");
const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-Bold.ttf");
const JETBRAINS_MONO_BOLD_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-BoldItalic.ttf");
const JETBRAINS_MONO_EXTRA_BOLD: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraBold.ttf");
const JETBRAINS_MONO_EXTRA_BOLD_ITALIC: &[u8] =
    include_bytes!("../../ui/Assets/Fonts/JetBrainsMonoNerdFont-ExtraBoldItalic.ttf");

const ICON_DEVICE: &str = "";
const ICON_LOADING: &str = "";
const ICON_FOLDER: &str = "";
const ICON_FILE: &str = "";
const ICON_SYMLINK: &str = "";
const ICON_UNKNOWN: &str = "";
const ICON_HOME: &str = "";
const ICON_GALLERY: &str = "";
const ICON_DESKTOP: &str = "";
const ICON_DOWNLOAD: &str = "";
const ICON_DOCUMENTS: &str = "";
const ICON_MUSIC: &str = "";
const ICON_VIDEO: &str = "";
const ICON_PC: &str = "";
const ICON_DRIVE: &str = "";
const ICON_NETWORK: &str = "";
const ICON_BACK: &str = "";
const ICON_FORWARD: &str = "";
const ICON_REFRESH: &str = "";
const ICON_SEARCH: &str = "";
const ICON_NEW: &str = "";
const ICON_CUT: &str = "";
const ICON_COPY: &str = "";
const ICON_PASTE: &str = "";
const ICON_SORT: &str = "";
const ICON_VIEW: &str = "";
const ICON_MORE: &str = "";
const ICON_ACTIONS: &str = "";
const ICON_OPEN: &str = "";
const ICON_RENAME: &str = "";
const ICON_DELETE: &str = "";
const ICON_CLOSE: &str = "";

const LOADING_INDICATOR_DELAY: Duration = Duration::from_millis(75);
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);
const TREE_MAX_DEPTH: usize = 4;
const TREE_MAX_CHILDREN: usize = 120;

#[derive(Debug, Clone, Copy)]
struct UiColors {
    chrome_background: Color,
    panel_background: Color,
    border: Color,
    sidebar_background: Color,
    accent: Color,
    text_primary: Color,
    text_muted: Color,
    selection: Color,
    selection_border: Color,
    hover: Color,
    pressed: Color,
}

#[derive(Debug, Clone, Copy)]
struct UiSpacing {
    xs: f32,
    sm: f32,
    md: f32,
    lg: f32,
    xl: f32,
}

#[derive(Debug, Clone, Copy)]
struct UiTypography {
    title: u16,
    body: u16,
    caption: u16,
    title_font: Font,
    body_font: Font,
    caption_font: Font,
}

#[derive(Debug, Clone, Copy)]
struct UiTokens {
    colors: UiColors,
    spacing: UiSpacing,
    typography: UiTypography,
}

impl Default for UiTokens {
    fn default() -> Self {
        Self {
            colors: UiColors {
                chrome_background: Color::from_rgb8(247, 247, 250),
                panel_background: Color::from_rgb8(255, 255, 255),
                border: Color::from_rgb8(223, 226, 232),
                sidebar_background: Color::from_rgb8(242, 244, 248),
                accent: Color::from_rgb8(0, 120, 215),
                text_primary: Color::from_rgb8(32, 34, 38),
                text_muted: Color::from_rgb8(110, 114, 122),
                selection: Color::from_rgb8(214, 230, 248),
                selection_border: Color::from_rgb8(178, 206, 236),
                hover: Color::from_rgb8(233, 239, 247),
                pressed: Color::from_rgb8(220, 230, 244),
            },
            spacing: UiSpacing {
                xs: 4.0,
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 20.0,
            },
            typography: UiTypography {
                title: 16,
                body: 14,
                caption: 12,
                title_font: Font {
                    family: Family::Name(FONT_NAME),
                    weight: Weight::Semibold,
                    ..Font::DEFAULT
                },
                body_font: Font {
                    family: Family::Name(FONT_NAME),
                    weight: Weight::Normal,
                    ..Font::DEFAULT
                },
                caption_font: Font {
                    family: Family::Name(FONT_NAME),
                    weight: Weight::Light,
                    style: Style::Italic,
                    ..Font::DEFAULT
                },
            },
        }
    }
}

#[derive(Debug)]
struct PagedEntries {
    total: usize,
    items: Vec<Option<FsEntry>>,
    loaded_pages: HashSet<usize>,
    page_size: usize,
}

impl PagedEntries {
    fn new(total: usize, page_size: usize) -> Self {
        Self {
            total,
            items: vec![None; total],
            loaded_pages: HashSet::new(),
            page_size: page_size.max(1),
        }
    }

    fn reset(&mut self) {
        self.total = 0;
        self.items.clear();
        self.loaded_pages.clear();
    }

    fn apply_page(&mut self, page_index: usize, page: Page<FsEntry>) {
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

    fn is_page_loaded(&self, page_index: usize) -> bool {
        self.loaded_pages.contains(&page_index)
    }

    fn get(&self, index: usize) -> Option<&FsEntry> {
        self.items.get(index).and_then(|entry| entry.as_ref())
    }
}

#[derive(Debug, Clone)]
struct TabState {
    title: String,
    path: PathBuf,
}

fn build_default_favorites() -> FavoritesService {
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

fn root_path_for(path: &PathBuf) -> Option<PathBuf> {
    if path.is_absolute() {
        path.ancestors()
            .last()
            .map(|ancestor| ancestor.to_path_buf())
    } else {
        None
    }
}

#[derive(Clone, Copy)]
struct DiskUsage {
    total: u64,
    available: u64,
}

fn disk_usage_for(path: &Path) -> Option<DiskUsage> {
    let disks = Disks::new_with_refreshed_list();
    let mut best_match: Option<(usize, DiskUsage)> = None;

    for disk in disks.iter() {
        let mount = disk.mount_point();
        if path.starts_with(mount) {
            let depth = mount.components().count();
            let usage = DiskUsage {
                total: disk.total_space(),
                available: disk.available_space(),
            };
            if best_match
                .as_ref()
                .map_or(true, |(best_depth, _)| depth > *best_depth)
            {
                best_match = Some((depth, usage));
            }
        }
    }

    best_match.map(|(_, usage)| usage)
}

fn format_gigabytes(bytes: u64) -> u64 {
    const BYTES_PER_GB: f64 = 1_000_000_000.0;
    ((bytes as f64) / BYTES_PER_GB).round() as u64
}

fn drive_label(root_path: &Path) -> String {
    let label = root_path.display().to_string();
    if label.len() >= 2 && label.as_bytes().get(1) == Some(&b':') {
        format!("Disque local ({})", &label[..2])
    } else {
        format!("Disque local ({})", label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardKind {
    Copy,
    Cut,
}

#[derive(Debug, Default, Clone)]
struct ClipboardState {
    kind: Option<ClipboardKind>,
    items: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct RenameDialog {
    path: PathBuf,
    input: String,
}

#[derive(Debug)]
pub struct XionApp {
    state: AppState,
    history: HistoryService,
    directory_loader: Arc<Mutex<DirectoryLoader>>,
    thumbnails: ThumbnailService,
    thumbnail_handles: HashMap<PathBuf, image::Handle>,
    thumbnails_in_flight: HashSet<PathBuf>,
    thumbnail_misses: HashSet<PathBuf>,
    entries: PagedEntries,
    stale_entries: Option<PagedEntries>,
    pending_pages: HashSet<usize>,
    is_loading: bool,
    is_refreshing: bool,
    show_loading_indicator: bool,
    loading_generation: u64,
    scroll_offset: f32,
    viewport_height: f32,
    error: Option<String>,
    modifiers: ModifiersState,
    context_menu_open: bool,
    context_menu_position: Option<Point>,
    history_menu_open: bool,
    history_menu_position: Option<Point>,
    cursor_position: Option<Point>,
    last_action: Option<String>,
    address_input: String,
    search_input: String,
    config_manager: ConfigManager,
    favorites: FavoritesService,
    tabs: Vec<TabState>,
    active_tab: usize,
    clipboard: ClipboardState,
    rename_dialog: Option<RenameDialog>,
    last_click_time: Option<Instant>,
    last_clicked_path: Option<PathBuf>,
}

impl XionApp {
    fn refresh_entries(&mut self) -> Task<UiMessage> {
        let page_size = self.entries.page_size;
        if self.entries.total > 0 {
            self.stale_entries = Some(std::mem::replace(
                &mut self.entries,
                PagedEntries::new(0, page_size),
            ));
        } else {
            self.entries.reset();
            self.stale_entries = None;
        }
        self.error = None;
        self.scroll_offset = 0.0;
        self.pending_pages.clear();
        self.is_loading = true;
        self.is_refreshing = true;
        self.show_loading_indicator = false;
        self.loading_generation = self.loading_generation.wrapping_add(1);
        self.clear_selection();

        let mut tasks = Vec::new();
        tasks.push(self.request_page(0));
        tasks.push(self.schedule_loading_indicator(self.loading_generation));
        Task::batch(tasks)
    }

    fn update_active_tab_path(&mut self, path: PathBuf) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.path = path.clone();
        }
        self.state.route.path = path;
        self.address_input = self.state.route.path.display().to_string();
    }

    fn navigate_to(&mut self, path: PathBuf) -> Task<UiMessage> {
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.refresh_entries()
    }

    fn address_target_from_input(&self) -> Option<PathBuf> {
        let trimmed = self.address_input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut target = PathBuf::from(trimmed);
        if !target.is_absolute() {
            target = self.state.route.path.join(target);
        }
        Some(target)
    }

    fn address_suggestions(&self) -> Vec<PathBuf> {
        let query = self.address_input.trim().to_lowercase();
        let mut suggestions = Vec::new();
        let mut seen = HashSet::new();
        for entry in self.history.entries().iter().rev() {
            if entry == &self.state.route.path {
                continue;
            }
            let display = entry.display().to_string();
            if !query.is_empty() && !display.to_lowercase().contains(&query) {
                continue;
            }
            if seen.insert(entry.clone()) {
                suggestions.push(entry.clone());
            }
            if suggestions.len() >= 6 {
                break;
            }
        }
        suggestions
    }

    fn request_page(&mut self, page_index: usize) -> Task<UiMessage> {
        if self.pending_pages.contains(&page_index) {
            return Task::none();
        }

        let offset = page_index * self.entries.page_size;
        let page_request = PageRequest::new(offset, self.entries.page_size);
        let path = self.state.route.path.clone();
        let list_config = self.state.config.list.clone();
        let loader = Arc::clone(&self.directory_loader);

        self.pending_pages.insert(page_index);
        self.is_loading = true;

        Task::perform(
            async move {
                let options = list_options_from_config(list_config);
                let filesystem = LocalFileSystem::new();
                let result = match loader.lock() {
                    Ok(mut loader) => loader
                        .load_page(&filesystem, &path, options, page_request)
                        .map_err(|error| error.to_string()),
                    Err(_) => Err("Le chargeur de dossiers est indisponible.".to_string()),
                };
                (path, page_index, result)
            },
            |(path, page_index, result)| UiMessage::PageLoaded {
                path,
                page_index,
                result,
            },
        )
    }

    fn schedule_loading_indicator(&self, generation: u64) -> Task<UiMessage> {
        Task::perform(
            async move {
                std::thread::sleep(LOADING_INDICATOR_DELAY);
                generation
            },
            UiMessage::LoadingDelayElapsed,
        )
    }

    fn add_tab(&mut self) -> Task<UiMessage> {
        let new_index = self.tabs.len() + 1;
        let title = if new_index == 1 {
            "Ce PC".to_string()
        } else {
            format!("Ce PC {}", new_index)
        };
        let path = self.state.config.start_path.clone();
        self.tabs.push(TabState { title, path });
        self.active_tab = self.tabs.len().saturating_sub(1);
        let active_path = self
            .tabs
            .get(self.active_tab)
            .map(|tab| tab.path.clone())
            .unwrap_or_else(|| self.state.config.start_path.clone());
        self.update_active_tab_path(active_path.clone());
        self.history.record(active_path);
        self.refresh_entries()
    }

    fn switch_tab(&mut self, index: usize) -> Task<UiMessage> {
        if index >= self.tabs.len() {
            return Task::none();
        }
        self.active_tab = index;
        let path = self.tabs[index].path.clone();
        self.update_active_tab_path(path.clone());
        self.history.record(path);
        self.refresh_entries()
    }

    fn close_tab(&mut self, index: usize) -> Task<UiMessage> {
        if self.tabs.len() <= 1 || index == 0 || index >= self.tabs.len() {
            return Task::none();
        }
        self.tabs.remove(index);
        if self.active_tab == index {
            self.active_tab = index.saturating_sub(1);
        } else if self.active_tab > index {
            self.active_tab = self.active_tab.saturating_sub(1);
        }
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let path = tab.path.clone();
            self.update_active_tab_path(path.clone());
            self.history.record(path);
            return self.refresh_entries();
        }
        Task::none()
    }
}

impl XionApp {
    fn new() -> (Self, Task<UiMessage>) {
        let config_manager = ConfigManager::new();
        let config_load = config_manager.load();
        let config = config_load.config;
        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.path.clone());
        let tabs = vec![TabState {
            title: "Ce PC".to_string(),
            path: state.route.path.clone(),
        }];

        let directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
            state.config.cache.directory_entries,
            Duration::from_secs(state.config.cache.directory_ttl_seconds),
            state.config.paging.page_size,
        )));
        let page_size = directory_loader
            .lock()
            .map(|loader| loader.page_size())
            .unwrap_or(state.config.paging.page_size);
        let thumbnails = ThumbnailService::new(
            state.config.cache.thumbnail_entries,
            Duration::from_secs(state.config.cache.thumbnail_ttl_seconds),
        );
        let entries = PagedEntries::new(0, page_size);
        let address_input = state.route.path.display().to_string();
        let favorites = build_default_favorites();
        let mut app = Self {
            state,
            history,
            directory_loader,
            thumbnails,
            thumbnail_handles: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            thumbnail_misses: HashSet::new(),
            entries,
            stale_entries: None,
            pending_pages: HashSet::new(),
            is_loading: false,
            is_refreshing: false,
            show_loading_indicator: false,
            loading_generation: 0,
            scroll_offset: 0.0,
            viewport_height: 480.0,
            error: None,
            modifiers: ModifiersState::default(),
            context_menu_open: false,
            context_menu_position: None,
            history_menu_open: false,
            history_menu_position: None,
            cursor_position: None,
            last_action: None,
            address_input,
            search_input: String::new(),
            config_manager,
            favorites,
            tabs,
            active_tab: 0,
            clipboard: ClipboardState::default(),
            rename_dialog: None,
            last_click_time: None,
            last_clicked_path: None,
        };
        if !config_load.warnings.is_empty() {
            app.last_action = Some(format!(
                "Config: {}",
                config_load
                    .warnings
                    .iter()
                    .map(|warning| warning.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        let task = app.refresh_entries();
        (app, task)
    }

    fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        match message {
            UiMessage::Noop => {}
            UiMessage::CursorMoved(position) => {
                self.cursor_position = Some(position);
            }
            UiMessage::NavigateTo(path) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddTab => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.add_tab());
            }
            UiMessage::SwitchTab(index) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.switch_tab(index));
            }
            UiMessage::CloseTab(index) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.close_tab(index));
            }
            UiMessage::Back => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                if let Some(path) = self.history.back() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Forward => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                if let Some(path) = self.history.forward() {
                    self.update_active_tab_path(path);
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Refresh => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.reload_config());
                tasks.push(self.refresh_entries());
            }
            UiMessage::FocusPane(pane) => {
                self.state.navigation.focused_pane = pane;
            }
            UiMessage::SelectEntry { path, kind } => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                let now = Instant::now();
                if kind != SelectionKind::Single {
                    self.last_click_time = None;
                    self.last_clicked_path = None;
                    self.apply_selection(path, kind);
                } else {
                    let is_double_click = self
                        .last_clicked_path
                        .as_ref()
                        .is_some_and(|last_path| last_path == &path)
                        && self.last_click_time.is_some_and(|last_click| {
                            now.duration_since(last_click) <= DOUBLE_CLICK_THRESHOLD
                        });
                    self.apply_selection(path.clone(), kind);
                    if is_double_click {
                        self.last_click_time = None;
                        self.last_clicked_path = None;
                        tasks.push(self.activate_entry(path));
                    } else {
                        self.last_click_time = Some(now);
                        self.last_clicked_path = Some(path);
                    }
                }
            }
            UiMessage::ActivateEntry(path) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                tasks.push(self.activate_entry(path));
            }
            UiMessage::KeyboardCommand(command) => {
                tasks.push(self.handle_keyboard_command(command));
            }
            UiMessage::ToggleContextMenu(force_open) => {
                self.context_menu_open = force_open;
                if force_open {
                    self.context_menu_position = self.cursor_position;
                } else {
                    self.context_menu_position = None;
                }
                self.history_menu_open = false;
                self.history_menu_position = None;
            }
            UiMessage::ToggleHistoryMenu(force_open) => {
                self.history_menu_open = force_open;
                if force_open {
                    self.history_menu_position = self.cursor_position;
                } else {
                    self.history_menu_position = None;
                }
            }
            UiMessage::OpenContextMenuForEntry(path) => {
                let is_selected = self.state.navigation.selection.selected.contains(&path);
                if !is_selected {
                    self.apply_selection(path, SelectionKind::Single);
                }
                self.context_menu_open = true;
                self.context_menu_position = self.cursor_position;
                self.history_menu_open = false;
                self.history_menu_position = None;
            }
            UiMessage::ContextAction(action) => {
                tasks.push(self.apply_context_action(action));
            }
            UiMessage::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            UiMessage::AddressInputChanged(value) => {
                self.address_input = value;
            }
            UiMessage::AddressSuggestionSelected(path) => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                self.address_input = path.display().to_string();
                tasks.push(self.navigate_to(path));
            }
            UiMessage::AddressInputSubmitted => {
                self.history_menu_open = false;
                self.history_menu_position = None;
                if let Some(target) = self.address_target_from_input() {
                    if target.is_dir() {
                        tasks.push(self.navigate_to(target));
                    } else if target.exists() {
                        self.last_action = Some(format!(
                            "Le chemin pointe vers un fichier : {}",
                            target.display()
                        ));
                    } else {
                        self.last_action =
                            Some(format!("Chemin introuvable : {}", target.display()));
                    }
                }
            }
            UiMessage::SearchInputChanged(value) => {
                self.search_input = value;
                self.scroll_offset = 0.0;
                self.clear_selection();
                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                } else {
                    tasks.push(self.ensure_visible_pages());
                }
            }
            UiMessage::SearchInputSubmitted => {
                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                }
            }
            UiMessage::Scroll(viewport) => {
                self.scroll_offset = viewport.offset_y;
                self.viewport_height = viewport.viewport_height.max(1.0);
                tasks.push(self.ensure_visible_pages());
            }
            UiMessage::ChangeSort(sort_key) => {
                if self.state.config.list.sort_key == sort_key {
                    self.state.config.list.sort_order = match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => SortOrderConfig::Desc,
                        SortOrderConfig::Desc => SortOrderConfig::Asc,
                    };
                } else {
                    self.state.config.list.sort_key = sort_key;
                    self.state.config.list.sort_order = SortOrderConfig::Asc;
                }
                self.last_action = Some(format!(
                    "Tri : {:?} ({:?})",
                    self.state.config.list.sort_key, self.state.config.list.sort_order
                ));
                tasks.push(self.refresh_entries());
            }
            UiMessage::LoadingDelayElapsed(generation) => {
                if self.is_refreshing
                    && self.is_loading
                    && generation == self.loading_generation
                    && self.entries.total == 0
                {
                    self.show_loading_indicator = true;
                }
            }
            UiMessage::PageLoaded {
                path,
                page_index,
                result,
            } => {
                if path != self.state.route.path {
                    return Task::batch(tasks);
                }

                self.pending_pages.remove(&page_index);
                match result {
                    Ok(page) => {
                        self.entries.apply_page(page_index, page);
                        self.error = None;
                        if self.is_refreshing {
                            self.stale_entries = None;
                        }
                        tasks.push(self.ensure_visible_pages());
                    }
                    Err(message) => {
                        self.error = Some(message);
                    }
                }

                if self.pending_pages.is_empty() {
                    self.is_loading = false;
                    self.is_refreshing = false;
                    self.show_loading_indicator = false;
                }

                if self.normalized_search_query().is_some() {
                    tasks.push(self.request_all_pages());
                }
            }
            UiMessage::ThumbnailLoaded { path, thumbnail } => {
                self.thumbnails_in_flight.remove(&path);
                match thumbnail {
                    Some(thumbnail) => {
                        self.thumbnail_handles.insert(
                            path.clone(),
                            image::Handle::from_bytes(thumbnail.bytes.clone()),
                        );
                        self.thumbnails.insert(path.clone(), thumbnail);
                        self.thumbnail_misses.remove(&path);
                    }
                    None => {
                        self.thumbnail_misses.insert(path);
                    }
                }
            }
            UiMessage::ClipboardCut => {
                self.capture_clipboard(ClipboardKind::Cut);
            }
            UiMessage::ClipboardCopy => {
                self.capture_clipboard(ClipboardKind::Copy);
            }
            UiMessage::ClipboardPaste => {
                tasks.push(self.paste_clipboard());
            }
            UiMessage::RenameInputChanged(value) => {
                if let Some(dialog) = &mut self.rename_dialog {
                    dialog.input = value;
                }
            }
            UiMessage::RenameSubmit => {
                tasks.push(self.submit_rename());
            }
            UiMessage::RenameCancel => {
                self.rename_dialog = None;
            }
            UiMessage::FileOperationFinished(report) => {
                self.handle_operation_report(&report);
                tasks.push(self.refresh_entries());
            }
        }

        tasks.push(self.request_visible_thumbnails());
        Task::batch(tasks)
    }

    fn subscription(&self) -> Subscription<UiMessage> {
        let shortcuts = self.state.config.shortcuts.clone();
        iced::event::listen().map(move |event| match event {
            iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                UiMessage::ModifiersChanged(ModifiersState {
                    shift: modifiers.shift(),
                    control: modifiers.control(),
                    alt: modifiers.alt(),
                })
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                command_from_key_press_with_shortcuts(&shortcuts, key, modifiers)
                    .map(UiMessage::KeyboardCommand)
                    .unwrap_or(UiMessage::Noop)
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                UiMessage::CursorMoved(position)
            }
            _ => UiMessage::Noop,
        })
    }

    fn selection_kind_from_modifiers(&self) -> SelectionKind {
        if self.modifiers.shift {
            SelectionKind::Range
        } else if self.modifiers.control {
            SelectionKind::Toggle
        } else {
            SelectionKind::Single
        }
    }

    fn reload_config(&mut self) -> Task<UiMessage> {
        let load = self.config_manager.load();
        let new_config = load.config;
        if new_config == self.state.config {
            return Task::none();
        }

        let should_reset_loader = new_config.cache.directory_entries
            != self.state.config.cache.directory_entries
            || new_config.cache.directory_ttl_seconds
                != self.state.config.cache.directory_ttl_seconds
            || new_config.paging.page_size != self.state.config.paging.page_size;

        if should_reset_loader {
            self.directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
                new_config.cache.directory_entries,
                Duration::from_secs(new_config.cache.directory_ttl_seconds),
                new_config.paging.page_size,
            )));
            self.entries = PagedEntries::new(0, new_config.paging.page_size);
            self.pending_pages.clear();
        }

        if new_config.cache.thumbnail_entries != self.state.config.cache.thumbnail_entries
            || new_config.cache.thumbnail_ttl_seconds
                != self.state.config.cache.thumbnail_ttl_seconds
        {
            self.thumbnails = ThumbnailService::new(
                new_config.cache.thumbnail_entries,
                Duration::from_secs(new_config.cache.thumbnail_ttl_seconds),
            );
            self.thumbnail_handles.clear();
            self.thumbnail_misses.clear();
            self.thumbnails_in_flight.clear();
        }

        if self.state.route.path == self.state.config.start_path {
            self.state.route.path = new_config.start_path.clone();
        }

        if !load.warnings.is_empty() {
            self.last_action = Some(format!(
                "Config: {}",
                load.warnings
                    .iter()
                    .map(|warning| warning.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        } else {
            self.last_action = Some("Config rechargée".to_string());
        }

        self.state.config = new_config;
        Task::none()
    }

    fn apply_selection(&mut self, path: PathBuf, kind: SelectionKind) {
        let anchor_path = self.state.navigation.selection.anchor.clone();
        let selection_kind = match kind {
            SelectionKind::Range if anchor_path.is_none() => SelectionKind::Single,
            SelectionKind::Range => SelectionKind::Range,
            other => other,
        };
        let target_index = self.index_for_path(&path);
        let anchor_index = anchor_path
            .as_ref()
            .and_then(|anchor_path| self.index_for_path(anchor_path));

        let selection = &mut self.state.navigation.selection;

        match selection_kind {
            SelectionKind::Single => {
                selection.selected.clear();
                selection.selected.insert(path.clone());
                selection.focused = Some(path.clone());
                selection.anchor = Some(path);
            }
            SelectionKind::Toggle => {
                if selection.selected.contains(&path) {
                    selection.selected.remove(&path);
                } else {
                    selection.selected.insert(path.clone());
                }
                selection.focused = Some(path.clone());
                selection.anchor.get_or_insert(path);
            }
            SelectionKind::Range => {
                let anchor_path = anchor_path.unwrap_or_else(|| path.clone());
                let target = target_index;
                let anchor = anchor_index;

                if let (Some(anchor), Some(target)) = (anchor, target) {
                    selection.selected.clear();
                    let (start, end) = if anchor <= target {
                        (anchor, target)
                    } else {
                        (target, anchor)
                    };
                    for index in start..=end {
                        if let Some(entry) = self.entries.get(index) {
                            selection.selected.insert(entry.path.clone());
                        }
                    }
                    selection.focused = Some(path.clone());
                    selection.anchor = Some(anchor_path);
                } else {
                    selection.selected.clear();
                    selection.selected.insert(path.clone());
                    selection.focused = Some(path.clone());
                    selection.anchor = Some(path);
                }
            }
        }

        if selection.selected.is_empty() {
            selection.focused = None;
            selection.anchor = None;
        }
        self.context_menu_open = false;
        self.context_menu_position = None;
    }

    fn selected_entry<'a>(&'a self, entries: &'a PagedEntries) -> Option<&'a FsEntry> {
        let selection = &self.state.navigation.selection;
        let selected_path = selection
            .focused
            .as_ref()
            .or_else(|| selection.selected.iter().next());
        let selected_path = selected_path?;
        entries
            .items
            .iter()
            .filter_map(|entry| entry.as_ref())
            .find(|entry| &entry.path == selected_path)
    }

    fn handle_keyboard_command(&mut self, command: KeyboardCommand) -> Task<UiMessage> {
        match command {
            KeyboardCommand::MoveUp { extend } => {
                self.move_focus_by(-1, extend);
                Task::none()
            }
            KeyboardCommand::MoveDown { extend } => {
                self.move_focus_by(1, extend);
                Task::none()
            }
            KeyboardCommand::MoveHome { extend } => {
                self.move_focus_to_start(extend);
                Task::none()
            }
            KeyboardCommand::MoveEnd { extend } => {
                self.move_focus_to_end(extend);
                Task::none()
            }
            KeyboardCommand::Activate => self.activate_focused_entry(),
            KeyboardCommand::Back => {
                if self.history.can_back() {
                    if let Some(path) = self.history.back() {
                        self.state.route.path = path;
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Forward => {
                if self.history.can_forward() {
                    if let Some(path) = self.history.forward() {
                        self.state.route.path = path;
                        return self.refresh_entries();
                    }
                }
                Task::none()
            }
            KeyboardCommand::Refresh => self.refresh_entries(),
            KeyboardCommand::SelectAll => {
                self.select_all_entries();
                Task::none()
            }
            KeyboardCommand::ClearSelection => {
                self.clear_selection();
                Task::none()
            }
            KeyboardCommand::ToggleContextMenu => {
                self.context_menu_open = !self.context_menu_open;
                if self.context_menu_open {
                    self.context_menu_position = self.cursor_position;
                } else {
                    self.context_menu_position = None;
                }
                Task::none()
            }
            KeyboardCommand::CyclePaneFocus => {
                self.cycle_focus();
                Task::none()
            }
        }
    }

    fn apply_context_action(&mut self, action: ContextAction) -> Task<UiMessage> {
        self.context_menu_open = false;
        self.context_menu_position = None;
        let selection = &self.state.navigation.selection;
        let selected_label = if selection.selected.len() == 1 {
            selection
                .selected
                .iter()
                .next()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".to_string())
        } else if selection.selected.is_empty() {
            "—".to_string()
        } else {
            format!("{} éléments", selection.selected.len())
        };

        self.last_action = Some(match action {
            ContextAction::Open => format!("Ouverture : {}", selected_label),
            ContextAction::Rename => format!("Renommer : {}", selected_label),
            ContextAction::Delete => format!("Supprimer : {}", selected_label),
            ContextAction::CopyPath => format!("Copier le chemin : {}", selected_label),
        });

        match action {
            ContextAction::Open => self.activate_focused_entry(),
            ContextAction::Rename => self.open_rename_dialog(),
            ContextAction::Delete => self.delete_selection(),
            ContextAction::CopyPath => {
                self.copy_selection_path();
                Task::none()
            }
        }
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        self.state
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect()
    }

    fn capture_clipboard(&mut self, kind: ClipboardKind) {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à mettre en presse-papiers".to_string());
            return;
        }
        let label = match kind {
            ClipboardKind::Copy => "Copie",
            ClipboardKind::Cut => "Déplacement",
        };
        self.clipboard.kind = Some(kind);
        self.clipboard.items = items;
        self.last_action = Some(format!(
            "{} : {} élément(s)",
            label,
            self.clipboard.items.len()
        ));
    }

    fn paste_clipboard(&mut self) -> Task<UiMessage> {
        let Some(kind) = self.clipboard.kind else {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        };
        if self.clipboard.items.is_empty() {
            self.last_action = Some("Presse-papiers vide".to_string());
            return Task::none();
        }

        let items = self.clipboard.items.clone();
        let destination = self.state.route.path.clone();
        Task::perform(
            async move {
                let operations = LocalFileOperations::new();
                match kind {
                    ClipboardKind::Copy => operations.copy_items(&items, &destination),
                    ClipboardKind::Cut => operations.move_items(&items, &destination),
                }
            },
            UiMessage::FileOperationFinished,
        )
    }

    fn open_rename_dialog(&mut self) -> Task<UiMessage> {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() != 1 {
            self.last_action = Some("Renommage : sélectionnez un seul élément".to_string());
            return Task::none();
        }
        let path = selection
            .selected
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| self.state.route.path.clone());
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        }
        self.rename_dialog = Some(RenameDialog { path, input: name });
        Task::none()
    }

    fn submit_rename(&mut self) -> Task<UiMessage> {
        let Some(dialog) = self.rename_dialog.take() else {
            return Task::none();
        };
        let trimmed = dialog.input.trim();
        if trimmed.is_empty() {
            self.last_action = Some("Renommage : nom invalide".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        if Path::new(trimmed).components().count() > 1 {
            self.last_action = Some("Renommage : le nom doit être simple".to_string());
            self.rename_dialog = Some(dialog);
            return Task::none();
        }
        let Some(parent) = dialog.path.parent() else {
            self.last_action = Some("Renommage impossible".to_string());
            return Task::none();
        };
        let target = parent.join(trimmed);
        if target == dialog.path {
            self.last_action = Some("Renommage : nom identique".to_string());
            return Task::none();
        }
        let source = dialog.path.clone();
        Task::perform(
            async move { LocalFileOperations::new().rename_item(&source, &target) },
            UiMessage::FileOperationFinished,
        )
    }

    fn delete_selection(&mut self) -> Task<UiMessage> {
        let items = self.selected_paths();
        if items.is_empty() {
            self.last_action = Some("Aucune sélection à supprimer".to_string());
            return Task::none();
        }
        self.clear_selection();
        Task::perform(
            async move { LocalFileOperations::new().delete_items(&items) },
            UiMessage::FileOperationFinished,
        )
    }

    fn copy_selection_path(&mut self) {
        let selection = &self.state.navigation.selection;
        if selection.selected.len() == 1 {
            if let Some(path) = selection.selected.iter().next() {
                self.last_action = Some(format!("Chemin copié : {}", path.display()));
                return;
            }
        }
        self.last_action = Some("Sélectionnez un élément pour copier le chemin".to_string());
    }

    fn handle_operation_report(&mut self, report: &OperationReport) {
        let success = report.succeeded.len();
        let failure = report.failed.len();
        let base_message = match report.action {
            FileOperationKind::Copy => format!("Copie : {} ok", success),
            FileOperationKind::Move => format!("Déplacement : {} ok", success),
            FileOperationKind::Rename => {
                if success == 1 {
                    report
                        .succeeded
                        .first()
                        .map(|path| format!("Renommé : {}", path.display()))
                        .unwrap_or_else(|| "Renommage terminé".to_string())
                } else {
                    format!("Renommage : {} ok", success)
                }
            }
            FileOperationKind::Delete => format!("Suppression : {} ok", success),
        };
        let full_message = if failure > 0 {
            format!("{base_message} / {failure} erreur(s)")
        } else {
            base_message
        };
        self.last_action = Some(full_message);
        if report.action == FileOperationKind::Move && failure == 0 {
            self.clipboard = ClipboardState::default();
        }
        if matches!(
            report.action,
            FileOperationKind::Rename | FileOperationKind::Delete
        ) {
            self.rename_dialog = None;
        }
    }

    fn activate_focused_entry(&mut self) -> Task<UiMessage> {
        let focused = self.state.navigation.selection.focused.clone();
        if let Some(path) = focused {
            return self.activate_entry(path);
        }
        Task::none()
    }

    fn activate_entry(&mut self, path: PathBuf) -> Task<UiMessage> {
        if let Some(entry) = self
            .entries
            .items
            .iter()
            .flatten()
            .find(|entry| entry.path == path)
        {
            if entry.entry_type == FsEntryType::Directory {
                return self.navigate_to(entry.path.clone());
            }
        }
        Task::none()
    }

    fn index_for_path(&self, path: &PathBuf) -> Option<usize> {
        self.entries.items.iter().position(|entry| {
            entry
                .as_ref()
                .map(|entry| &entry.path == path)
                .unwrap_or(false)
        })
    }

    fn normalized_search_query(&self) -> Option<String> {
        let trimmed = self.search_input.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }

    fn matches_search(entry: &FsEntry, query: &str) -> bool {
        entry.name.to_lowercase().contains(query)
    }

    fn filtered_indices(&self, query: &str) -> Vec<usize> {
        self.entries
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let entry = entry.as_ref()?;
                if Self::matches_search(entry, query) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn filtered_position_for_path(&self, indices: &[usize], path: &PathBuf) -> Option<usize> {
        indices.iter().position(|index| {
            self.entries
                .get(*index)
                .map(|entry| &entry.path == path)
                .unwrap_or(false)
        })
    }

    fn first_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().position(|entry| entry.is_some())
    }

    fn last_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().rposition(|entry| entry.is_some())
    }

    fn move_focus_by(&mut self, offset: isize, extend: bool) {
        let selection = &self.state.navigation.selection;
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            let start_index = selection
                .focused
                .as_ref()
                .and_then(|path| self.filtered_position_for_path(&indices, path))
                .or_else(|| if indices.is_empty() { None } else { Some(0) });

            let Some(start_index) = start_index else {
                return;
            };

            let target_index = if offset.is_negative() {
                start_index.saturating_sub(offset.unsigned_abs() as usize)
            } else {
                (start_index + offset as usize).min(indices.len().saturating_sub(1))
            };

            if let Some(actual_index) = indices.get(target_index).copied() {
                self.move_focus_to_actual_index(actual_index, extend);
            }
        } else {
            let start_index = selection
                .focused
                .as_ref()
                .and_then(|path| self.index_for_path(path))
                .or_else(|| self.first_entry_index());

            let Some(start_index) = start_index else {
                return;
            };

            let target_index = if offset.is_negative() {
                start_index.saturating_sub(offset.unsigned_abs() as usize)
            } else {
                (start_index + offset as usize).min(self.entries.total.saturating_sub(1))
            };

            self.move_focus_to_actual_index(target_index, extend);
        }
    }

    fn move_focus_to_start(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.first().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.first_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    fn move_focus_to_end(&mut self, extend: bool) {
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if let Some(index) = indices.last().copied() {
                self.move_focus_to_actual_index(index, extend);
            }
        } else if let Some(index) = self.last_entry_index() {
            self.move_focus_to_actual_index(index, extend);
        }
    }

    fn move_focus_to_actual_index(&mut self, index: usize, extend: bool) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let kind = if extend {
            SelectionKind::Range
        } else {
            SelectionKind::Single
        };
        self.apply_selection(entry.path.clone(), kind);
    }

    fn select_all_entries(&mut self) {
        let selected_paths: Vec<PathBuf> = if let Some(query) = self.normalized_search_query() {
            self.filtered_indices(&query)
                .into_iter()
                .filter_map(|index| self.entries.get(index).map(|entry| entry.path.clone()))
                .collect()
        } else {
            self.entries
                .items
                .iter()
                .flatten()
                .map(|entry| entry.path.clone())
                .collect()
        };

        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        for path in selected_paths {
            selection.selected.insert(path);
        }
        selection.focused = selection.selected.iter().next().cloned();
        selection.anchor = selection.focused.clone();
    }

    fn clear_selection(&mut self) {
        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        selection.focused = None;
        selection.anchor = None;
        self.context_menu_open = false;
        self.context_menu_position = None;
    }

    fn cycle_focus(&mut self) {
        self.state.navigation.focused_pane = match self.state.navigation.focused_pane {
            crate::ui::PaneKind::Tree => crate::ui::PaneKind::List,
            crate::ui::PaneKind::List => crate::ui::PaneKind::Preview,
            crate::ui::PaneKind::Preview => crate::ui::PaneKind::Tree,
        };
    }

    fn request_all_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let total_pages =
            (self.entries.total + self.entries.page_size - 1) / self.entries.page_size;
        let mut tasks = Vec::new();
        for page_index in 0..total_pages {
            if !self.entries.is_page_loaded(page_index) {
                tasks.push(self.request_page(page_index));
            }
        }
        Task::batch(tasks)
    }

    fn ensure_visible_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        if self.normalized_search_query().is_some() {
            return self.request_all_pages();
        }

        let window = self.virtual_window();
        if window.len() == 0 {
            return Task::none();
        }

        let start_page = window.start / self.entries.page_size;
        let end_page = (window.end.saturating_sub(1)) / self.entries.page_size;

        let mut tasks = Vec::new();
        for page_index in start_page..=end_page {
            if !self.entries.is_page_loaded(page_index) {
                tasks.push(self.request_page(page_index));
            }
        }

        Task::batch(tasks)
    }

    fn virtual_window(&self) -> VirtualWindow {
        self.virtual_window_for(self.entries.total)
    }

    fn virtual_window_for(&self, total: usize) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: self.state.config.view.row_height,
            viewport_height: self.viewport_height,
            overscan: self.state.config.view.overscan,
        };
        virtual_list.visible_range(self.scroll_offset, total)
    }

    fn request_visible_thumbnails(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let mut tasks = Vec::new();
        let thumbnail_size = self.state.config.view.thumbnail_size;
        if let Some(query) = self.normalized_search_query() {
            let indices = self.filtered_indices(&query);
            if indices.is_empty() {
                return Task::none();
            }
            let window = self.virtual_window_for(indices.len());
            if window.len() == 0 {
                return Task::none();
            }
            for display_index in window.start..window.end {
                let Some(actual_index) = indices.get(display_index).copied() else {
                    continue;
                };
                let Some(entry) = self.entries.get(actual_index) else {
                    continue;
                };

                if entry.entry_type != FsEntryType::File {
                    continue;
                }

                if let Some(thumbnail) = self.thumbnails.get(&entry.path) {
                    if !self.thumbnail_handles.contains_key(&entry.path) {
                        self.thumbnail_handles.insert(
                            entry.path.clone(),
                            image::Handle::from_bytes(thumbnail.bytes.clone()),
                        );
                    }
                    continue;
                }

                self.thumbnail_handles.remove(&entry.path);

                if self.thumbnail_misses.contains(&entry.path)
                    || self.thumbnails_in_flight.contains(&entry.path)
                {
                    continue;
                }

                let path = entry.path.clone();
                self.thumbnails_in_flight.insert(path.clone());
                tasks.push(Task::perform(
                    async move {
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size);
                        (path, thumbnail)
                    },
                    |(path, thumbnail)| UiMessage::ThumbnailLoaded { path, thumbnail },
                ));
            }
        } else {
            let window = self.virtual_window();
            if window.len() == 0 {
                return Task::none();
            }

            for index in window.start..window.end {
                let Some(entry) = self.entries.get(index) else {
                    continue;
                };

                if entry.entry_type != FsEntryType::File {
                    continue;
                }

                if let Some(thumbnail) = self.thumbnails.get(&entry.path) {
                    if !self.thumbnail_handles.contains_key(&entry.path) {
                        self.thumbnail_handles.insert(
                            entry.path.clone(),
                            image::Handle::from_bytes(thumbnail.bytes.clone()),
                        );
                    }
                    continue;
                }

                self.thumbnail_handles.remove(&entry.path);

                if self.thumbnail_misses.contains(&entry.path)
                    || self.thumbnails_in_flight.contains(&entry.path)
                {
                    continue;
                }

                let path = entry.path.clone();
                self.thumbnails_in_flight.insert(path.clone());
                tasks.push(Task::perform(
                    async move {
                        let thumbnail = generate_thumbnail(path.as_path(), thumbnail_size);
                        (path, thumbnail)
                    },
                    |(path, thumbnail)| UiMessage::ThumbnailLoaded { path, thumbnail },
                ));
            }
        }

        Task::batch(tasks)
    }

    fn view(&self) -> Element<'_, UiMessage> {
        let tokens = UiTokens::default();
        let colors = tokens.colors;
        let spacing = tokens.spacing;
        let typography = tokens.typography;
        let user_dirs = UserDirs::new();
        let home_dir = user_dirs.as_ref().map(|dirs| dirs.home_dir().to_path_buf());
        let desktop_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.desktop_dir().map(|path| path.to_path_buf()));
        let downloads_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.download_dir().map(|path| path.to_path_buf()));
        let documents_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.document_dir().map(|path| path.to_path_buf()));
        let pictures_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.picture_dir().map(|path| path.to_path_buf()));
        let music_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.audio_dir().map(|path| path.to_path_buf()));
        let video_dir = user_dirs
            .as_ref()
            .and_then(|dirs| dirs.video_dir().map(|path| path.to_path_buf()));

        let toolbar_button = |label: String| {
            button(text(label).size(typography.body).font(typography.body_font))
                .padding([spacing.xs, spacing.sm])
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };

                    match status {
                        ButtonStatus::Hovered => {
                            style.background = Some(Background::Color(colors.hover));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        ButtonStatus::Pressed => {
                            style.background = Some(Background::Color(colors.pressed));
                            style.border = border::rounded(6.0).color(colors.border).width(1.0);
                        }
                        ButtonStatus::Disabled => {
                            style.text_color = Color::from_rgb8(150, 150, 150);
                        }
                        ButtonStatus::Active => {}
                    }

                    style
                })
        };

        let sidebar_button =
            |icon: &str, label: &str, target: Option<PathBuf>| -> Element<'_, UiMessage> {
                let icon = icon.to_string();
                let label = label.to_string();
                let content: Element<'_, UiMessage> = row![
                    text(icon).size(typography.body).font(typography.body_font),
                    text(label).size(typography.body).font(typography.body_font)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center)
                .into();

                match target {
                    Some(path) => button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            match status {
                                ButtonStatus::Hovered => {
                                    style.background = Some(Background::Color(colors.hover));
                                    style.border =
                                        border::rounded(6.0).color(colors.border).width(1.0);
                                }
                                ButtonStatus::Pressed => {
                                    style.background = Some(Background::Color(colors.pressed));
                                }
                                ButtonStatus::Active | ButtonStatus::Disabled => {}
                            }

                            style
                        })
                        .on_press(UiMessage::NavigateTo(path))
                        .into(),
                    None => container(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .into(),
                }
            };

        let section_title = |label: String| {
            text(label)
                .size(typography.caption)
                .font(typography.caption_font)
                .style(move |_| iced::widget::text::Style {
                    color: Some(colors.text_muted),
                })
        };

        let format_sidebar_label = |path: &PathBuf| {
            path.file_name()
                .and_then(|name| name.to_str())
                .filter(|label| !label.is_empty())
                .map(|label| label.to_string())
                .unwrap_or_else(|| path.display().to_string())
        };

        let tab_button = |label: String, active: bool| {
            button(text(label).size(typography.body).font(typography.body_font))
                .padding([spacing.xs, spacing.md])
                .style(move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: colors.text_primary,
                        ..Default::default()
                    };

                    if active {
                        style.background = Some(Background::Color(colors.panel_background));
                        style.border = border::rounded(8.0).color(colors.border).width(1.0);
                    }

                    if matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(colors.hover));
                    }

                    style
                })
        };

        let back_button = if self.history.can_back() {
            toolbar_button(ICON_BACK.to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button(ICON_BACK.to_string())
        };

        let forward_button = if self.history.can_forward() {
            toolbar_button(ICON_FORWARD.to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button(ICON_FORWARD.to_string())
        };

        let refresh_button = toolbar_button(ICON_REFRESH.to_string()).on_press(UiMessage::Refresh);

        let navigation = row![back_button, forward_button, refresh_button].spacing(spacing.sm);

        let mut tabs = row![];
        for (index, tab) in self.tabs.iter().enumerate() {
            let label = format!("{} {}", ICON_PC, tab.title);
            let mut button = tab_button(label, index == self.active_tab);
            if index != self.active_tab {
                button = button.on_press(UiMessage::SwitchTab(index));
            }
            let mut tab_row = row![button].spacing(spacing.xs).align_y(Alignment::Center);
            if index != 0 {
                tab_row = tab_row.push(
                    tab_button(ICON_CLOSE.to_string(), false).on_press(UiMessage::CloseTab(index)),
                );
            }
            tabs = tabs.push(tab_row);
        }
        tabs = tabs.push(tab_button(ICON_NEW.to_string(), false).on_press(UiMessage::AddTab));
        let tabs = tabs.spacing(spacing.sm);

        let address_input = text_input("Chemin…", &self.address_input)
            .on_input(UiMessage::AddressInputChanged)
            .on_submit(UiMessage::AddressInputSubmitted)
            .size(typography.body)
            .font(typography.body_font)
            .padding([spacing.xs, spacing.md]);

        let address_bar = container(address_input)
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            });

        let address_validation = self.address_target_from_input().map(|target| {
            if target.is_dir() {
                ("Dossier".to_string(), Color::from_rgb8(55, 125, 60))
            } else if target.exists() {
                ("Fichier".to_string(), Color::from_rgb8(186, 120, 40))
            } else {
                ("Introuvable".to_string(), Color::from_rgb8(176, 72, 72))
            }
        });

        let address_status: Element<'_, UiMessage> =
            if let Some((label, status_color)) = address_validation {
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(status_color),
                        }),
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(999.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
            } else {
                container(row![]).into()
            };

        let suggestion_button = |label: String, target: PathBuf| {
            button(
                text(label)
                    .size(typography.caption)
                    .font(typography.body_font),
            )
            .padding([spacing.xs, spacing.sm])
            .width(Length::Fill)
            .style(move |_theme: &Theme, status: ButtonStatus| {
                let mut style = iced::widget::button::Style {
                    text_color: colors.text_primary,
                    ..Default::default()
                };

                match status {
                    ButtonStatus::Hovered => {
                        style.background = Some(Background::Color(colors.hover));
                        style.border = border::rounded(6.0).color(colors.border).width(1.0);
                    }
                    ButtonStatus::Pressed => {
                        style.background = Some(Background::Color(colors.pressed));
                        style.border = border::rounded(6.0).color(colors.border).width(1.0);
                    }
                    ButtonStatus::Active | ButtonStatus::Disabled => {}
                }

                style
            })
            .on_press(UiMessage::AddressSuggestionSelected(target))
        };

        let address_suggestions = self.address_suggestions();
        let history_button = toolbar_button("▼".to_string())
            .on_press(UiMessage::ToggleHistoryMenu(!self.history_menu_open));

        let history_menu: Option<Element<'_, UiMessage>> =
            if self.history_menu_open && !address_suggestions.is_empty() {
                let mut suggestions_list = column![
                    text("Historique")
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        })
                ]
                .spacing(spacing.xs);
                for suggestion in address_suggestions {
                    suggestions_list = suggestions_list.push(suggestion_button(
                        suggestion.display().to_string(),
                        suggestion,
                    ));
                }
                let position = self.history_menu_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);
                let menu = container(suggestions_list)
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fixed(420.0))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    });
                let menu_layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(menu)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                let dismiss_layer: Element<'_, UiMessage> =
                    mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                        .on_press(UiMessage::ToggleHistoryMenu(false))
                        .into();
                Some(stack![dismiss_layer, menu_layer].into())
            } else {
                None
            };

        let address_row = row![address_bar, history_button, address_status]
            .spacing(spacing.xs)
            .align_y(Alignment::Center);

        let address_section: Element<'_, UiMessage> =
            container(address_row).width(Length::Fill).into();

        let active_tab_title = self
            .tabs
            .get(self.active_tab)
            .map(|tab| tab.title.as_str())
            .unwrap_or("Ce PC");
        let search_input = text_input(
            &format!("Rechercher dans : {}", active_tab_title),
            &self.search_input,
        )
        .on_input(UiMessage::SearchInputChanged)
        .on_submit(UiMessage::SearchInputSubmitted)
        .size(typography.caption)
        .font(typography.caption_font)
        .padding([spacing.xs, spacing.sm])
        .width(Length::Fill);
        let search_bar = container(
            row![
                text(ICON_SEARCH)
                    .size(typography.caption)
                    .font(typography.caption_font),
                search_input
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .width(Length::Fixed(240.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(6.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let has_selection = !self.state.navigation.selection.selected.is_empty();
        let has_clipboard = self.clipboard.kind.is_some() && !self.clipboard.items.is_empty();

        let cut_button = if has_selection {
            toolbar_button(format!("{} Couper", ICON_CUT)).on_press(UiMessage::ClipboardCut)
        } else {
            toolbar_button(format!("{} Couper", ICON_CUT))
        };

        let copy_button = if has_selection {
            toolbar_button(format!("{} Copier", ICON_COPY)).on_press(UiMessage::ClipboardCopy)
        } else {
            toolbar_button(format!("{} Copier", ICON_COPY))
        };

        let paste_button = if has_clipboard {
            toolbar_button(format!("{} Coller", ICON_PASTE)).on_press(UiMessage::ClipboardPaste)
        } else {
            toolbar_button(format!("{} Coller", ICON_PASTE))
        };

        let command_bar = row![
            toolbar_button(format!("{} Nouveau", ICON_NEW)),
            cut_button,
            copy_button,
            paste_button,
            toolbar_button(format!("{} Trier", ICON_SORT)),
            toolbar_button(format!("{} Afficher", ICON_VIEW)),
            toolbar_button(ICON_MORE.to_string()),
            toolbar_button(format!("{} Actions", ICON_ACTIONS))
                .on_press(UiMessage::ToggleContextMenu(!self.context_menu_open))
        ]
        .spacing(spacing.sm);

        let loading_badge: Element<'_, UiMessage> = if self.show_loading_indicator {
            let content: Element<'_, UiMessage> = row![
                text("Chargement…")
                    .size(typography.caption)
                    .font(typography.caption_font),
                progress_bar(0.0..=1.0, 0.5)
                    .height(Length::Fixed(4.0))
                    .width(Length::Fixed(64.0)),
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center)
            .into();

            container(content)
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.hover)),
                    border: border::rounded(999.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            container(row![]).into()
        };

        let context_actions = column![
            toolbar_button(format!("{} Ouvrir", ICON_OPEN))
                .on_press(UiMessage::ContextAction(ContextAction::Open,)),
            toolbar_button(format!("{} Renommer", ICON_RENAME))
                .on_press(UiMessage::ContextAction(ContextAction::Rename,)),
            toolbar_button(format!("{} Supprimer", ICON_DELETE))
                .on_press(UiMessage::ContextAction(ContextAction::Delete,)),
            toolbar_button(format!("{} Copier le chemin", ICON_SYMLINK))
                .on_press(UiMessage::ContextAction(ContextAction::CopyPath),)
        ]
        .spacing(spacing.sm);

        let context_menu: Option<Element<'_, UiMessage>> =
            if self.context_menu_open && !self.state.navigation.selection.selected.is_empty() {
                let position = self.context_menu_position.unwrap_or(Point::ORIGIN);
                let position_x = position.x.max(0.0);
                let position_y = position.y.max(0.0);
                let menu = container(context_actions)
                    .padding([spacing.sm, spacing.md])
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(colors.panel_background)),
                        border: border::rounded(8.0).color(colors.border).width(1.0),
                        ..Default::default()
                    });
                let menu_layer: Element<'_, UiMessage> = container(
                    column![
                        vertical_space().height(Length::Fixed(position_y)),
                        row![
                            horizontal_space().width(Length::Fixed(position_x)),
                            opaque(menu)
                        ]
                    ]
                    .spacing(0),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
                let dismiss_layer: Element<'_, UiMessage> =
                    mouse_area(container(row![]).width(Length::Fill).height(Length::Fill))
                        .on_press(UiMessage::ToggleContextMenu(false))
                        .into();
                Some(stack![dismiss_layer, menu_layer].into())
            } else {
                None
            };

        let header = container(
            column![
                row![tabs].spacing(8).align_y(Alignment::Center),
                row![navigation, address_section, search_bar, loading_badge]
                    .spacing(spacing.md)
                    .align_y(Alignment::Center),
                command_bar
            ]
            .spacing(spacing.sm),
        )
        .padding(iced::Padding {
            top: spacing.sm + 2.0,
            right: spacing.md,
            bottom: spacing.sm,
            left: spacing.md,
        })
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.chrome_background)),
            border: border::rounded(10.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let display_entries = if self.is_refreshing && self.entries.total == 0 {
            self.stale_entries.as_ref().unwrap_or(&self.entries)
        } else {
            &self.entries
        };

        let column_specs = column_specs(&self.state.config.view.columns);
        let search_query = self.normalized_search_query();
        let filtered_indices = search_query.as_ref().map(|query| {
            display_entries
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    let entry = entry.as_ref()?;
                    if Self::matches_search(entry, query) {
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier")
                    .size(typography.title)
                    .font(typography.title_font),
                text(message)
                    .size(typography.body)
                    .font(typography.body_font),
                button(
                    text("Réessayer")
                        .size(typography.body)
                        .font(typography.body_font),
                )
                .on_press(UiMessage::Refresh)
            ]
            .spacing(spacing.sm)
        } else if let Some(indices) = &filtered_indices {
            if indices.is_empty() && !self.is_loading {
                column![
                    text("Aucun résultat")
                        .size(typography.body)
                        .font(typography.body_font)
                ]
            } else if indices.is_empty() {
                column![]
            } else {
                let window = self.virtual_window_for(indices.len());
                let mut list = column![];

                if window.padding_top > 0.0 {
                    list = list.push(vertical_space().height(Length::Fixed(window.padding_top)));
                }

                for display_index in window.start..window.end {
                    let Some(actual_index) = indices.get(display_index).copied() else {
                        continue;
                    };
                    let entry = display_entries.get(actual_index);
                    if let Some(entry) = entry {
                        let is_selected = self
                            .state
                            .navigation
                            .selection
                            .selected
                            .contains(&entry.path);
                        let is_focused = self
                            .state
                            .navigation
                            .selection
                            .focused
                            .as_ref()
                            .map(|path| path == &entry.path)
                            .unwrap_or(false);
                        let mut entry_row = row![].spacing(spacing.md).align_y(Alignment::Center);
                        for spec in &column_specs {
                            let cell: Element<'_, UiMessage> = match spec.column {
                                ViewColumn::Name => {
                                    let leading: Element<'_, UiMessage> = match entry.entry_type {
                                        FsEntryType::Directory => text(ICON_FOLDER)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                        FsEntryType::File => self
                                            .thumbnail_handles
                                            .get(&entry.path)
                                            .map(|handle| {
                                                image(handle.clone())
                                                    .width(Length::Fixed(
                                                        self.state.config.view.thumbnail_size
                                                            as f32,
                                                    ))
                                                    .height(Length::Fixed(
                                                        self.state.config.view.thumbnail_size
                                                            as f32,
                                                    ))
                                                    .into()
                                            })
                                            .unwrap_or_else(|| {
                                                text(ICON_FILE)
                                                    .size(typography.body)
                                                    .font(typography.body_font)
                                                    .into()
                                            }),
                                        FsEntryType::Symlink => text(ICON_SYMLINK)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                        FsEntryType::Other => text(ICON_UNKNOWN)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                    };
                                    let name_row = row![
                                        leading,
                                        text(&entry.name)
                                            .size(typography.body)
                                            .font(typography.body_font),
                                        horizontal_space()
                                    ]
                                    .spacing(spacing.sm)
                                    .align_y(Alignment::Center);
                                    container(name_row)
                                        .width(spec.width)
                                        .align_x(spec.align)
                                        .into()
                                }
                                ViewColumn::Type => container(
                                    text(entry_type_label(entry.entry_type))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                                ViewColumn::Size => container(
                                    text(format_entry_size(entry))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                                ViewColumn::Modified => container(
                                    text(format_modified(entry.metadata.modified))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                            };
                            entry_row = entry_row.push(cell);
                        }
                        let selection_kind = self.selection_kind_from_modifiers();
                        let message = UiMessage::SelectEntry {
                            path: entry.path.clone(),
                            kind: selection_kind,
                        };
                        let context_path = entry.path.clone();
                        list = list.push(
                            mouse_area(
                                button(entry_row)
                                    .padding([spacing.xs, spacing.sm])
                                    .style(move |_theme: &Theme, status: ButtonStatus| {
                                        let mut style = iced::widget::button::Style {
                                            text_color: colors.text_primary,
                                            ..Default::default()
                                        };

                                        if is_selected {
                                            style.background =
                                                Some(Background::Color(colors.selection));
                                            style.border = border::rounded(6.0)
                                                .color(colors.selection_border)
                                                .width(if is_focused { 2.0 } else { 1.0 });
                                        }

                                        if matches!(status, ButtonStatus::Hovered) {
                                            style.background =
                                                Some(Background::Color(colors.hover));
                                        }

                                        if matches!(status, ButtonStatus::Pressed) {
                                            style.background =
                                                Some(Background::Color(colors.pressed));
                                        }

                                        style
                                    })
                                    .on_press(message),
                            )
                            .on_right_press(UiMessage::OpenContextMenuForEntry(context_path)),
                        );
                    }
                }

                if window.padding_bottom > 0.0 {
                    list = list.push(vertical_space().height(Length::Fixed(window.padding_bottom)));
                }

                list
            }
        } else if display_entries.total == 0 && !self.is_loading {
            column![
                text("Dossier vide")
                    .size(typography.body)
                    .font(typography.body_font)
            ]
        } else if display_entries.total == 0 {
            column![]
        } else {
            let window = self.virtual_window_for(display_entries.total);
            let mut list = column![];

            if window.padding_top > 0.0 {
                list = list.push(vertical_space().height(Length::Fixed(window.padding_top)));
            }

            for index in window.start..window.end {
                let entry = display_entries.get(index);
                let entry_element: Element<'_, UiMessage> = match entry {
                    Some(entry) => {
                        let is_selected = self
                            .state
                            .navigation
                            .selection
                            .selected
                            .contains(&entry.path);
                        let is_focused = self
                            .state
                            .navigation
                            .selection
                            .focused
                            .as_ref()
                            .map(|path| path == &entry.path)
                            .unwrap_or(false);
                        let mut entry_row = row![].spacing(spacing.md).align_y(Alignment::Center);
                        for spec in &column_specs {
                            let cell: Element<'_, UiMessage> = match spec.column {
                                ViewColumn::Name => {
                                    let leading: Element<'_, UiMessage> = match entry.entry_type {
                                        FsEntryType::Directory => text(ICON_FOLDER)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                        FsEntryType::File => self
                                            .thumbnail_handles
                                            .get(&entry.path)
                                            .map(|handle| {
                                                image(handle.clone())
                                                    .width(Length::Fixed(
                                                        self.state.config.view.thumbnail_size
                                                            as f32,
                                                    ))
                                                    .height(Length::Fixed(
                                                        self.state.config.view.thumbnail_size
                                                            as f32,
                                                    ))
                                                    .into()
                                            })
                                            .unwrap_or_else(|| {
                                                text(ICON_FILE)
                                                    .size(typography.body)
                                                    .font(typography.body_font)
                                                    .into()
                                            }),
                                        FsEntryType::Symlink => text(ICON_SYMLINK)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                        FsEntryType::Other => text(ICON_UNKNOWN)
                                            .size(typography.body)
                                            .font(typography.body_font)
                                            .into(),
                                    };
                                    let name_row = row![
                                        leading,
                                        text(&entry.name)
                                            .size(typography.body)
                                            .font(typography.body_font),
                                        horizontal_space()
                                    ]
                                    .spacing(spacing.sm)
                                    .align_y(Alignment::Center);
                                    container(name_row)
                                        .width(spec.width)
                                        .align_x(spec.align)
                                        .into()
                                }
                                ViewColumn::Type => container(
                                    text(entry_type_label(entry.entry_type))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                                ViewColumn::Size => container(
                                    text(format_entry_size(entry))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                                ViewColumn::Modified => container(
                                    text(format_modified(entry.metadata.modified))
                                        .size(typography.caption)
                                        .font(typography.caption_font),
                                )
                                .width(spec.width)
                                .align_x(spec.align)
                                .into(),
                            };
                            entry_row = entry_row.push(cell);
                        }
                        let selection_kind = self.selection_kind_from_modifiers();
                        let message = UiMessage::SelectEntry {
                            path: entry.path.clone(),
                            kind: selection_kind,
                        };
                        let context_path = entry.path.clone();
                        mouse_area(
                            button(entry_row)
                                .padding([spacing.xs, spacing.sm])
                                .style(move |_theme: &Theme, status: ButtonStatus| {
                                    let mut style = iced::widget::button::Style {
                                        text_color: colors.text_primary,
                                        ..Default::default()
                                    };

                                    if is_selected {
                                        style.background =
                                            Some(Background::Color(colors.selection));
                                        style.border = border::rounded(6.0)
                                            .color(colors.selection_border)
                                            .width(if is_focused { 2.0 } else { 1.0 });
                                    }

                                    if matches!(status, ButtonStatus::Hovered) {
                                        style.background = Some(Background::Color(colors.hover));
                                    }

                                    if matches!(status, ButtonStatus::Pressed) {
                                        style.background = Some(Background::Color(colors.pressed));
                                    }

                                    style
                                })
                                .on_press(message),
                        )
                        .on_right_press(UiMessage::OpenContextMenuForEntry(context_path))
                        .into()
                    }
                    None => {
                        let mut placeholder_row =
                            row![].spacing(spacing.md).align_y(Alignment::Center);
                        for (index, spec) in column_specs.iter().enumerate() {
                            let cell: Element<'_, UiMessage> = if index == 0 {
                                let content = row![
                                    text(ICON_LOADING)
                                        .size(typography.body)
                                        .font(typography.body_font),
                                    text("Chargement…")
                                        .size(typography.body)
                                        .font(typography.body_font)
                                ]
                                .spacing(spacing.sm)
                                .align_y(Alignment::Center);
                                container(content)
                                    .width(spec.width)
                                    .align_x(spec.align)
                                    .into()
                            } else {
                                container(row![])
                                    .width(spec.width)
                                    .align_x(spec.align)
                                    .into()
                            };
                            placeholder_row = placeholder_row.push(cell);
                        }
                        button(placeholder_row).into()
                    }
                };
                list = list.push(entry_element);
            }

            if window.padding_bottom > 0.0 {
                list = list.push(vertical_space().height(Length::Fixed(window.padding_bottom)));
            }

            list
        };

        let list_header: Element<'_, UiMessage> = if self.error.is_none()
            && if let Some(indices) = &filtered_indices {
                !indices.is_empty()
            } else {
                display_entries.total > 0 || self.is_loading
            } {
            let mut header_row = row![].spacing(spacing.md).align_y(Alignment::Center);
            for spec in &column_specs {
                let is_active_sort = spec
                    .sort_key
                    .is_some_and(|key| key == self.state.config.list.sort_key);
                let sort_indicator = if is_active_sort {
                    match self.state.config.list.sort_order {
                        SortOrderConfig::Asc => "↑",
                        SortOrderConfig::Desc => "↓",
                    }
                } else {
                    ""
                };
                let label = if sort_indicator.is_empty() {
                    spec.label.to_string()
                } else {
                    format!("{} {}", spec.label, sort_indicator)
                };
                let header_text = text(label)
                    .size(typography.caption)
                    .font(typography.caption_font);
                let cell: Element<'_, UiMessage> = if let Some(sort_key) = spec.sort_key {
                    button(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            if matches!(status, ButtonStatus::Hovered) {
                                style.background = Some(Background::Color(colors.hover));
                            }

                            style
                        })
                        .on_press(UiMessage::ChangeSort(sort_key))
                        .into()
                } else {
                    container(header_text)
                        .padding([spacing.xs, spacing.sm])
                        .into()
                };
                header_row = header_row.push(container(cell).width(spec.width).align_x(spec.align));
            }

            container(header_row)
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.chrome_background)),
                    border: border::rounded(6.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
                .into()
        } else {
            container(row![]).into()
        };

        let rename_prompt = if let Some(dialog) = &self.rename_dialog {
            let input = text_input("Nouveau nom…", &dialog.input)
                .on_input(UiMessage::RenameInputChanged)
                .on_submit(UiMessage::RenameSubmit)
                .size(typography.body)
                .font(typography.body_font)
                .padding([spacing.xs, spacing.md]);
            container(
                row![
                    text("Renommer :")
                        .size(typography.body)
                        .font(typography.body_font),
                    input,
                    toolbar_button("Valider".to_string()).on_press(UiMessage::RenameSubmit),
                    toolbar_button("Annuler".to_string()).on_press(UiMessage::RenameCancel)
                ]
                .spacing(spacing.sm)
                .align_y(Alignment::Center),
            )
            .padding([spacing.sm, spacing.md])
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(8.0).color(colors.border).width(1.0),
                ..Default::default()
            })
        } else {
            container(row![])
        };

        let list_column = column![rename_prompt, list_header, list_content].spacing(spacing.xl);

        let list = scrollable(container(list_column).padding(spacing.md)).on_scroll(|viewport| {
            UiMessage::Scroll(ScrollViewport {
                offset_y: viewport.absolute_offset().y,
                viewport_height: viewport.bounds().height,
                content_height: viewport.content_bounds().height,
            })
        });

        let tree_root = root_path_for(&self.state.route.path)
            .unwrap_or_else(|| self.state.route.path.clone());
        let tree_options = ListOptions {
            show_hidden: self.state.config.list.show_hidden,
            sort_by: SortKey::Name,
            sort_order: SortOrder::Asc,
            directories_first: true,
            filter: EntryFilter::OnlyDirectories,
            name_query: None,
        };
        let tree_nodes = build_tree_nodes(
            &LocalFileSystem::new(),
            &tree_root,
            &self.state.route.path,
            TREE_MAX_DEPTH,
            &tree_options,
        );

        let mut tree_section =
            column![section_title("Arborescence".to_string())].spacing(spacing.xs);
        if tree_nodes.is_empty() {
            tree_section = tree_section.push(
                text("Arborescence indisponible")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    }),
            );
        } else {
            for node in tree_nodes {
                let label = node.label.clone();
                let path = node.path.clone();
                let selected = node.selected;
                let depth = node.depth;
                let expanded = node.expanded;
                let icon = if depth == 0 { ICON_PC } else { ICON_FOLDER };
                let chevron = if expanded { "▾" } else { "▸" };
                let indent = horizontal_space()
                    .width(Length::Fixed(depth as f32 * (spacing.sm + 2.0)));
                let content: Element<'_, UiMessage> = row![
                    indent,
                    text(chevron)
                        .size(typography.caption)
                        .font(typography.caption_font),
                    text(icon).size(typography.body).font(typography.body_font),
                    text(label).size(typography.body).font(typography.body_font)
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center)
                .into();

                let button = button(content)
                    .padding([spacing.xs, spacing.sm])
                    .width(Length::Fill)
                    .style(move |_theme: &Theme, status: ButtonStatus| {
                        let mut style = iced::widget::button::Style {
                            text_color: colors.text_primary,
                            ..Default::default()
                        };

                        if selected {
                            style.background = Some(Background::Color(colors.selection));
                            style.border =
                                border::rounded(6.0).color(colors.selection_border).width(1.0);
                        }

                        if matches!(status, ButtonStatus::Hovered) {
                            style.background = Some(Background::Color(colors.hover));
                        }

                        if matches!(status, ButtonStatus::Pressed) {
                            style.background = Some(Background::Color(colors.pressed));
                        }

                        style
                    })
                    .on_press(UiMessage::NavigateTo(path));

                tree_section = tree_section.push(button);
            }
        }

        let mut quick_access =
            column![section_title("Accès rapide".to_string())].spacing(spacing.xs);
        quick_access = quick_access.push(sidebar_button(ICON_HOME, "Accueil", home_dir.clone()));
        quick_access =
            quick_access.push(sidebar_button(ICON_DESKTOP, "Bureau", desktop_dir.clone()));
        quick_access = quick_access.push(sidebar_button(
            ICON_DOWNLOAD,
            "Téléchargements",
            downloads_dir,
        ));

        let mut favorites_section =
            column![section_title("Favoris".to_string())].spacing(spacing.xs);
        if self.favorites.list().is_empty() {
            favorites_section = favorites_section.push(
                text("Aucun favori")
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        } else {
            for favorite in self.favorites.list() {
                let label = format_sidebar_label(favorite);
                favorites_section = favorites_section.push(sidebar_button(
                    ICON_FOLDER,
                    &label,
                    Some(favorite.clone()),
                ));
            }
        }

        let mut drive_section = column![section_title("Lecteurs".to_string())].spacing(spacing.xs);
        if let Some(root_path) = root_path_for(&self.state.route.path) {
            if let Some(usage) = disk_usage_for(&root_path) {
                let total_gb = format_gigabytes(usage.total);
                let free_gb = format_gigabytes(usage.available);
                let used_ratio = if usage.total == 0 {
                    0.0
                } else {
                    1.0 - (usage.available as f32 / usage.total as f32)
                };
                let content: Element<'_, UiMessage> = column![
                    row![
                        text(ICON_DEVICE)
                            .size(typography.caption)
                            .font(typography.caption_font),
                        text(drive_label(&root_path))
                            .size(typography.caption)
                            .font(typography.caption_font)
                    ]
                    .spacing(spacing.xs)
                    .align_y(Alignment::Center),
                    progress_bar(0.0..=1.0, used_ratio).height(Length::Fixed(6.0)),
                    text(format!("{} Go libres sur {} Go", free_gb, total_gb))
                        .size(typography.caption)
                        .font(typography.caption_font)
                ]
                .spacing(spacing.xs)
                .into();

                drive_section = drive_section.push(
                    button(content)
                        .padding([spacing.xs, spacing.sm])
                        .width(Length::Fill)
                        .style(move |_theme: &Theme, status: ButtonStatus| {
                            let mut style = iced::widget::button::Style {
                                text_color: colors.text_primary,
                                ..Default::default()
                            };

                            match status {
                                ButtonStatus::Hovered => {
                                    style.background = Some(Background::Color(colors.hover));
                                    style.border =
                                        border::rounded(6.0).color(colors.border).width(1.0);
                                }
                                ButtonStatus::Pressed => {
                                    style.background = Some(Background::Color(colors.pressed));
                                }
                                ButtonStatus::Active | ButtonStatus::Disabled => {}
                            }

                            style
                        })
                        .on_press(UiMessage::NavigateTo(root_path)),
                );
            } else {
                let label = format_sidebar_label(&root_path);
                drive_section =
                    drive_section.push(sidebar_button(ICON_DRIVE, &label, Some(root_path)));
            }
        } else {
            drive_section = drive_section.push(
                text("Aucun lecteur")
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        }
        drive_section = drive_section.push(sidebar_button(ICON_NETWORK, "Réseau", None));

        let sidebar = container(
            column![
                tree_section,
                quick_access,
                favorites_section,
                drive_section,
                section_title("Raccourcis".to_string()),
                sidebar_button(ICON_DOCUMENTS, "Documents", documents_dir),
                sidebar_button(ICON_GALLERY, "Images", pictures_dir),
                sidebar_button(ICON_MUSIC, "Musique", music_dir),
                sidebar_button(ICON_VIDEO, "Vidéos", video_dir)
            ]
            .spacing(spacing.sm),
        )
        .padding(spacing.md)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.sidebar_background)),
            border: Border {
                color: colors.border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        let preview_row = |label: String, value: String| {
            row![
                container(
                    text(label)
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                )
                .width(Length::Fixed(90.0)),
                text(value)
                    .size(typography.caption)
                    .font(typography.caption_font)
            ]
            .spacing(spacing.xs)
            .align_y(Alignment::Center)
        };

        let preview_entry = self.selected_entry(display_entries);
        let preview_body: Element<'_, UiMessage> = match preview_entry {
            Some(entry) => {
                let icon = match entry.entry_type {
                    FsEntryType::Directory => ICON_FOLDER,
                    FsEntryType::File => ICON_FILE,
                    FsEntryType::Symlink => ICON_SYMLINK,
                    FsEntryType::Other => ICON_UNKNOWN,
                };
                let preview_media_size = (self.state.config.view.thumbnail_size as f32 * 3.0)
                    .max(120.0)
                    .min(220.0);
                let preview_media: Element<'_, UiMessage> = match entry.entry_type {
                    FsEntryType::File => self
                        .thumbnail_handles
                        .get(&entry.path)
                        .map(|handle| {
                            image(handle.clone())
                                .width(Length::Fixed(preview_media_size))
                                .height(Length::Fixed(preview_media_size))
                                .into()
                        })
                        .unwrap_or_else(|| {
                            text(icon)
                                .size(typography.title)
                                .font(typography.title_font)
                                .into()
                        }),
                    _ => text(icon)
                        .size(typography.title)
                        .font(typography.title_font)
                        .into(),
                };

                let metadata = column![
                    preview_row(
                        "Type".to_string(),
                        entry_type_label(entry.entry_type).to_string()
                    ),
                    preview_row("Taille".to_string(), format_entry_size(entry)),
                    preview_row(
                        "Modifié".to_string(),
                        format_modified(entry.metadata.modified),
                    ),
                    preview_row("Créé".to_string(), format_modified(entry.metadata.created)),
                    preview_row(
                        "Accès".to_string(),
                        format_modified(entry.metadata.accessed)
                    ),
                    preview_row(
                        "Lecture seule".to_string(),
                        if entry.metadata.readonly {
                            "Oui".to_string()
                        } else {
                            "Non".to_string()
                        },
                    ),
                ]
                .spacing(spacing.xs);

                column![
                    preview_media,
                    text(&entry.name)
                        .size(typography.body)
                        .font(typography.body_font),
                    text(entry.path.display().to_string())
                        .size(typography.caption)
                        .font(typography.caption_font)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(colors.text_muted),
                        }),
                    metadata
                ]
                .spacing(spacing.sm)
                .align_x(Alignment::Center)
                .into()
            }
            None if self.state.navigation.selection.selected.is_empty() => column![
                text("Sélectionnez un élément")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
            None => column![
                text("Aperçu en cours de chargement…")
                    .size(typography.caption)
                    .font(typography.caption_font)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(colors.text_muted),
                    })
            ]
            .align_x(Alignment::Center)
            .spacing(spacing.sm)
            .into(),
        };

        let preview_panel = container(
            column![section_title("Prévisualisation".to_string()), preview_body]
                .spacing(spacing.md),
        )
        .padding(spacing.md)
        .width(Length::Fixed(280.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(10.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let body = row![
            sidebar.width(Length::Fixed(220.0)),
            container(list)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(10.0).color(colors.border).width(1.0),
                    ..Default::default()
                }),
            preview_panel
        ]
        .height(Length::Fill)
        .spacing(spacing.md);

        let selection = &self.state.navigation.selection;
        let selection_status = if selection.selected.is_empty() {
            "Aucune sélection".to_string()
        } else if selection.selected.len() == 1 {
            let path = selection
                .selected
                .iter()
                .next()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".to_string());
            format!("Sélection : {}", path)
        } else {
            format!("Sélection : {} éléments", selection.selected.len())
        };

        let entry_count = if let Some(indices) = &filtered_indices {
            indices.len()
        } else {
            display_entries.total
        };
        let entries_status = if entry_count == 0 {
            "Aucun élément".to_string()
        } else if entry_count == 1 {
            "1 élément".to_string()
        } else {
            format!("{} éléments", entry_count)
        };

        let mut status_left = row![
            text(selection_status)
                .size(typography.caption)
                .font(typography.caption_font),
            text(entries_status)
                .size(typography.caption)
                .font(typography.caption_font)
                .style(move |_| iced::widget::text::Style {
                    color: Some(colors.text_muted),
                })
        ]
        .spacing(spacing.md)
        .align_y(Alignment::Center);

        if self.is_loading {
            status_left = status_left.push(
                row![
                    text(ICON_LOADING)
                        .size(typography.caption)
                        .font(typography.caption_font),
                    text("Chargement…")
                        .size(typography.caption)
                        .font(typography.caption_font)
                ]
                .spacing(spacing.xs)
                .align_y(Alignment::Center),
            );
        }

        let mut status_right = row![].spacing(spacing.md).align_y(Alignment::Center);
        if let Some(status) = self.last_action.clone() {
            status_right = status_right.push(
                text(status)
                    .size(typography.caption)
                    .font(typography.caption_font),
            );
        }

        let status_bar = container(
            row![status_left, horizontal_space(), status_right].align_y(Alignment::Center),
        )
        .padding([spacing.xs, spacing.md])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(8.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let content = column![header, body, status_bar]
            .spacing(spacing.md)
            .padding(spacing.lg)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        let base: Element<'_, UiMessage> = container(content)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                ..Default::default()
            })
            .into();

        let mut layered: Element<'_, UiMessage> = base;
        if let Some(menu) = history_menu {
            layered = stack![layered, menu].into();
        }
        if let Some(menu) = context_menu {
            layered = stack![layered, menu].into();
        }
        layered
    }
}

struct ColumnSpec {
    column: ViewColumn,
    label: &'static str,
    width: Length,
    align: Horizontal,
    sort_key: Option<SortKeyConfig>,
}

fn column_specs(columns: &[ViewColumn]) -> Vec<ColumnSpec> {
    let mut specs = columns
        .iter()
        .map(|column| ColumnSpec {
            column: column.clone(),
            label: column_label(column),
            width: column_width(column),
            align: column_alignment(column),
            sort_key: column_sort_key(column),
        })
        .collect::<Vec<_>>();

    let has_fill = specs
        .iter()
        .any(|spec| matches!(spec.width, Length::Fill | Length::FillPortion(_)));
    if !has_fill {
        if let Some(first) = specs.first_mut() {
            first.width = Length::Fill;
        }
    }

    specs
}

fn column_label(column: &ViewColumn) -> &'static str {
    match column {
        ViewColumn::Name => "Nom",
        ViewColumn::Type => "Type",
        ViewColumn::Size => "Taille",
        ViewColumn::Modified => "Modifié",
    }
}

fn column_width(column: &ViewColumn) -> Length {
    match column {
        ViewColumn::Name => Length::FillPortion(4),
        ViewColumn::Type => Length::Fixed(120.0),
        ViewColumn::Size => Length::Fixed(100.0),
        ViewColumn::Modified => Length::Fixed(160.0),
    }
}

fn column_alignment(column: &ViewColumn) -> Horizontal {
    match column {
        ViewColumn::Size | ViewColumn::Modified => Horizontal::Right,
        _ => Horizontal::Left,
    }
}

fn column_sort_key(column: &ViewColumn) -> Option<SortKeyConfig> {
    match column {
        ViewColumn::Name => Some(SortKeyConfig::Name),
        ViewColumn::Size => Some(SortKeyConfig::Size),
        ViewColumn::Modified => Some(SortKeyConfig::Modified),
        ViewColumn::Type => None,
    }
}

fn entry_type_label(entry_type: FsEntryType) -> &'static str {
    match entry_type {
        FsEntryType::Directory => "Dossier",
        FsEntryType::File => "Fichier",
        FsEntryType::Symlink => "Lien",
        FsEntryType::Other => "Autre",
    }
}

fn format_entry_size(entry: &FsEntry) -> String {
    match entry.entry_type {
        FsEntryType::Directory => "—".to_string(),
        _ => format_bytes(entry.metadata.size),
    }
}

fn format_bytes(bytes: u64) -> String {
    let units = ["o", "Ko", "Mo", "Go", "To"];
    let mut size = bytes as f64;
    let mut index = 0;
    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} {}", units[index])
    } else {
        format!("{:.1} {}", size, units[index])
    }
}

fn format_modified(modified: Option<std::time::SystemTime>) -> String {
    modified
        .map(|time| {
            let datetime: DateTime<Local> = time.into();
            datetime.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_else(|| "—".to_string())
}

fn list_options_from_config(list_config: crate::core::ListConfig) -> ListOptions {
    ListOptions {
        show_hidden: list_config.show_hidden,
        sort_by: match list_config.sort_key {
            SortKeyConfig::Name => crate::filesystem::SortKey::Name,
            SortKeyConfig::Modified => crate::filesystem::SortKey::Modified,
            SortKeyConfig::Size => crate::filesystem::SortKey::Size,
        },
        sort_order: match list_config.sort_order {
            SortOrderConfig::Asc => crate::filesystem::SortOrder::Asc,
            SortOrderConfig::Desc => crate::filesystem::SortOrder::Desc,
        },
        directories_first: list_config.directories_first,
        filter: match list_config.filter {
            EntryFilterConfig::All => crate::filesystem::EntryFilter::All,
            EntryFilterConfig::OnlyDirectories => crate::filesystem::EntryFilter::OnlyDirectories,
            EntryFilterConfig::OnlyFiles => crate::filesystem::EntryFilter::OnlyFiles,
        },
        name_query: None,
    }
}

#[derive(Debug)]
struct TreeNode {
    path: PathBuf,
    label: String,
    depth: usize,
    expanded: bool,
    selected: bool,
}

fn tree_label_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|label| !label.is_empty())
        .map(|label| label.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn build_tree_nodes(
    filesystem: &LocalFileSystem,
    root: &Path,
    current_path: &Path,
    max_depth: usize,
    options: &ListOptions,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    if !root.exists() {
        return nodes;
    }
    collect_tree_nodes(
        filesystem,
        root,
        current_path,
        0,
        max_depth,
        options,
        &mut nodes,
    );
    nodes
}

fn collect_tree_nodes(
    filesystem: &LocalFileSystem,
    path: &Path,
    current_path: &Path,
    depth: usize,
    max_depth: usize,
    options: &ListOptions,
    nodes: &mut Vec<TreeNode>,
) {
    let expanded = current_path.starts_with(path) && depth < max_depth;
    nodes.push(TreeNode {
        path: path.to_path_buf(),
        label: tree_label_for_path(path),
        depth,
        expanded,
        selected: current_path == path,
    });

    if !expanded {
        return;
    }

    let entries: Vec<FsEntry> = match filesystem.list_dir(path, options.clone()) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut directories: Vec<FsEntry> = entries
        .into_iter()
        .filter(|entry| matches!(entry.entry_type, FsEntryType::Directory))
        .collect();
    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for entry in directories.into_iter().take(TREE_MAX_CHILDREN) {
        collect_tree_nodes(
            filesystem,
            &entry.path,
            current_path,
            depth + 1,
            max_depth,
            options,
            nodes,
        );
    }
}

fn command_from_key_press_with_shortcuts(
    shortcuts: &crate::core::ShortcutBindings,
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<KeyboardCommand> {
    let input = key_input_from_event(key, modifiers)?;
    let extend = modifiers.shift();

    if shortcuts.move_up.matches(&input, true) {
        return Some(KeyboardCommand::MoveUp { extend });
    }
    if shortcuts.move_down.matches(&input, true) {
        return Some(KeyboardCommand::MoveDown { extend });
    }
    if shortcuts.move_home.matches(&input, true) {
        return Some(KeyboardCommand::MoveHome { extend });
    }
    if shortcuts.move_end.matches(&input, true) {
        return Some(KeyboardCommand::MoveEnd { extend });
    }
    if shortcuts.activate.matches(&input, false) {
        return Some(KeyboardCommand::Activate);
    }
    if shortcuts.clear_selection.matches(&input, false) {
        return Some(KeyboardCommand::ClearSelection);
    }
    if shortcuts.cycle_pane_focus.matches(&input, false) {
        return Some(KeyboardCommand::CyclePaneFocus);
    }
    if shortcuts.back.matches(&input, false) {
        return Some(KeyboardCommand::Back);
    }
    if shortcuts.forward.matches(&input, false) {
        return Some(KeyboardCommand::Forward);
    }
    if shortcuts.refresh.matches(&input, false) {
        return Some(KeyboardCommand::Refresh);
    }
    if shortcuts.select_all.matches(&input, false) {
        return Some(KeyboardCommand::SelectAll);
    }
    if shortcuts.toggle_context_menu.matches(&input, false) {
        return Some(KeyboardCommand::ToggleContextMenu);
    }

    None
}

fn key_input_from_event(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<KeyInput> {
    let key_kind = match key {
        keyboard::Key::Named(named) => match named {
            keyboard::key::Named::ArrowUp => KeyKind::Named(NamedKey::ArrowUp),
            keyboard::key::Named::ArrowDown => KeyKind::Named(NamedKey::ArrowDown),
            keyboard::key::Named::ArrowLeft => KeyKind::Named(NamedKey::ArrowLeft),
            keyboard::key::Named::ArrowRight => KeyKind::Named(NamedKey::ArrowRight),
            keyboard::key::Named::Home => KeyKind::Named(NamedKey::Home),
            keyboard::key::Named::End => KeyKind::Named(NamedKey::End),
            keyboard::key::Named::Enter => KeyKind::Named(NamedKey::Enter),
            keyboard::key::Named::Escape => KeyKind::Named(NamedKey::Escape),
            keyboard::key::Named::Tab => KeyKind::Named(NamedKey::Tab),
            _ => return None,
        },
        keyboard::Key::Character(character) => {
            let normalized = character.to_lowercase();
            if normalized.is_empty() {
                return None;
            }
            KeyKind::Character(normalized)
        }
        _ => return None,
    };

    Some(KeyInput {
        key: key_kind,
        ctrl: modifiers.control(),
        alt: modifiers.alt(),
        shift: modifiers.shift(),
    })
}

pub fn run() -> iced::Result {
    iced::application(
        |state: &XionApp| format!("Xion — {}", state.state.route.path.display()),
        XionApp::update,
        XionApp::view,
    )
    .theme(|_| Theme::Light)
    .font(JETBRAINS_MONO_REGULAR)
    .font(JETBRAINS_MONO_ITALIC)
    .font(JETBRAINS_MONO_THIN)
    .font(JETBRAINS_MONO_THIN_ITALIC)
    .font(JETBRAINS_MONO_EXTRA_LIGHT)
    .font(JETBRAINS_MONO_EXTRA_LIGHT_ITALIC)
    .font(JETBRAINS_MONO_LIGHT)
    .font(JETBRAINS_MONO_LIGHT_ITALIC)
    .font(JETBRAINS_MONO_MEDIUM)
    .font(JETBRAINS_MONO_MEDIUM_ITALIC)
    .font(JETBRAINS_MONO_SEMI_BOLD)
    .font(JETBRAINS_MONO_SEMI_BOLD_ITALIC)
    .font(JETBRAINS_MONO_BOLD)
    .font(JETBRAINS_MONO_BOLD_ITALIC)
    .font(JETBRAINS_MONO_EXTRA_BOLD)
    .font(JETBRAINS_MONO_EXTRA_BOLD_ITALIC)
    .default_font(Font::with_name(FONT_NAME))
    .subscription(XionApp::subscription)
    .run_with(XionApp::new)
}
