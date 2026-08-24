//! Measures the cost of listing and paging a large directory.
//!
//! The roadmap tracks "benchmarks ciblés" as missing; this is the first one. It
//! answers one question: what does it cost to scroll a big folder from top to
//! bottom?
//!
//! Run with:
//!     cargo run --release --example bench_listing -- 50000

use std::time::Instant;

use xion::filesystem::{FileSystem, ListOptions, LocalFileSystem, PageRequest};
use xion::services::DirectoryLoader;

fn main() -> std::io::Result<()> {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(50_000);
    let page_size: usize = 120;

    let dir = tempfile::tempdir()?;
    print!("Création de {count} fichiers… ");
    let start = Instant::now();
    for index in 0..count {
        std::fs::write(dir.path().join(format!("fichier-{index:06}.txt")), b"x")?;
    }
    println!("{:?}", start.elapsed());

    let filesystem = LocalFileSystem::default();
    let options = ListOptions::default();

    // One full read_dir + sort — the unit of work every page used to repeat.
    let start = Instant::now();
    let entries = filesystem.list_dir(dir.path(), options.clone()).unwrap();
    let single_listing = start.elapsed();
    println!(
        "\nListage complet (read_dir + tri) : {single_listing:?} pour {} entrées",
        entries.len()
    );

    let pages = count.div_ceil(page_size);

    // What the code does now: one listing, then every page is a slice of cache.
    let mut loader = DirectoryLoader::new(256, std::time::Duration::from_secs(45), page_size);
    let start = Instant::now();
    for page_index in 0..pages {
        let request = PageRequest::new(page_index * page_size, page_size);
        loader
            .load_page(&filesystem, dir.path(), options.clone(), request)
            .unwrap();
    }
    let paging_now = start.elapsed();

    println!("\n{pages} pages parcourues :");
    println!("  maintenant           : {paging_now:?}");
    println!(
        "  avant (1 listage complet par page, mesuré) : {:?}",
        single_listing * pages as u32
    );
    println!(
        "  facteur              : x{:.0}",
        (single_listing.as_secs_f64() * pages as f64) / paging_now.as_secs_f64().max(1e-9)
    );

    println!(
        "\nTaille de FsEntry : {} octets ; {count} entrées ≈ {:.1} Mo pour le seul vecteur",
        std::mem::size_of::<xion::FsEntry>(),
        (std::mem::size_of::<xion::FsEntry>() * count) as f64 / 1_048_576.0
    );
    Ok(())
}
