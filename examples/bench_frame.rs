//! Measures the two per-frame costs the audit flagged: the name filter and the
//! status-bar selection scan.
//!
//! Every candidate is implemented here so the comparison is a measurement, not
//! an estimate. It is what showed that the obvious "allocation-free" filter is
//! in fact three times SLOWER than the allocating one it replaced: folding
//! `char::to_lowercase` per character is O(n*m), while `to_lowercase()` pays one
//! allocation and then uses an optimised substring search. The shipped version
//! is the third variant — an ASCII byte scan with a Unicode fallback.
//!
//! Run with:
//!     cargo run --release --example bench_frame -- 50000

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

fn names(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            if index % 7 == 0 {
                format!("Rapport Facture {index:06}.pdf")
            } else {
                format!("fichier-{index:06}.txt")
            }
        })
        .collect()
}

/// What the code did before: one heap allocation per entry, per pass.
fn filter_old(names: &[String], needle: &str) -> usize {
    names
        .iter()
        .filter(|name| name.to_lowercase().contains(needle))
        .count()
}

/// Candidate B: fold on the fly. Allocation-free, but measurably slower.
fn filter_new(names: &[String], needle: &str) -> usize {
    names
        .iter()
        .filter(|name| contains_lowercased(name, needle))
        .count()
}

fn contains_lowercased(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .char_indices()
        .any(|(offset, _)| starts_with_lowercased(&haystack[offset..], needle))
}

fn starts_with_lowercased(haystack: &str, needle: &str) -> bool {
    let mut folded = haystack.chars().flat_map(char::to_lowercase);
    let mut wanted = needle.chars();
    loop {
        match wanted.next() {
            None => return true,
            Some(expected) => match folded.next() {
                None => return false,
                Some(actual) if actual != expected => return false,
                Some(_) => {}
            },
        }
    }
}

/// Candidate C, the one that shipped: an ASCII byte scan for the overwhelming
/// majority of file names, falling back to the Unicode walk only when needed.
fn filter_ascii(names: &[String], needle: &str) -> usize {
    names
        .iter()
        .filter(|name| contains_fast(name, needle))
        .count()
}

fn contains_fast(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        let hay = haystack.as_bytes();
        let need = needle.as_bytes();
        if need.len() > hay.len() {
            return false;
        }
        return hay
            .windows(need.len())
            .any(|window| window.eq_ignore_ascii_case(need));
    }
    contains_lowercased(haystack, needle)
}

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(50_000);
    let names = names(count);

    println!("Filtre par nom sur {count} entrées, requête « facture » :");
    let start = Instant::now();
    let hits_old = filter_old(&names, "facture");
    let old = start.elapsed();
    let start = Instant::now();
    let hits_new = filter_new(&names, "facture");
    let new = start.elapsed();
    assert_eq!(
        hits_old, hits_new,
        "les deux filtres doivent trouver la même chose"
    );
    println!("  A. to_lowercase + contains (l'ancien) : {old:?}");
    println!("  B. fold Unicode intégral (ÉCARTÉ)      : {new:?}   <- plus lent que A");
    println!("     x{:.1} vs A", old.as_secs_f64() / new.as_secs_f64());
    let start = Instant::now();
    let hits_ascii = filter_ascii(&names, "facture");
    let ascii = start.elapsed();
    assert_eq!(hits_ascii, hits_new);
    println!("  C. ASCII + repli Unicode (RETENU)      : {ascii:?}");
    println!(
        "     x{:.1} vs A, et sans allocation",
        old.as_secs_f64() / ascii.as_secs_f64()
    );
    println!("  ({hits_new} correspondances)");

    // ── Status bar: size of the selection ────────────────────────────────────
    let paths: Vec<PathBuf> = names.iter().map(PathBuf::from).collect();
    let selected: HashSet<PathBuf> = paths.iter().take(count / 10).cloned().collect();

    println!(
        "\nTaille de la sélection ({} éléments sur {count}) :",
        selected.len()
    );
    let start = Instant::now();
    // Before: for each selected path, scan every entry.
    let total_old: usize = selected
        .iter()
        .filter_map(|path| paths.iter().find(|entry| *entry == path))
        .count();
    let old = start.elapsed();

    let start = Instant::now();
    // Now: one pass over the entries, membership tested in the set.
    let total_new: usize = paths
        .iter()
        .filter(|entry| selected.contains(*entry))
        .count();
    let new = start.elapsed();
    assert_eq!(total_old, total_new);
    println!("  avant (O(sélection x entrées)) : {old:?}");
    println!("  maintenant (une passe)         : {new:?}");
    println!(
        "  facteur                        : x{:.0}",
        old.as_secs_f64() / new.as_secs_f64()
    );
}
