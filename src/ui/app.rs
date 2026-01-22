use std::path::PathBuf;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Task, Theme};

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

        let header = row![
            back_button,
            forward_button,
            refresh_button,
            text(self.state.route.path.display().to_string())
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let list_content = if let Some(message) = &self.error {
            column![text("Impossible de charger le dossier"), text(message)]
        } else if self.entries.is_empty() {
            column![text("Dossier vide")]
        } else {
            self.entries
                .iter()
                .fold(column![], |column, entry| {
                    column.push(text(Self::entry_label(entry)))
                })
        };

        let list = scrollable(container(list_content.spacing(6)).padding(4));

        let content = column![header, list]
            .spacing(16)
            .padding(16)
            .align_x(Alignment::Start);

        container(content).into()
    }
}

pub fn run() -> iced::Result {
    iced::application(
        |state: &XionApp| format!("Xion — {}", state.state.route.path.display()),
        XionApp::update,
        XionApp::view,
    )
    .theme(|_| Theme::Dark)
    .run_with(|| (XionApp::new(), Task::none()))
}
