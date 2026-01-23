use std::collections::{HashMap, HashSet};
use std::path::{Component, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{
    button, column, container, horizontal_space, image, progress_bar, row, scrollable, text,
    vertical_space,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Subscription, Task, Theme, border,
    keyboard, mouse,
};

use crate::core::AppConfig;
use crate::filesystem::{FsEntry, FsEntryType, ListOptions, LocalFileSystem, Page, PageRequest};
use crate::services::{
    DirectoryLoader, HistoryService, ThumbnailService, VirtualList, VirtualWindow,
    generate_thumbnail,
};
use crate::ui::{
    AppState, ContextAction, KeyboardCommand, ModifiersState, ScrollViewport, SelectionKind,
    UiMessage,
};

const ROW_HEIGHT: f32 = 32.0;
const OVERSCAN: usize = 6;
const CACHE_SIZE: usize = 256;
const CACHE_TTL_SECONDS: u64 = 45;
const PAGE_SIZE: usize = 120;

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
    pending_pages: HashSet<usize>,
    is_loading: bool,
    scroll_offset: f32,
    viewport_height: f32,
    error: Option<String>,
    modifiers: ModifiersState,
    context_menu_open: bool,
    last_action: Option<String>,
}

impl XionApp {
    fn refresh_entries(&mut self) -> Task<UiMessage> {
        self.entries.reset();
        self.error = None;
        self.scroll_offset = 0.0;
        self.thumbnail_handles.clear();
        self.thumbnails_in_flight.clear();
        self.thumbnail_misses.clear();
        self.pending_pages.clear();
        self.is_loading = true;
        self.clear_selection();

        self.request_page(0)
    }

    fn navigate_to(&mut self, path: PathBuf) -> Task<UiMessage> {
        self.state.route.path = path.clone();
        self.history.record(path);
        self.refresh_entries()
    }

    fn request_page(&mut self, page_index: usize) -> Task<UiMessage> {
        if self.pending_pages.contains(&page_index) {
            return Task::none();
        }

        let offset = page_index * self.entries.page_size;
        let page_request = PageRequest::new(offset, self.entries.page_size);
        let path = self.state.route.path.clone();
        let show_hidden = self.state.config.show_hidden;
        let loader = Arc::clone(&self.directory_loader);

        self.pending_pages.insert(page_index);
        self.is_loading = true;

        Task::perform(
            async move {
                let options = ListOptions {
                    show_hidden,
                    ..ListOptions::default()
                };
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
}

impl XionApp {
    fn new() -> (Self, Task<UiMessage>) {
        let config = AppConfig::default();
        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.path.clone());

        let directory_loader = Arc::new(Mutex::new(DirectoryLoader::new(
            CACHE_SIZE,
            Duration::from_secs(CACHE_TTL_SECONDS),
            PAGE_SIZE,
        )));
        let page_size = directory_loader
            .lock()
            .map(|loader| loader.page_size())
            .unwrap_or(PAGE_SIZE);
        let thumbnails = ThumbnailService::new(
            state.config.thumbnail_cache_entries,
            Duration::from_secs(state.config.thumbnail_cache_ttl_seconds),
        );
        let entries = PagedEntries::new(0, page_size);
        let mut app = Self {
            state,
            history,
            directory_loader,
            thumbnails,
            thumbnail_handles: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            thumbnail_misses: HashSet::new(),
            entries,
            pending_pages: HashSet::new(),
            is_loading: false,
            scroll_offset: 0.0,
            viewport_height: 480.0,
            error: None,
            modifiers: ModifiersState::default(),
            context_menu_open: false,
            last_action: None,
        };
        let task = app.refresh_entries();
        (app, task)
    }

    fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        let mut tasks = Vec::new();
        match message {
            UiMessage::NavigateTo(path) => {
                tasks.push(self.navigate_to(path));
            }
            UiMessage::Back => {
                if let Some(path) = self.history.back() {
                    self.state.route.path = path;
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Forward => {
                if let Some(path) = self.history.forward() {
                    self.state.route.path = path;
                    tasks.push(self.refresh_entries());
                }
            }
            UiMessage::Refresh => {
                tasks.push(self.refresh_entries());
            }
            UiMessage::FocusPane(pane) => {
                self.state.navigation.focused_pane = pane;
            }
            UiMessage::SelectEntry { path, kind } => {
                self.apply_selection(path, kind);
            }
            UiMessage::ActivateEntry(path) => {
                tasks.push(self.activate_entry(path));
            }
            UiMessage::KeyboardCommand(command) => {
                tasks.push(self.handle_keyboard_command(command));
            }
            UiMessage::ToggleContextMenu(force_open) => {
                self.context_menu_open = force_open;
            }
            UiMessage::ContextAction(action) => {
                tasks.push(self.apply_context_action(action));
            }
            UiMessage::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            UiMessage::Scroll(viewport) => {
                self.scroll_offset = viewport.offset_y;
                self.viewport_height = viewport.viewport_height.max(1.0);
                tasks.push(self.ensure_visible_pages());
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
                        tasks.push(self.ensure_visible_pages());
                    }
                    Err(message) => {
                        self.error = Some(message);
                    }
                }

                if self.pending_pages.is_empty() {
                    self.is_loading = false;
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
        }

        tasks.push(self.request_visible_thumbnails());
        Task::batch(tasks)
    }

    fn subscription(&self) -> Subscription<UiMessage> {
        iced::subscription::events_with(|event, status| {
            if status == iced::event::Status::Captured {
                return None;
            }

            match event {
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(UiMessage::ModifiersChanged(ModifiersState {
                        shift: modifiers.shift(),
                        control: modifiers.control(),
                        alt: modifiers.alt(),
                    }))
                }
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    let extend = modifiers.shift();
                    let command = match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            Some(KeyboardCommand::MoveUp { extend })
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            Some(KeyboardCommand::MoveDown { extend })
                        }
                        keyboard::Key::Named(keyboard::key::Named::Home) => {
                            Some(KeyboardCommand::MoveHome { extend })
                        }
                        keyboard::Key::Named(keyboard::key::Named::End) => {
                            Some(KeyboardCommand::MoveEnd { extend })
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter) => {
                            Some(KeyboardCommand::Activate)
                        }
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            Some(KeyboardCommand::ClearSelection)
                        }
                        keyboard::Key::Named(keyboard::key::Named::Tab) => {
                            Some(KeyboardCommand::CyclePaneFocus)
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft)
                            if modifiers.alt() =>
                        {
                            Some(KeyboardCommand::Back)
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight)
                            if modifiers.alt() =>
                        {
                            Some(KeyboardCommand::Forward)
                        }
                        keyboard::Key::Character(character) => {
                            let character = character.to_lowercase();
                            if modifiers.control() && character == "a" {
                                Some(KeyboardCommand::SelectAll)
                            } else if modifiers.control() && character == "r" {
                                Some(KeyboardCommand::Refresh)
                            } else if modifiers.control() && character == "m" {
                                Some(KeyboardCommand::ToggleContextMenu)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    command.map(UiMessage::KeyboardCommand)
                }
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    Some(UiMessage::ToggleContextMenu(true))
                }
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    Some(UiMessage::ToggleContextMenu(false))
                }
                _ => None,
            }
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
            _ => Task::none(),
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

    fn first_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().position(|entry| entry.is_some())
    }

    fn last_entry_index(&self) -> Option<usize> {
        self.entries.items.iter().rposition(|entry| entry.is_some())
    }

    fn move_focus_by(&mut self, offset: isize, extend: bool) {
        let selection = &self.state.navigation.selection;
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

        self.move_focus_to_index(target_index, extend);
    }

    fn move_focus_to_start(&mut self, extend: bool) {
        if let Some(index) = self.first_entry_index() {
            self.move_focus_to_index(index, extend);
        }
    }

    fn move_focus_to_end(&mut self, extend: bool) {
        if let Some(index) = self.last_entry_index() {
            self.move_focus_to_index(index, extend);
        }
    }

    fn move_focus_to_index(&mut self, index: usize, extend: bool) {
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
        let selection = &mut self.state.navigation.selection;
        selection.selected.clear();
        for entry in self.entries.items.iter().flatten() {
            selection.selected.insert(entry.path.clone());
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
    }

    fn cycle_focus(&mut self) {
        self.state.navigation.focused_pane = match self.state.navigation.focused_pane {
            crate::ui::PaneKind::Tree => crate::ui::PaneKind::List,
            crate::ui::PaneKind::List => crate::ui::PaneKind::Preview,
            crate::ui::PaneKind::Preview => crate::ui::PaneKind::Tree,
        };
    }

    fn ensure_visible_pages(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
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
        let virtual_list = VirtualList {
            item_height: ROW_HEIGHT,
            viewport_height: self.viewport_height,
            overscan: OVERSCAN,
        };
        virtual_list.visible_range(self.scroll_offset, self.entries.total)
    }

    fn request_visible_thumbnails(&mut self) -> Task<UiMessage> {
        if self.entries.total == 0 {
            return Task::none();
        }

        let window = self.virtual_window();
        if window.len() == 0 {
            return Task::none();
        }

        let mut tasks = Vec::new();
        let thumbnail_size = self.state.config.thumbnail_size;

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

        Task::batch(tasks)
    }

    fn breadcrumbs(&self) -> Element<'_, UiMessage> {
        let mut row = row![];
        let mut current_path = PathBuf::new();
        let mut has_component = false;

        for component in self.state.route.path.components() {
            let label = match component {
                Component::Prefix(prefix) => prefix.as_os_str().to_string_lossy().to_string(),
                Component::RootDir => String::from(std::path::MAIN_SEPARATOR),
                Component::CurDir => ".".to_string(),
                Component::ParentDir => "..".to_string(),
                Component::Normal(part) => part.to_string_lossy().to_string(),
            };

            if !has_component {
                has_component = true;
            } else {
                row = row.push(text("›"));
            }

            current_path.push(component.as_os_str());
            let target = current_path.clone();
            row = row.push(
                button(text(label))
                    .padding([2, 6])
                    .on_press(UiMessage::NavigateTo(target)),
            );
        }

        if !has_component {
            row = row.push(text("—"));
        }

        row.align_y(Alignment::Center).spacing(6).into()
    }

    fn view(&self) -> Element<'_, UiMessage> {
        let tokens = UiTokens::default();
        let colors = tokens.colors;
        let spacing = tokens.spacing;
        let typography = tokens.typography;

        let toolbar_button = |label: String| {
            button(text(label).size(typography.body))
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

        let tab_button = |label: String, active: bool| {
            button(text(label).size(typography.body))
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
            toolbar_button("←".to_string()).on_press(UiMessage::Back)
        } else {
            toolbar_button("←".to_string())
        };

        let forward_button = if self.history.can_forward() {
            toolbar_button("→".to_string()).on_press(UiMessage::Forward)
        } else {
            toolbar_button("→".to_string())
        };

        let refresh_button = toolbar_button("⟳".to_string()).on_press(UiMessage::Refresh);

        let navigation = row![back_button, forward_button, refresh_button].spacing(spacing.sm);

        let tabs = row![
            tab_button("Ce PC".to_string(), true),
            tab_button("+".to_string(), false)
        ]
        .spacing(spacing.sm);

        let address_bar = container(self.breadcrumbs())
            .padding([spacing.xs, spacing.md])
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            });

        let search_bar = container(text("Rechercher dans : Ce PC").size(typography.caption))
            .padding([spacing.xs, spacing.md])
            .width(Length::Fixed(240.0))
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.panel_background)),
                border: border::rounded(6.0).color(colors.border).width(1.0),
                ..Default::default()
            });

        let command_bar = row![
            toolbar_button("Nouveau".to_string()),
            toolbar_button("Couper".to_string()),
            toolbar_button("Copier".to_string()),
            toolbar_button("Coller".to_string()),
            toolbar_button("Trier".to_string()),
            toolbar_button("Afficher".to_string()),
            toolbar_button("...".to_string()),
            toolbar_button("Actions".to_string())
                .on_press(UiMessage::ToggleContextMenu(!self.context_menu_open))
        ]
        .spacing(spacing.sm);

        let context_actions = row![
            toolbar_button("Ouvrir".to_string())
                .on_press(UiMessage::ContextAction(ContextAction::Open,)),
            toolbar_button("Renommer".to_string())
                .on_press(UiMessage::ContextAction(ContextAction::Rename,)),
            toolbar_button("Supprimer".to_string())
                .on_press(UiMessage::ContextAction(ContextAction::Delete,)),
            toolbar_button("Copier le chemin".to_string())
                .on_press(UiMessage::ContextAction(ContextAction::CopyPath),)
        ]
        .spacing(spacing.sm);

        let context_menu =
            if self.context_menu_open && !self.state.navigation.selection.selected.is_empty() {
                Some(
                    container(context_actions)
                        .padding([spacing.sm, spacing.md])
                        .style(move |_| iced::widget::container::Style {
                            background: Some(Background::Color(colors.panel_background)),
                            border: border::rounded(8.0).color(colors.border).width(1.0),
                            ..Default::default()
                        }),
                )
            } else {
                None
            };

        let header = container(
            column![
                row![tabs].spacing(8).align_y(Alignment::Center),
                row![navigation, address_bar, search_bar]
                    .spacing(spacing.md)
                    .align_y(Alignment::Center),
                command_bar,
                context_menu.unwrap_or_else(|| container(row![]))
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

        let drive_summary = container(
            column![
                text("Périphériques et lecteurs").size(typography.title),
                row![
                    text("🖥️"),
                    column![
                        text("Disque local (C:)"),
                        progress_bar(0.0..=1.0, 0.12),
                        text("109 Go libres sur 930 Go").size(typography.caption)
                    ]
                    .spacing(spacing.sm)
                ]
                .spacing(spacing.md)
                .align_y(Alignment::Center)
            ]
            .spacing(spacing.md),
        )
        .padding(spacing.md)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(colors.panel_background)),
            border: border::rounded(8.0).color(colors.border).width(1.0),
            ..Default::default()
        });

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier").size(typography.title),
                text(message),
                button(text("Réessayer")).on_press(UiMessage::Refresh)
            ]
            .spacing(spacing.sm)
        } else if self.is_loading && self.entries.total == 0 {
            column![
                text("Chargement du dossier…").size(typography.title),
                progress_bar(0.0..=1.0, 0.4)
            ]
            .spacing(spacing.md)
        } else if self.entries.total == 0 {
            column![text("Dossier vide")]
        } else {
            let window = self.virtual_window();
            let mut list = column![];

            if window.padding_top > 0.0 {
                list = list.push(vertical_space().height(Length::Fixed(window.padding_top)));
            }

            for index in window.start..window.end {
                let entry = self.entries.get(index);
                list = list.push(match entry {
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
                        let leading: Element<'_, UiMessage> = match entry.entry_type {
                            FsEntryType::Directory => text("📁").into(),
                            FsEntryType::File => self
                                .thumbnail_handles
                                .get(&entry.path)
                                .map(|handle| {
                                    image(handle.clone())
                                        .width(Length::Fixed(
                                            self.state.config.thumbnail_size as f32,
                                        ))
                                        .height(Length::Fixed(
                                            self.state.config.thumbnail_size as f32,
                                        ))
                                        .into()
                                })
                                .unwrap_or_else(|| text("📄").into()),
                            FsEntryType::Symlink => text("🔗").into(),
                            FsEntryType::Other => text("❓").into(),
                        };
                        let open_button: Element<'_, UiMessage> =
                            if entry.entry_type == FsEntryType::Directory {
                                button(text("Ouvrir").size(typography.caption))
                                    .padding([spacing.xs, spacing.sm])
                                    .on_press(UiMessage::ActivateEntry(entry.path.clone()))
                                    .into()
                            } else {
                                container(row![]).into()
                            };
                        let entry_row = row![
                            leading,
                            text(&entry.name).size(typography.body),
                            horizontal_space(),
                            open_button
                        ]
                        .spacing(spacing.md)
                        .align_y(Alignment::Center);
                        let selection_kind = self.selection_kind_from_modifiers();
                        let message = UiMessage::SelectEntry {
                            path: entry.path.clone(),
                            kind: selection_kind,
                        };
                        button(entry_row)
                            .padding([spacing.xs, spacing.sm])
                            .style(move |_theme: &Theme, status: ButtonStatus| {
                                let mut style = iced::widget::button::Style {
                                    text_color: colors.text_primary,
                                    ..Default::default()
                                };

                                if is_selected {
                                    style.background = Some(Background::Color(colors.selection));
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
                            .on_press(message)
                    }
                    None => {
                        let placeholder = row![text("⏳"), text("Chargement…")]
                            .spacing(spacing.md)
                            .align_y(Alignment::Center);
                        button(placeholder)
                    }
                });
            }

            if window.padding_bottom > 0.0 {
                list = list.push(vertical_space().height(Length::Fixed(window.padding_bottom)));
            }

            list
        };

        let selection = &self.state.navigation.selection;
        let selection_status = if selection.selected.is_empty() {
            "Sélection : —".to_string()
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

        let action_status = self
            .last_action
            .clone()
            .unwrap_or_else(|| "Action : —".to_string());

        let list = scrollable(
            container(
                column![
                    drive_summary,
                    list_content,
                    text(selection_status).size(typography.caption),
                    text(action_status).size(typography.caption)
                ]
                .spacing(spacing.xl),
            )
            .padding(spacing.md),
        )
        .on_scroll(|viewport| {
            UiMessage::Scroll(ScrollViewport {
                offset_y: viewport.absolute_offset().y,
                viewport_height: viewport.bounds().height,
                content_height: viewport.content_bounds().height,
            })
        });

        let sidebar = container(
            column![
                row![text("🏠"), text("Accueil").size(typography.body)].spacing(spacing.sm),
                row![text("🖼️"), text("Galerie").size(typography.body)].spacing(spacing.sm),
                text("—").size(typography.caption),
                row![text("🗂️"), text("Bureau").size(typography.body)].spacing(spacing.sm),
                row![text("⬇️"), text("Téléchargement").size(typography.body)].spacing(spacing.sm),
                row![text("📄"), text("Documents").size(typography.body)].spacing(spacing.sm),
                row![text("🖼️"), text("Images").size(typography.body)].spacing(spacing.sm),
                row![text("🎵"), text("Musique").size(typography.body)].spacing(spacing.sm),
                row![text("🎬"), text("Vidéos").size(typography.body)].spacing(spacing.sm),
                text("—").size(typography.caption),
                container(
                    row![text("💻"), text("Ce PC").size(typography.body)].spacing(spacing.sm)
                )
                .padding([spacing.xs, spacing.sm])
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.selection)),
                    border: border::rounded(6.0).color(colors.accent).width(1.0),
                    ..Default::default()
                }),
                row![text("💽"), text("Disque local (C:)").size(typography.body)]
                    .spacing(spacing.sm),
                row![text("🌐"), text("Réseau").size(typography.body)].spacing(spacing.sm)
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

        let body = row![
            sidebar.width(Length::Fixed(220.0)),
            container(list)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(colors.panel_background)),
                    border: border::rounded(10.0).color(colors.border).width(1.0),
                    ..Default::default()
                })
        ]
        .height(Length::Fill)
        .spacing(spacing.md);

        let content = column![header, body]
            .spacing(spacing.md)
            .padding(spacing.lg)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        container(content)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(colors.chrome_background)),
                ..Default::default()
            })
            .into()
    }
}

pub fn run() -> iced::Result {
    iced::application(
        |state: &XionApp| format!("Xion — {}", state.state.route.path.display()),
        XionApp::update,
        XionApp::view,
    )
    .theme(|_| Theme::Light)
    .subscription(XionApp::subscription)
    .run_with(XionApp::new)
}
