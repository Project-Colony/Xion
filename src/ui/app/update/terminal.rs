//! The integrated terminal panel.
//!
//! Extracted from the single 2161-line `update()` match. The arms are
//! unchanged: this is a move, not a rewrite.

use std::path::PathBuf;

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::{Flow, spawn_shell_task};
use crate::ui::app::XionApp;

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_terminal(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::ToggleTerminal => {
                if self.terminal_anim_target > 0.5 {
                    // Close: drop processes on all tabs
                    for tab in &mut self.terminal.tabs {
                        tab.process = None;
                    }
                    self.terminal_anim_target = 0.0;
                } else {
                    // Open: spawn a persistent cmd.exe session
                    self.terminal_anim_target = 1.0;
                    let cwd = self
                        .state
                        .route
                        .local_path()
                        .cloned()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    self.terminal.active().cwd = Some(cwd.clone());
                    let shell = self.state.config.terminal_shell.clone();
                    tasks.push(spawn_shell_task(shell, cwd));
                }
            }
            UiMessage::TerminalSpawned(result) => match result {
                Ok(process) => {
                    self.terminal.active().process = Some(process);
                    // A fresh pty starts at the default geometry; hand it the
                    // real panel size straight away.
                    self.resize_terminal_pty();
                }
                Err(e) => {
                    self.terminal
                        .push_notice(format!("Erreur démarrage terminal : {e}"));
                    self.terminal_anim_target = 0.0;
                }
            },
            UiMessage::TerminalInputChanged(input) => {
                self.terminal.active().input = input;
            }
            UiMessage::TerminalInputSubmitted => {
                let cmd = self.terminal.active_ref().input.trim().to_string();
                if cmd.is_empty() {
                    return Ok(Flow::Stop(Task::batch(std::mem::take(tasks))));
                }
                self.terminal.active().input.clear();
                // No local prompt echo any more: the shell owns the pty and
                // echoes the command itself, so printing our own `cwd>` line
                // produced a duplicate. `cd` tracking is gone for the same
                // reason — the shell keeps its own working directory, and the
                // 30 lines that re-parsed `cd` by hand could only ever drift.
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    tasks.push(Task::perform(
                        // Writing to a pty can block when the child is not
                        // reading, so it does not belong on the UI thread.
                        async move {
                            let _ =
                                tokio::task::spawn_blocking(move || process.write_line(&cmd)).await;
                        },
                        |()| UiMessage::Noop,
                    ));
                } else {
                    self.terminal
                        .push_notice("Erreur : terminal non démarré.".to_string());
                }
            }
            UiMessage::TerminalPollOutput => {
                // Drain every tab, not only the visible one.
                let active = self.terminal.active_tab;
                let mut active_changed = false;
                for (index, tab) in self.terminal.tabs.iter_mut().enumerate() {
                    let Some(process) = tab.process.clone() else {
                        continue;
                    };
                    // `take_output` returns None when the screen has not
                    // changed, so a quiet terminal costs one atomic read
                    // instead of a full rebuild of the cached output.
                    let Some(lines) = process.take_output() else {
                        continue;
                    };
                    tab.set_lines(lines);
                    if index == active {
                        active_changed = true;
                    }
                }
                if active_changed {
                    // Auto-scroll terminal to bottom
                    tasks.push(iced::widget::operation::scroll_to(
                        iced::widget::Id::new("terminal_output"),
                        iced::widget::scrollable::AbsoluteOffset {
                            x: 0.0,
                            y: f32::MAX,
                        },
                    ));
                }
            }
            UiMessage::TerminalAddTab => {
                const MAX_TERMINAL_TABS: usize = 10;
                if self.terminal.tabs.len() >= MAX_TERMINAL_TABS {
                    self.last_action =
                        Some(format!("Maximum {MAX_TERMINAL_TABS} onglets terminal"));
                    return Ok(Flow::Stop(Task::none()));
                }
                let count = self.terminal.tabs.len() + 1;
                let tab = TerminalTab {
                    title: format!("Terminal {}", count),
                    ..Default::default()
                };
                self.terminal.tabs.push(tab);
                self.terminal.active_tab = self.terminal.tabs.len() - 1;
                let cwd = self
                    .state
                    .route
                    .local_path()
                    .cloned()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                self.terminal.active().cwd = Some(cwd.clone());
                let shell = self.state.config.terminal_shell.clone();
                tasks.push(spawn_shell_task(shell, cwd));
            }
            UiMessage::TerminalCloseTab(index) => {
                if self.terminal.tabs.len() > 1 {
                    if let Some(tab) = self.terminal.tabs.get_mut(index) {
                        tab.process = None;
                    }
                    if index < self.terminal.tabs.len() {
                        self.terminal.tabs.remove(index);
                    }
                    if self.terminal.active_tab >= self.terminal.tabs.len() {
                        self.terminal.active_tab = self.terminal.tabs.len() - 1;
                    }
                }
            }
            UiMessage::TerminalSwitchTab(index) => {
                if index < self.terminal.tabs.len() {
                    self.terminal.active_tab = index;
                }
            }
            UiMessage::TerminalAnimTick => {
                let speed = 0.15;
                let diff = self.terminal_anim_target - self.terminal_anim_progress;
                if diff.abs() < 0.005 {
                    self.terminal_anim_progress = self.terminal_anim_target;
                } else {
                    self.terminal_anim_progress += diff * speed;
                }
            }
            // Feature 1: Trash
            UiMessage::TerminalAutoComplete => {
                let input = self.terminal.active_ref().input.clone();
                if !input.is_empty() {
                    let fallback = self
                        .state
                        .route
                        .local_path()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("."));
                    let cwd = self.terminal.effective_cwd(&fallback).to_path_buf();
                    // Try to complete the last word as a path
                    let last_word = input.split_whitespace().last().unwrap_or("");
                    let (search_dir, prefix) = if let Some(sep_pos) = last_word.rfind(['\\', '/']) {
                        let dir_part = &last_word[..=sep_pos];
                        let file_part = &last_word[sep_pos + 1..];
                        let search_path = if std::path::Path::new(dir_part).is_absolute() {
                            PathBuf::from(dir_part)
                        } else {
                            cwd.join(dir_part)
                        };
                        (search_path, file_part.to_lowercase())
                    } else {
                        (cwd.clone(), last_word.to_lowercase())
                    };
                    if let Ok(entries) = std::fs::read_dir(&search_dir) {
                        let mut matches: Vec<String> = entries
                            .flatten()
                            .filter_map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                if name.to_lowercase().starts_with(&prefix) {
                                    Some(name)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        matches.sort();
                        if let Some(first_match) = matches.first() {
                            // Replace the last word with the completion
                            let words: Vec<&str> = input.split_whitespace().collect();
                            let completed = if words.len() > 1 {
                                let leading = &words[..words.len() - 1];
                                let last_parts: Vec<&str> =
                                    last_word.rsplitn(2, ['\\', '/']).collect();
                                if last_parts.len() > 1 {
                                    // Has directory prefix: preserve it
                                    let dir_prefix =
                                        &last_word[..last_word.len() - last_parts[0].len()];
                                    format!("{} {}{}", leading.join(" "), dir_prefix, first_match)
                                } else {
                                    format!("{} {}", leading.join(" "), first_match)
                                }
                            } else {
                                let last_parts: Vec<&str> =
                                    last_word.rsplitn(2, ['\\', '/']).collect();
                                if last_parts.len() > 1 {
                                    let dir_prefix =
                                        &last_word[..last_word.len() - last_parts[0].len()];
                                    format!("{}{}", dir_prefix, first_match)
                                } else {
                                    first_match.clone()
                                }
                            };
                            self.terminal.active().input = completed;
                        }
                    }
                }
            }
            // ── #21: Trash browsing ──────────────────────────────────────────
            UiMessage::TerminalInterrupt => {
                if let Some(process) = self.terminal.active_ref().process.clone() {
                    process.interrupt();
                } else {
                    self.terminal
                        .push_notice("Aucun processus à interrompre.".to_string());
                }
            }
            UiMessage::SetShell(shell) => {
                self.state.config.terminal_shell = shell;
                // Kill current process so next open uses new shell
                self.terminal.active().process = None;
                self.config_manager.save(&self.state.config);
            }
            // ── Feature K: Hex Viewer ─────────────────────────────────────────
            UiMessage::WindowResized(width, height) => {
                self.window_size = (width, height);
                self.resize_terminal_pty();
            }
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }
}
