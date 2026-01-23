use std::collections::{HashMap, HashSet};
use std::path::{Component, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::button::Status as ButtonStatus;
use iced::widget::{
    button, column, container, image, progress_bar, row, scrollable, text, vertical_space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Task, Theme, border};

use crate::core::AppConfig;
use crate::filesystem::{FsEntry, FsEntryType, ListOptions, LocalFileSystem, Page, PageRequest};
use crate::services::{
    DirectoryLoader, HistoryService, ThumbnailService, VirtualList, VirtualWindow,
    generate_thumbnail,
};
use crate::ui::{AppState, ScrollViewport, UiMessage};

const ROW_HEIGHT: f32 = 32.0;
const OVERSCAN: usize = 6;
const CACHE_SIZE: usize = 256;
const CACHE_TTL_SECONDS: u64 = 45;
const PAGE_SIZE: usize = 120;

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
            UiMessage::SelectEntry(path) => {
                self.state.navigation.selection = Some(path);
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
        let chrome_background = Color::from_rgb8(247, 247, 250);
        let panel_background = Color::from_rgb8(255, 255, 255);
        let border_color = Color::from_rgb8(223, 226, 232);
        let sidebar_background = Color::from_rgb8(242, 244, 248);
        let accent = Color::from_rgb8(0, 120, 215);
        let text_primary = Color::from_rgb8(32, 34, 38);

        let toolbar_button = |label: &str| {
            button(text(label).size(14)).padding([6, 10]).style(
                move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: text_primary,
                        ..Default::default()
                    };

                    match status {
                        ButtonStatus::Hovered => {
                            style.background =
                                Some(Background::Color(Color::from_rgb8(236, 239, 245)));
                            style.border = border::rounded(6.0).color(border_color).width(1.0);
                        }
                        ButtonStatus::Pressed => {
                            style.background =
                                Some(Background::Color(Color::from_rgb8(224, 230, 240)));
                            style.border = border::rounded(6.0).color(border_color).width(1.0);
                        }
                        ButtonStatus::Disabled => {
                            style.text_color = Color::from_rgb8(150, 150, 150);
                        }
                        ButtonStatus::Active => {}
                    }

                    style
                },
            )
        };

        let tab_button = |label: &str, active: bool| {
            button(text(label).size(14)).padding([6, 12]).style(
                move |_theme: &Theme, status: ButtonStatus| {
                    let mut style = iced::widget::button::Style {
                        text_color: text_primary,
                        ..Default::default()
                    };

                    if active {
                        style.background = Some(Background::Color(panel_background));
                        style.border = border::rounded(8.0).color(border_color).width(1.0);
                    }

                    if matches!(status, ButtonStatus::Hovered) {
                        style.background = Some(Background::Color(Color::from_rgb8(236, 239, 245)));
                    }

                    style
                },
            )
        };

        let back_button = if self.history.can_back() {
            toolbar_button("←").on_press(UiMessage::Back)
        } else {
            toolbar_button("←")
        };

        let forward_button = if self.history.can_forward() {
            toolbar_button("→").on_press(UiMessage::Forward)
        } else {
            toolbar_button("→")
        };

        let refresh_button = toolbar_button("⟳").on_press(UiMessage::Refresh);

        let navigation = row![back_button, forward_button, refresh_button].spacing(6);

        let tabs = row![tab_button("Ce PC", true), tab_button("+", false)].spacing(6);

        let address_bar = container(self.breadcrumbs())
            .padding([6, 12])
            .width(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(panel_background)),
                border: border::rounded(6.0).color(border_color).width(1.0),
                ..Default::default()
            });

        let search_bar = container(text("Rechercher dans : Ce PC").size(13))
            .padding([6, 12])
            .width(Length::Fixed(240.0))
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(panel_background)),
                border: border::rounded(6.0).color(border_color).width(1.0),
                ..Default::default()
            });

        let command_bar = row![
            toolbar_button("Nouveau"),
            toolbar_button("Couper"),
            toolbar_button("Copier"),
            toolbar_button("Coller"),
            toolbar_button("Trier"),
            toolbar_button("Afficher"),
            toolbar_button("...")
        ]
        .spacing(6);

        let header = container(
            column![
                row![tabs].spacing(8).align_y(Alignment::Center),
                row![navigation, address_bar, search_bar]
                    .spacing(12)
                    .align_y(Alignment::Center),
                command_bar
            ]
            .spacing(8),
        )
        .padding([10, 12, 8, 12])
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(chrome_background)),
            border: border::rounded(10.0).color(border_color).width(1.0),
            ..Default::default()
        });

        let drive_summary = container(
            column![
                text("Périphériques et lecteurs").size(16),
                row![
                    text("🖥️"),
                    column![
                        text("Disque local (C:)"),
                        progress_bar(0.0..=1.0, 0.12),
                        text("109 Go libres sur 930 Go").size(12)
                    ]
                    .spacing(6)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            ]
            .spacing(12),
        )
        .padding(12)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(panel_background)),
            border: border::rounded(8.0).color(border_color).width(1.0),
            ..Default::default()
        });

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier").size(16),
                text(message),
                button(text("Réessayer")).on_press(UiMessage::Refresh)
            ]
            .spacing(8)
        } else if self.is_loading && self.entries.total == 0 {
            column![
                text("Chargement du dossier…").size(16),
                progress_bar(0.0..=1.0, 0.4)
            ]
            .spacing(12)
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
                        let entry_row = row![leading, text(&entry.name)]
                            .spacing(12)
                            .align_y(Alignment::Center);
                        let message = match entry.entry_type {
                            FsEntryType::Directory => UiMessage::NavigateTo(entry.path.clone()),
                            _ => UiMessage::SelectEntry(entry.path.clone()),
                        };
                        button(entry_row)
                            .padding([6, 10])
                            .style(move |_theme: &Theme, status: ButtonStatus| {
                                let mut style = iced::widget::button::Style {
                                    text_color: text_primary,
                                    ..Default::default()
                                };

                                if is_selected {
                                    style.background =
                                        Some(Background::Color(Color::from_rgb8(214, 230, 248)));
                                    style.border = border::rounded(6.0)
                                        .color(Color::from_rgb8(178, 206, 236))
                                        .width(1.0);
                                }

                                if matches!(status, ButtonStatus::Hovered) {
                                    style.background =
                                        Some(Background::Color(Color::from_rgb8(233, 239, 247)));
                                }

                                if matches!(status, ButtonStatus::Pressed) {
                                    style.background =
                                        Some(Background::Color(Color::from_rgb8(220, 230, 244)));
                                }

                                style
                            })
                            .on_press(message)
                    }
                    None => {
                        let placeholder = row![text("⏳"), text("Chargement…")]
                            .spacing(12)
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

        let selection_status = self
            .state
            .navigation
            .selection
            .as_ref()
            .map(|path| format!("Sélection : {}", path.display()))
            .unwrap_or_else(|| "Sélection : —".to_string());

        let list = scrollable(
            container(
                column![drive_summary, list_content, text(selection_status).size(12)].spacing(20),
            )
            .padding(12),
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
                row![text("🏠"), text("Accueil").size(15)].spacing(8),
                row![text("🖼️"), text("Galerie").size(15)].spacing(8),
                text("—").size(12),
                row![text("🗂️"), text("Bureau").size(14)].spacing(8),
                row![text("⬇️"), text("Téléchargement").size(14)].spacing(8),
                row![text("📄"), text("Documents").size(14)].spacing(8),
                row![text("🖼️"), text("Images").size(14)].spacing(8),
                row![text("🎵"), text("Musique").size(14)].spacing(8),
                row![text("🎬"), text("Vidéos").size(14)].spacing(8),
                text("—").size(12),
                container(row![text("💻"), text("Ce PC").size(15)].spacing(8))
                    .padding([4, 6])
                    .style(|_| iced::widget::container::Style {
                        background: Some(Background::Color(Color::from_rgb8(226, 238, 252))),
                        border: border::rounded(6.0).color(accent).width(1.0),
                        ..Default::default()
                    }),
                row![text("💽"), text("Disque local (C:)").size(14)].spacing(8),
                row![text("🌐"), text("Réseau").size(14)].spacing(8)
            ]
            .spacing(10),
        )
        .padding(12)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(sidebar_background)),
            border: Border {
                color: border_color,
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
                .style(|_| iced::widget::container::Style {
                    background: Some(Background::Color(panel_background)),
                    border: border::rounded(10.0).color(border_color).width(1.0),
                    ..Default::default()
                })
        ]
        .height(Length::Fill)
        .spacing(12);

        let content = column![header, body]
            .spacing(12)
            .padding(16)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        container(content)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(chrome_background)),
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
    .run_with(XionApp::new)
}
