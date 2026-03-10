use std::time::SystemTime;

use crate::filesystem::{
    sorting::{compare_entries, matches_filter},
    FsEntry, FsEntryType, FsMetadata, ListOptions, Page, PageRequest,
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

