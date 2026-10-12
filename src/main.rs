//! Xion - A modern file explorer written in Rust.
//!
//! This is the main entry point for the Xion application.

// The GUI subsystem is a Windows notion; applying it unconditionally is
// harmless but misleading, so it is gated like the rest of the platform code.
#![cfg_attr(windows, windows_subsystem = "windows")]

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Sub-commands run before the GUI starts and never return to it.
    if let Some(flag) = args.first().filter(|arg| arg.starts_with("--")) {
        std::process::exit(xion::cli::run(flag));
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

    // Before anything that starts a thread: tokio, zbus and winit come later.
    xion::platform::prefer_vulkan_when_available();

    let start_path = args.first().and_then(|arg| xion::cli::start_path(arg));

    // Before opening anything: is a Xion already running in this session? If so
    // it has just been handed the path and will open it in a tab, so there is
    // nothing left for this process to do. Starting a second window would mean
    // a second ~38 MiB and a folder stranded away from what the user already
    // had open.
    let primary = match xion::platform::single_instance::claim(start_path.as_deref()) {
        xion::platform::single_instance::Claim::Primary(primary) => primary,
        xion::platform::single_instance::Claim::Secondary => {
            tracing::info!("Instance already running: path handed over, exiting");
            return Ok(());
        }
    };

    xion::ui::run(start_path, primary)
}
