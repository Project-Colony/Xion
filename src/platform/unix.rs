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
