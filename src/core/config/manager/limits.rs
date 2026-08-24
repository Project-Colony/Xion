//! Bounds every numeric setting is clamped to.

pub(super) const CURRENT_CONFIG_VERSION: u32 = 1;
pub(super) const MIN_THUMBNAIL_SIZE: u32 = 24;
pub(super) const MAX_THUMBNAIL_SIZE: u32 = 256;
pub(super) const MIN_CACHE_ENTRIES: usize = 32;
pub(super) const MAX_CACHE_ENTRIES: usize = 8192;
pub(super) const MIN_CACHE_TTL_SECONDS: u64 = 30;
pub(super) const MAX_CACHE_TTL_SECONDS: u64 = 86400; // 24h max
pub(super) const MIN_PAGE_SIZE: usize = 24;
pub(super) const MAX_PAGE_SIZE: usize = 2048;
pub(super) const MIN_ROW_HEIGHT: f32 = 20.0;
pub(super) const MAX_ROW_HEIGHT: f32 = 72.0;
pub(super) const MIN_GRID_COLUMNS: usize = 1;
pub(super) const MAX_GRID_COLUMNS: usize = 12;
pub(super) const MIN_GRID_ROW_HEIGHT: f32 = 72.0;
pub(super) const MAX_GRID_ROW_HEIGHT: f32 = 240.0;
pub(super) const MAX_OVERSCAN: usize = 128;
pub(super) const MIN_METADATA_BATCH_SIZE: usize = 16;
pub(super) const MAX_METADATA_BATCH_SIZE: usize = 4096;
pub(super) const MIN_METADATA_PARALLELISM: usize = 1;
pub(super) const MAX_METADATA_PARALLELISM: usize = 32;
