use std::fs::Metadata;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FsMetadata {
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub readonly: bool,
}

impl FsMetadata {
    pub fn from_metadata(metadata: Metadata) -> Self {
        Self {
            size: metadata.len(),
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
        }
    }
}
