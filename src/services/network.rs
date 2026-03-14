use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::filesystem::{
    sorting::{compare_entries, matches_filter},
    FsEntry, FsEntryType, FsMetadata, ListOptions, Page, PageRequest,
};

/// How long before cached network scan results expire.
const SCAN_TTL: Duration = Duration::from_secs(30);

/// Maximum scan duration before we consider it stuck.
const SCAN_TIMEOUT: Duration = Duration::from_secs(120);

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
    pub fn last_error(&self) -> Option<String> {
        self.cache.lock().ok().and_then(|c| c.last_error.clone())
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
    /// Tries native WinAPI first (WNetEnumResource), then falls back to `net view`.
    pub fn run_scan() -> Vec<NetworkResource> {
        tracing::info!("Réseau: début du scan réseau");
        let start = Instant::now();

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
                    return resources;
                }
                Ok(_) => {
                    tracing::debug!("Réseau: WinAPI n'a trouvé aucune ressource, fallback sur net view");
                }
                Err(e) => {
                    tracing::warn!("Réseau: WinAPI échouée ({e}), fallback sur net view");
                }
            }
        }

        // Fallback to net view
        let mut results = discover_via_net_view();
        tracing::info!(
            "Réseau: scan net view terminé en {:?}, {} ressources trouvées",
            start.elapsed(),
            results.len()
        );

        // Also try mDNS discovery for additional services
        let mdns_results = discover_mdns_services();
        if !mdns_results.is_empty() {
            tracing::info!("Réseau: {} services mDNS découverts", mdns_results.len());
            results.extend(mdns_results);
        }

        results
    }
}

// ── WinAPI discovery (WNetEnumResource) ─────────────────────────────────────

#[cfg(windows)]
fn discover_via_wnet() -> Result<Vec<NetworkResource>, String> {
    use std::ffi::c_void;
    use windows_sys::Win32::NetworkManagement::WNet::*;
    use windows_sys::Win32::Foundation::*;

    let mut results = Vec::new();

    unsafe {
        let mut handle: *mut c_void = std::ptr::null_mut();
        let ret = WNetOpenEnumW(
            RESOURCE_GLOBALNET,
            RESOURCETYPE_DISK,
            0,
            std::ptr::null(),
            &mut handle,
        );
        if ret != NO_ERROR {
            return Err(format!("WNetOpenEnumW échoué: code {ret}"));
        }

        let buf_size: usize = 16384;
        let mut buffer: Vec<u8> = vec![0u8; buf_size];

        loop {
            let mut actual_buf_size = buf_size as u32;
            let mut count: u32 = u32::MAX; // -1 = enumerate all
            let ret = WNetEnumResourceW(
                handle,
                &mut count,
                buffer.as_mut_ptr() as *mut _,
                &mut actual_buf_size,
            );

            if ret == ERROR_NO_MORE_ITEMS {
                break;
            }
            if ret != NO_ERROR {
                let _ = WNetCloseEnum(handle);
                return Err(format!("WNetEnumResourceW échoué: code {ret}"));
            }

            // Safety: WNetEnumResourceW filled `count` NETRESOURCEW structs into buffer
            let resources = std::slice::from_raw_parts(
                buffer.as_ptr() as *const NETRESOURCEW,
                count as usize,
            );

            for res in resources {
                let remote_name = if res.lpRemoteName.is_null() {
                    continue;
                } else {
                    wstr_to_string(res.lpRemoteName)
                };

                if remote_name.is_empty() {
                    continue;
                }

                // It could be a server or a share — check dwType
                let display_name = remote_name.trim_start_matches('\\').to_string();
                let is_container = res.dwUsage & RESOURCEUSAGE_CONTAINER != 0;

                if !is_container {
                    results.push(NetworkResource {
                        name: display_name,
                        path: format!("smb:{}", remote_name),
                        status: NetworkStatus::Online,
                    });
                } else {
                    // Container (server/domain) - enumerate its shares
                    match enumerate_shares_wnet(res) {
                        Ok(shares) if !shares.is_empty() => {
                            results.extend(shares);
                        }
                        _ => {
                            results.push(NetworkResource {
                                name: display_name,
                                path: format!("smb:{}", remote_name),
                                status: NetworkStatus::Online,
                            });
                        }
                    }
                }
            }
        }

        let _ = WNetCloseEnum(handle);
    }

    Ok(results)
}

#[cfg(windows)]
unsafe fn enumerate_shares_wnet(
    resource: &windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW,
) -> Result<Vec<NetworkResource>, String> {
    use std::ffi::c_void;
    use windows_sys::Win32::NetworkManagement::WNet::*;
    use windows_sys::Win32::Foundation::*;

    let mut results = Vec::new();
    let mut handle: *mut c_void = std::ptr::null_mut();

    let ret = unsafe {
        WNetOpenEnumW(
            RESOURCE_GLOBALNET,
            RESOURCETYPE_DISK,
            0,
            resource as *const _ as *const _,
            &mut handle,
        )
    };
    if ret != NO_ERROR {
        return Err(format!("WNetOpenEnumW (shares) échoué: code {ret}"));
    }

    let buf_size: usize = 16384;
    let mut buffer: Vec<u8> = vec![0u8; buf_size];

    loop {
        let mut count: u32 = u32::MAX;
        let mut actual_buf_size = buf_size as u32;
        let ret = unsafe {
            WNetEnumResourceW(
                handle,
                &mut count,
                buffer.as_mut_ptr() as *mut _,
                &mut actual_buf_size,
            )
        };

        if ret == ERROR_NO_MORE_ITEMS {
            break;
        }
        if ret != NO_ERROR {
            break;
        }

        let resources = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const NETRESOURCEW,
                count as usize,
            )
        };

        for res in resources {
            let remote_name = if res.lpRemoteName.is_null() {
                continue;
            } else {
                unsafe { wstr_to_string(res.lpRemoteName) }
            };

            if remote_name.is_empty() {
                continue;
            }

            // Skip admin shares
            let share_name = remote_name.rsplit('\\').next().unwrap_or("");
            if share_name.ends_with('$') || share_name.eq_ignore_ascii_case("IPC$") {
                continue;
            }

            let display_name = remote_name.trim_start_matches('\\').to_string();
            results.push(NetworkResource {
                name: display_name,
                path: format!("smb:{}", remote_name),
                status: NetworkStatus::Online,
            });
        }
    }

    unsafe { let _ = WNetCloseEnum(handle); }
    Ok(results)
}

#[cfg(windows)]
unsafe fn wstr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

// ── net view fallback ──────────────────────────────────────────────────────

fn discover_via_net_view() -> Vec<NetworkResource> {
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
                status: NetworkStatus::Online,
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
                    status: NetworkStatus::Online,
                });
            }
        }
    }

    results
}

/// Parse `net view` output to get a list of `\\COMPUTERNAME` entries.
fn discover_computers_net_view() -> Vec<String> {
    let output = std::process::Command::new("net")
        .args(["view", "/all"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("Réseau: commande 'net view /all' échouée: {e}");
            return vec![];
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            "Réseau: 'net view /all' code de sortie {:?}, stderr: {}",
            output.status.code(),
            stderr.trim()
        );
    }

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
            if !name.is_empty() && is_valid_computer_name(&name) {
                computers.push(name);
            }
        }
    }
    computers
}

/// Returns `true` if `name` looks like a valid `\\COMPUTERNAME`.
fn is_valid_computer_name(name: &str) -> bool {
    name.starts_with("\\\\")
        && name.len() <= 17 // \\COMPUTERNAME (max 15 chars + \\)
        && name[2..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Parse `net view \\COMPUTERNAME` output to get share names.
fn list_shares_net_view(computer: &str) -> Vec<String> {
    if !is_valid_computer_name(computer) {
        return vec![];
    }

    let output = std::process::Command::new("net")
        .args(["view", computer])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("Réseau: commande 'net view {computer}' échouée: {e}");
            return vec![];
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::debug!(
            "Réseau: 'net view {computer}' code de sortie {:?}: {}",
            output.status.code(),
            stderr.trim()
        );
    }

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
        // Share names may contain spaces, so instead of splitting by whitespace
        // we locate the type keyword ("Disk"/"Disque") and treat everything
        // before it as the share name.
        let disk_pos = trimmed.find("Disk").or_else(|| trimmed.find("Disque"));
        if let Some(pos) = disk_pos {
            let share_name = trimmed[..pos].trim();
            if share_name.is_empty() || share_name.ends_with('$') {
                continue;
            }
            shares.push(share_name.to_string());
        }
    }
    shares
}

/// #3: Connect to a password-protected network share using WNetAddConnection2W.
/// Returns Ok(()) on success, or an error message on failure.
#[cfg(windows)]
pub fn connect_authenticated(remote_path: &str, username: &str, password: &str) -> Result<(), String> {
    use windows_sys::Win32::NetworkManagement::WNet::*;
    use windows_sys::Win32::Foundation::*;

    let remote_wide: Vec<u16> = remote_path.encode_utf16().chain(std::iter::once(0)).collect();
    let user_wide: Vec<u16> = username.encode_utf16().chain(std::iter::once(0)).collect();
    let pass_wide: Vec<u16> = password.encode_utf16().chain(std::iter::once(0)).collect();

    let net_resource = NETRESOURCEW {
        dwScope: 0,
        dwType: RESOURCETYPE_DISK,
        dwDisplayType: 0,
        dwUsage: 0,
        lpLocalName: std::ptr::null_mut(),
        lpRemoteName: remote_wide.as_ptr() as *mut u16,
        lpComment: std::ptr::null_mut(),
        lpProvider: std::ptr::null_mut(),
    };

    let ret = unsafe {
        WNetAddConnection2W(
            &net_resource,
            pass_wide.as_ptr(),
            user_wide.as_ptr(),
            0, // no flags
        )
    };

    if ret == NO_ERROR {
        tracing::info!("Réseau: connexion authentifiée à {} réussie", remote_path);
        Ok(())
    } else {
        let msg = format!("WNetAddConnection2W échoué: code {ret}");
        tracing::warn!("Réseau: {msg}");
        Err(msg)
    }
}

#[cfg(not(windows))]
pub fn connect_authenticated(_remote_path: &str, _username: &str, _password: &str) -> Result<(), String> {
    Err("Connexion authentifiée non supportée sur cette plateforme".to_string())
}

/// #5: mDNS/DNS-SD discovery.
/// Browses the local network for SMB/FTP/HTTP/printer services via mDNS.
/// Timeout: 3 seconds of browsing then returns whatever was found.
pub fn discover_mdns_services() -> Vec<NetworkResource> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    use std::time::Duration;

    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Réseau: impossible de démarrer mDNS daemon: {e}");
            return Vec::new();
        }
    };

    let service_types = [
        "_smb._tcp.local.",
        "_ftp._tcp.local.",
        "_http._tcp.local.",
        "_ipp._tcp.local.",
    ];

    let mut receivers = Vec::new();
    for stype in &service_types {
        match mdns.browse(stype) {
            Ok(receiver) => receivers.push((stype.to_string(), receiver)),
            Err(e) => tracing::debug!("Réseau: mDNS browse {stype} échoué: {e}"),
        }
    }

    let mut results = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline {
        let mut got_event = false;
        for (stype, receiver) in &receivers {
            while let Ok(event) = receiver.try_recv() {
                got_event = true;
                if let ServiceEvent::ServiceResolved(info) = event {
                    let name = info.get_fullname().to_string();
                    let host = info.get_hostname().trim_end_matches('.').to_string();
                    let port = info.get_port();
                    let protocol = if stype.contains("smb") {
                        "smb"
                    } else if stype.contains("ftp") {
                        "ftp"
                    } else if stype.contains("ipp") {
                        "ipp"
                    } else {
                        "http"
                    };
                    let path = format!("{protocol}://{host}:{port}");
                    let display = format!("{} ({})", name.split('.').next().unwrap_or(&name), protocol);
                    results.push(NetworkResource {
                        name: display,
                        path,
                        status: NetworkStatus::Online,
                    });
                }
            }
        }
        if !got_event {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // Shutdown daemon
    let _ = mdns.shutdown();

    tracing::info!("Réseau: mDNS découverte {} services", results.len());
    results
}

/// #4: FTP directory listing.
/// Connects to an FTP server and lists the contents of the given path.
pub fn list_ftp_directory(host: &str, port: u16, path: &str, username: Option<&str>, password: Option<&str>) -> Result<Vec<FtpEntry>, String> {
    use suppaftp::FtpStream;

    let addr = format!("{host}:{port}");
    let mut ftp = FtpStream::connect(&addr)
        .map_err(|e| format!("FTP connexion à {addr} échouée: {e}"))?;

    let user = username.unwrap_or("anonymous");
    let pass = password.unwrap_or("xion@explorer");
    ftp.login(user, pass)
        .map_err(|e| format!("FTP login échoué: {e}"))?;

    ftp.cwd(path)
        .map_err(|e| format!("FTP cwd '{path}' échoué: {e}"))?;

    let listing = ftp.list(None)
        .map_err(|e| format!("FTP list échoué: {e}"))?;

    let _ = ftp.quit();

    let entries: Vec<FtpEntry> = listing
        .iter()
        .filter_map(|line| parse_ftp_list_line(line))
        .collect();

    tracing::info!("FTP: listé {} entrées dans {path}", entries.len());
    Ok(entries)
}

/// A parsed FTP directory entry.
#[derive(Debug, Clone)]
pub struct FtpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Parse a single line from FTP LIST output (Unix-style).
/// Example: "drwxr-xr-x  2 user group  4096 Jan 15 10:30 dirname"
fn parse_ftp_list_line(line: &str) -> Option<FtpEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let is_dir = line.starts_with('d');
    // Unix LIST format: split by whitespace, name is the last field
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        // Try simpler format: just name
        if parts.len() == 1 {
            return Some(FtpEntry {
                name: parts[0].to_string(),
                is_dir: false,
                size: 0,
            });
        }
        return None;
    }

    let size: u64 = parts[4].parse().unwrap_or(0);
    // Name may contain spaces — join everything from index 8 onwards
    let name = parts[8..].join(" ");

    // Skip . and ..
    if name == "." || name == ".." {
        return None;
    }

    Some(FtpEntry { name, is_dir, size })
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
            NetworkResource { name: "Test".to_string(), path: "smb://test".to_string(), status: NetworkStatus::Online },
        ]);
        let options = ListOptions::default();
        let page = svc.list_page(options, PageRequest { offset: 0, limit: 100 });
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
}
