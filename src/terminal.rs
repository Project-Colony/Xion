//! Persistent shell session backed by a real pseudo-terminal.
//!
//! The previous implementation piped stdin/stdout of a `cmd.exe` child. That
//! design could not be fixed by degrees: without a tty there is no Ctrl-C, no
//! window size, no VT sequences, and no output at all until a `\n` arrives — so
//! `pause` and password prompts hung with a blank panel. It also depended on a
//! `PROMPT $_` hack and a `---XION_READY---` sentinel that, if it never came,
//! left the terminal mute for the rest of the session.
//!
//! This module opens a real pty through `portable-pty` (ConPTY on Windows,
//! `openpty` on Unix) and runs the bytes through a `vte` parser, so escape
//! sequences are interpreted instead of being printed as garbage.
//!
//! Scope: the screen model is a scrollback of lines with a cursor inside the
//! current one. That covers prompts, colours, `\r` progress bars and clearing.
//! It is **not** a full-screen grid, so `vim` and `htop` are out of scope.
//!
//! The handle is cheap to clone (Arc-backed) and can travel through UiMessage.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Lines of scrollback kept per session.
///
/// The old buffer held 500 lines *and* was drained every 50 ms into a second
/// 500-line buffer, so a build log lost its beginning twice over.
const SCROLLBACK_MAX: usize = 10_000;

const DEFAULT_ROWS: u16 = 30;
const DEFAULT_COLS: u16 = 120;

/// ETX — what a terminal sends when you press Ctrl-C.
const CTRL_C: u8 = 0x03;

// ── Screen model ─────────────────────────────────────────────────────────────

/// A scrollback of completed lines plus the line being written.
///
/// The cursor is a column inside `current`, so `\r` followed by more text
/// overwrites in place — which is how every progress bar works — instead of
/// appending a second copy.
#[derive(Debug, Default)]
struct Screen {
    lines: VecDeque<String>,
    current: Vec<char>,
    cursor: usize,
    dirty: bool,
    dropped: u64,
}

impl Screen {
    fn newline(&mut self) {
        self.lines.push_back(self.current.iter().collect());
        self.current.clear();
        self.cursor = 0;
        if self.lines.len() > SCROLLBACK_MAX {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.dirty = true;
    }

    fn put(&mut self, c: char) {
        if self.cursor < self.current.len() {
            self.current[self.cursor] = c;
        } else {
            self.current.resize(self.cursor, ' ');
            self.current.push(c);
        }
        self.cursor += 1;
        self.dirty = true;
    }

    fn push_notice(&mut self, notice: &str) {
        if !self.current.is_empty() {
            self.newline();
        }
        self.lines.push_back(notice.to_string());
        self.dirty = true;
    }

    /// The full rendered buffer, including the line in progress.
    fn snapshot(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.lines.len() + 2);
        if self.dropped > 0 {
            out.push(format!(
                "… {} lignes plus anciennes tronquées …",
                self.dropped
            ));
        }
        out.extend(self.lines.iter().cloned());
        if !self.current.is_empty() {
            // Rendering the partial line is what makes `Password:` and progress
            // bars visible; they never reach a `\n`.
            out.push(self.current.iter().collect());
        }
        out
    }
}

impl vte::Perform for Screen {
    fn print(&mut self, c: char) {
        self.put(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => {
                self.cursor = 0;
                self.dirty = true;
            }
            0x08 => {
                self.cursor = self.cursor.saturating_sub(1);
                self.dirty = true;
            }
            b'\t' => {
                let stop = (self.cursor / 8 + 1) * 8;
                while self.cursor < stop {
                    self.put(' ');
                }
            }
            // Bell and anything else carry no text.
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let first = params
            .iter()
            .next()
            .and_then(|group| group.first().copied())
            .unwrap_or(0) as usize;

        match action {
            // Erase in line.
            'K' => {
                match first {
                    0 => self.current.truncate(self.cursor),
                    1 => {
                        let end = self.cursor.min(self.current.len());
                        self.current[..end].fill(' ');
                    }
                    _ => {
                        self.current.clear();
                        self.cursor = 0;
                    }
                }
                self.dirty = true;
            }
            // Erase in display: `clear` / `cls`.
            'J' if first >= 2 => {
                self.lines.clear();
                self.current.clear();
                self.cursor = 0;
                self.dropped = 0;
                self.dirty = true;
            }
            'C' => {
                self.cursor = self.cursor.saturating_add(first.max(1));
                self.dirty = true;
            }
            'D' => {
                self.cursor = self.cursor.saturating_sub(first.max(1));
                self.dirty = true;
            }
            'G' => {
                self.cursor = first.saturating_sub(1);
                self.dirty = true;
            }
            // SGR ('m'), cursor positioning, mode switches and the rest are
            // consumed and dropped. That alone is the fix for the raw `[0m`
            // litter the pipe-based version printed.
            _ => {}
        }
    }
}

// ── Inner state (Arc-shared across all clones) ───────────────────────────────

struct Inner {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    screen: Arc<Mutex<Screen>>,
    /// `\r\n` for cmd.exe and PowerShell, `\n` for POSIX shells. Sending CRLF
    /// to bash made every command arrive with a trailing `\r`.
    line_ending: &'static str,
    running: Arc<AtomicBool>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
    }
}

// ── Public handle ────────────────────────────────────────────────────────────

/// Which shell a session runs, and the line ending it expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Crlf,
    Lf,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "\r\n",
            Self::Lf => "\n",
        }
    }
}

/// Cloneable, cheaply shareable handle to a live shell session.
///
/// Dropping the last clone kills the child.
#[derive(Clone)]
pub struct TerminalProcess {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for TerminalProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalProcess").finish_non_exhaustive()
    }
}

impl TerminalProcess {
    /// Open a pty and start `program` in it.
    pub fn spawn(
        program: &str,
        args: &[&str],
        cwd: &Path,
        line_ending: LineEnding,
    ) -> std::io::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        let mut command = CommandBuilder::new(program);
        for arg in args {
            command.arg(arg);
        }
        command.cwd(cwd);
        // Tells the shell and its children they are on a colour-capable
        // terminal; without it many tools disable colour entirely.
        command.env("TERM", "xterm-256color");

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let killer = child.clone_killer();

        // The slave must be dropped now: while this process holds it open, the
        // master never sees EOF and the reader thread would hang forever after
        // the shell exits.
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        let screen = Arc::new(Mutex::new(Screen::default()));
        let running = Arc::new(AtomicBool::new(true));

        // A thread that cannot start is a real failure, not a panic: with
        // `panic = "abort"` in release it would have taken the whole app down.
        Self::spawn_reader(reader, Arc::clone(&screen))?;

        // The pty read loop cannot report the exit code, so the child is waited
        // on separately. Previously a dead shell was indistinguishable from an
        // idle one, and the UI kept accepting input into a closed pipe.
        {
            let screen = Arc::clone(&screen);
            let running = Arc::clone(&running);
            std::thread::Builder::new()
                .name("xion-pty-wait".to_string())
                .spawn(move || {
                    let note = match child.wait() {
                        Ok(status) if status.success() => "[shell terminé]".to_string(),
                        Ok(status) => format!("[shell terminé — code {}]", status.exit_code()),
                        Err(error) => format!("[shell interrompu : {error}]"),
                    };
                    running.store(false, Ordering::Release);
                    if let Ok(mut screen) = screen.lock() {
                        screen.push_notice(&note);
                    }
                })
                .map_err(std::io::Error::other)?;
        }

        Ok(Self {
            inner: Arc::new(Inner {
                writer: Mutex::new(writer),
                master: Mutex::new(pair.master),
                killer: Mutex::new(killer),
                screen,
                line_ending: line_ending.as_str(),
                running,
            }),
        })
    }

    /// Read the pty on a dedicated OS thread.
    ///
    /// `portable_pty`'s reader is blocking, so it must not sit on a tokio
    /// worker. stdout and stderr arrive already merged by the pty, which also
    /// removes the interleaving races of the two-pipe design.
    fn spawn_reader(
        mut reader: Box<dyn Read + Send>,
        screen: Arc<Mutex<Screen>>,
    ) -> std::io::Result<()> {
        std::thread::Builder::new()
            .name("xion-pty-read".to_string())
            .spawn(move || {
                let mut parser = vte::Parser::new();
                let mut buffer = [0u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => {
                            if let Ok(mut screen) = screen.lock() {
                                parser.advance(&mut *screen, &buffer[..read]);
                            }
                        }
                    }
                }
            })
            .map(|_| ())
    }

    /// Send a command line, terminated the way this shell expects.
    pub fn write_line(&self, line: &str) {
        self.write_bytes(format!("{line}{}", self.inner.line_ending).as_bytes());
    }

    /// Send Ctrl-C. The pty turns it into a real interrupt for the foreground
    /// process group, which the pipe-based version simply could not do.
    pub fn interrupt(&self) {
        self.write_bytes(&[CTRL_C]);
    }

    fn write_bytes(&self, bytes: &[u8]) {
        let Ok(mut writer) = self.inner.writer.lock() else {
            return;
        };
        if let Err(error) = writer.write_all(bytes) {
            tracing::warn!("Écriture terminal échouée : {error}");
            return;
        }
        if let Err(error) = writer.flush() {
            tracing::warn!("Flush terminal échoué : {error}");
        }
    }

    /// Tell the shell how big the panel is, so `less`, `git log` and friends
    /// wrap correctly.
    pub fn resize(&self, rows: u16, cols: u16) {
        let Ok(master) = self.inner.master.lock() else {
            return;
        };
        let _ = master.resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    /// The rendered buffer, or `None` when nothing changed since the last call.
    ///
    /// Returning `None` lets the UI skip rebuilding its cached output on the
    /// vast majority of polls.
    pub fn take_output(&self) -> Option<Vec<String>> {
        let mut screen = self.inner.screen.lock().ok()?;
        if !screen.dirty {
            return None;
        }
        screen.dirty = false;
        Some(screen.snapshot())
    }

    /// Whether the shell is still alive.
    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vte::Perform;

    fn feed(screen: &mut Screen, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        parser.advance(screen, bytes);
    }

    #[test]
    fn plain_lines_are_split_on_newline() {
        let mut screen = Screen::default();
        feed(&mut screen, b"premier\nsecond\n");
        assert_eq!(screen.snapshot(), vec!["premier", "second"]);
    }

    /// Regression: output without a trailing newline was never shown, so a
    /// password prompt left the panel blank.
    #[test]
    fn a_partial_line_is_still_rendered() {
        let mut screen = Screen::default();
        feed(&mut screen, b"Password: ");
        assert_eq!(screen.snapshot(), vec!["Password: "]);
    }

    /// A progress bar rewrites the same line with `\r`; appending would show
    /// every intermediate state.
    #[test]
    fn carriage_return_overwrites_in_place() {
        let mut screen = Screen::default();
        feed(&mut screen, b"10%\r100%");
        assert_eq!(screen.snapshot(), vec!["100%"]);
    }

    #[test]
    fn carriage_return_keeps_the_tail_it_did_not_overwrite() {
        let mut screen = Screen::default();
        feed(&mut screen, b"abcdef\rXY");
        assert_eq!(screen.snapshot(), vec!["XYcdef"]);
    }

    /// Regression: colour sequences were printed literally.
    #[test]
    fn sgr_sequences_are_consumed_not_printed() {
        let mut screen = Screen::default();
        feed(&mut screen, b"\x1b[31mrouge\x1b[0m\n");
        assert_eq!(screen.snapshot(), vec!["rouge"]);
    }

    #[test]
    fn erase_in_line_clears_from_the_cursor() {
        let mut screen = Screen::default();
        feed(&mut screen, b"abcdef\r\x1b[K");
        assert!(screen.snapshot().is_empty());
    }

    #[test]
    fn clear_screen_empties_the_scrollback() {
        let mut screen = Screen::default();
        feed(&mut screen, b"vieux\n\x1b[2J");
        assert!(screen.snapshot().is_empty());
    }

    #[test]
    fn backspace_moves_the_cursor_back() {
        let mut screen = Screen::default();
        feed(&mut screen, b"abc\x08X");
        assert_eq!(screen.snapshot(), vec!["abX"]);
    }

    #[test]
    fn tabs_advance_to_the_next_stop() {
        let mut screen = Screen::default();
        feed(&mut screen, b"ab\tc");
        assert_eq!(screen.snapshot(), vec!["ab      c"]);
    }

    #[test]
    fn utf8_survives_a_chunk_boundary() {
        let mut screen = Screen::default();
        let mut parser = vte::Parser::new();
        // "é" is two bytes; split them across two reads.
        parser.advance(&mut screen, &[0xC3]);
        parser.advance(&mut screen, &[0xA9, b'\n']);
        assert_eq!(screen.snapshot(), vec!["é"]);
    }

    /// Regression: the buffer silently dropped the head of long output.
    #[test]
    fn scrollback_overflow_is_announced() {
        let mut screen = Screen::default();
        for index in 0..(SCROLLBACK_MAX + 5) {
            screen.push_notice(&format!("ligne {index}"));
            screen.newline();
        }
        let snapshot = screen.snapshot();
        assert!(
            snapshot[0].contains("tronquées"),
            "première ligne : {}",
            snapshot[0]
        );
    }

    #[test]
    fn dirty_flag_gates_snapshots() {
        let mut screen = Screen::default();
        assert!(!screen.dirty);
        screen.print('x');
        assert!(screen.dirty);
    }

    /// End-to-end: a real pty, a real shell, real bytes coming back.
    #[cfg(unix)]
    #[test]
    fn a_real_shell_echoes_through_the_pty() {
        let process = TerminalProcess::spawn("/bin/sh", &[], Path::new("/tmp"), LineEnding::Lf)
            .expect("le pty doit s'ouvrir");
        process.write_line("echo xion-pty-ok");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut seen = Vec::new();
        while std::time::Instant::now() < deadline {
            if let Some(lines) = process.take_output() {
                seen = lines;
                if seen.iter().any(|line| line.contains("xion-pty-ok")) {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("aucune sortie contenant le marqueur ; vu : {seen:?}");
    }

    /// The shell exiting must flip `is_running` and leave a note, instead of
    /// leaving the panel silently accepting input into a dead process.
    #[cfg(unix)]
    #[test]
    fn shell_exit_is_detected() {
        let process = TerminalProcess::spawn("/bin/sh", &[], Path::new("/tmp"), LineEnding::Lf)
            .expect("le pty doit s'ouvrir");
        process.write_line("exit 3");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if !process.is_running() {
                let lines = process.take_output().unwrap_or_default();
                assert!(
                    lines.iter().any(|line| line.contains("code 3")),
                    "note de sortie absente ; vu : {lines:?}"
                );
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("la fin du shell n'a jamais été détectée");
    }

    /// Ctrl-C must actually interrupt: a pipe could not do this at all.
    #[cfg(unix)]
    #[test]
    fn interrupt_stops_a_running_command() {
        let process = TerminalProcess::spawn("/bin/sh", &[], Path::new("/tmp"), LineEnding::Lf)
            .expect("le pty doit s'ouvrir");
        process.write_line("sleep 60; echo after-sleep");
        std::thread::sleep(std::time::Duration::from_millis(500));
        process.interrupt();
        process.write_line("echo interrupted-ok");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut seen = Vec::new();
        while std::time::Instant::now() < deadline {
            if let Some(lines) = process.take_output() {
                seen = lines;
                // The shell echoes the command line back, so "after-sleep"
                // appears inside the echo of `sleep 60; echo after-sleep`.
                // What proves the interrupt worked is that the *output* line
                // was never produced — and that we got here in under ten
                // seconds rather than waiting out a sixty-second sleep.
                if seen.iter().any(|line| line.trim() == "interrupted-ok") {
                    assert!(
                        !seen.iter().any(|line| line.trim() == "after-sleep"),
                        "le sleep aurait dû être interrompu ; vu : {seen:?}"
                    );
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("la commande n'a pas été interrompue ; vu : {seen:?}");
    }

    #[test]
    fn line_endings_match_the_shell_family() {
        assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
        assert_eq!(LineEnding::Lf.as_str(), "\n");
    }
}
