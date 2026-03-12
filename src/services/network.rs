use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::filesystem::{
    sorting::{compare_entries, matches_filter},
    FsEntry, FsEntryType, FsMetadata, ListOptions, Page, PageRequest,
};

/// How long before cached network scan results expire.
const SCAN_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct NetworkResource {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
struct DiscoveryCache {
    resources: Vec<NetworkResource>,
    fetched_at: Option<Instant>,
    scanning: bool,
}

impl DiscoveryCache {
    fn is_stale(&self) -> bool {
        self.fetched_at
            .map(|t| t.elapsed() > SCAN_TTL)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetworkDiscoveryService {
    cache: Arc<Mutex<DiscoveryCache>>,
}

impl NetworkDiscoveryService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if a background scan should be started.
    pub fn needs_scan(&self) -> bool {
        self.cache
            .lock()
            .map(|c| c.is_stale() && !c.scanning)
            .unwrap_or(false)
    }

    /// Mark a scan as in-progress.
    pub fn mark_scanning(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.scanning = true;
        }
    }

    /// Update cache with scan results.
    pub fn update(&self, resources: Vec<NetworkResource>) {
        if let Ok(mut c) = self.cache.lock() {
            c.resources = resources;
            c.fetched_at = Some(Instant::now());
            c.scanning = false;
        }
    }

    /// Returns the current (possibly stale) cached list page.
    pub fn list_page(&self, options: ListOptions, page: PageRequest) -> Page<FsEntry> {
        let resources = self
            .cache
            .lock()
            .map(|c| c.resources.clone())
            .unwrap_or_default();

        let mut entries: Vec<FsEntry> = resources
            .into_iter()
            .map(|r| FsEntry {
                path: r.path.into(),
                name: r.name,
                entry_type: FsEntryType::Directory,
                metadata: synthetic_metadata(),
            })
            .filter(|e| matches_filter(e, &options))
            .collect();

        entries.sort_by(|a, b| compare_entries(a, b, &options));
        page.apply(entries)
    }

    /// Runs the actual network discovery. Blocking — call via spawn_blocking.
    pub fn run_scan() -> Vec<NetworkResource> {
        let mut results = Vec::new();

        // Step 1: enumerate network computers via `net view`
        let computers = discover_computers_net_view();

        // Step 2: for each computer, list its shares
        for computer in &computers {
            let shares = list_shares_net_view(computer);
            if shares.is_empty() {
                // Add the computer itself as a browse target
                results.push(NetworkResource {
                    name: computer.trim_start_matches('\\').to_string(),
                    path: format!("smb:{}", computer),
                });
            } else {
                for share in shares {
                    results.push(NetworkResource {
                        name: format!(
                            "{}\\{}",
                            computer.trim_start_matches('\\'),
                            share
                        ),
                        path: format!("smb:{}\\{}", computer, share),
                    });
                }
            }
        }

        results
    }
}

/// Parse `net view` output to get a list of `\\COMPUTERNAME` entries.
fn discover_computers_net_view() -> Vec<String> {
    let output = std::process::Command::new("net")
        .args(["view", "/all"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    parse_net_view_computers(&text)
}

fn parse_net_view_computers(text: &str) -> Vec<String> {
    let mut computers = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\\\\") {
            // Line like: \\SERVERNAME    Remark text
            let name = trimmed
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                computers.push(name);
            }
        }
    }
    computers
}

/// Parse `net view \\COMPUTERNAME` output to get share names.
fn list_shares_net_view(computer: &str) -> Vec<String> {
    let output = std::process::Command::new("net")
        .args(["view", computer])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    parse_net_view_shares(&text)
}

fn parse_net_view_shares(text: &str) -> Vec<String> {
    let mut shares = Vec::new();
    let mut in_shares = false;
    for line in text.lines() {
        let trimmed = line.trim();
        // Header separator signals start of share list
        if trimmed.starts_with("---") {
            in_shares = true;
            continue;
        }
        if !in_shares {
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        // Columns: Share name  Type  Used as  Comment
        // We only want Disk shares (skip PRINT, IPC$, ADMIN$, etc.)
        // Use split_whitespace() to handle multiple-space column separators,
        // then take at most 4 tokens.
        let cols: Vec<&str> = trimmed.split_whitespace().take(4).collect();
        if cols.len() >= 2 {
            let share_name = cols[0];
            let share_type = cols[1];
            // Skip hidden admin shares and non-disk shares
            if share_name.ends_with('$') {
                continue;
            }
            if share_type.eq_ignore_ascii_case("Disk") || share_type.eq_ignore_ascii_case("Disque") {
                shares.push(share_name.to_string());
            }
        }
    }
    shares
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_computers_from_net_view() {
        let input = "\
Shared resources at \\\\
Server Name            Remark
-------------------------------------------------------------------------------
\\\\NAS-ATELIER         Network Attached Storage
\\\\DESKTOP-ABC123
\\\\MEDIA-SRV           Media server
The command completed successfully.
";
        let computers = parse_net_view_computers(input);
        assert_eq!(computers, vec![
            "\\\\NAS-ATELIER",
            "\\\\DESKTOP-ABC123",
            "\\\\MEDIA-SRV",
        ]);
    }

    #[test]
    fn parse_shares_from_net_view() {
        let input = "\
Shared resources at \\\\NAS-ATELIER

Share name  Type  Used as  Comment
---
Films       Disk           Films et séries
Musique     Disk           Collection musicale
ADMIN$      Disk           Admin distante
print$      Print          Drivers
IPC$        IPC

The command completed successfully.
";
        let shares = parse_net_view_shares(input);
        assert_eq!(shares, vec!["Films", "Musique"]);
    }

    #[test]
    fn cache_starts_stale() {
        let svc = NetworkDiscoveryService::new();
        assert!(svc.needs_scan());
    }

    #[test]
    fn cache_not_stale_after_update() {
        let svc = NetworkDiscoveryService::new();
        svc.update(vec![]);
        assert!(!svc.needs_scan());
    }

    #[test]
    fn list_page_returns_cached() {
        let svc = NetworkDiscoveryService::new();
        svc.update(vec![
            NetworkResource { name: "Test".to_string(), path: "smb://test".to_string() },
        ]);
        let options = ListOptions::default();
        let page = svc.list_page(options, PageRequest { offset: 0, limit: 100 });
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "Test");
    }
}
