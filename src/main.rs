//! Xion - A modern file explorer written in Rust.
//!
//! This is the main entry point for the Xion application.

#![windows_subsystem = "windows"]

use std::path::PathBuf;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Handle --register / --unregister before initializing the GUI.
    if let Some(flag) = args.first() {
        match flag.as_str() {
            "--register" => {
                if let Err(error) = xion::registry::register() {
                    eprintln!("Erreur lors de l'enregistrement : {error}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--unregister" => {
                if let Err(error) = xion::registry::unregister() {
                    eprintln!("Erreur lors de la désinscription : {error}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            _ => {}
        }
    }

    // Initialize tracing for structured logging.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xion=info,warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // If a path argument is provided, use it as the start directory.
    let start_path = args.first().map(|arg| {
        let path = PathBuf::from(arg);
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    });

    tracing::info!("Starting Xion file explorer");

    xion::ui::run(start_path)
}
