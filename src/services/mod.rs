//! Business logic services for Xion.
//!
//! This module contains the high-level services that coordinate between
//! the UI and filesystem layers. Each service encapsulates a specific
//! domain concern:
//!
//! - [`FavoritesService`] - Manages user's favorite paths
//! - [`HistoryService`] - Navigation history (back/forward)
//! - [`DirectoryLoader`] - Orchestrates directory loading with caching
//! - [`NetworkDiscoveryService`] - Network resource discovery
//! - [`SearchService`] - Search indexing and filtering
//! - [`ThumbnailService`] - Thumbnail generation and caching
//! - [`VirtualList`] - Virtual list calculations for efficient rendering

pub mod favorites;
pub mod highlight;
pub mod history;
pub mod loader;
pub mod network;
pub mod search;
pub mod thumbnails;
pub mod virtualization;

pub use favorites::FavoritesService;
pub use history::HistoryService;
pub use loader::DirectoryLoader;
pub use network::{NetworkDiscoveryService, NetworkResource};
pub use search::{SearchIndex, SearchIndexOptions, SearchQuery, SearchService};
pub use thumbnails::{
    PreviewImageService, Thumbnail, ThumbnailService, generate_preview, generate_thumbnail,
};
pub use virtualization::{VirtualList, VirtualWindow};
