//! # Xion
//!
//! A modern, performant file explorer written in Rust.
//!
//! ## Architecture
//!
//! Xion follows a modular architecture with five layers:
//!
//! - **`core`** - Shared types, configuration, and error handling
//! - **`filesystem`** - Filesystem abstraction, caching, and operations
//! - **`services`** - Business logic (history, search, thumbnails, terminal)
//! - **`platform`** - OS integration that exists on some targets only
//! - **`ui`** - Iced-based user interface
//!
//! ## Features
//!
//! - Tab-based navigation
//! - Virtual list rendering for large directories
//! - Thumbnail generation and caching
//! - Advanced search with filters
//! - Keyboard navigation
//! - Drag and drop support
//! - File watcher for automatic refresh
//!
//! ## Example
//!
//! ```no_run
//! // The application is typically run via the main binary:
//! // cargo run --release
//! ```

pub mod core;
pub mod filesystem;
pub mod platform;
pub mod services;
pub mod ui;

/// Windows shell integration (`--register` / `--unregister`).
///
/// Only built on Windows: the whole module drives `reg.exe` through
/// `std::os::windows`, which does not exist on other targets.
#[cfg(windows)]
pub mod registry;

// Re-export commonly used types for convenience
pub use core::{AppConfig, AppResult, XionError};
pub use filesystem::{FileSystem, FsEntry, LocalFileSystem};
