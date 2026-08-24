//! Attributes Xion's heap to the structures that hold it.
//!
//! No profiler is available on this machine, so the measurement is built in: a
//! global allocator that tracks live bytes, sampled between steps. Every number
//! printed is counted, not estimated.
//!
//! Run with:
//!     cargo run --release --example bench_memory -- 20000

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use xion::filesystem::{FileSystem, ListOptions, LocalFileSystem};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Tracking;

// SAFETY: every method forwards to `System` unchanged; the counters are the only
// addition and they never touch the returned pointers.
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let out = unsafe { System.realloc(ptr, layout, new_size) };
        if !out.is_null() {
            if new_size >= layout.size() {
                let live = LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed) + new_size
                    - layout.size();
                PEAK.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        out
    }
}

#[global_allocator]
static ALLOCATOR: Tracking = Tracking;

fn live_mb() -> f64 {
    LIVE.load(Ordering::Relaxed) as f64 / 1_048_576.0
}

fn step(label: &str, before: f64) -> f64 {
    let now = live_mb();
    println!(
        "  {label:<46} {:+8.2} Mo   (total {now:6.2} Mo)",
        now - before
    );
    now
}

fn main() -> std::io::Result<()> {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(20_000);

    let dir = tempfile::tempdir()?;
    for index in 0..count {
        std::fs::write(
            dir.path()
                .join(format!("un-nom-de-fichier-assez-realiste-{index:06}.txt")),
            b"x",
        )?;
    }

    println!(
        "Dossier de {count} entrées, chemin parent de {} caractères\n",
        dir.path().as_os_str().len()
    );
    println!("Tailles de structures :");
    println!(
        "  size_of::<FsEntry>()                           {:>4} octets",
        std::mem::size_of::<xion::FsEntry>()
    );
    println!(
        "  size_of::<Option<FsEntry>>()                   {:>4} octets",
        std::mem::size_of::<Option<xion::FsEntry>>()
    );
    println!(
        "  size_of::<FsMetadata>()                        {:>4} octets",
        std::mem::size_of::<xion::filesystem::FsMetadata>()
    );

    let filesystem = LocalFileSystem::default();
    println!("\nCoût mesuré, étape par étape :");
    let mut mark = live_mb();
    println!("  {:<46} {:>8}     (total {mark:6.2} Mo)", "départ", "");

    let entries = filesystem
        .list_dir(dir.path(), ListOptions::default())
        .unwrap();
    mark = step("un listage complet (Vec<FsEntry>)", mark);

    // What a second copy costs — the app holds several at once.
    let copy = entries.clone();
    mark = step("une deuxième copie du même listage", mark);
    drop(copy);
    mark = step("libération de la copie", mark);

    // The paged buffer the UI actually renders from.
    let paged: Vec<Option<xion::FsEntry>> = entries.iter().cloned().map(Some).collect();
    mark = step("le tampon paginé Vec<Option<FsEntry>>", mark);
    drop(paged);
    step("libération du tampon", mark);

    // How much of an entry is the parent path repeated on every line.
    let parent_bytes: usize = entries
        .iter()
        .map(|entry| entry.path.parent().map_or(0, |p| p.as_os_str().len()))
        .sum();
    let name_bytes: usize = entries.iter().map(|entry| entry.name.len()).sum();
    println!(
        "\n  Dont chemin parent répété sur chaque entrée : {:.2} Mo",
        parent_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Dont noms de fichiers (irréductible)        : {:.2} Mo",
        name_bytes as f64 / 1_048_576.0
    );
    println!(
        "  Un Arc<Path> partagé économiserait          : {:.2} Mo ({:.0} %)",
        (parent_bytes.saturating_sub(dir.path().as_os_str().len())) as f64 / 1_048_576.0,
        100.0 * parent_bytes as f64 / (parent_bytes + name_bytes) as f64
    );

    // The search index: one entry per file, plus whatever it stores alongside.
    {
        use xion::services::{SearchIndexOptions, SearchService};
        let before = live_mb();
        let index = SearchService
            .build_index_with_options(
                &filesystem,
                dir.path(),
                SearchIndexOptions {
                    include_hidden: false,
                    recursive: false,
                    max_entries: 0,
                },
                ListOptions::default(),
            )
            .unwrap();
        let after = live_mb();
        println!(
            "\n  Index de recherche ({count} entrées)        : {:.2} Mo, soit {:.0} octets/entrée",
            after - before,
            (after - before) * 1_048_576.0 / count as f64
        );
        drop(index);
    }

    drop(entries);
    let end = live_mb();
    println!("\n  après tout libérer : {end:.2} Mo");
    println!(
        "  pic observé        : {:.2} Mo",
        PEAK.load(Ordering::Relaxed) as f64 / 1_048_576.0
    );
    Ok(())
}
