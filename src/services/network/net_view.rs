//! Fallback SMB discovery by parsing `net view` output.
//!
//! Used when the WNet API is unavailable; the parsers are the tested part.

use super::*;

/// Enumerates SMB hosts and shares with `net view`.
#[cfg(windows)]
pub(super) fn discover_via_net_view() -> Result<Vec<NetworkResource>, String> {
    let mut results = Vec::new();

    // Step 1: enumerate network computers via `net view`
    let computers = discover_computers_net_view()?;

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
                    name: format!("{}\\{}", computer.trim_start_matches('\\'), share),
                    path: format!("smb:{}\\{}", computer, share),
                    status: NetworkStatus::Online,
                });
            }
        }
    }

    Ok(results)
}

/// `net view` is a Windows command. Elsewhere `net` is Samba's binary, which has
/// no `view` subcommand: running it produced a usage page the parser silently
/// dropped, so an unsupported platform looked exactly like a network with no
/// shares. Fail loudly instead.
#[cfg(not(windows))]
pub(super) fn discover_via_net_view() -> Result<Vec<NetworkResource>, String> {
    Err(
        "Découverte SMB indisponible sur cette plateforme (net view est spécifique à Windows)"
            .to_string(),
    )
}

/// Parse `net view` output to get a list of `\\COMPUTERNAME` entries.
#[cfg(windows)]
pub(super) fn discover_computers_net_view() -> Result<Vec<String>, String> {
    let output = net_command()
        .args(["view", "/all"])
        .output()
        .map_err(|e| format!("commande 'net view /all' échouée: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            "Réseau: 'net view /all' code de sortie {:?}, stderr: {}",
            output.status.code(),
            stderr.trim()
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_net_view_computers(&text))
}

/// `net.exe` by absolute path and without a console flash.
///
/// A bare `Command::new("net")` lets Windows search the directory of the running
/// executable before System32.
#[cfg(windows)]
pub(super) fn net_command() -> std::process::Command {
    use std::os::windows::process::CommandExt;

    let mut command = std::process::Command::new(crate::platform::system_binary("net.exe"));
    command.creation_flags(crate::platform::CREATE_NO_WINDOW);
    command
}

// The `net view` parsers stay platform-independent so they remain unit-testable
// everywhere, but only Windows ever feeds them.
#[cfg(any(windows, test))]
pub(super) fn parse_net_view_computers(text: &str) -> Vec<String> {
    let mut computers = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\\\\") {
            // Line like: \\SERVERNAME    Remark text
            let name = trimmed.split_whitespace().next().unwrap_or("").to_string();
            if !name.is_empty() && is_valid_computer_name(&name) {
                computers.push(name);
            }
        }
    }
    computers
}

/// Returns `true` if `name` looks like a valid `\\COMPUTERNAME`.
#[cfg(any(windows, test))]
pub(super) fn is_valid_computer_name(name: &str) -> bool {
    name.starts_with("\\\\")
        && name.len() <= 17 // \\COMPUTERNAME (max 15 chars + \\)
        && name[2..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Parse `net view \\COMPUTERNAME` output to get share names.
#[cfg(windows)]
pub(super) fn list_shares_net_view(computer: &str) -> Vec<String> {
    if !is_valid_computer_name(computer) {
        return vec![];
    }

    let output = net_command().args(["view", computer]).output();

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

/// Splits a `net view` row on runs of two or more spaces.
///
/// Share names may contain single spaces, so whitespace splitting is wrong; two
/// spaces is what `net view` uses between columns in every locale.
#[cfg(any(windows, test))]
pub(super) fn split_net_view_columns(line: &str) -> Vec<&str> {
    line.split("  ")
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect()
}

/// Best-effort rejection of non-disk shares.
///
/// The previous code did the opposite — it *required* the literal "Disk" or
/// "Disque" — so a German, Spanish or Japanese Windows yielded no share at all.
/// Recognising the few printer/IPC labels we know and keeping everything else
/// degrades the right way round: an unknown locale shows too much, not nothing.
#[cfg(any(windows, test))]
pub(super) fn is_non_disk_share_type(kind: &str) -> bool {
    let lowered = kind.to_lowercase();
    ["print", "impr", "ipc", "druck", "stamp"]
        .iter()
        .any(|marker| lowered.contains(marker))
}

#[cfg(any(windows, test))]
pub(super) fn parse_net_view_shares(text: &str) -> Vec<String> {
    let mut shares = Vec::new();
    let mut in_shares = false;
    for line in text.lines() {
        let trimmed = line.trim();
        // Header separator signals start of share list; a row of dashes is
        // formatting, so it is the one locale-independent marker available.
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
        let fields = split_net_view_columns(trimmed);
        // A single-field line is the trailing "command completed" message, not a
        // share; requiring the type column keeps it out whatever the language.
        if fields.len() < 2 {
            continue;
        }
        let share_name = fields[0];
        if share_name.is_empty() || share_name.ends_with('$') {
            continue;
        }
        if is_non_disk_share_type(fields[1]) {
            continue;
        }
        shares.push(share_name.to_string());
    }
    shares
}
