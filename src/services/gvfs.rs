//! Remote and virtual locations mounted by gvfs.
//!
//! gvfs is the GNOME virtual filesystem layer: it is what gives a desktop SMB
//! shares, SFTP, MTP phones, cameras, WebDAV and Google Drive. Talking to it
//! properly would mean binding GIO — glib, gobject, a large C dependency chain,
//! Linux-only, and the opposite direction from the C dependency this crate just
//! removed to cross-compile for Windows.
//!
//! It is not necessary. `gvfsd-fuse` already exposes every active mount as an
//! ordinary directory under `$XDG_RUNTIME_DIR/gvfs`, so [`LocalFileSystem`]
//! reads them unchanged. All this module does is find them and give them a name
//! a person would recognise.
//!
//! [`LocalFileSystem`]: crate::filesystem::LocalFileSystem

use std::path::{Path, PathBuf};

/// One mounted gvfs location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GvfsMount {
    /// Where it lives on disk, and what to hand the filesystem layer.
    pub path: PathBuf,
    /// What to show the user: `"media sur nas"`, `"bob@example.com"`.
    pub label: String,
    /// The gvfs backend: `smb-share`, `sftp`, `mtp`, `google-drive`…
    pub scheme: String,
}

/// The directory `gvfsd-fuse` mounts itself on.
///
/// `XDG_RUNTIME_DIR` is the documented location; the `/run/user/<uid>` fallback
/// covers sessions that do not export it.
pub fn mount_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let candidate = PathBuf::from(dir).join("gvfs");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    #[cfg(unix)]
    {
        // The owner of `/proc/self` is this process's real uid. Reaching for
        // `libc::getuid` would mean a new dependency, and an `unsafe` block, for
        // a number `std` already knows.
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata("/proc/self") {
            let candidate = PathBuf::from(format!("/run/user/{}/gvfs", metadata.uid()));
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Whether `path` is inside a gvfs mount.
///
/// Worth knowing before doing anything expensive: these are network filesystems
/// behind FUSE, where a `stat` can take a second and a disconnected server makes
/// it take thirty.
pub fn is_gvfs_path(path: &Path) -> bool {
    match mount_root() {
        Some(root) => path.starts_with(root),
        None => false,
    }
}

/// Every currently mounted gvfs location.
///
/// Returns an empty vector when gvfs is not running, which is the normal state
/// on a machine without a GNOME session — this is an addition, not a
/// requirement.
pub fn mounts() -> Vec<GvfsMount> {
    let Some(root) = mount_root() else {
        return Vec::new();
    };

    // `read_dir` on the gvfs root only touches the FUSE daemon's own table, not
    // the remote servers, so it stays cheap even with an unreachable share
    // mounted. Descending into an entry would not be.
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };

    let mut mounts: Vec<GvfsMount> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let spec = MountSpec::parse(name)?;
            Some(GvfsMount {
                path: entry.path(),
                label: spec.label(),
                scheme: spec.scheme,
            })
        })
        .collect();

    mounts.sort_by(|left, right| left.label.cmp(&right.label));
    mounts
}

/// A gvfs mount directory name, taken apart.
///
/// The convention is `<scheme>:<key>=<value>[,<key>=<value>]…`, with the values
/// percent-encoded. `archive:host=file%253A%252F%252F%252Ftmp%252Fessai.zip` is
/// a real one: the value is encoded twice because it is itself a URI.
#[derive(Debug, PartialEq, Eq)]
struct MountSpec {
    scheme: String,
    params: Vec<(String, String)>,
}

impl MountSpec {
    fn parse(name: &str) -> Option<Self> {
        let (scheme, rest) = name.split_once(':')?;
        if scheme.is_empty() {
            return None;
        }

        let params = rest
            .split(',')
            .filter_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                Some((key.to_string(), percent_decode(value)))
            })
            .collect();

        Some(Self {
            scheme: scheme.to_string(),
            params,
        })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// A name a person would recognise.
    ///
    /// Falling back to the raw directory name would show the user a line of
    /// percent-encoding, so every branch ends somewhere readable.
    fn label(&self) -> String {
        let host = self.get("host").unwrap_or_default();
        let user = self.get("user").unwrap_or_default();

        match self.scheme.as_str() {
            // A Windows share: the share name is what people call it.
            "smb-share" => match (self.get("share"), self.get("server")) {
                (Some(share), Some(server)) => format!("{share} sur {server}"),
                (Some(share), None) => share.to_string(),
                _ => "Partage Windows".to_string(),
            },
            "smb-browse" => match self.get("server") {
                Some(server) => format!("Réseau {server}"),
                None => "Réseau Windows".to_string(),
            },
            "sftp" | "ssh" | "ftp" | "ftps" => {
                if user.is_empty() {
                    self.non_empty(host, "Serveur distant")
                } else {
                    format!("{user}@{host}")
                }
            }
            "dav" | "davs" | "http" | "https" => self.non_empty(host, "Emplacement web"),
            "google-drive" => {
                if user.is_empty() {
                    self.non_empty(host, "Google Drive")
                } else {
                    format!("{user} (Google Drive)")
                }
            }
            "mtp" | "gphoto2" => "Appareil connecté".to_string(),
            "afp-volume" => self.get("volume").unwrap_or("Volume AFP").to_string(),
            "cdda" => "Disque audio".to_string(),
            "trash" => "Corbeille".to_string(),
            "recent" => "Récents".to_string(),
            "computer" => "Ordinateur".to_string(),
            // An archive mounted as a folder: the file name is the only useful
            // part of a doubly-encoded `file://` URI.
            "archive" => Path::new(&percent_decode(host))
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| "Archive".to_string()),
            _ => self.non_empty(host, &self.scheme),
        }
    }

    fn non_empty(&self, value: &str, fallback: &str) -> String {
        if value.is_empty() {
            fallback.to_string()
        } else {
            value.to_string()
        }
    }
}

/// Decodes the `%XX` escapes gvfs puts in mount directory names.
///
/// Deliberately not a dependency: this is the whole of what is needed, and an
/// invalid escape is left as written rather than dropped, so a name that does
/// not follow the convention still comes back readable.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_of(name: &str) -> String {
        MountSpec::parse(name)
            .expect("nom de montage valide")
            .label()
    }

    #[test]
    fn a_windows_share_reads_like_one() {
        assert_eq!(
            label_of("smb-share:server=nas,share=media"),
            "media sur nas"
        );
    }

    #[test]
    fn an_sftp_mount_names_its_user_and_host() {
        assert_eq!(label_of("sftp:host=exemple.fr,user=bob"), "bob@exemple.fr");
    }

    #[test]
    fn an_sftp_mount_without_a_user_still_has_a_name() {
        assert_eq!(label_of("sftp:host=exemple.fr"), "exemple.fr");
    }

    /// Le nom relevé sur un vrai montage : la valeur y est encodée deux fois,
    /// parce qu'elle est elle-même une URI.
    #[test]
    fn an_archive_mount_shows_the_file_name() {
        let name = "archive:host=file%253A%252F%252F%252Ftmp%252Fdossier%252Fessai.zip";
        assert_eq!(label_of(name), "essai.zip");
    }

    #[test]
    fn a_google_drive_mount_says_whose_it_is() {
        assert_eq!(
            label_of("google-drive:host=gmail.com,user=alice"),
            "alice (Google Drive)"
        );
    }

    #[test]
    fn a_phone_is_a_phone() {
        assert_eq!(
            label_of("mtp:host=%5Busb%3A001%2C005%5D"),
            "Appareil connecté"
        );
    }

    /// Un schéma inconnu ne doit pas afficher une ligne de pourcents.
    #[test]
    fn an_unknown_scheme_falls_back_to_its_host() {
        assert_eq!(
            label_of("quelquechose:host=machine.locale"),
            "machine.locale"
        );
    }

    #[test]
    fn an_unknown_scheme_without_a_host_falls_back_to_the_scheme() {
        assert_eq!(label_of("quelquechose:autre=valeur"), "quelquechose");
    }

    #[test]
    fn a_name_that_is_not_a_mount_spec_is_refused() {
        assert!(MountSpec::parse("pas-un-montage").is_none());
        assert!(MountSpec::parse(":sans-schema").is_none());
    }

    #[test]
    fn percent_decoding_handles_the_ordinary_cases() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%2F%2F"), "//");
        assert_eq!(percent_decode("rien"), "rien");
    }

    /// Une séquence tronquée ou invalide est laissée telle quelle : mieux vaut
    /// un nom légèrement bizarre qu'un caractère avalé en silence.
    #[test]
    fn a_broken_escape_is_left_alone() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("fin%2"), "fin%2");
    }

    #[test]
    fn utf8_survives_decoding() {
        assert_eq!(percent_decode("caf%C3%A9"), "café");
    }

    #[test]
    fn enumerating_mounts_never_panics_without_gvfs() {
        // Sur une machine sans gvfs la liste est vide, ce n'est pas une erreur.
        let _ = mounts();
    }
}
