//! Percent-encoding, and the `file://` URIs desktop launchers hand over.
//!
//! Two callers needed the same decoder: gvfs mount directory names, which are
//! percent-encoded, and the argument a `.desktop` entry passes through `%U`.
//! Rather than let a second copy appear, it lives here.

use std::path::PathBuf;

/// Decodes `%XX` escapes.
///
/// Deliberately not a dependency: this is the whole of what is needed. An
/// invalid or truncated escape is left as written rather than dropped, so text
/// that does not follow the convention still comes back readable instead of
/// silently losing a character.
pub fn percent_decode(input: &str) -> String {
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

/// Turns a launcher's argument into a path.
///
/// A `.desktop` entry's `%U` hands over a URI, not a path: activating Xion from
/// a file manager, a dock or `xdg-open` produces
/// `file:///home/alice/Mes%20documents`. Treating that as a path looks for a
/// directory literally named `file:` and finds nothing, so the launcher would
/// appear to do nothing at all.
///
/// Anything that is not a `file://` URI is returned unchanged, because that is
/// what a path typed in a terminal looks like.
pub fn path_from_argument(argument: &str) -> PathBuf {
    let Some(rest) = argument.strip_prefix("file://") else {
        return PathBuf::from(argument);
    };

    // `file://host/path` is legal; an empty or `localhost` authority means this
    // machine, which is the only case a file manager can act on.
    let path = match rest.find('/') {
        Some(slash) => &rest[slash..],
        // `file://` with nothing after it names no file.
        None => return PathBuf::from(argument),
    };

    let decoded = percent_decode(path);

    // RFC 8089 : `file:///C:/…` désigne un chemin à lettre de lecteur, et la
    // barre oblique qui précède fait partie de la syntaxe de l'URI, pas du
    // chemin. Laissée en place, elle produisait `/C:/Users/alice`, que Windows
    // ne sait pas ouvrir — et le projet vise Windows autant que Linux.
    //
    // La règle ne s'applique qu'à une lettre unique suivie de deux-points, donc
    // un dossier POSIX nommé `ab:` garde bien sa barre.
    if is_drive_rooted(&decoded) {
        return PathBuf::from(&decoded[1..]);
    }

    PathBuf::from(decoded)
}

/// `/C:/…` ou `/C:` — la forme que RFC 8089 réserve aux lettres de lecteur.
fn is_drive_rooted(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b':'
        && (bytes.len() == 3 || bytes[3] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_plain_path_passes_through() {
        assert_eq!(path_from_argument("/home/alice"), Path::new("/home/alice"));
        assert_eq!(path_from_argument("documents"), Path::new("documents"));
    }

    /// Le cas qui motivait tout : ce que `%U` transmet réellement.
    #[test]
    fn a_file_uri_becomes_a_path() {
        assert_eq!(
            path_from_argument("file:///home/alice/Documents"),
            Path::new("/home/alice/Documents")
        );
    }

    #[test]
    fn spaces_and_accents_survive_the_trip() {
        assert_eq!(
            path_from_argument("file:///home/alice/Mes%20documents"),
            Path::new("/home/alice/Mes documents")
        );
        assert_eq!(
            path_from_argument("file:///home/alice/Vid%C3%A9os"),
            Path::new("/home/alice/Vidéos")
        );
    }

    /// RFC 8089 : `file:///C:/…` désigne un chemin à lettre de lecteur. Rendu
    /// tel quel, il produisait `/C:/Users/alice`, que Windows ne sait pas
    /// ouvrir — et le projet vise Windows.
    #[test]
    fn a_windows_drive_uri_loses_its_leading_slash() {
        assert_eq!(
            path_from_argument("file:///C:/Users/alice"),
            Path::new("C:/Users/alice")
        );
        assert_eq!(path_from_argument("file:///D:/"), Path::new("D:/"));
    }

    /// Un chemin POSIX ordinaire ne doit pas être amputé par cette règle.
    #[test]
    fn a_posix_path_keeps_its_leading_slash() {
        assert_eq!(path_from_argument("file:///home/a"), Path::new("/home/a"));
        assert_eq!(path_from_argument("file:///ab:/x"), Path::new("/ab:/x"));
    }

    #[test]
    fn a_localhost_authority_is_this_machine() {
        assert_eq!(
            path_from_argument("file://localhost/srv/data"),
            Path::new("/srv/data")
        );
    }

    /// Un chemin qui contient littéralement « file:// » ne doit pas être
    /// charcuté, et une URI vide ne doit pas produire un chemin vide.
    #[test]
    fn degenerate_inputs_are_left_alone() {
        assert_eq!(path_from_argument("file://"), Path::new("file://"));
        assert_eq!(path_from_argument(""), Path::new(""));
    }

    #[test]
    fn percent_decoding_handles_the_ordinary_cases() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%2F%2F"), "//");
        assert_eq!(percent_decode("rien"), "rien");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
    }

    #[test]
    fn a_broken_escape_is_left_alone() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("fin%2"), "fin%2");
    }
}
