mod credentials;
#[cfg(feature = "ftp")]
mod ftp;
mod mdns;
mod net_view;
mod wnet;

pub use credentials::*;
#[cfg(feature = "ftp")]
pub use ftp::*;
pub use mdns::*;
use net_view::*;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::filesystem::{
    FsEntry, FsEntryType, FsMetadata, ListOptions, Page, PageRequest,
    sorting::{compare_entries, matches_filter},
};

/// How long before cached network scan results expire.
const SCAN_TTL: Duration = Duration::from_secs(30);

/// Maximum scan duration before we consider it stuck.
const SCAN_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound on retained mDNS entries. Announcements come from any machine on
/// the LAN and the browse loop used to append for three seconds unchecked.
const MAX_MDNS_RESULTS: usize = 64;

/// Longest mDNS service label we display; instance names are attacker-chosen.
const MAX_MDNS_LABEL: usize = 48;

/// Diagnostics of the last scan run through [`NetworkDiscoveryService::run_scan`].
///
/// `run_scan` is an associated function — the UI hands it to `spawn_blocking` as
/// a bare function pointer — so it has no `&self` to report through. Failures
/// used to end up in the log while the caller received an empty `Vec`,
/// indistinguishable from "there is nothing on this network". The scan parks its
/// error here and [`NetworkDiscoveryService::last_error`] reads it back.
static LAST_SCAN_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn record_scan_error(message: Option<String>) {
    if let Ok(mut slot) = LAST_SCAN_ERROR.lock() {
        *slot = message;
    }
}

fn recorded_scan_error() -> Option<String> {
    LAST_SCAN_ERROR.lock().ok().and_then(|slot| slot.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkStatus {
    Unknown,
    Online,
    Offline,
    Scanning,
}

#[derive(Debug, Clone)]
pub struct NetworkResource {
    pub name: String,
    pub path: String,
    pub status: NetworkStatus,
}

/// Result of a discovery run: what was found *and* what failed.
#[derive(Debug, Clone, Default)]
pub struct ScanOutcome {
    pub resources: Vec<NetworkResource>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DiscoveryCache {
    resources: Vec<NetworkResource>,
    fetched_at: Option<Instant>,
    scanning: bool,
    last_error: Option<String>,
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
            .map(|c| {
                if c.scanning {
                    // Safety: if scanning has been true for > 2 minutes, assume the scan
                    // task crashed and allow a new scan to start.
                    c.fetched_at
                        .map(|t| t.elapsed() > SCAN_TIMEOUT)
                        .unwrap_or(true)
                } else {
                    c.is_stale()
                }
            })
            .unwrap_or(false)
    }

    /// Mark a scan as in-progress.
    pub fn mark_scanning(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.scanning = true;
            // Record the time so we can detect stuck scans
            c.fetched_at = Some(Instant::now());
        }
    }

    /// Update cache with scan results.
    pub fn update(&self, resources: Vec<NetworkResource>) {
        if let Ok(mut c) = self.cache.lock() {
            c.resources = resources;
            c.fetched_at = Some(Instant::now());
            c.scanning = false;
            c.last_error = None;
        }
    }

    /// Update cache with an error.
    pub fn update_error(&self, error: String) {
        if let Ok(mut c) = self.cache.lock() {
            c.scanning = false;
            c.last_error = Some(error);
            c.fetched_at = Some(Instant::now());
        }
    }

    /// Returns the last scan error, if any.
    ///
    /// Falls back to what the last [`Self::run_scan`] recorded, so a discovery
    /// backend that is simply unavailable on this platform is reportable even
    /// though `run_scan` has no handle on this instance.
    pub fn last_error(&self) -> Option<String> {
        self.cache
            .lock()
            .ok()
            .and_then(|c| c.last_error.clone())
            .or_else(recorded_scan_error)
    }

    /// Returns whether a scan is currently in progress.
    pub fn is_scanning(&self) -> bool {
        self.cache.lock().map(|c| c.scanning).unwrap_or(false)
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
    ///
    /// Kept returning a bare `Vec` for the existing `spawn_blocking` call site;
    /// the diagnostics go to [`LAST_SCAN_ERROR`]. Prefer [`Self::run_scan_detailed`].
    pub fn run_scan() -> Vec<NetworkResource> {
        let outcome = Self::run_scan_detailed();
        record_scan_error(outcome.error);
        outcome.resources
    }

    /// Same discovery, with the failures a bare `Vec` cannot express.
    ///
    /// Tries native WinAPI first (WNetEnumResource), then falls back to `net view`,
    /// and always adds whatever mDNS announces.
    pub fn run_scan_detailed() -> ScanOutcome {
        tracing::info!("Réseau: début du scan réseau");
        let start = Instant::now();
        let mut errors: Vec<String> = Vec::new();

        // Try WinAPI first
        #[cfg(windows)]
        {
            match discover_via_wnet() {
                Ok(resources) if !resources.is_empty() => {
                    tracing::info!(
                        "Réseau: scan WinAPI terminé en {:?}, {} ressources trouvées",
                        start.elapsed(),
                        resources.len()
                    );
                    return ScanOutcome {
                        resources,
                        error: None,
                    };
                }
                Ok(_) => {
                    tracing::debug!(
                        "Réseau: WinAPI n'a trouvé aucune ressource, fallback sur net view"
                    );
                }
                Err(e) => {
                    tracing::warn!("Réseau: WinAPI échouée ({e}), fallback sur net view");
                    errors.push(format!("WinAPI: {e}"));
                }
            }
        }

        // Fallback to net view
        let mut results = match discover_via_net_view() {
            Ok(found) => {
                tracing::info!(
                    "Réseau: scan net view terminé en {:?}, {} ressources trouvées",
                    start.elapsed(),
                    found.len()
                );
                found
            }
            Err(e) => {
                tracing::warn!("Réseau: découverte SMB indisponible: {e}");
                errors.push(e);
                Vec::new()
            }
        };

        // Also try mDNS discovery for additional services
        let mdns_results = discover_mdns_services();
        if !mdns_results.is_empty() {
            tracing::info!("Réseau: {} services mDNS découverts", mdns_results.len());
            results.extend(mdns_results);
        }

        ScanOutcome {
            resources: results,
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join(" ; "))
            },
        }
    }
}

/// Placeholder metadata for entries that have no local file behind them
/// (network shares, remote listings).
fn synthetic_metadata() -> FsMetadata {
    FsMetadata {
        size: 0,
        modified: Some(std::time::SystemTime::now()),
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
        assert_eq!(
            computers,
            vec!["\\\\NAS-ATELIER", "\\\\DESKTOP-ABC123", "\\\\MEDIA-SRV",]
        );
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
    fn parse_shares_with_spaces_in_name() {
        let input = "\
Shared resources at \\\\NAS-ATELIER

Share name  Type  Used as  Comment
---
My Films    Disk           Films et séries
ADMIN$      Disk           Admin distante

The command completed successfully.
";
        let shares = parse_net_view_shares(input);
        assert_eq!(shares, vec!["My Films"]);
    }

    #[test]
    fn parse_shares_on_a_non_english_windows() {
        // German and Japanese type labels used to yield an empty list because the
        // parser looked for the literal "Disk"/"Disque".
        let german = "\
Freigegebene Ressourcen auf \\\\NAS-ATELIER

Freigabename  Typ    Verwendet als  Kommentar
-------------------------------------------------------------------------------
Filme         Platte
Drucker       Druck
IPC$          IPC

Der Befehl wurde erfolgreich ausgeführt.
";
        assert_eq!(parse_net_view_shares(german), vec!["Filme"]);

        let japanese = "\
\\\\NAS-ATELIER の共有リソース

共有名        タイプ  制限  コメント
-------------------------------------------------------------------------------
共有フォルダ  Disk
IPC$          IPC

コマンドは正常に終了しました。
";
        assert_eq!(parse_net_view_shares(japanese), vec!["共有フォルダ"]);
    }

    #[test]
    fn parse_shares_ignores_trailing_message_without_blank_line() {
        // A one-column line is prose, never a share row.
        let input = "\
Share name  Type
---
Films       Disk
The command completed successfully.
";
        assert_eq!(parse_net_view_shares(input), vec!["Films"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn smb_discovery_fails_explicitly_off_windows() {
        // It used to return an empty Vec, indistinguishable from "no shares".
        let error = discover_via_net_view().expect_err("doit échouer hors Windows");
        assert!(error.contains("Windows"), "message inattendu: {error}");
    }

    #[test]
    fn mdns_hostname_validation_rejects_hostile_names() {
        assert!(is_valid_mdns_hostname("nas-atelier.local"));
        assert!(is_valid_mdns_hostname("PC_01"));
        assert!(!is_valid_mdns_hostname(""));
        assert!(!is_valid_mdns_hostname("nas atelier.local"));
        assert!(!is_valid_mdns_hostname("nas\u{202E}local"));
        assert!(!is_valid_mdns_hostname("nas\u{0007}.local"));
        assert!(!is_valid_mdns_hostname("nas..local"));
        assert!(!is_valid_mdns_hostname(&"a".repeat(300)));
    }

    #[test]
    fn mdns_label_is_sanitized_and_bounded() {
        let hostile = "Ser\u{202E}veur\u{0007}\u{200B}";
        let clean = sanitize_mdns_label(hostile);
        assert_eq!(clean, "Serveur");
        assert!(!clean.chars().any(|c| c.is_control()));

        let long = "N".repeat(500);
        assert_eq!(sanitize_mdns_label(&long).chars().count(), MAX_MDNS_LABEL);
    }

    #[test]
    fn secret_wide_is_scrubbed() {
        let mut secret = SecretWide::new("mot-de-passe");
        assert!(secret.buf.iter().any(|&c| c != 0));
        secret.zeroize();
        assert!(secret.buf.iter().all(|&c| c == 0));
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
        svc.update(vec![NetworkResource {
            name: "Test".to_string(),
            path: "smb://test".to_string(),
            status: NetworkStatus::Online,
        }]);
        let options = ListOptions::default();
        let page = svc.list_page(
            options,
            PageRequest {
                offset: 0,
                limit: 100,
            },
        );
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "Test");
    }

    #[test]
    fn error_tracking() {
        let svc = NetworkDiscoveryService::new();
        svc.mark_scanning();
        assert!(svc.is_scanning());
        svc.update_error("test error".to_string());
        assert!(!svc.is_scanning());
        assert_eq!(svc.last_error(), Some("test error".to_string()));
    }

    #[test]
    fn scan_error_reaches_last_error() {
        // `run_scan` has no `&self`, so its failures used to be unreportable.
        let svc = NetworkDiscoveryService::new();
        record_scan_error(Some("panne de scan".to_string()));
        assert_eq!(svc.last_error(), Some("panne de scan".to_string()));
        record_scan_error(None);
    }
}
