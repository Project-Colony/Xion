//! NTFS permissions helpers: parsing `icacls` output into [`AclEntry`] values.

use crate::ui::AclEntry;

/// Parse the text output of `icacls <path>` into a list of ACL entries.
///
/// Lines look like:
/// ```text
///   BUILTIN\Administrators:(I)(F)
///   NT AUTHORITY\SYSTEM:(I)(OI)(CI)(F)
/// ```
pub(super) fn parse_icacls_output(text: &str) -> Vec<AclEntry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(colon_pos) = line.rfind(":(") {
            let principal = line.get(..colon_pos).unwrap_or("").trim().to_string();
            if principal.is_empty() || principal.starts_with("Successfully") {
                continue;
            }
            let perms_str = line.get(colon_pos + 1..).unwrap_or("");
            let allow = !perms_str.contains("(DENY)");
            let mut permissions = Vec::new();
            if perms_str.contains("(F)")  { permissions.push("Contrôle total".to_string()); }
            if perms_str.contains("(M)")  { permissions.push("Modification".to_string()); }
            if perms_str.contains("(RX)") { permissions.push("Lecture & Exécution".to_string()); }
            if perms_str.contains("(R)") && !perms_str.contains("(RX)") {
                permissions.push("Lecture".to_string());
            }
            if perms_str.contains("(W)")  { permissions.push("Écriture".to_string()); }
            if perms_str.contains("(D)")  { permissions.push("Suppression".to_string()); }
            if !principal.is_empty() && !permissions.is_empty() {
                entries.push(AclEntry { principal, allow, permissions });
            }
        }
    }
    entries
}
