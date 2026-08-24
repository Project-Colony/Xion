//! Service discovery over mDNS/DNS-SD, and the validation applied to the
//! untrusted names it returns.

use super::*;
use std::time::Instant;

/// Accepts only DNS label characters.
///
/// mDNS answers come from any machine on the LAN; a hostname was interpolated
/// straight into the displayed path, so it could carry control characters or
/// Unicode direction overrides and impersonate another entry.
pub(super) fn is_valid_mdns_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })
}

/// Zero-width and direction-control characters, which can reorder a label on
/// screen so it reads as a different host than the one stored.
pub(super) fn is_format_control(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}')
}

/// Keeps an announced service name printable and bounded.
pub(super) fn sanitize_mdns_label(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control() && !is_format_control(*c))
        .take(MAX_MDNS_LABEL)
        .collect::<String>()
        .trim()
        .to_string()
}

/// #5: mDNS/DNS-SD discovery.
/// Browses the local network for SMB/FTP/HTTP/printer services via mDNS.
/// Timeout: 3 seconds of browsing then returns whatever was found.
pub fn discover_mdns_services() -> Vec<NetworkResource> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    use std::collections::HashSet;
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

    let mut results: Vec<NetworkResource> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline && results.len() < MAX_MDNS_RESULTS {
        let mut got_event = false;
        for (stype, receiver) in &receivers {
            while let Ok(event) = receiver.try_recv() {
                got_event = true;
                if let ServiceEvent::ServiceResolved(info) = event {
                    let host = info.get_hostname().trim_end_matches('.').to_string();
                    if !is_valid_mdns_hostname(&host) {
                        tracing::debug!("Réseau: nom d'hôte mDNS rejeté");
                        continue;
                    }
                    let port = info.get_port();
                    if port == 0 {
                        continue;
                    }
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
                    if !seen.insert(path.clone()) {
                        continue;
                    }
                    let raw_name = info.get_fullname().to_string();
                    let label =
                        sanitize_mdns_label(raw_name.split('.').next().unwrap_or(&raw_name));
                    let label = if label.is_empty() { host } else { label };
                    // The provenance is part of the label: nothing here has been
                    // verified, anyone on the LAN can announce any name.
                    let display = format!("{label} ({protocol}, mDNS non vérifié)");
                    results.push(NetworkResource {
                        name: display,
                        path,
                        status: NetworkStatus::Online,
                    });
                    if results.len() >= MAX_MDNS_RESULTS {
                        break;
                    }
                }
            }
            if results.len() >= MAX_MDNS_RESULTS {
                break;
            }
        }
        if !got_event {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // Shutdown daemon
    if let Err(e) = mdns.shutdown() {
        tracing::debug!("Réseau: arrêt du daemon mDNS échoué: {e}");
    }

    tracing::info!("Réseau: mDNS découverte {} services", results.len());
    results
}
