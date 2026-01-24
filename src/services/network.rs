use std::cmp::Ordering;
use std::time::SystemTime;

use crate::filesystem::{
    EntryFilter, FsEntry, FsEntryType, FsMetadata, ListOptions, Page, PageRequest, SortKey,
    SortOrder,
};

#[derive(Debug, Clone)]
struct NetworkResource {
    name: String,
    path: String,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkDiscoveryService;

impl NetworkDiscoveryService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_page(&self, options: ListOptions, page: PageRequest) -> Page<FsEntry> {
        let mut entries = self
            .resources()
            .into_iter()
            .map(|resource| FsEntry {
                path: resource.path.into(),
                name: resource.name,
                entry_type: FsEntryType::Directory,
                metadata: synthetic_metadata(),
            })
            .filter(|entry| matches_filter(entry, &options))
            .collect::<Vec<_>>();

        entries.sort_by(|left, right| compare_entries(left, right, &options));
        if matches!(options.sort_order, SortOrder::Desc) {
            entries.reverse();
        }

        page.apply(entries)
    }

    fn resources(&self) -> Vec<NetworkResource> {
        vec![
            NetworkResource {
                name: "NAS-Atelier (SMB)".to_string(),
                path: "smb://nas-atelier/partages".to_string(),
            },
            NetworkResource {
                name: "Studio Photo (SMB)".to_string(),
                path: "smb://studio-photo".to_string(),
            },
            NetworkResource {
                name: "Serveur Média (SMB)".to_string(),
                path: "smb://media-srv/films".to_string(),
            },
            NetworkResource {
                name: "Imprimante Bureau (Bonjour)".to_string(),
                path: "bonjour://printer-bureau".to_string(),
            },
            NetworkResource {
                name: "Salle de réunion (mDNS)".to_string(),
                path: "mdns://salle-reunion".to_string(),
            },
        ]
    }
}

fn synthetic_metadata() -> FsMetadata {
    FsMetadata {
        size: 0,
        modified: Some(SystemTime::now()),
        accessed: None,
        created: None,
        readonly: false,
    }
}

fn matches_filter(entry: &FsEntry, options: &ListOptions) -> bool {
    let passes_filter = match options.filter {
        EntryFilter::All => true,
        EntryFilter::OnlyDirectories => entry.entry_type == FsEntryType::Directory,
        EntryFilter::OnlyFiles => entry.entry_type == FsEntryType::File,
    };
    if !passes_filter {
        return false;
    }

    match &options.name_query {
        Some(query) if !query.is_empty() => {
            entry.name.to_lowercase().contains(&query.to_lowercase())
        }
        _ => true,
    }
}

fn compare_entries(left: &FsEntry, right: &FsEntry, options: &ListOptions) -> Ordering {
    if options.directories_first && left.entry_type != right.entry_type {
        return match (left.entry_type, right.entry_type) {
            (FsEntryType::Directory, _) => Ordering::Less,
            (_, FsEntryType::Directory) => Ordering::Greater,
            _ => Ordering::Equal,
        };
    }

    match options.sort_by {
        SortKey::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        SortKey::Modified => left.metadata.modified.cmp(&right.metadata.modified),
        SortKey::Size => left.metadata.size.cmp(&right.metadata.size),
    }
}
