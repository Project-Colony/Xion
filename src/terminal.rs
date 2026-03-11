//! Persistent CMD terminal session for Xion.
//!
//! Spawns a real `cmd.exe` process with piped stdin/stdout/stderr.
//! The process stays alive between commands so `cd`, environment variables,
//! and shell state all persist — exactly like a real terminal.
//!
//! The handle is cheap to clone (Arc-backed) and can travel through UiMessage.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::Mutex as AsyncMutex;

#[cfg(windows)]
#[allow(unused_imports)]
use std::os::windows::process::CommandExt;

/// Maximum lines buffered before oldest lines are dropped.
const OUTPUT_BUF_MAX: usize = 500;

/// Sentinel echoed by the init sequence. The stdout reader discards all
/// output until it sees this line, hiding the Windows banner and init echoes.
const READY_SENTINEL: &str = "---XION_READY---";

// ── Inner state (Arc-shared across all clones) ────────────────────────────────

struct Inner {
    stdin: AsyncMutex<ChildStdin>,
    output_buf: Arc<Mutex<VecDeque<String>>>,
    /// Kept so `Drop` can force-kill cmd.exe, allowing the tokio runtime to
    /// shut down cleanly when Xion closes.
    child: Mutex<Option<Child>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                // Force-kill cmd.exe. start_kill() is synchronous and does not
                // block — it just sends the termination signal.
                let _ = child.start_kill();
            }
        }
    }
}

// ── Public handle ─────────────────────────────────────────────────────────────

/// Cloneable, cheaply shareable handle to a live `cmd.exe` session.
///
/// Dropping the last clone kills cmd.exe and closes stdin.
#[derive(Clone)]
pub struct TerminalProcess {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for TerminalProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalProcess").finish_non_exhaustive()
    }
}

/// Decode a byte chunk, split on '\n', and push complete lines to the buffer.
/// `ready` gates whether lines are pushed (stdout only needs gating).
/// Leftover bytes without a trailing '\n' are kept in `partial`.
fn push_chunk(
    chunk: &[u8],
    partial: &mut Vec<u8>,
    buf: &Arc<Mutex<VecDeque<String>>>,
    ready: &Arc<AtomicBool>,
) {
    partial.extend_from_slice(chunk);
    while let Some(pos) = partial.iter().position(|&b| b == b'\n') {
        let line_bytes = &partial[..pos];
        let line_bytes = if line_bytes.last() == Some(&b'\r') {
            &line_bytes[..line_bytes.len() - 1]
        } else {
            line_bytes
        };
        let line = String::from_utf8_lossy(line_bytes).into_owned();

        if line == READY_SENTINEL {
            ready.store(true, Ordering::Relaxed);
        } else if ready.load(Ordering::Relaxed) {
            let mut locked = buf.lock().unwrap();
            locked.push_back(line);
            if locked.len() > OUTPUT_BUF_MAX {
                locked.pop_front();
            }
        }
        *partial = partial[pos + 1..].to_vec();
    }
}

impl TerminalProcess {
    /// Spawn a new `cmd.exe` session in `cwd`.
    pub async fn spawn(cwd: &Path) -> std::io::Result<Self> {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let mut stdout = child.stdout.take().expect("stdout was piped");
        let mut stderr = child.stderr.take().expect("stderr was piped");

        let output_buf: Arc<Mutex<VecDeque<String>>> =
            Arc::new(Mutex::new(VecDeque::new()));

        // Stdout is gated: all output before READY_SENTINEL is discarded.
        let stdout_ready = Arc::new(AtomicBool::new(false));
        let buf_out = Arc::clone(&output_buf);
        let ready_out = Arc::clone(&stdout_ready);
        tokio::spawn(async move {
            let mut tmp = vec![0u8; 4096];
            let mut partial: Vec<u8> = Vec::new();
            loop {
                match stdout.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => push_chunk(&tmp[..n], &mut partial, &buf_out, &ready_out),
                }
            }
        });

        // Stderr is always forwarded (errors must always be visible).
        let stderr_ready = Arc::new(AtomicBool::new(true));
        let buf_err = Arc::clone(&output_buf);
        let ready_err = Arc::clone(&stderr_ready);
        tokio::spawn(async move {
            let mut tmp = vec![0u8; 4096];
            let mut partial: Vec<u8> = Vec::new();
            loop {
                match stderr.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => push_chunk(&tmp[..n], &mut partial, &buf_err, &ready_err),
                }
            }
        });

        let inner = Arc::new(Inner {
            stdin: AsyncMutex::new(stdin),
            output_buf,
            child: Mutex::new(Some(child)),
        });

        // Init sequence:
        //  chcp 65001     → force UTF-8 output
        //  @echo off      → suppress cmd.exe echoing our commands back
        //  PROMPT $_      → prompt emits "\r\n", preventing partial-line merging
        //  echo SENTINEL  → marks end of init; stdout reader unmutes after this
        let init = Arc::clone(&inner);
        tokio::spawn(async move {
            let mut s = init.stdin.lock().await;
            let _ = s.write_all(b"chcp 65001 > nul\r\n").await;
            let _ = s.write_all(b"@echo off\r\n").await;
            let _ = s.write_all(b"PROMPT $_\r\n").await;
            let ready_line = format!("echo {}\r\n", READY_SENTINEL);
            let _ = s.write_all(ready_line.as_bytes()).await;
            let _ = s.flush().await;
        });

        Ok(Self { inner })
    }

    /// Write a command line to stdin (appends `\r\n` automatically).
    pub async fn write_line(&self, line: &str) {
        let mut s = self.inner.stdin.lock().await;
        let bytes = format!("{}\r\n", line);
        let _ = s.write_all(bytes.as_bytes()).await;
        let _ = s.flush().await;
    }

    /// Drain all pending output lines from the buffer.
    /// Returns an empty Vec if no new output is available.
    pub fn poll(&self) -> Vec<String> {
        let mut buf = self.inner.output_buf.lock().unwrap();
        buf.drain(..).collect()
    }
}
