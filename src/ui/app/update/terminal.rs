//! The integrated terminal panel.
//!
//! Emulation belongs to `iced_term`, backed by `alacritty_terminal`: a cell
//! grid, a two-dimensional cursor, the alternate screen, keystrokes passed
//! through untouched. This module only decides *when* an emulator is born and
//! dies, and which tab receives what it produces.
//!
//! Gone with the old model: the input line at the bottom of the panel, the Tab
//! completion it forced on us, and the 50 ms polling. The shell does its own
//! completion, and the pseudo-terminal reports on its own that it has written.

use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

/// Emulator identifiers, unique for the whole life of the program.
///
/// `iced_term` uses them to pair a subscription with its terminal. The tab
/// index would not do: closing a tab shifts the ones after it, and events in
/// flight would be delivered to the wrong one.
static NEXT_TERMINAL_ID: AtomicU64 = AtomicU64::new(0);

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
                    // Shutdown: the emulators go, and their shells with them.
                    for index in 0..self.terminal.tabs.len() {
                        self.retire_terminal(index);
                    }
                    self.terminal_anim_target = 0.0;
                } else {
                    self.terminal_anim_target = 1.0;
                    let cwd = self.current_directory();
                    self.terminal.active().cwd = Some(cwd);
                    self.start_active_terminal(tasks);
                }
            }
            UiMessage::TerminalEvent(event) => {
                let iced_term::Event::BackendCall(id, command) = event;

                let Some(tab) = self
                    .terminal
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.terminal.as_ref().is_some_and(|term| term.id == id))
                else {
                    // The tab was closed while the event was in flight.
                    return Ok(Flow::Continue);
                };

                let Some(terminal) = tab.terminal.as_mut() else {
                    return Ok(Flow::Continue);
                };

                let mut shutting_down = false;
                match terminal.handle(iced_term::Command::ProxyToBackend(command)) {
                    iced_term::actions::Action::Shutdown => {
                        // The shell gave control back: `exit`, or a crash. The
                        // stream has seen `Exit` go by, so it will not panic,
                        // but it still goes through the retirement list so
                        // there is a single way out.
                        shutting_down = true;
                    }
                    iced_term::actions::Action::ChangeTitle(title) => {
                        // What the program wants to be called: the shell usually
                        // puts the current directory or the command there.
                        if !title.trim().is_empty() {
                            tab.title = title;
                        }
                    }
                    iced_term::actions::Action::Ignore => {}
                }

                if shutting_down {
                    let index = self
                        .terminal
                        .tabs
                        .iter()
                        .position(|tab| tab.terminal.as_ref().is_some_and(|t| t.id == id));
                    if let Some(index) = index {
                        self.retire_terminal(index);
                        // Without this word the panel fell back to "Starting
                        // the shell..." and kept it forever: a message that
                        // lies, since nothing is starting.
                        if let Some(tab) = self.terminal.tabs.get_mut(index) {
                            tab.notice = Some(
                                "L'interpréteur s'est arrêté. Repliez et rouvrez le panneau \
                                 pour en relancer un."
                                    .to_string(),
                            );
                        }
                    }
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
                self.terminal.tabs.push(TerminalTab {
                    title: format!("Terminal {count}"),
                    ..Default::default()
                });
                self.terminal.active_tab = self.terminal.tabs.len() - 1;
                let cwd = self.current_directory();
                self.terminal.active().cwd = Some(cwd);
                self.start_active_terminal(tasks);
            }
            UiMessage::TerminalCloseTab(index) => {
                if self.terminal.tabs.len() > 1 && index < self.terminal.tabs.len() {
                    // Removing the tab takes its emulator with it, so it has to
                    // be retired first, otherwise it is dropped right here.
                    self.retire_terminal(index);
                    let previously_active = self.terminal.active_tab;
                    self.terminal.tabs.remove(index);
                    self.terminal.active_tab =
                        active_after_close(previously_active, index, self.terminal.tabs.len());
                }
            }
            UiMessage::TerminalSwitchTab(index) => {
                if index < self.terminal.tabs.len() {
                    self.terminal.active_tab = index;
                    // A tab opened while the panel was collapsed has no shell
                    // yet: it earns one when the user comes to it.
                    if self.terminal_anim_target > 0.5 {
                        self.start_active_terminal(tasks);

                        // And the focus, even when the shell already existed.
                        //
                        // Focus state lives in the widget tree, and `iced_term`
                        // resets it as soon as the terminal id changes, so on
                        // every tab switch. Without this the displayed widget
                        // was deaf: keystrokes went back to Xion's shortcuts,
                        // which is exactly the symptom we thought was gone.
                        if let Some(widget_id) = self
                            .terminal
                            .active_ref()
                            .and_then(|tab| tab.terminal.as_ref())
                            .map(|terminal| terminal.widget_id().clone())
                        {
                            tasks.push(iced_term::TerminalView::focus(widget_id));
                        }
                    }
                }
            }
            UiMessage::ReapRetiredTerminals => {
                // Nobody listens on their channel any more: iced recomputed its
                // subscriptions between the previous batch and this one.
                self.retired_terminals.0.clear();
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
            UiMessage::TerminalInterrupt => {
                // `0x03`, the byte Ctrl+C sends. The shortcut now works on its
                // own when the terminal has focus; this button stays for anyone
                // who does not know it.
                const ETX: u8 = 0x03;
                match self.terminal.active().terminal.as_mut() {
                    Some(terminal) => {
                        terminal.handle(iced_term::Command::ProxyToBackend(
                            iced_term::BackendCommand::Write(vec![ETX]),
                        ));
                    }
                    None => self
                        .terminal
                        .push_notice("Aucun processus à interrompre.".to_string()),
                }
            }
            UiMessage::SetShell(shell) => {
                self.state.config.terminal_shell = shell;
                // The running emulator carries the old shell: it goes.
                self.retire_terminal(self.terminal.active_tab);
                self.config_manager.save(&self.state.config);

                // And it has to be replaced right away. The original comment
                // said "so next open uses new shell", but the shell can only be
                // changed with the panel open: there is no next open. The
                // terminal stayed silent, and the first command typed answered
                // "terminal not started" with nothing to explain why.
                if self.terminal_anim_target > 0.5 {
                    self.start_active_terminal(tasks);
                }
            }
            UiMessage::WindowResized(width, height) => {
                self.window_size = (width, height);
            }
            other => return Err(other),
        }

        // While retired emulators remain, ask again for their destruction in a
        // later batch. Not doing it in place is the whole point: iced handles a
        // full batch before recomputing its subscriptions.
        if !self.retired_terminals.0.is_empty() {
            tasks.push(Task::done(UiMessage::ReapRetiredTerminals));
        }

        Ok(Flow::Continue)
    }

    /// Retire an emulator instead of dropping it in place.
    ///
    /// See [`RetiredTerminals`]: dropping a `Terminal` closes its channel, and
    /// the subscription still listening to it calls `panic!` when it notices.
    /// Keeping it alive until the next round gives iced time to remove that
    /// subscription.
    fn retire_terminal(&mut self, index: usize) {
        if let Some(tab) = self.terminal.tabs.get_mut(index)
            && let Some(terminal) = tab.terminal.take()
        {
            self.retired_terminals.0.push(terminal);
        }
    }

    /// The directory the terminal should open in.
    fn current_directory(&self) -> std::path::PathBuf {
        self.state
            .route
            .local_path()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Give the active tab a shell if it does not have one yet.
    ///
    /// Does nothing when an emulator is already alive: reopening the panel or
    /// coming back to a tab must not abandon the program running in it.
    fn start_active_terminal(&mut self, tasks: &mut Vec<Task<UiMessage>>) {
        if self
            .terminal
            .active_ref()
            .is_some_and(|tab| tab.terminal.is_some())
        {
            return;
        }

        let fallback = self.current_directory();
        let working_directory = self
            .terminal
            .active_ref()
            .and_then(|tab| tab.cwd.clone())
            .unwrap_or(fallback);

        let (program, args) =
            match crate::ui::app::shell::resolve(&self.state.config.terminal_shell) {
                Ok(resolved) => resolved,
                Err(error) => {
                    self.terminal
                        .push_notice(format!("Interpréteur introuvable : {error}"));
                    return;
                }
            };

        // Xion's font is already monospaced (JetBrains Mono Nerd Font), so the
        // terminal inherits it, and the Nerd Font glyphs modern prompts draw
        // render without a fallback font.
        let tokens = crate::ui::theme::UiTokens::for_theme(&self.state.config.theme);

        let settings = iced_term::settings::Settings {
            font: iced_term::settings::FontSettings {
                size: tokens.typography.body,
                font_type: tokens.typography.body_font,
                ..Default::default()
            },
            theme: iced_term::settings::ThemeSettings::new(Box::new(terminal_palette(
                tokens.colors,
            ))),
            backend: iced_term::settings::BackendSettings {
                program,
                args,
                working_directory: Some(working_directory),
                env: terminal_environment(),
            },
        };

        let id = NEXT_TERMINAL_ID.fetch_add(1, Ordering::Relaxed);
        match iced_term::Terminal::new(id, settings) {
            Ok(mut terminal) => {
                // Give a plausible size right away, in pixels.
                //
                // The crate's `TerminalSize::default()` declares an area of
                // 80x50 *pixels* with 1x1 cells. On the first font sync (which
                // `handle` triggers on every command) the real cell size
                // arrives while the area is still 80x50 pixels, and the grid
                // drops to `80/8 = 10` columns by `50/18 = 2` rows. On ten
                // columns a zsh prompt wraps on every keystroke and zle redraws
                // endlessly: characters seemed to duplicate while the shell
                // received exactly what was sent.
                //
                // The estimate does not need to be right, only of the right
                // order: the widget publishes the exact measure as soon as it
                // is in the tree, and the kernel only tells the shell when the
                // size really changes.
                let (window_width, _) = self.window_size;
                terminal.handle(iced_term::Command::ProxyToBackend(
                    iced_term::BackendCommand::Resize(
                        Some(iced::Size {
                            width: window_width.max(200.0),
                            height: crate::ui::theme::layout::TERMINAL_DEFAULT_HEIGHT,
                        }),
                        None,
                    ),
                ));

                let widget_id = terminal.widget_id().clone();
                let tab = self.terminal.active();
                tab.notice = None;
                tab.terminal = Some(terminal);

                // Without this, the grid had to be clicked before any key
                // reached it: the widget ignores the keyboard until it has
                // focus. The crate's official example does the same every time
                // it creates a pane.
                tasks.push(iced_term::TerminalView::focus(widget_id));
            }
            Err(error) => self
                .terminal
                .push_notice(format!("Le terminal n'a pas démarré : {error}")),
        }
    }
}

/// The shell's environment: what it needs to know about the terminal hosting it.
///
/// **This is the fix for duplicated characters.** `alacritty_terminal` provides
/// `setup_env()`, which sets `TERM` and `COLORTERM`, but `iced_term` never calls
/// it. The shell therefore inherited Xion's `TERM`, which is *nothing* when Xion
/// is started from the desktop rather than from a terminal.
///
/// Without `TERM`, zsh assumes a terminal that cannot move its cursor. To
/// recolour its line after each keystroke it does not go back to the start, it
/// rewrites the line after itself. Hence `wwrriittee` for "write", while the
/// output of `echo hello` stayed perfectly clean: printing text needs no
/// positioning.
fn terminal_environment() -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    env.insert("TERM".to_string(), terminfo_name().to_string());
    // The emulator renders true colour; without this variable, programs that
    // read it fall back to 256 colours.
    env.insert("COLORTERM".to_string(), "truecolor".to_string());
    env
}

/// The terminfo name to announce.
///
/// The same choice as alacritty: its own entry when the machine has it,
/// otherwise the universal fallback. Announcing `alacritty` on a system without
/// the entry would be worse than announcing nothing: the shell would look for
/// capabilities it cannot find.
fn terminfo_name() -> &'static str {
    if terminfo_exists("alacritty") {
        "alacritty"
    } else {
        "xterm-256color"
    }
}

/// Look for a terminfo entry where the C library would look for it.
///
/// Entries live under a directory named after their first letter, literal on
/// most systems and in hexadecimal on those that follow the recent ncurses
/// convention. Both are tried.
fn terminfo_exists(name: &str) -> bool {
    let first = match name.chars().next() {
        Some(letter) => letter,
        None => return false,
    };
    let directories = [format!("{first}"), format!("{:x}", first as usize)];

    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(explicit) = std::env::var_os("TERMINFO") {
        roots.push(std::path::PathBuf::from(explicit));
    }
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(std::path::Path::new(&home).join(".terminfo"));
    }
    roots.extend(
        ["/usr/share/terminfo", "/lib/terminfo", "/usr/lib/terminfo"]
            .into_iter()
            .map(std::path::PathBuf::from),
    );

    roots.iter().any(|root| {
        directories
            .iter()
            .any(|directory| root.join(directory).join(name).exists())
    })
}

/// The terminal's background and text, taken from the active Colony theme.
///
/// The sixteen ANSI colours stay those of `iced_term`: they describe what *the
/// program* asks for ("red", "green"), and deriving them from an interface
/// palette would make `ls --color` lie about what it announces. The background
/// and foreground, on the other hand, belong to the window.
fn terminal_palette(colors: crate::ui::theme::UiColors) -> iced_term::ColorPalette {
    iced_term::ColorPalette {
        background: hex(colors.panel_background),
        foreground: hex(colors.text_primary),
        ..Default::default()
    }
}

/// `iced_term` expects its colours as hexadecimal text.
fn hex(color: iced::Color) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

#[cfg(test)]
mod tests {
    use super::hex;
    use iced::Color;

    #[test]
    fn a_colour_becomes_the_hex_string_the_emulator_expects() {
        assert_eq!(hex(Color::BLACK), "#000000");
        assert_eq!(hex(Color::WHITE), "#FFFFFF");
        assert_eq!(hex(Color::from_rgb(1.0, 0.0, 0.0)), "#FF0000");
    }

    /// The shell must know which terminal it is talking to.
    ///
    /// Without `TERM`, zsh believes it cannot move its cursor and rewrites its
    /// line after itself instead of redrawing it: every character appeared
    /// twice. The crate does not set this variable; that is the host's job, and
    /// nothing reminds it.
    #[test]
    fn the_shell_is_told_which_terminal_it_speaks_to() {
        let env = super::terminal_environment();
        let term = env.get("TERM").expect("TERM must be passed to the shell");
        assert!(
            !term.trim().is_empty(),
            "an empty TERM is as good as a missing TERM for the shell"
        );
        assert_eq!(env.get("COLORTERM").map(String::as_str), Some("truecolor"));
    }

    /// And that name must exist in the system database, otherwise the shell
    /// looks for capabilities it cannot find, which is worse than announcing
    /// nothing.
    ///
    /// Unix only: Windows has no terminfo database, ConPTY does the translating.
    #[cfg(not(windows))]
    #[test]
    fn the_announced_terminfo_entry_exists_on_this_machine() {
        let name = super::terminfo_name();
        assert!(
            super::terminfo_exists(name),
            "\"{name}\" is announced but missing from the terminfo database"
        );
    }

    /// The universal fallback must stay reachable: it is what rescues machines
    /// without alacritty's entry.
    #[test]
    fn an_unknown_entry_is_not_claimed_to_exist() {
        assert!(!super::terminfo_exists("no-such-terminal-entry"));
    }

    /// An out-of-range component must not overflow the byte.
    ///
    /// Built field by field rather than with `Color::from_rgb`, which asserts
    /// against exactly what this test wants to exercise.
    #[test]
    fn an_out_of_range_channel_is_clamped() {
        let out_of_range = Color {
            r: 2.0,
            g: -1.0,
            b: 0.5,
            a: 1.0,
        };
        assert_eq!(hex(out_of_range), "#FF0080");
    }
}

/// The active tab after the tab at index `closed` is closed.
///
/// Closing a tab *before* the active one shifts every following tab by one. The
/// previous version only clamped the upper bound, so closing the first tab while
/// on the second showed the third: the user ended up in front of a different
/// terminal from the one they were looking at. The "active is last" case worked
/// by accident.
fn active_after_close(active: usize, closed: usize, remaining: usize) -> usize {
    let shifted = if closed < active { active - 1 } else { active };
    shifted.min(remaining.saturating_sub(1))
}

#[cfg(test)]
mod close_tab_tests {
    use super::active_after_close;

    /// The case that was wrong: closing before the active tab shifts it.
    #[test]
    fn closing_before_the_active_tab_follows_the_shift() {
        // [T1, T2, T3], looking at T2, closing T1 leaves [T2, T3] and T2 is
        // now at position 0.
        assert_eq!(active_after_close(1, 0, 2), 0);
        assert_eq!(active_after_close(2, 0, 2), 1);
    }

    /// Closing after the active tab moves nothing.
    #[test]
    fn closing_after_the_active_tab_leaves_it_alone() {
        assert_eq!(active_after_close(0, 1, 2), 0);
        assert_eq!(active_after_close(1, 2, 2), 1);
    }

    /// Closing the active tab itself: stay in place, unless it was the last.
    #[test]
    fn closing_the_active_tab_keeps_the_position_or_steps_back() {
        assert_eq!(active_after_close(1, 1, 2), 1);
        assert_eq!(active_after_close(2, 2, 2), 1);
    }

    /// The case that worked by accident, and must keep working.
    #[test]
    fn the_last_tab_never_points_past_the_end() {
        assert_eq!(active_after_close(1, 1, 1), 0);
        assert_eq!(active_after_close(0, 0, 1), 0);
    }
}
