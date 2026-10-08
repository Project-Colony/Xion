//! One Xion per session, and later launches reach the one already running.
//!
//! Opening a folder from a desktop, a terminal or `xdg-open` starts the binary
//! again. Without this, that means a second process: a second window, a second
//! ~38 MiB, a second copy of every cache, and no way to end up with the folder
//! next to what you already had open. Nautilus does not behave that way — a
//! second `nautilus` call reaches the running one over D-Bus, which is the whole
//! reason its second window appears in 139 ms where its first took 700.
//!
//! So this does the same thing: the first process claims a well-known name, and
//! every later one hands it the path and exits.
//!
//! The transport differs by platform — D-Bus on Linux, a named pipe on Windows —
//! but the decision is the same, so both backends answer the one question in
//! [`claim`] and the rest of the crate never learns which is in use.

use std::path::PathBuf;

/// Claims this session's Xion, or hands the path to the one already running.
///
/// Implemented per platform; re-exported here so callers reach the decision and
/// the types it returns through one name.
pub use super::claim;

/// What a launch found when it looked for an existing Xion.
pub enum Claim {
    /// Nothing was running. This process owns the name.
    ///
    /// The handle must be held for as long as the process lives: dropping it
    /// releases the name, and the next launch would start a second window.
    Primary(Primary),
    /// Another instance answered and has been handed the path.
    ///
    /// The caller exits; the running window is doing the work.
    Secondary,
}

/// The owned claim, plus the paths later launches ask for.
pub struct Primary {
    /// Kept alive for its `Drop`: releases the name, closes the socket.
    #[allow(dead_code)]
    pub(super) guard: Box<dyn std::any::Any + Send>,
    /// Paths sent by later launches, in arrival order.
    pub(super) receiver: std::sync::mpsc::Receiver<PathBuf>,
}

impl Primary {
    /// The next path a later launch asked for, if one is waiting.
    ///
    /// Never blocks: it is polled from the UI's own subscription tick.
    pub fn try_recv(&self) -> Option<PathBuf> {
        self.receiver.try_recv().ok()
    }
}

impl std::fmt::Debug for Primary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Primary")
    }
}

/// The D-Bus well-known name the Linux backend claims. Windows has no backend
/// yet (see `platform::windows::claim`), so nothing there reads it.
#[cfg(not(windows))]
pub(super) const SERVICE_NAME: &str = "org.xion.Xion";
