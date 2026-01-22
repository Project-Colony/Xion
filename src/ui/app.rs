use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{
    button, column, container, progress_bar, row, scrollable, text, vertical_space,
};
use iced::{Alignment, Element, Length, Task, Theme};

use crate::core::AppConfig;
use crate::filesystem::{FsEntry, FsEntryType, ListOptions, LocalFileSystem, Page, PageRequest};
use crate::services::{DirectoryLoader, HistoryService, VirtualList, VirtualWindow};
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
    filesystem: LocalFileSystem,
    directory_loader: DirectoryLoader,
    entries: PagedEntries,
    scroll_offset: f32,
    viewport_height: f32,
    error: Option<String>,
}

impl XionApp {
    fn load_entries(
        &mut self,
        path: &PathBuf,
        page_request: PageRequest,
    ) -> Result<Page<FsEntry>, String> {
        let options = ListOptions {
            show_hidden: self.state.config.show_hidden,
            ..ListOptions::default()
        };
        self.directory_loader
            .load_page(&self.filesystem, path, options, page_request)
            .map_err(|error| error.to_string())
    }

    fn refresh_entries(&mut self) {
        self.entries.reset();
        self.error = None;
        self.scroll_offset = 0.0;

        match self.load_page(0) {
            Ok(()) => {}
            Err(message) => {
                self.error = Some(message);
            }
        }
    }

    fn navigate_to(&mut self, path: PathBuf) {
        self.state.route.path = path.clone();
        self.history.record(path);
        self.refresh_entries();
    }
}

impl XionApp {
    fn new() -> Self {
        let config = AppConfig::default();
        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.path.clone());

        let filesystem = LocalFileSystem::new();
        let directory_loader = DirectoryLoader::new(
            CACHE_SIZE,
            Duration::from_secs(CACHE_TTL_SECONDS),
            PAGE_SIZE,
        );
        let entries = PagedEntries::new(0, directory_loader.page_size());
        let mut app = Self {
            state,
            history,
            filesystem,
            directory_loader,
            entries,
            scroll_offset: 0.0,
            viewport_height: 480.0,
            error: None,
        };
        app.refresh_entries();
        app
    }

    fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        match message {
            UiMessage::NavigateTo(path) => {
                self.navigate_to(path);
            }
            UiMessage::Back => {
                if let Some(path) = self.history.back() {
                    self.state.route.path = path;
                    self.refresh_entries();
                }
            }
            UiMessage::Forward => {
                if let Some(path) = self.history.forward() {
                    self.state.route.path = path;
                    self.refresh_entries();
                }
            }
            UiMessage::Refresh => {
                self.refresh_entries();
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
                self.ensure_visible_pages();
            }
        }

        Task::none()
    }

    fn load_page(&mut self, page_index: usize) -> Result<(), String> {
        let offset = page_index * self.entries.page_size;
        let page_request = PageRequest::new(offset, self.entries.page_size);
        let path = self.state.route.path.clone();
        let page = self.load_entries(&path, page_request)?;
        self.entries.apply_page(page_index, page);
        Ok(())
    }

    fn ensure_visible_pages(&mut self) {
        if self.entries.total == 0 {
            return;
        }

        let window = self.virtual_window();
        if window.len() == 0 {
            return;
        }

        let start_page = window.start / self.entries.page_size;
        let end_page = (window.end.saturating_sub(1)) / self.entries.page_size;

        for page_index in start_page..=end_page {
            if !self.entries.is_page_loaded(page_index) {
                if let Err(message) = self.load_page(page_index) {
                    self.error = Some(message);
                    break;
                }
            }
        }
    }

    fn virtual_window(&self) -> VirtualWindow {
        let virtual_list = VirtualList {
            item_height: ROW_HEIGHT,
            viewport_height: self.viewport_height,
            overscan: OVERSCAN,
        };
        virtual_list.visible_range(self.scroll_offset, self.entries.total)
    }

    fn view(&self) -> Element<'_, UiMessage> {
        let back_button = if self.history.can_back() {
            button(text("←")).on_press(UiMessage::Back)
        } else {
            button(text("←"))
        };

        let forward_button = if self.history.can_forward() {
            button(text("→")).on_press(UiMessage::Forward)
        } else {
            button(text("→"))
        };

        let refresh_button = button(text("⟳")).on_press(UiMessage::Refresh);

        let navigation = row![back_button, forward_button, refresh_button].spacing(8);

        let tabs = row![button(text("Ce PC")), button(text("+"))].spacing(6);

        let address_bar = container(text(self.state.route.path.display().to_string()))
            .padding([6, 12])
            .width(Length::Fill);

        let search_bar = container(text("Rechercher dans : Ce PC"))
            .padding([6, 12])
            .width(Length::Fixed(220.0));

        let command_bar = row![
            button(text("Nouveau")),
            button(text("Couper")),
            button(text("Copier")),
            button(text("Coller")),
            button(text("Trier")),
            button(text("Afficher")),
            button(text("..."))
        ]
        .spacing(8);

        let header = column![
            row![tabs].spacing(8).align_y(Alignment::Center),
            row![navigation, address_bar, search_bar]
                .spacing(12)
                .align_y(Alignment::Center),
            command_bar
        ]
        .spacing(10);

        let drive_summary = column![
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
        .spacing(12);

        let list_content = if let Some(message) = &self.error {
            column![
                text("Impossible de charger le dossier").size(16),
                text(message)
            ]
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
                        let entry_row = row![
                            text(match entry.entry_type {
                                FsEntryType::Directory => "📁",
                                FsEntryType::File => "📄",
                                FsEntryType::Symlink => "🔗",
                                FsEntryType::Other => "❓",
                            }),
                            text(&entry.name)
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center);
                        let message = match entry.entry_type {
                            FsEntryType::Directory => UiMessage::NavigateTo(entry.path.clone()),
                            _ => UiMessage::SelectEntry(entry.path.clone()),
                        };
                        button(entry_row).on_press(message)
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

        let sidebar = column![
            text("Accueil").size(16),
            text("Galerie").size(16),
            text("—").size(12),
            text("Bureau").size(14),
            text("Téléchargement").size(14),
            text("Documents").size(14),
            text("Images").size(14),
            text("Musique").size(14),
            text("Vidéos").size(14),
            text("—").size(12),
            text("Ce PC").size(16),
            text("Disque local (C:)").size(14),
            text("Réseau").size(14)
        ]
        .spacing(10)
        .padding(12);

        let body = row![
            container(sidebar).width(Length::Fixed(220.0)),
            container(list).width(Length::Fill).height(Length::Fill)
        ]
        .height(Length::Fill)
        .spacing(16);

        let content = column![header, body]
            .spacing(12)
            .padding(16)
            .align_x(Alignment::Start)
            .height(Length::Fill);

        container(content).into()
    }
}

pub fn run() -> iced::Result {
    iced::application(
        |state: &XionApp| format!("Xion — {}", state.state.route.path.display()),
        XionApp::update,
        XionApp::view,
    )
    .theme(|_| Theme::Light)
    .run_with(|| (XionApp::new(), Task::none()))
}
