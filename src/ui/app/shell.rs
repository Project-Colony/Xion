//! Which shell the integrated terminal starts, and under which name.
//!
//! The picker used to show the same three buttons on both systems (`CMD`,
//! `PS`, `Bash`) because the enumeration had been written with Windows in mind.
//! On Linux that gave a "CMD" button that actually started the user's
//! `$SHELL`, so merely misnamed, and a "PS" button that started nothing at all:
//! the code calls `powershell`, while even PowerShell installed on Linux is
//! called `pwsh`.
//!
//! Here the list is **discovered** when it is displayed, and the default
//! variant finally carries the name of what it does.

use crate::core::ShellConfig;

/// One picker entry: what is displayed, and what it starts.
pub(super) struct ShellChoice {
    pub(super) label: String,
    pub(super) config: ShellConfig,
}

/// The program to start and its arguments.
///
/// Starts nothing: `iced_term` opens the pseudo-terminal, and it expects a
/// program name. Xion only says which one.
pub(super) fn resolve(shell: &ShellConfig) -> std::io::Result<(String, Vec<String>)> {
    let resolved = match shell {
        ShellConfig::System => (system_shell(), Vec::new()),
        ShellConfig::PowerShell => (
            find_powershell().ok_or_else(|| not_found("pwsh"))?,
            vec!["-NoLogo".to_string(), "-NoProfile".to_string()],
        ),
        ShellConfig::Bash => (bash_path().ok_or_else(|| not_found("bash"))?, Vec::new()),
        ShellConfig::Custom(path) => {
            // A custom shell is an arbitrary path read from config.toml: it is
            // resolved and checked before being passed on.
            (validated_custom_shell(path)?, Vec::new())
        }
    };
    Ok(resolved)
}

/// A shell that could not be located. `iced_term` only ever receives absolute
/// paths: a bare name would be resolved by Windows from the running
/// executable's directory first.
fn not_found(name: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())
}

/// The shells offered on this machine, in display order.
///
/// The first is always the user's own; the others only appear when they really
/// exist. A button that cannot start anything helps nobody: it takes up room
/// and casts doubt on the rest.
#[cfg(not(windows))]
pub(super) fn available_shells() -> Vec<ShellChoice> {
    let system = system_shell();
    let mut choices = vec![ShellChoice {
        label: display_name(&system),
        config: ShellConfig::System,
    }];

    // Bash second, unless it already is the login shell: two buttons for the
    // same program read as two choices.
    if let Some(bash) = bash_path()
        && display_name(&bash) != display_name(&system)
    {
        choices.push(ShellChoice {
            label: display_name(&bash),
            config: ShellConfig::Bash,
        });
    }

    choices
}

/// See the Unix version. On Windows the three shells are distinct programs, and
/// the choice has a meaning the name alone does not convey.
#[cfg(windows)]
pub(super) fn available_shells() -> Vec<ShellChoice> {
    let mut choices = vec![
        ShellChoice {
            label: display_name(&system_shell()),
            config: ShellConfig::System,
        },
        ShellChoice {
            label: "PowerShell".to_string(),
            config: ShellConfig::PowerShell,
        },
    ];

    // Git Bash does not ship with Windows: only offer it when it is there.
    if bash_path().is_some() {
        choices.push(ShellChoice {
            label: "Git Bash".to_string(),
            config: ShellConfig::Bash,
        });
    }

    choices
}

/// The display name of a configuration, for the preferences as well as the
/// picker.
pub(super) fn label_for(shell: &ShellConfig) -> String {
    match shell {
        ShellConfig::System => display_name(&system_shell()),
        ShellConfig::PowerShell => "PowerShell".to_string(),
        ShellConfig::Bash => {
            bash_path().map_or_else(|| "bash".to_string(), |path| display_name(&path))
        }
        ShellConfig::Custom(path) => display_name(path),
    }
}

/// The user's login shell.
///
/// `$SHELL` first, because it is the convention and the session sets it. But it
/// can be missing depending on how Xion is started, and falling straight back
/// to `/bin/sh` would give a shell the user did not choose while the system
/// knows which one it is.
#[cfg(not(windows))]
fn system_shell() -> String {
    if let Some(shell) = std::env::var_os("SHELL") {
        let shell = shell.to_string_lossy().into_owned();
        if !shell.trim().is_empty() {
            return shell;
        }
    }

    shell_from_passwd().unwrap_or_else(|| "/bin/sh".to_string())
}

#[cfg(windows)]
fn system_shell() -> String {
    std::env::var("ComSpec").unwrap_or_else(|_| {
        crate::platform::system_binary("cmd.exe")
            .to_string_lossy()
            .into_owned()
    })
}

/// The shell recorded in the account database, when `$SHELL` is silent.
///
/// Reads `/etc/passwd` rather than calling `getpwuid_r`: that avoids an
/// `unsafe` block and a direct dependency, and above all keeps the parsing
/// testable. The trade-off is a user declared elsewhere (LDAP, systemd-homed)
/// that we will not see; but such a user has a session that sets `$SHELL`, so
/// they never get this far.
#[cfg(target_os = "linux")]
fn shell_from_passwd() -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    // `/proc/self` belongs to the real uid of the process. Reading it avoids
    // depending on `$USER`, which can be missing exactly when `$SHELL` is.
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    shell_for_uid(&passwd, uid)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn shell_from_passwd() -> Option<String> {
    None
}

/// Extract the shell of the given uid from the contents of `/etc/passwd`.
///
/// Kept apart from reading the file so it can be exercised without making one.
#[cfg(target_os = "linux")]
fn shell_for_uid(passwd: &str, uid: u32) -> Option<String> {
    // nom:mot_de_passe:uid:gid:commentaire:accueil:shell
    const UID: usize = 2;
    const SHELL: usize = 6;

    for line in passwd.lines() {
        // Index rather than advance an iterator: the first version chained two
        // `nth(2)` calls, which landed on the home directory and not the
        // shell. And its `?` returned from the whole function at the first
        // malformed line, instead of skipping it.
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() <= SHELL {
            continue;
        }
        if fields[UID].parse::<u32>() != Ok(uid) {
            continue;
        }
        let shell = fields[SHELL].trim();
        if !shell.is_empty() {
            return Some(shell.to_string());
        }
    }
    None
}

/// The short name of a shell: `/usr/bin/zsh` becomes `zsh`.
fn display_name(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    // `cmd.exe` reads better as `cmd`, and the extension tells nothing.
    match name.rfind('.') {
        Some(dot) if name[dot..].eq_ignore_ascii_case(".exe") => name[..dot].to_string(),
        _ => name,
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

/// PowerShell, by its canonical path when it is known.
///
/// Not the bare name: on Windows the search visits the running executable's
/// directory before the system one, which is the same trap as for a custom
/// shell. `pwsh` first, it is the maintained version.
fn find_powershell() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(root) = std::env::var("SystemRoot") {
            let canonical =
                std::path::Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
            if canonical.is_file() {
                return Some(canonical.to_string_lossy().into_owned());
            }
        }
    }

    for candidate in [
        "/usr/bin/pwsh",
        "/usr/local/bin/pwsh",
        "/opt/microsoft/powershell/7/pwsh",
    ] {
        if std::path::Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }

    // Last resort: an absolute path from PATH, never the bare name.
    crate::services::thumbnails::find_in_path("pwsh")
        .map(|path| path.to_string_lossy().into_owned())
}

/// Where bash is, or `None` when it is not installed.
///
/// The hard-coded `C:\\Program Files\\Git` pair missed scoop, winget and
/// per-user installs, and the `where bash` fallback returned
/// `C:\\Windows\\System32\\bash.exe` (the WSL launcher, not Git Bash) on any
/// machine with WSL enabled.
pub(super) fn bash_path() -> Option<String> {
    if let Ok(env_path) = std::env::var("GIT_BASH")
        && std::path::Path::new(&env_path).is_file()
    {
        return Some(env_path);
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
                    return Some(candidate.to_string_lossy().into_owned());
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for candidate in ["/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash"] {
            if std::path::Path::new(candidate).is_file() {
                return Some(candidate.to_string());
            }
        }
        // NixOS and friends keep bash out of /bin. Not on Windows: there the
        // PATH answer is System32\bash.exe, the WSL launcher.
        if let Some(path) = crate::services::thumbnails::find_in_path("bash") {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_shows_under_its_short_name() {
        assert_eq!(display_name("/usr/bin/zsh"), "zsh");
        assert_eq!(display_name("/bin/bash"), "bash");
        assert_eq!(display_name("fish"), "fish");
    }

    /// `cmd.exe` displayed as is would be the only button with an extension.
    ///
    /// On the bare name, not a full Windows path: `Path` only treats the
    /// backslash as a separator *on* Windows, so such an assertion would fail
    /// here for a reason unrelated to what it claims to check.
    #[test]
    fn a_windows_executable_loses_its_extension() {
        assert_eq!(display_name("cmd.exe"), "cmd");
        assert_eq!(display_name("PowerShell.EXE"), "PowerShell");
    }

    /// A dot that is not a Windows extension must not cut anything.
    #[test]
    fn a_dot_that_is_not_an_extension_is_kept() {
        assert_eq!(display_name("/usr/bin/python3.11"), "python3.11");
    }

    #[test]
    fn the_label_of_a_custom_shell_is_its_file_name() {
        assert_eq!(
            label_for(&ShellConfig::Custom("/usr/bin/fish".to_string())),
            "fish"
        );
        assert_eq!(label_for(&ShellConfig::PowerShell), "PowerShell");
    }

    /// The first choice offered is always the user's own.
    #[test]
    fn the_first_choice_is_the_users_own_shell() {
        let choices = available_shells();
        assert!(!choices.is_empty());
        assert_eq!(choices[0].config, ShellConfig::System);
    }

    /// Every offered shell must point at an executable that exists.
    ///
    /// The test the previous version was missing: the "PS" button showed on
    /// Linux without being able to start anything, and nothing said so.
    /// Offering a choice is promising that it works.
    #[test]
    fn every_offered_shell_resolves_to_a_real_program() {
        for choice in available_shells() {
            let resolved = resolve(&choice.config);
            let (program, _) = resolved.expect("an offered shell must resolve");
            // Neither the path nor the label goes into the message: both can
            // come from the user's account entry, and CodeQL flags formatting
            // them as cleartext logging. The variant names the shell.
            assert!(
                std::path::Path::new(&program).is_file(),
                "{:?} is offered but its program does not exist",
                choice.config
            );
        }
    }

    /// No entry may repeat, otherwise the same program would get two buttons:
    /// the case of someone whose login shell is bash.
    #[test]
    fn no_two_choices_carry_the_same_label() {
        let choices = available_shells();
        let mut labels: Vec<&str> = choices.iter().map(|c| c.label.as_str()).collect();
        labels.sort_unstable();
        let before = labels.len();
        labels.dedup();
        assert_eq!(before, labels.len(), "a shell offered twice");
    }

    #[cfg(target_os = "linux")]
    mod passwd {
        use super::*;

        const SAMPLE: &str = "root:x:0:0:root:/root:/bin/bash\n\
                              alice:x:1000:1000:Alice:/home/alice:/usr/bin/zsh\n\
                              bob:x:1001:1001:Bob:/home/bob:/usr/bin/fish\n";

        #[test]
        fn the_shell_of_the_matching_uid_is_returned() {
            assert_eq!(shell_for_uid(SAMPLE, 1000).as_deref(), Some("/usr/bin/zsh"));
            assert_eq!(
                shell_for_uid(SAMPLE, 1001).as_deref(),
                Some("/usr/bin/fish")
            );
            assert_eq!(shell_for_uid(SAMPLE, 0).as_deref(), Some("/bin/bash"));
        }

        #[test]
        fn an_unknown_uid_yields_nothing() {
            assert_eq!(shell_for_uid(SAMPLE, 4242), None);
        }

        /// A system account with no declared shell must not produce an empty
        /// string, which would then be started as a program.
        #[test]
        fn an_empty_shell_field_is_not_a_shell() {
            assert_eq!(shell_for_uid("daemon:x:2:2:daemon:/sbin:\n", 2), None);
        }

        /// A truncated line must not make the parser panic.
        #[test]
        fn a_malformed_line_is_skipped() {
            assert_eq!(
                shell_for_uid("broken\nalice:x:1000:1000::/home:/bin/sh\n", 1000).as_deref(),
                Some("/bin/sh")
            );
            assert_eq!(shell_for_uid("a:x:not-a-number:0::/:/bin/sh\n", 0), None);
        }
    }
}
