//! Clamping every value read from disk, recording a warning for each one
//! that had to be corrected.

use std::path::PathBuf;

use crate::core::config::types::{ConfigWarning, ViewColumn};

use super::*;
// ── Validation helpers ────────────────────────────────────────────────────────

pub(super) fn validated_path(
    path: PathBuf,
    fallback: &std::path::Path,
    warnings: &mut Vec<ConfigWarning>,
) -> PathBuf {
    if path.is_dir() {
        return path;
    }
    warnings.push(ConfigWarning {
        message: format!("start_path invalide, fallback sur {}", fallback.display()),
    });
    fallback.to_path_buf()
}

pub(super) fn validated_thumbnail_size(
    value: u32,
    fallback: u32,
    warnings: &mut Vec<ConfigWarning>,
) -> u32 {
    if (MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("thumbnail_size hors limites ({MIN_THUMBNAIL_SIZE}-{MAX_THUMBNAIL_SIZE}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_cache_entries(
    value: usize,
    fallback: usize,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_CACHE_ENTRIES..=MAX_CACHE_ENTRIES).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("{label} hors limites ({MIN_CACHE_ENTRIES}-{MAX_CACHE_ENTRIES}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_cache_ttl(
    value: u64,
    fallback: u64,
    label: &str,
    warnings: &mut Vec<ConfigWarning>,
) -> u64 {
    if (MIN_CACHE_TTL_SECONDS..=MAX_CACHE_TTL_SECONDS).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("{label} hors limites ({MIN_CACHE_TTL_SECONDS}-{MAX_CACHE_TTL_SECONDS}s), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_metadata_batch_size(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_METADATA_BATCH_SIZE..=MAX_METADATA_BATCH_SIZE).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("filesystem.metadata_batch_size hors limites ({MIN_METADATA_BATCH_SIZE}-{MAX_METADATA_BATCH_SIZE}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_metadata_parallelism(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_METADATA_PARALLELISM..=MAX_METADATA_PARALLELISM).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("filesystem.metadata_parallelism hors limites ({MIN_METADATA_PARALLELISM}-{MAX_METADATA_PARALLELISM}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_page_size(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!(
            "page_size hors limites ({MIN_PAGE_SIZE}-{MAX_PAGE_SIZE}), fallback sur {fallback}"
        ),
    });
    fallback
}

pub(super) fn validated_row_height(
    value: f32,
    fallback: f32,
    warnings: &mut Vec<ConfigWarning>,
) -> f32 {
    if (MIN_ROW_HEIGHT..=MAX_ROW_HEIGHT).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!(
            "row_height hors limites ({MIN_ROW_HEIGHT}-{MAX_ROW_HEIGHT}), fallback sur {fallback}"
        ),
    });
    fallback
}

pub(super) fn validated_grid_columns(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    if (MIN_GRID_COLUMNS..=MAX_GRID_COLUMNS).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("grid_columns hors limites ({MIN_GRID_COLUMNS}-{MAX_GRID_COLUMNS}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_grid_row_height(
    value: f32,
    fallback: f32,
    warnings: &mut Vec<ConfigWarning>,
) -> f32 {
    if (MIN_GRID_ROW_HEIGHT..=MAX_GRID_ROW_HEIGHT).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!("grid_row_height hors limites ({MIN_GRID_ROW_HEIGHT}-{MAX_GRID_ROW_HEIGHT}), fallback sur {fallback}"),
    });
    fallback
}

pub(super) fn validated_overscan(
    value: usize,
    fallback: usize,
    warnings: &mut Vec<ConfigWarning>,
) -> usize {
    const MIN_OVERSCAN: usize = 1;
    if (MIN_OVERSCAN..=MAX_OVERSCAN).contains(&value) {
        return value;
    }
    warnings.push(ConfigWarning {
        message: format!(
            "overscan hors limites ({MIN_OVERSCAN}-{MAX_OVERSCAN}), fallback sur {fallback}"
        ),
    });
    fallback
}

pub(super) fn validated_view_columns(
    value: Vec<ViewColumn>,
    fallback: Vec<ViewColumn>,
    warnings: &mut Vec<ConfigWarning>,
) -> Vec<ViewColumn> {
    if value.is_empty() {
        warnings.push(ConfigWarning {
            message: "view.columns vide, fallback sur la configuration par défaut".to_string(),
        });
        return fallback;
    }
    value
}
