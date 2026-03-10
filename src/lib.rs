//! # Xion
//!
//! A modern, performant file explorer written in Rust.
//!
//! ## Architecture
//!
//! Xion follows a modular architecture with four main layers:
//!
//! - **`core`** - Shared types, configuration, and error handling
//! - **`filesystem`** - Filesystem abstraction, caching, and operations
//! - **`services`** - Business logic (history, search, thumbnails)
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
pub mod registry;
pub mod services;
pub mod ui;

// Re-export commonly used types for convenience
pub use core::{AppConfig, AppResult, XionError};
pub use filesystem::{FileSystem, FsEntry, LocalFileSystem};
