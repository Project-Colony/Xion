//! Shell selection for the integrated terminal.

use crate::terminal::{LineEnding, TerminalProcess};

/// Start the configured shell inside a pseudo-terminal.
///
/// Blocking (it forks a process), so callers must run it on a blocking task.
pub(super) fn spawn_shell_process(
    shell: &crate::core::ShellConfig,
    cwd: &std::path::Path,
) -> std::io::Result<TerminalProcess> {
    match shell {
        crate::core::ShellConfig::Cmd => default_shell(cwd),
        crate::core::ShellConfig::PowerShell => TerminalProcess::spawn(
            "powershell",
            &["-NoLogo", "-NoProfile"],
            cwd,
            LineEnding::Crlf,
        ),
        crate::core::ShellConfig::GitBash => {
            let git_bash = find_git_bash();
            TerminalProcess::spawn(&git_bash, &[], cwd, LineEnding::Lf)
        }
        crate::core::ShellConfig::Custom(path) => {
            // A custom shell is an arbitrary executable path read from
            // config.toml, so it is resolved and checked before being run
            // rather than handed straight to the process API.
            let resolved = validated_custom_shell(path)?;
            TerminalProcess::spawn(&resolved, &[], cwd, native_line_ending())
        }
    }
}

/// The platform's default interactive shell.
///
/// `ShellConfig::Cmd` is the stored default; on a non-Windows machine that
/// name means nothing, so the user's `$SHELL` is used instead of failing.
fn default_shell(cwd: &std::path::Path) -> std::io::Result<TerminalProcess> {
    if cfg!(windows) {
        return TerminalProcess::spawn("cmd.exe", &[], cwd, LineEnding::Crlf);
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    TerminalProcess::spawn(&shell, &[], cwd, LineEnding::Lf)
}

fn native_line_ending() -> LineEnding {
    if cfg!(windows) {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

/// Resolve a user-configured shell path, refusing anything that is not an
/// existing absolute path.
///
/// A bare name would be resolved by the OS, and on Windows that search visits
/// the directory of the running executable before the system one.
fn validated_custom_shell(path: &str) -> std::io::Result<String> {
    let candidate = std::path::Path::new(path);
    if !candidate.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("terminal_shell doit être un chemin absolu : {path}"),
        ));
    }
    let resolved = candidate.canonicalize()?;
    if !resolved.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("terminal_shell introuvable : {path}"),
        ));
    }
    Ok(resolved.to_string_lossy().into_owned())
}

/// Locate Git Bash.
///
/// The hard-coded `C:\\Program Files\\Git` pair missed scoop, winget and
/// per-user installs, and the `where bash` fallback returned
/// `C:\\Windows\\System32\\bash.exe` — the WSL launcher, not Git Bash — on any
/// machine with WSL enabled.
pub(super) fn find_git_bash() -> String {
    if let Ok(env_path) = std::env::var("GIT_BASH") {
        if std::path::Path::new(&env_path).is_file() {
            return env_path;
        }
    }

    #[cfg(windows)]
    {
        let roots = [
            "ProgramFiles",
            "ProgramW6432",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
        ];
        for root in roots {
            let Ok(base) = std::env::var(root) else {
                continue;
            };
            for suffix in [r"Git\bin\bash.exe", r"Programs\Git\bin\bash.exe"] {
                let candidate = std::path::Path::new(&base).join(suffix);
                if candidate.is_file() {
                    return candidate.to_string_lossy().into_owned();
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for candidate in ["/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash"] {
            if std::path::Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
    }

    // Last resort: let the OS resolve it and report a clear error if it cannot.
    "bash".to_string()
}
