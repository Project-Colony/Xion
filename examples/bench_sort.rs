//! Mesure le coût d'un listage trié par taille : le chemin qui clonait
//! chaque `PathBuf` du dossier avant d'appeler `metadata_batch`.
use std::time::Instant;
use xion::filesystem::{FileSystem, ListOptions, LocalFileSystem, SortKey};

fn main() -> std::io::Result<()> {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(50_000);
    let dir = tempfile::tempdir()?;
    for index in 0..count {
        std::fs::write(dir.path().join(format!("fichier-{index:06}.txt")), b"x")?;
    }
    let filesystem = LocalFileSystem::default();
    let options = ListOptions {
        sort_by: SortKey::Size,
        ..ListOptions::default()
    };

    // chauffe le cache du système de fichiers
    let _ = filesystem.list_dir(dir.path(), options.clone()).unwrap();

    let mut best = std::time::Duration::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let entries = filesystem.list_dir(dir.path(), options.clone()).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(entries.len(), count);
        best = best.min(elapsed);
    }
    println!("tri par taille sur {count} entrées : {best:?} (meilleur sur 5)");
    Ok(())
}
