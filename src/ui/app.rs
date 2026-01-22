use std::path::PathBuf;

use iced::widget::{button, column, container, progress_bar, row, scrollable, text};
use iced::{Alignment, Element, Length, Task, Theme};

use crate::core::AppConfig;
use crate::filesystem::{FileSystem, FsEntry, FsEntryType, ListOptions, LocalFileSystem};
use crate::services::HistoryService;
use crate::ui::{AppState, UiMessage};

#[derive(Debug)]
pub struct XionApp {
    state: AppState,
    history: HistoryService,
    filesystem: LocalFileSystem,
    entries: Vec<FsEntry>,
    error: Option<String>,
}

impl XionApp {
    fn load_entries(&self, path: &PathBuf) -> Result<Vec<FsEntry>, String> {
        let options = ListOptions {
            show_hidden: self.state.config.show_hidden,
        };
        self.filesystem
            .list_dir(path, options)
            .map_err(|error| error.to_string())
    }

    fn refresh_entries(&mut self) {
        match self.load_entries(&self.state.route.path) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            }
            Err(message) => {
                self.entries.clear();
                self.error = Some(message);
            }
        }
    }

    fn navigate_to(&mut self, path: PathBuf) {
        self.state.route.path = path.clone();
        self.history.record(path);
        self.refresh_entries();
    }

    fn entry_label(entry: &FsEntry) -> String {
        let icon = match entry.entry_type {
            FsEntryType::Directory => "📁",
            FsEntryType::File => "📄",
            FsEntryType::Symlink => "🔗",
            FsEntryType::Other => "❓",
        };
        format!("{icon} {}", entry.name)
    }
}

impl XionApp {
    fn new() -> Self {
        let config = AppConfig::default();
        let state = AppState::new(config);
        let mut history = HistoryService::default();
        history.record(state.route.path.clone());

        let filesystem = LocalFileSystem::new();
        let mut app = Self {
            state,
            history,
            filesystem,
            entries: Vec::new(),
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
        }

        Task::none()
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
        } else if self.entries.is_empty() {
            column![text("Dossier vide")]
        } else {
            self.entries.iter().fold(column![], |column, entry| {
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
                column.push(entry_row)
            })
        };

        let list = scrollable(
            container(column![drive_summary, list_content].spacing(20)).padding(12),
        );

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
