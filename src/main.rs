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

    // Avant tout ce qui démarre un fil : tokio, zbus et winit viennent après.
    prefer_vulkan_when_available();

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

/// La variable par laquelle wgpu accepte qu'on limite ses moteurs de rendu.
///
/// C'est le seul levier disponible : iced 0.14 ne permet pas de choisir le
/// moteur. `iced_wgpu` lit l'adaptateur retenu mais laisse wgpu décider seul,
/// via `Backends::from_env_or_default()`.
#[cfg(target_os = "linux")]
const WGPU_BACKEND: &str = "WGPU_BACKEND";

/// Restreint wgpu à Vulkan, mais seulement si Vulkan répond.
///
/// Sans restriction, wgpu instancie **tous** ses moteurs pour énumérer leurs
/// adaptateurs, puis choisit. Sur cette machine il choisissait déjà Vulkan —
/// l'adaptateur OpenGL était même rejeté explicitement, « not compatible with
/// surface ». Mais l'avoir sondé laisse `libnvidia-eglcore` et le `libLLVM` de
/// Mesa chargés pour la durée du programme : 39 Mo résidents pour une
/// énumération dont le résultat était écarté.
///
/// La restriction n'est posée qu'après avoir vérifié qu'un adaptateur Vulkan
/// existe, et c'est tout l'intérêt de la fonction. `WGPU_BACKEND=vulkan` posé
/// à l'aveugle sur une machine sans pilote Vulkan — le paquet du chargeur n'est
/// pas toujours installé — laisserait wgpu sans aucun adaptateur, et Xion ne
/// démarrerait pas du tout. Le repli sur OpenGL doit rester possible.
///
/// Linux seulement : sous Windows le moteur par défaut est DX12, que rien ne
/// dit qu'il faille abandonner pour Vulkan, et macOS n'a que Metal.
#[cfg(target_os = "linux")]
fn prefer_vulkan_when_available() {
    // Un choix explicite prime : qui pose la variable veut ce qu'il a écrit.
    if std::env::var_os(WGPU_BACKEND).is_some() {
        return;
    }

    let vulkan_answers = {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        !instance
            .enumerate_adapters(wgpu::Backends::VULKAN)
            .is_empty()
        // L'instance meurt ici, avant que la variable ne soit posée.
    };

    if !vulkan_answers {
        tracing::debug!("Aucun adaptateur Vulkan : wgpu choisira seul");
        return;
    }

    // SAFETY: `set_var` exige qu'aucun autre fil ne lise l'environnement. On est
    // au tout début de `main` : ni tokio, ni zbus, ni winit n'existent encore,
    // et l'instance du sondage ci-dessus a été détruite avant cette ligne.
    unsafe { std::env::set_var(WGPU_BACKEND, "vulkan") };
    tracing::info!("Adaptateur Vulkan trouvé : rendu restreint à Vulkan");
}

/// Ailleurs, wgpu décide seul.
#[cfg(not(target_os = "linux"))]
fn prefer_vulkan_when_available() {}

/// Resolve a command-line path argument against the current directory.
///
/// Returns `None` when the path does not exist so the caller falls back to the
/// configured start directory instead of opening a dead location.
fn resolve_start_path(arg: &str) -> Option<PathBuf> {
    // Not `PathBuf::from(arg)`: a `.desktop` entry's `%U` hands over a URI, so
    // opening a folder from a dock or `xdg-open` arrives as
    // `file:///home/alice/Mes%20documents`. Read as a path, that names a
    // directory called `file:` which does not exist, and the launcher appears
    // to do nothing at all.
    let path = xion::core::uri::path_from_argument(arg);
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
