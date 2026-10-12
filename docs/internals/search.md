# Search and indexing

Xion has two searches. Typing in the search field filters the folder on screen
through an in-memory index, `SearchService` in `src/services/search.rs`. The
full-text search, submitted separately, walks the tree on disk. This page is
mostly about the first.

## `SearchService`

A stateless struct, so `SearchService` and `SearchService::default()` are the
same thing. It has three public methods, in two groups.

**Without an index**, reading the folder on every call:

| Method | What it does |
|---|---|
| `search_in_dir` | filters one folder's entries by name |

**With an index**, built once and queried many times:

| Method | What it does |
|---|---|
| `build_index_with_options` | indexes a root, with the given `ListOptions` |
| `count_index_matches` | counts the matches without building a result list |

The filter itself is written once, in `SearchService::matching`, and every
query goes through it.

Recursion stops at `MAX_SEARCH_DEPTH` (32), so a very deep tree or a symbolic
link loop cannot overflow the stack.

## `SearchIndex`

The indexed entries, each stored with lowercased fields for fast comparison.

- `root: PathBuf`, the indexed folder (a public field);
- `len()` and `is_empty()`;
- `entries()`, an iterator over the `&FsEntry` it holds.

## `SearchIndexOptions`

| Field | Default | Meaning |
|---|---|---|
| `include_hidden` | `false` | index hidden files too |
| `recursive` | `true` | descend into sub-folders |
| `max_entries` | `50_000` | stop indexing past this many entries, to bound memory; `0` means no limit |

The defaults are written by hand (`impl Default for SearchIndexOptions`), not
derived. Building the struct without `..SearchIndexOptions::default()` fails
with `E0063: missing field`, which is how a forgotten `max_entries` shows up.

## `SearchQuery`

| Field | Meaning |
|---|---|
| `text: Option<String>` | text to find in names |
| `extensions: Vec<String>` | allowed extensions, such as `"rs"` or `"png"` |
| `entry_filter: EntryFilter` | everything, folders only, or files only |
| `min_size`, `max_size: Option<u64>` | size bounds, in bytes |
| `modified_after`, `modified_before: Option<SystemTime>` | date bounds |
| `case_sensitive: bool` | `false` by default |

Every filter set must match (a logical AND). An entry with no modification date
never matches once a date bound is set.

## Example

```rust
use std::path::Path;
use xion::filesystem::{EntryFilter, ListOptions, LocalFileSystem};
use xion::services::{SearchIndexOptions, SearchQuery, SearchService};

let filesystem = LocalFileSystem::new();
let service = SearchService;

let index = service.build_index_with_options(
    &filesystem,
    Path::new("/tmp"),
    SearchIndexOptions {
        recursive: true,
        ..SearchIndexOptions::default()
    },
    ListOptions::default(),
)?;

let query = SearchQuery {
    text: Some("report".into()),
    extensions: vec!["pdf".into()],
    entry_filter: EntryFilter::OnlyFiles,
    min_size: Some(10_000),
    ..SearchQuery::default()
};

let matches = service.count_index_matches(&index, &query);
```

No test compiles this block: `cargo test --doc` only sees the crate's own doc
comments, not the files in `docs/`.

## How the interface drives it

Indexing is asynchronous. `XionApp::start_search_indexing`
(`src/ui/app/state.rs`):

1. does nothing while the search field is empty. The index is a second copy of
   the folder, more memory than the listing itself, so it is built on the first
   query, not on every navigation;
2. does nothing if an index of the same folder is already there or being
   built. Indexing is started by typing, and without this guard every
   keystroke would start a new walk of the folder;
3. otherwise starts a `Task::perform` that runs `build_index_with_options`
   off the render loop, and sets `search.indexing` meanwhile;
4. receives the result as `UiMessage::SearchIndexBuilt { path, result }`,
   applied only if the route has not changed in the meantime.

There is no cache of past indexes. Keeping several meant several full copies
of folders for a narrow case (search in A, leave, come back, search again), and
it could serve an index older than the files.

Two details:

- the interface sets `recursive: false`, so the index covers the folder on
  screen, not the tree below it;
- `include_hidden` follows the list settings (`[list] show_hidden`).

The recursive search is a different path: `UiMessage::FullTextSearchSubmit`
walks the tree with `ignore::WalkBuilder`, and skips what `.gitignore` files
exclude when `respect_gitignore` is on.

## Tests

`src/services/search.rs` has no unit tests. The only coverage is
`search_service_text_filter` in `tests/integration_tests.rs`, which exercises
`search_in_dir` and none of the index paths. `max_entries`,
`MAX_SEARCH_DEPTH` and the combination of filters are untested.
