//! Windows integration: system binaries, shell verbs, elevation state.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// `CREATE_NO_WINDOW`: keeps console helpers from flashing a window.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Absolute path to a binary shipped in `%SystemRoot%\System32`.
///
/// `Command::new("reg")` lets Windows resolve the name, and that search visits
/// the directory of the running executable *before* System32. A `reg.exe`
/// dropped next to `xion.exe` would therefore win. Always pass an absolute
/// path for system tools.
pub fn system_binary(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    Path::new(&root).join("System32").join(name)
}

/// NUL-terminated UTF-16, as every `*W` entry point expects.
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// Open a path with its registered handler.
///
/// Uses `ShellExecuteW` rather than `cmd /C start`: the path is passed as a
/// single argument, so no amount of quoting, `&` or `%VAR%` in a file name can
/// turn into a command. Directories, documents and executables all work.
pub fn shell_open(path: &Path) -> io::Result<()> {
    let file = wide(path.as_os_str());
    let directory = path.parent().map(|parent| wide(parent.as_os_str()));
    let verb = wide(OsStr::new("open"));

    // SAFETY: all four pointers are NUL-terminated UTF-16 buffers that outlive
    // the call; ShellExecuteW does not retain them.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            directory
                .as_ref()
                .map_or(std::ptr::null(), |dir| dir.as_ptr()),
            SW_SHOWNORMAL,
        )
    };

    // Documented contract: a value greater than 32 means success, anything
    // else is an error code.
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "ShellExecuteW a échoué (code {}) pour {}",
            result as isize,
            path.display()
        )))
    }
}

/// Show the "Open with" picker for a path.
///
/// Replaces `rundll32 shell32.dll,OpenAs_RunDLL "<path>"`, which was broken
/// anyway: the manual quotes were escaped again by `std::process`, so rundll32
/// received `\"C:\dir\file.txt\"` and could not find the file.
pub fn open_with(path: &Path) -> io::Result<()> {
    use windows_sys::Win32::UI::Shell::{OAIF_EXEC, OPENASINFO, SHOpenWithDialog};

    let file = wide(path.as_os_str());
    let info = OPENASINFO {
        pcszFile: file.as_ptr(),
        pcszClass: std::ptr::null(),
        oaifInFlags: OAIF_EXEC,
    };

    // SAFETY: `info` borrows `file`, a NUL-terminated buffer that outlives the
    // call; SHOpenWithDialog does not retain either pointer.
    let result = unsafe { SHOpenWithDialog(std::ptr::null_mut(), &info) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "SHOpenWithDialog a échoué (HRESULT {result:#x})"
        )))
    }
}

/// Select a path inside a new Explorer window instead of opening it.
///
/// Used after extracting an archive: revealing the result is safe, executing
/// it is not.
pub fn reveal_in_file_manager(path: &Path) -> io::Result<()> {
    let mut command = std::process::Command::new(system_binary("explorer.exe"));
    // `/select,` needs the path glued to it, and explorer.exe parses its own
    // command line, so this is the one place a single argument is required.
    let mut argument = std::ffi::OsString::from("/select,");
    argument.push(path.as_os_str());
    command.arg(argument);
    command.spawn().map(|_| ())
}

/// Read the ACL of a path by driving `icacls`.
///
/// The binary is resolved to its absolute System32 path: `Command::new("icacls")`
/// lets Windows search the directory of the running executable first, so a
/// dropped `icacls.exe` would run instead.
pub fn read_acl_text(path: &Path) -> io::Result<String> {
    let output = std::process::Command::new(system_binary("icacls.exe"))
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ── Instance unique ───────────────────────────────────────────────────────────

use super::single_instance::{Claim, Primary};

/// Always claims this process as the primary one.
///
/// Windows has no session bus. The equivalent is a named pipe: `CreateNamedPipeW`
/// under a per-session name, a thread accepting connections, `ReadFile` for the
/// path. That is roughly a hundred and fifty lines of `unsafe` Win32 — and this
/// crate is developed on Linux, where it can be neither run nor even compiled,
/// because cross-compiling to `x86_64-pc-windows-msvc` still stops on the C
/// dependency `libz-sys` pulls in through `git2`.
///
/// Untested `unsafe` in a file manager is a worse trade than the thing it buys,
/// which is only a faster second launch — never correctness. So Windows keeps
/// the behaviour it has always had, one process per launch, and says so here
/// rather than in a commit message nobody will find.
///
/// The seam is what matters: [`claim`] is the whole contract, and filling it in
/// touches this function and nothing else.
pub fn claim(_open: Option<&Path>) -> Claim {
    // A receiver whose sender is dropped immediately: `try_recv` returns
    // `None` for ever, which is exactly "no later launch will reach us".
    let (_, receiver) = std::sync::mpsc::channel();
    Claim::Primary(Primary {
        guard: Box::new(()),
        receiver,
    })
}
