//! Command-line handling: `--flag` sub-commands and the start path argument.

use std::path::PathBuf;

use crate::core::Language;
use crate::i18n::{t, tf};

/// Exit code returned when a `--flag` sub-command fails.
const EXIT_FAILURE: i32 = 1;

/// Run a `--flag` sub-command and return the process exit code.
pub fn run(flag: &str) -> i32 {
    // Without this the `windows_subsystem = "windows"` attribute leaves the
    // process with no stdout/stderr, so every message below goes nowhere.
    crate::platform::attach_parent_console();
    // No configuration is read for a sub-command: the session's language it is.
    crate::i18n::apply(Language::Auto);

    match flag {
        "--register" => run_registry_command(RegistryCommand::Register),
        "--unregister" => run_registry_command(RegistryCommand::Unregister),
        "--help" | "--version" => {
            println!("Xion {}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("{}", t("cli_usage"));
            println!("       xion --register     {}", t("cli_register_help"));
            println!("       xion --unregister   {}", t("cli_unregister_help"));
            0
        }
        other => {
            eprintln!("{}", tf("cli_unknown_option", &[("option", &other)]));
            eprintln!("{}", t("cli_see_help"));
            EXIT_FAILURE
        }
    }
}

/// Resolve a command-line path argument against the current directory.
///
/// Returns `None` when the path does not exist so the caller falls back to the
/// configured start directory instead of opening a dead location.
pub fn start_path(arg: &str) -> Option<PathBuf> {
    // Not `PathBuf::from(arg)`: a `.desktop` entry's `%U` hands over a URI, so
    // opening a folder from a dock or `xdg-open` arrives as
    // `file:///home/alice/My%20documents`. Read as a path, that names a
    // directory called `file:` which does not exist, and the launcher appears
    // to do nothing at all.
    let path = crate::core::uri::path_from_argument(arg);
    let resolved = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    if resolved.exists() {
        Some(resolved)
    } else {
        tracing::warn!("Invalid start path: {}", resolved.display());
        None
    }
}

/// Which side of the Windows shell integration a sub-command targets.
enum RegistryCommand {
    Register,
    Unregister,
}

#[cfg(windows)]
fn run_registry_command(command: RegistryCommand) -> i32 {
    let result = match command {
        RegistryCommand::Register => crate::registry::register(),
        RegistryCommand::Unregister => crate::registry::unregister(),
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", tf("cli_registry_failed", &[("error", &error)]));
            EXIT_FAILURE
        }
    }
}

#[cfg(not(windows))]
fn run_registry_command(_command: RegistryCommand) -> i32 {
    eprintln!("{}", t("cli_registry_windows_only"));
    EXIT_FAILURE
}
