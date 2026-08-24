//! Listing a remote directory over FTP.

/// #4: FTP directory listing over FTPS.
///
/// The session is upgraded with AUTH TLS *before* the credentials are sent and
/// there is no plaintext fallback: the previous version logged in over a clear
/// socket, with `xion@explorer` as a built-in default password.
pub fn list_ftp_directory(
    host: &str,
    port: u16,
    path: &str,
    username: &str,
    password: &str,
) -> Result<Vec<FtpEntry>, String> {
    use suppaftp::native_tls::TlsConnector;
    use suppaftp::{NativeTlsConnector, NativeTlsFtpStream};

    if username.is_empty() {
        return Err("FTP: identifiant requis".to_string());
    }

    let addr = format!("{host}:{port}");
    let stream = NativeTlsFtpStream::connect(&addr)
        .map_err(|e| format!("FTP connexion à {addr} échouée: {e}"))?;

    let connector =
        TlsConnector::new().map_err(|e| format!("FTP: initialisation TLS échouée: {e}"))?;
    let mut ftp = stream
        .into_secure(NativeTlsConnector::from(connector), host)
        .map_err(|e| format!("FTP: passage en TLS échoué pour {host}: {e}"))?;

    ftp.login(username, password)
        .map_err(|e| format!("FTP login échoué: {e}"))?;

    ftp.cwd(path)
        .map_err(|e| format!("FTP cwd '{path}' échoué: {e}"))?;

    let listing = ftp
        .list(None)
        .map_err(|e| format!("FTP list échoué: {e}"))?;

    if let Err(e) = ftp.quit() {
        tracing::debug!("FTP: fermeture de session imparfaite: {e}");
    }

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
pub(super) fn parse_ftp_list_line(line: &str) -> Option<FtpEntry> {
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
