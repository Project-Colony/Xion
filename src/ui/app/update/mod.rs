//! Message handling for XionApp.
//!
//! Contains the main `update()` method that processes all `UiMessage` variants,
//! and the `update_for_test()` wrapper for integration tests.

use std::path::PathBuf;

use iced::Task;

use crate::ui::UiMessage;

use super::XionApp;
use super::archive;
use super::shell;

/// Cap on messages held back during a mouse gesture.
///
/// A gesture is short; anything beyond this means something is producing
/// messages far faster than the user can drag, and queueing them all would
/// only delay the interface further.
const MAX_DEFERRED_MESSAGES: usize = 256;

impl XionApp {
    /// Visible to integration tests; the Iced framework calls this without `pub`.
    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn update_for_test(&mut self, message: UiMessage) -> Task<UiMessage> {
        self.update(message)
    }

    pub(crate) fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        let mut tasks = Vec::new();

        let message = if self.is_user_selecting {
            match self.intercept_during_selection(message) {
                Ok(task) => return task,
                Err(message) => message,
            }
        } else {
            message
        };

        // Each handler owns one domain and hands anything else to the next.
        // `Flow::Stop` reproduces the arms that used to `return` from `update()`
        // directly, deliberately skipping the end-of-update work below.
        macro_rules! dispatch {
            ($handler:ident, $message:expr) => {
                match self.$handler($message, &mut tasks) {
                    Ok(Flow::Continue) => return self.finish_update(tasks),
                    Ok(Flow::Stop(task)) => return task,
                    Err(message) => message,
                }
            };
        }

        let message = dispatch!(update_navigation, message);
        let message = dispatch!(update_input, message);
        let message = dispatch!(update_file_ops, message);
        let message = dispatch!(update_media, message);
        let message = dispatch!(update_search, message);
        let message = dispatch!(update_archive, message);
        let message = dispatch!(update_terminal, message);

        match self.update_tools(message, &mut tasks) {
            Flow::Continue => self.finish_update(tasks),
            Flow::Stop(task) => task,
        }
    }

    /// Work that runs at the end of every message that did not return early.
    fn finish_update(&mut self, mut tasks: Vec<Task<UiMessage>>) -> Task<UiMessage> {
        tasks.push(self.request_visible_thumbnails());
        tasks.push(self.request_selected_preview());
        Task::batch(tasks)
    }

    /// Filter messages arriving while a mouse gesture is in progress.
    ///
    /// `Err(message)` means "let it through to the normal handlers".
    fn intercept_during_selection(
        &mut self,
        message: UiMessage,
    ) -> Result<Task<UiMessage>, UiMessage> {
        match &message {
            UiMessage::Refresh => {
                self.pending_refresh = true;
                self.pending_refresh_reload_config = true;
                return Ok(Task::none());
            }
            UiMessage::FileWatchTick => {
                let events = self.poll_watcher();
                if let (Some(watched_path), Some(events)) = (self.watched_path.as_deref(), events) {
                    let should_refresh = events
                        .iter()
                        .any(|event| Self::is_event_relevant(event, watched_path));
                    if should_refresh && !self.is_refreshing {
                        self.pending_refresh = true;
                    }
                }
                return Ok(Task::none());
            }
            // Mouse tracking, modifier keys, navigation and keyboard commands
            // must keep working during a gesture.
            UiMessage::MouseReleased
            | UiMessage::CursorMoved(_)
            | UiMessage::ModifiersChanged(_)
            | UiMessage::NavigateTo(_)
            | UiMessage::KeyboardCommand(_) => return Err(message),
            _ => {}
        }

        // Everything else is queued rather than dropped: page loads, thumbnails
        // and finished file operations that landed while the mouse was held
        // down used to vanish, leaving the list half-loaded.
        if Self::is_deferrable(&message) && self.deferred_messages.len() < MAX_DEFERRED_MESSAGES {
            self.deferred_messages.push(message);
        }
        Ok(Task::none())
    }
}

/// What a domain handler did with the message.
pub(super) enum Flow {
    /// The arm ran; the shared end-of-update work still applies.
    Continue,
    /// The arm returned early, deliberately skipping the end-of-update work.
    Stop(Task<UiMessage>),
}

mod archives;
mod file_ops;
mod input;
mod media;
mod navigation;
mod search;
mod terminal;
mod tools;

/// Start the configured shell on a blocking task.
///
/// Opening a pty forks a process; doing it inside a plain future would hold a
/// tokio worker for the duration.
fn spawn_shell_task(shell: crate::core::ShellConfig, cwd: std::path::PathBuf) -> Task<UiMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                shell::spawn_shell_process(&shell, &cwd).map_err(|error| error.to_string())
            })
            .await
            .unwrap_or_else(|error| Err(error.to_string()))
        },
        UiMessage::TerminalSpawned,
    )
}

impl XionApp {
    /// Tell the pty how many rows and columns the panel can show.
    ///
    /// Approximated from the window geometry and the monospace metrics of the
    /// caption font. Being a few cells off is harmless; having no size at all
    /// is not — the shell then assumes 80x24 and wraps in the wrong place.
    pub(super) fn resize_terminal_pty(&self) {
        let Some(process) = self.terminal.active_ref().process.clone() else {
            return;
        };
        let (width, _height) = self.window_size;
        let tokens = crate::ui::theme::UiTokens::for_theme(&self.state.config.theme);
        let cell_width = (tokens.typography.caption * 0.6).max(1.0);
        let cell_height = (tokens.typography.caption * 1.35).max(1.0);

        let cols = (width / cell_width).floor().clamp(20.0, 1000.0) as u16;
        let rows = (crate::ui::theme::layout::TERMINAL_DEFAULT_HEIGHT / cell_height)
            .floor()
            .clamp(5.0, 500.0) as u16;
        process.resize(rows, cols);
    }
}

impl XionApp {
    /// Whether a message queued during a mouse gesture is worth replaying.
    ///
    /// Periodic ticks are not: they fire again on their own, and replaying a
    /// backlog of them would just burn a frame each.
    fn is_deferrable(message: &UiMessage) -> bool {
        !matches!(
            message,
            UiMessage::FileWatchTick
                | UiMessage::TerminalPollOutput
                | UiMessage::AnimatedPreviewTick(_)
                | UiMessage::OperationProgressTick
                | UiMessage::PreviewAnimTick
                | UiMessage::TerminalAnimTick
                | UiMessage::Noop
        )
    }
}

/// Which archive format a compress action produces.
#[derive(Debug, Clone, Copy)]
pub(super) enum ArchiveFormat {
    Zip,
    TarGz,
    SevenZ,
}

impl ArchiveFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarGz => "tar.gz",
            Self::SevenZ => "7z",
        }
    }
}

impl XionApp {
    /// Compress the current selection into a new archive.
    ///
    /// The three formats used to be three near-identical blocks inside
    /// `update()`. They also shared three defects: the archive name came from
    /// `selected[0]` of a set iterated in hash order, so it changed between
    /// runs; an existing archive of that name was silently truncated; and the
    /// archive was written into the directory being compressed, so it could end
    /// up inside itself.
    pub(super) fn compress_selection(&mut self, format: ArchiveFormat) -> Task<UiMessage> {
        let mut selected: Vec<PathBuf> = self
            .state
            .navigation
            .selection
            .selected
            .iter()
            .cloned()
            .collect();
        if selected.is_empty() {
            self.last_action = Some("Aucune sélection à compresser".to_string());
            return Task::none();
        }
        // Hash-set order is not stable across runs; sorting makes both the name
        // and the archive contents reproducible.
        selected.sort();

        let first = selected[0].clone();
        let output_dir = self.state.route.local_path().cloned().unwrap_or_else(|| {
            first
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        });
        let stem = first
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("archive")
            .to_string();

        let Some(out_path) = Self::available_archive_path(&output_dir, &stem, format.extension())
        else {
            self.last_action =
                Some("Impossible de trouver un nom d'archive disponible".to_string());
            return Task::none();
        };

        // Never feed the archive to itself.
        selected.retain(|source| source != &out_path);

        self.last_action = Some("Compression en cours...".to_string());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match format {
                    ArchiveFormat::Zip => archive::create_zip(&selected, &out_path),
                    ArchiveFormat::TarGz => archive::create_tar_gz(&selected, &out_path),
                    ArchiveFormat::SevenZ => archive::create_7z(&selected, &out_path),
                })
                .await
                .unwrap_or_else(|error| Err(error.to_string()))
            },
            UiMessage::CompressCompleted,
        )
    }

    /// First unused `<stem>.<ext>` / `<stem> (N).<ext>` in `dir`.
    ///
    /// `File::create` truncates, so writing straight to `<stem>.<ext>`
    /// destroyed an existing archive of the same name without a word.
    fn available_archive_path(
        dir: &std::path::Path,
        stem: &str,
        extension: &str,
    ) -> Option<PathBuf> {
        let candidate = dir.join(format!("{stem}.{extension}"));
        if !candidate.exists() {
            return Some(candidate);
        }
        (2..1000).find_map(|index| {
            let candidate = dir.join(format!("{stem} ({index}).{extension}"));
            (!candidate.exists()).then_some(candidate)
        })
    }
}
