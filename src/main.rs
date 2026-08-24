//! Xion - A modern file explorer written in Rust.
//!
//! This is the main entry point for the Xion application.

// The GUI subsystem is a Windows notion; applying it unconditionally is
// harmless but misleading, so it is gated like the rest of the platform code.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Exit code returned when a `--flag` sub-command fails.
const EXIT_FAILURE: i32 = 1;

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Sub-commands run before the GUI starts and never return to it.
    if let Some(flag) = args.first().filter(|arg| arg.starts_with("--")) {
        std::process::exit(run_cli(flag));
    }

    // Initialize tracing for structured logging.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xion=info,warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Xion file explorer");

    let start_path = args.first().and_then(|arg| resolve_start_path(arg));

    // Before opening anything: is a Xion already running in this session? If so
    // it has just been handed the path and will open it in a tab, so there is
    // nothing left for this process to do. Starting a second window would mean
    // a second ~38 MiB and a folder stranded away from what the user already
    // had open.
    let primary = match xion::platform::single_instance::claim(start_path.as_deref()) {
        xion::platform::single_instance::Claim::Primary(primary) => primary,
        xion::platform::single_instance::Claim::Secondary => {
            tracing::info!("Instance déjà en cours : chemin transmis, sortie");
            return Ok(());
        }
    };

    xion::ui::run(start_path, primary)
}

/// Resolve a command-line path argument against the current directory.
///
/// Returns `None` when the path does not exist so the caller falls back to the
/// configured start directory instead of opening a dead location.
fn resolve_start_path(arg: &str) -> Option<PathBuf> {
    let path = PathBuf::from(arg);
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
        tracing::warn!("Chemin de démarrage invalide: {}", resolved.display());
        None
    }
}

/// Run a `--flag` sub-command and return the process exit code.
fn run_cli(flag: &str) -> i32 {
    // Without this the `windows_subsystem = "windows"` attribute leaves the
    // process with no stdout/stderr, so every message below goes nowhere.
    attach_parent_console();

    match flag {
        "--register" => run_registry_command(RegistryCommand::Register),
        "--unregister" => run_registry_command(RegistryCommand::Unregister),
        "--help" | "--version" => {
            println!("Xion {}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Usage: xion [CHEMIN]");
            println!("       xion --register     Ajoute Xion au menu contextuel Windows");
            println!("       xion --unregister   Retire Xion du menu contextuel Windows");
            0
        }
        other => {
            eprintln!("Option inconnue : {other}");
            eprintln!("Utilisez `xion --help` pour la liste des options.");
            EXIT_FAILURE
        }
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
        RegistryCommand::Register => xion::registry::register(),
        RegistryCommand::Unregister => xion::registry::unregister(),
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Échec de l'opération sur le registre : {error}");
            EXIT_FAILURE
        }
    }
}

#[cfg(not(windows))]
fn run_registry_command(_command: RegistryCommand) -> i32 {
    eprintln!("L'intégration au menu contextuel n'est disponible que sous Windows.");
    EXIT_FAILURE
}

/// Reattach stdout/stderr to the console that launched the process.
#[cfg(windows)]
fn attach_parent_console() {
    // ATTACH_PARENT_PROCESS. Failure just means there is no parent console
    // (launched from Explorer), in which case there is nothing to attach to.
    unsafe {
        let _ = windows_sys::Win32::System::Console::AttachConsole(u32::MAX);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}
