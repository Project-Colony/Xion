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

// Note: tokio::process::Command provides creation_flags() natively on Windows,
// so no std::os::windows::process::CommandExt import is needed.

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
                // Try to reap the process to avoid zombie. try_wait is non-blocking.
                let _ = child.try_wait();
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
    // Safety cap: if no newline arrives for a very long time, truncate to avoid unbounded growth.
    // Truncate at a valid UTF-8 char boundary to avoid corrupting multi-byte sequences.
    const PARTIAL_MAX: usize = 64 * 1024;
    if partial.len() > PARTIAL_MAX {
        let drain_to = partial.len() - PARTIAL_MAX;
        // Find the next valid UTF-8 boundary after drain_to
        let safe_drain = (drain_to..partial.len())
            .find(|&i| std::str::from_utf8(&partial[i..]).is_ok()
                || partial.get(i).map_or(true, |b| (*b & 0b1100_0000) != 0b1000_0000))
            .unwrap_or(drain_to);
        partial.drain(..safe_drain);
    }
    while let Some(pos) = partial.iter().position(|&b| b == b'\n') {
        let line_bytes = &partial[..pos];
        let line_bytes = if line_bytes.last() == Some(&b'\r') {
            &line_bytes[..line_bytes.len() - 1]
        } else {
            line_bytes
        };
        let line = String::from_utf8_lossy(line_bytes).into_owned();

        if line == READY_SENTINEL {
            ready.store(true, Ordering::Release);
        } else if ready.load(Ordering::Acquire) {
            if let Ok(mut locked) = buf.lock() {
                locked.push_back(line);
                if locked.len() > OUTPUT_BUF_MAX {
                    locked.pop_front();
                }
            }
        }
        partial.drain(..=pos);
    }
}

impl TerminalProcess {
    /// Spawn a custom shell process.
    pub async fn spawn_custom(shell_path: &str, cwd: &Path) -> std::io::Result<Self> {
        let mut cmd = tokio::process::Command::new(shell_path);
        cmd.current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| std::io::Error::other("stdin not available"))?;
        let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("stdout not available"))?;
        let stderr = child.stderr.take().ok_or_else(|| std::io::Error::other("stderr not available"))?;
        Ok(Self::build_process(child, stdin, stdout, stderr, b"echo ---XION_READY---\r\n".to_vec()).await)
    }

    /// Spawn a PowerShell session.
    pub async fn spawn_powershell(cwd: &Path) -> std::io::Result<Self> {
        let mut cmd = tokio::process::Command::new("powershell");
        cmd.args(["-NoLogo", "-NoProfile"])
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| std::io::Error::other("stdin not available"))?;
        let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("stdout not available"))?;
        let stderr = child.stderr.take().ok_or_else(|| std::io::Error::other("stderr not available"))?;
        let init = b"$env:TERM = 'xterm'\r\nfunction prompt { \"`n\" }\r\nWrite-Output \"---XION_READY---\"\r\n".to_vec();
        Ok(Self::build_process(child, stdin, stdout, stderr, init).await)
    }

    async fn build_process(
        child: tokio::process::Child,
        stdin: tokio::process::ChildStdin,
        mut stdout: tokio::process::ChildStdout,
        mut stderr: tokio::process::ChildStderr,
        init_bytes: Vec<u8>,
    ) -> Self {
        let output_buf: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        // Shared ready gate: both stdout and stderr wait for READY_SENTINEL.
        // stderr uses the same gate so it doesn't output before stdout is ready.
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

        let buf_err = Arc::clone(&output_buf);
        let ready_err = Arc::clone(&stdout_ready); // Same gate as stdout
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

        // Write init bytes synchronously (awaited) to ensure the shell is
        // initialized before returning. This prevents race conditions where
        // the caller sends commands before the init sequence completes.
        {
            let mut s = inner.stdin.lock().await;
            if let Err(e) = s.write_all(&init_bytes).await {
                tracing::warn!("Terminal init write échoué: {e}");
            }
            if let Err(e) = s.flush().await {
                tracing::warn!("Terminal init flush échoué: {e}");
            }
        }

        Self { inner }
    }

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

        let stdin = child.stdin.take().ok_or_else(|| std::io::Error::other("stdin not available"))?;
        let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("stdout not available"))?;
        let stderr = child.stderr.take().ok_or_else(|| std::io::Error::other("stderr not available"))?;

        // Init sequence:
        //  chcp 65001     → force UTF-8 output
        //  @echo off      → suppress cmd.exe echoing our commands back
        //  PROMPT $_      → prompt emits "\r\n", preventing partial-line merging
        //  echo SENTINEL  → marks end of init; stdout reader unmutes after this
        let ready_line = format!("echo {}\r\n", READY_SENTINEL);
        let mut init_bytes = Vec::new();
        init_bytes.extend_from_slice(b"chcp 65001 > nul\r\n");
        init_bytes.extend_from_slice(b"@echo off\r\n");
        init_bytes.extend_from_slice(b"PROMPT $_\r\n");
        init_bytes.extend_from_slice(ready_line.as_bytes());

        Ok(Self::build_process(child, stdin, stdout, stderr, init_bytes).await)
    }

    /// Write a command line to stdin (appends `\r\n` automatically).
    pub async fn write_line(&self, line: &str) {
        let mut s = self.inner.stdin.lock().await;
        let bytes = format!("{}\r\n", line);
        if let Err(e) = s.write_all(bytes.as_bytes()).await {
            tracing::warn!("Terminal stdin write échoué: {e}");
        }
        if let Err(e) = s.flush().await {
            tracing::warn!("Terminal stdin flush échoué: {e}");
        }
    }

    /// Drain all pending output lines from the buffer.
    /// Returns an empty Vec if no new output is available.
    pub fn poll(&self) -> Vec<String> {
        match self.inner.output_buf.lock() {
            Ok(mut buf) => buf.drain(..).collect(),
            Err(_) => vec![],
        }
    }
}
