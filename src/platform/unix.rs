//! Non-Windows integration: shell verbs via the desktop portal.

use std::io;
use std::path::{Path, PathBuf};

/// No-op on Unix; kept so callers do not need `#[cfg]` around process spawns.
pub const CREATE_NO_WINDOW: u32 = 0;

/// On Unix there is no protected system directory to anchor to, and `PATH`
/// does not include the current directory, so the plain name is correct.
pub fn system_binary(name: &str) -> PathBuf {
    PathBuf::from(name)
}

/// Open a path with the desktop's registered handler.
pub fn shell_open(path: &Path) -> io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Show the "Open with" picker for a path.
///
/// No desktop-independent equivalent exists on Unix, so this reports failure
/// rather than pretending to have opened something.
pub fn open_with(path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "« Ouvrir avec » n'est pas disponible sur cette plateforme ({})",
            path.display()
        ),
    ))
}

/// Select a path in the desktop file manager, falling back to opening its
/// parent directory when no file manager exposes a select verb.
pub fn reveal_in_file_manager(path: &Path) -> io::Result<()> {
    if cfg!(target_os = "macos") {
        return std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map(|_| ());
    }

    // `dbus-send` to org.freedesktop.FileManager1 is the portable verb, but it
    // is not always present; opening the parent directory always works.
    let target = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open")
        .arg(target)
        .spawn()
        .map(|_| ())
}

/// Render POSIX permission bits in the same shape the ACL view expects.
///
/// `icacls` does not exist here. The previous code called it unconditionally,
/// so the whole panel was dead weight on Unix: it reported "command not found"
/// and offered nothing in its place.
pub fn read_acl_text(path: &Path) -> io::Result<String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)?.permissions().mode();
    let render = |shift: u32| {
        let bits = (mode >> shift) & 0b111;
        format!(
            "{}{}{}",
            if bits & 0b100 != 0 { "R" } else { "-" },
            if bits & 0b010 != 0 { "W" } else { "-" },
            if bits & 0b001 != 0 { "X" } else { "-" },
        )
    };

    Ok(format!(
        "{}:(propriétaire)({})\n{}:(groupe)({})\n{}:(autres)({})\n",
        path.display(),
        render(6),
        path.display(),
        render(3),
        path.display(),
        render(0),
    ))
}

// ── Instance unique ───────────────────────────────────────────────────────────

use super::single_instance::{Claim, Primary, SERVICE_NAME};

/// The D-Bus object every instance agrees to talk on.
const SERVICE_PATH: &str = "/org/xion/Xion";

/// The interface later launches call to hand over a path.
struct OpenService {
    sender: std::sync::mpsc::Sender<PathBuf>,
}

#[zbus::interface(name = "org.xion.Xion")]
impl OpenService {
    /// Asks the running window to open `path`.
    ///
    /// Returns nothing and never fails: a caller that is about to exit has no
    /// use for an error, and the running instance decides what a bad path
    /// means — the same decision it makes for a path typed in the address bar.
    fn open(&self, path: String) {
        let _ = self.sender.send(PathBuf::from(path));
    }
}

/// Claims this session's Xion, or hands `open` to the one already running.
///
/// A session bus that is missing or refuses the name yields `Primary` with a
/// receiver that never produces anything: a desktop without D-Bus gets the old
/// behaviour, one process per launch, rather than no file manager.
pub fn claim(open: Option<&Path>) -> Claim {
    let (sender, receiver) = std::sync::mpsc::channel();

    match try_serve(sender) {
        Some(connection) => Claim::Primary(Primary {
            guard: Box::new(connection),
            receiver,
        }),
        None => match forward(open) {
            // Somebody answered: this process has done its job.
            true => Claim::Secondary,
            // The name is taken but nothing answered — a stale owner, or a
            // peer that is not us. Running a second window is a far better
            // outcome than exiting and opening nothing at all.
            false => Claim::Primary(Primary {
                guard: Box::new(()),
                receiver,
            }),
        },
    }
}

/// Tries to become the owner of the well-known name.
///
/// `None` means somebody else already owns it, or there is no session bus.
fn try_serve(sender: std::sync::mpsc::Sender<PathBuf>) -> Option<zbus::blocking::Connection> {
    let connection = zbus::blocking::connection::Builder::session()
        .ok()?
        .serve_at(SERVICE_PATH, OpenService { sender })
        .ok()?
        .build()
        .ok()?;

    // `DoNotQueue`, and the reply checked, both matter. The builder's `.name()`
    // shortcut asks without it: a second launch was then *queued* behind the
    // running instance, the request reported success, and the process went on
    // to open its own window — the exact duplicate this module exists to
    // prevent. Measured: two windows, two processes.
    let reply = connection
        .request_name_with_flags(SERVICE_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .ok()?;

    match reply {
        zbus::fdo::RequestNameReply::PrimaryOwner => Some(connection),
        // AlreadyOwner cannot happen — this connection was just created — and
        // InQueue is impossible with DoNotQueue. Both mean "not ours".
        _ => None,
    }
}

/// Hands `open` to the running instance. `false` if nobody answered.
fn forward(open: Option<&Path>) -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) =
        zbus::blocking::Proxy::new(&connection, SERVICE_NAME, SERVICE_PATH, "org.xion.Xion")
    else {
        return false;
    };

    // With no path to hand over there is still a reason to make the call: it
    // proves somebody is there, and it is what tells the running window to
    // raise itself.
    let path = open
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    proxy.call::<_, _, ()>("Open", &(path,)).is_ok()
}
