//! Archive listing helpers (ZIP is handled by the zip crate elsewhere;
//! this module covers TAR.GZ and 7Z).

use crate::ui::ArchiveEntry;

/// List entries inside a `.tar.gz` / `.tgz` file.
pub(super) fn list_tar_gz(path: &std::path::Path) -> Option<Vec<ArchiveEntry>> {
    use std::io::BufReader;
    let file = std::fs::File::open(path).ok()?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut archive = tar::Archive::new(gz);
    let mut entries = Vec::new();
    for entry in archive.entries().ok()? {
        if let Ok(e) = entry {
            let inner_path = e.path().ok()?.to_string_lossy().to_string();
            let name = inner_path
                .split('/')
                .filter(|s| !s.is_empty())
                .last()
                .unwrap_or(&inner_path)
                .to_string();
            let is_dir = e.header().entry_type().is_dir();
            let size = e.header().size().unwrap_or(0);
            entries.push(ArchiveEntry {
                name,
                inner_path,
                is_dir,
                size,
                compressed_size: 0,
            });
        }
    }
    Some(entries)
}

/// List entries inside a `.7z` file.
pub(super) fn list_7z(path: &std::path::Path) -> Option<Vec<ArchiveEntry>> {
    let mut archive =
        sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty()).ok()?;
    let mut entries = Vec::new();
    archive
        .for_each_entries(|entry, _reader| {
            let inner_path = entry.name().to_string();
            let name = inner_path
                .split('/')
                .filter(|s| !s.is_empty())
                .last()
                .unwrap_or(&inner_path)
                .to_string();
            let is_dir = entry.is_directory();
            let size = entry.size();
            entries.push(ArchiveEntry {
                name,
                inner_path,
                is_dir,
                size,
                compressed_size: 0,
            });
            Ok(true)
        })
        .ok()?;
    Some(entries)
}
