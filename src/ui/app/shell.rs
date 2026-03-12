//! Shell process spawning helpers for the integrated terminal.

/// Spawn a terminal process for the configured shell in the given working directory.
pub(super) async fn spawn_shell_process(
    shell: &crate::core::ShellConfig,
    cwd: &std::path::Path,
) -> std::io::Result<crate::terminal::TerminalProcess> {
    match shell {
        crate::core::ShellConfig::Cmd => {
            crate::terminal::TerminalProcess::spawn(cwd).await
        }
        crate::core::ShellConfig::PowerShell => {
            crate::terminal::TerminalProcess::spawn_powershell(cwd).await
        }
        crate::core::ShellConfig::GitBash => {
            let git_bash = find_git_bash();
            crate::terminal::TerminalProcess::spawn_custom(&git_bash, cwd).await
        }
        crate::core::ShellConfig::Custom(path) => {
            crate::terminal::TerminalProcess::spawn_custom(path, cwd).await
        }
    }
}

/// Returns the path to `bash.exe` shipped with Git for Windows, falling back
/// to `"bash"` if Git is not found in the default install locations.
pub(super) fn find_git_bash() -> String {
    let candidates = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }
    "bash".to_string()
}
