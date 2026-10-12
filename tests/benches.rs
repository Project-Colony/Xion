//! Measurements, kept as ignored tests so that CI keeps compiling them.
//!
//! None of them runs with a plain `cargo test`. Run them in release, one at a
//! time so that no other test shares the timings or the allocation counter:
//!
//! ```text
//! cargo test --release --test benches -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! `XION_BENCH_ENTRIES` overrides the number of directory entries each one
//! uses. Every number printed is measured, not estimated.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use xion::FsEntry;
use xion::filesystem::{
    FileSystem, FsMetadata, ListOptions, LocalFileSystem, PageRequest, SortKey,
};
use xion::services::{DirectoryLoader, SearchIndexOptions, SearchService};

/// The number of entries to use: `XION_BENCH_ENTRIES`, or `default`.
fn entry_count(default: usize) -> usize {
    std::env::var("XION_BENCH_ENTRIES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Fills `dir` with `count` one-byte files named `<stem>-<index>.txt`.
fn fill(dir: &Path, count: usize, stem: &str) -> io::Result<()> {
    for index in 0..count {
        std::fs::write(dir.join(format!("{stem}-{index:06}.txt")), b"x")?;
    }
    Ok(())
}

// ── Listing and paging ──────────────────────────────────────────────────────

/// What it costs to scroll a large folder from top to bottom.
#[test]
#[ignore = "benchmark: run with --ignored in release"]
fn listing_and_paging_a_large_directory() -> io::Result<()> {
    let count = entry_count(50_000);
    let page_size = 120;
    let dir = tempfile::tempdir()?;
    let start = Instant::now();
    fill(dir.path(), count, "file")?;
    println!("Created {count} files in {:?}", start.elapsed());

    let filesystem = LocalFileSystem::default();
    let options = ListOptions::default();

    // One full read_dir plus sort: the unit of work every page used to repeat.
    let start = Instant::now();
    let entries = filesystem.list_dir(dir.path(), options.clone()).unwrap();
    let single_listing = start.elapsed();
    println!(
        "Full listing (read_dir + sort): {single_listing:?} for {} entries",
        entries.len()
    );

    // What the loader does: one listing, then every page is a slice of cache.
    let pages = count.div_ceil(page_size);
    let mut loader = DirectoryLoader::new(256, Duration::from_secs(45), page_size);
    let start = Instant::now();
    for page_index in 0..pages {
        let request = PageRequest::new(page_index * page_size, page_size);
        loader
            .load_page(&filesystem, dir.path(), options.clone(), request)
            .unwrap();
    }
    let paging = start.elapsed();

    println!("{pages} pages walked:");
    println!("  through the loader cache          : {paging:?}");
    println!(
        "  one full listing per page (before): {:?}",
        single_listing * pages as u32
    );
    println!(
        "  ratio                             : x{:.0}",
        (single_listing.as_secs_f64() * pages as f64) / paging.as_secs_f64().max(1e-9)
    );
    println!(
        "size_of::<FsEntry>() is {} bytes; {count} entries take about {:.1} MB for the vector alone",
        size_of::<FsEntry>(),
        (size_of::<FsEntry>() * count) as f64 / 1_048_576.0
    );
    Ok(())
}

/// A listing sorted by size: the path that used to clone every `PathBuf` of the
/// folder before calling `metadata_batch`.
#[test]
#[ignore = "benchmark: run with --ignored in release"]
fn listing_sorted_by_size() -> io::Result<()> {
    let count = entry_count(50_000);
    let dir = tempfile::tempdir()?;
    fill(dir.path(), count, "file")?;
    let filesystem = LocalFileSystem::default();
    let options = ListOptions {
        sort_by: SortKey::Size,
        ..ListOptions::default()
    };

    // Warm the operating system's directory cache first.
    let _ = filesystem.list_dir(dir.path(), options.clone()).unwrap();

    let mut best = Duration::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let entries = filesystem.list_dir(dir.path(), options.clone()).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(entries.len(), count);
        best = best.min(elapsed);
    }
    println!("Sort by size over {count} entries: {best:?} (best of 5)");
    Ok(())
}

// ── Per-frame costs ─────────────────────────────────────────────────────────

/// The two per-frame costs an audit flagged: the name filter and the status
/// bar's selection scan.
///
/// Every candidate filter is implemented here so that the comparison is a
/// measurement. It is what showed that the obvious allocation-free filter (B)
/// is in fact slower than the allocating one it was meant to replace (A):
/// folding `char::to_lowercase` character by character is O(n*m), while
/// `to_lowercase()` pays one allocation and then runs an optimised substring
/// search. The shipped version, `contains_lowercased` in
/// `src/ui/app/navigation/mod.rs`, is C: an ASCII byte scan with a Unicode
/// fallback.
#[test]
#[ignore = "benchmark: run with --ignored in release"]
fn name_filter_and_selection_scan() {
    let count = entry_count(50_000);
    let names: Vec<String> = (0..count)
        .map(|index| {
            if index % 7 == 0 {
                format!("Report Invoice {index:06}.pdf")
            } else {
                format!("file-{index:06}.txt")
            }
        })
        .collect();
    let needle = "invoice";
    let filter = |matches: fn(&str, &str) -> bool| {
        let start = Instant::now();
        let hits = names.iter().filter(|name| matches(name, needle)).count();
        (hits, start.elapsed())
    };

    println!("Name filter over {count} entries, query {needle:?}:");
    let (hits_a, a) = filter(|name, needle| name.to_lowercase().contains(needle));
    let (hits_b, b) = filter(contains_folded);
    let (hits_c, c) = filter(contains_ascii_first);
    assert_eq!(hits_a, hits_b, "every filter must find the same entries");
    assert_eq!(hits_a, hits_c, "every filter must find the same entries");
    println!("  A. to_lowercase + contains (the old one) : {a:?}");
    println!("  B. full Unicode fold (rejected)          : {b:?}");
    println!("  C. ASCII scan, Unicode fallback (shipped): {c:?}");
    println!(
        "  C is x{:.1} against A, and allocates nothing ({hits_c} matches)",
        a.as_secs_f64() / c.as_secs_f64()
    );

    let paths: Vec<PathBuf> = names.iter().map(PathBuf::from).collect();
    let selected: HashSet<PathBuf> = paths.iter().take(count / 10).cloned().collect();
    println!("Selection size ({} of {count} entries):", selected.len());

    // Before: for every selected path, scan every entry.
    let start = Instant::now();
    let found_before = selected
        .iter()
        .filter_map(|path| paths.iter().find(|entry| *entry == path))
        .count();
    let before = start.elapsed();

    // Now: one pass over the entries, membership tested in the set.
    let start = Instant::now();
    let found_now = paths
        .iter()
        .filter(|entry| selected.contains(*entry))
        .count();
    let now = start.elapsed();

    assert_eq!(found_before, found_now);
    println!("  before (selection x entries) : {before:?}");
    println!("  now (one pass)               : {now:?}");
    println!(
        "  ratio                        : x{:.0}",
        before.as_secs_f64() / now.as_secs_f64()
    );
}

/// Candidate B: folds on the fly. Allocation-free, but measurably slower.
fn contains_folded(haystack: &str, needle: &str) -> bool {
    needle.is_empty()
        || haystack.char_indices().any(|(offset, _)| {
            let mut folded = haystack[offset..].chars().flat_map(char::to_lowercase);
            needle.chars().all(|wanted| folded.next() == Some(wanted))
        })
}

/// Candidate C, the one that shipped: an ASCII byte scan for the overwhelming
/// majority of file names, the Unicode walk only when needed.
fn contains_ascii_first(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        let (hay, need) = (haystack.as_bytes(), needle.as_bytes());
        return need.len() <= hay.len()
            && hay
                .windows(need.len())
                .any(|window| window.eq_ignore_ascii_case(need));
    }
    contains_folded(haystack, needle)
}

// ── Memory ──────────────────────────────────────────────────────────────────

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// Counts live heap bytes, so that the memory bench needs no profiler.
///
/// It is this test binary's allocator, so it also counts what any other test
/// running at the same moment allocates: hence `--test-threads=1`.
struct Tracking;

// SAFETY: every method forwards to `System` unchanged; the counters are the only
// addition and they never touch the returned pointers.
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
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
                grow(new_size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        out
    }
}

fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

#[global_allocator]
static ALLOCATOR: Tracking = Tracking;

fn live_mb() -> f64 {
    LIVE.load(Ordering::Relaxed) as f64 / 1_048_576.0
}

/// Prints how much the live heap moved since `before`, and returns the new mark.
fn step(label: &str, before: f64) -> f64 {
    let now = live_mb();
    println!(
        "  {label:<44} {:+8.2} MB   (total {now:6.2} MB)",
        now - before
    );
    now
}

/// Attributes Xion's heap to the structures that hold it.
#[test]
#[ignore = "benchmark: run with --ignored --test-threads=1 in release"]
fn heap_held_by_a_large_listing() -> io::Result<()> {
    let count = entry_count(20_000);
    let dir = tempfile::tempdir()?;
    fill(dir.path(), count, "a-reasonably-realistic-file-name")?;

    println!(
        "Folder of {count} entries, parent path of {} characters",
        dir.path().as_os_str().len()
    );
    println!("Structure sizes:");
    println!(
        "  size_of::<FsEntry>()          {:>4} bytes",
        size_of::<FsEntry>()
    );
    println!(
        "  size_of::<Option<FsEntry>>()  {:>4} bytes",
        size_of::<Option<FsEntry>>()
    );
    println!(
        "  size_of::<FsMetadata>()       {:>4} bytes",
        size_of::<FsMetadata>()
    );

    let filesystem = LocalFileSystem::default();
    println!("Measured cost, step by step:");
    let mut mark = live_mb();
    println!("  {:<44} {:>8}      (total {mark:6.2} MB)", "start", "");

    let entries = filesystem
        .list_dir(dir.path(), ListOptions::default())
        .unwrap();
    mark = step("one full listing (Vec<FsEntry>)", mark);

    // What a second copy costs: the app holds several at once.
    let copy = entries.clone();
    mark = step("a second copy of the same listing", mark);
    drop(copy);
    mark = step("copy freed", mark);

    // The paged buffer the UI renders from.
    let paged: Vec<Option<FsEntry>> = entries.iter().cloned().map(Some).collect();
    mark = step("the paged buffer Vec<Option<FsEntry>>", mark);
    drop(paged);
    step("paged buffer freed", mark);

    // How much of an entry is the parent path, repeated on every line.
    let parent_bytes: usize = entries
        .iter()
        .map(|entry| entry.path.parent().map_or(0, |p| p.as_os_str().len()))
        .sum();
    let name_bytes: usize = entries.iter().map(|entry| entry.name.len()).sum();
    println!(
        "  of which the parent path repeated per entry : {:.2} MB",
        parent_bytes as f64 / 1_048_576.0
    );
    println!(
        "  of which the file names (irreducible)       : {:.2} MB",
        name_bytes as f64 / 1_048_576.0
    );
    println!(
        "  a shared Arc<Path> would save               : {:.2} MB ({:.0} %)",
        parent_bytes.saturating_sub(dir.path().as_os_str().len()) as f64 / 1_048_576.0,
        100.0 * parent_bytes as f64 / (parent_bytes + name_bytes) as f64
    );

    // The search index: one entry per file, plus whatever it stores alongside.
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
        "  search index ({count} entries): {:.2} MB, {:.0} bytes per entry",
        after - before,
        (after - before) * 1_048_576.0 / count as f64
    );
    drop(index);

    drop(entries);
    println!("  after freeing everything : {:.2} MB", live_mb());
    println!(
        "  peak observed            : {:.2} MB",
        PEAK.load(Ordering::Relaxed) as f64 / 1_048_576.0
    );
    Ok(())
}
