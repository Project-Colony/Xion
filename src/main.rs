//! Xion - A modern file explorer written in Rust.
//!
//! This is the main entry point for the Xion application.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> iced::Result {
    // Initialize tracing for structured logging.
    // Log level can be configured via RUST_LOG environment variable.
    // Example: RUST_LOG=xion=debug,warn
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xion=info,warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Xion file explorer");

    xion::ui::run()
}
