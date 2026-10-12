# Architecture

Xion is one Rust crate, `xion`, that builds a library and the `xion` binary. The
binary (`src/main.rs`) only reads its arguments, starts logging, claims the
single instance and opens the window; everything else is in the library, so the
integration tests in `tests/` can drive it without a window.

References below name a file and a symbol rather than a line number: a line
number is wrong one commit later, a symbol can be found with `git grep`.

## Modules

`src/lib.rs` declares them:

```
cli          --register, --unregister, --help, and the start path argument
core/        shared types, errors, configuration and its storage
i18n/        interface text: en.json (canonical) and fr.json
filesystem/  file system access, metadata, caches, operations, watcher
services/    history, favourites, directory loading, search, thumbnails,
             network discovery, gvfs mounts, highlighting
platform/    everything that exists on one target only
ui/          the iced application: state, messages, update, view
registry     Windows context-menu entries (Windows builds only)
```

The intended direction of dependencies is `ui` over `services` over
`filesystem`, with `core` at the bottom depending on nothing. Two places break
it; see [Where the code falls short](#where-the-code-falls-short).

### core/

- `XionError` and `AppResult`, the crate's error type (`src/core/mod.rs`). The
  `Rejected` variant marks a deliberate refusal, such as copying a folder into
  itself or an archive entry that escapes its destination, as opposed to an
  I/O failure.
- `core::uri` decodes percent-encoded URIs. A desktop launcher hands Xion
  `file:///home/alice/My%20documents`, and gvfs names its mounts the same way.
- `core::config`: `AppConfig` (`types.rs`), the keyboard shortcuts
  (`shortcuts.rs`), the session store (`session.rs`) and the configuration
  manager (`manager/`). See [Configuration storage](#configuration-storage).

### i18n/

Every user-visible string is a key in `src/i18n/en.json` and `src/i18n/fr.json`,
read with `i18n::t(key)`, or `i18n::tf(key, args)` for a string with
`{placeholders}`. English is canonical. A lookup falls back to English, then to
`colony_ui::i18n::t`, which owns the labels shared by every Colony program
(theme names, accents, the standard Preferences categories). Xion never copies
those.

There is one active locale for the process, colony-ui's.
`i18n::apply(config.language)` sets it at startup and whenever the language
setting changes, so Xion's strings and the shared widgets always agree. The
tests in `src/i18n/mod.rs` fail when the two files have different keys, when a
value is empty, when a key duplicates a shared colony-ui label, or when a
literal key used in `src/` exists in neither file.

### filesystem/

- `FileSystem`, the trait the rest of the crate lists and reads through, and
  `LocalFileSystem`, its implementation (`access.rs`). `FsEntry`,
  `ListOptions`, `EntryFilter`, `SortKey` and `SortOrder` describe a listing.
- `Page` and `PageRequest` (`paging.rs`): a large folder is loaded a page at a
  time.
- `DirectoryCache`, built on `TimedCache` (`cache.rs`): bounded in size and in
  age, like every cache in Xion.
- `LocalFileOperations` (`operations.rs`): copy, move, rename, delete. A copy
  refuses an occupied destination and a destination inside its own source.
- `FileWatcher` (`watcher.rs`), implemented by `NativeFileWatcher` on top of
  `notify`, non-recursive, with a bounded event queue. `NoopFileWatcher`
  replaces it only when the native watcher cannot be created
  (`src/ui/app/construction.rs`).

### services/

- `HistoryService` (back and forward), `FavoritesService`.
- `DirectoryLoader`: one listing per folder, then every page is a slice of the
  cache instead of a new `read_dir`.
- `SearchService`: the in-memory search index. See [search.md](search.md).
- `ThumbnailService`: images through the `image` crate, videos through
  `ffmpeg`, PDFs through `mutool` or `pdftoppm`. Each tool is looked up in
  `PATH` once, used only when found, and stopped after a timeout.
- `NetworkDiscoveryService` (`network/`): mDNS browsing for SMB, FTP, HTTP and
  printer services (`mdns.rs`), and on Windows the SMB shares the system can
  see (`wnet.rs`, then `net_view.rs` as a fallback). It lists; it never
  connects.
- `gvfs`: the locations a Linux desktop has mounted through gvfs (SMB, SFTP,
  phones, Google Drive), found under `$XDG_RUNTIME_DIR/gvfs` where
  `gvfsd-fuse` exposes them as ordinary folders. No GIO binding:
  `LocalFileSystem` reads them as it reads anything else.
- `VirtualList`: the window of rows actually rendered.
- `highlight`: syntax highlighting for text previews, through `syntect`.

### platform/

Everything that exists on one target only, behind one API, so the rest of the
crate does not write `#[cfg]` inline (`src/platform/mod.rs`). The non-Windows
backend degrades explicitly rather than silently doing nothing.

- `platform::single_instance::claim`: the first Xion of a session owns a
  well-known name, and a later launch hands it the path to open and exits. On
  Linux this is D-Bus, through `zbus`; a desktop without D-Bus gets one process
  per launch. **Windows has no backend yet**: every launch is a new process.
- `platform::prefer_vulkan_when_available` restricts wgpu to Vulkan on Linux,
  but only after checking that a Vulkan adapter answers. Letting wgpu probe
  every backend leaves the OpenGL drivers loaded for the life of the process.
- `attach_parent_console` gives `--help` a console on Windows, where the
  binary is built for the GUI subsystem.
- `system_binary` resolves `reg.exe`, `cmd`, `icacls` and the like to their
  absolute `System32` path on Windows, so a file with the same name next to
  the executable is never run instead.

### ui/

- `AppState` holds the configuration, the current route and the navigation
  state (`src/ui/mod.rs`). Routes, panes and selection are described in
  [ui-routing.md](ui-routing.md).
- `UiMessage` (`src/ui/mod.rs`) is every event the interface handles. The enum
  is its own reference; listing it here would be wrong at the next commit.
- `XionApp` (`src/ui/app/`) is the iced application. `update/` dispatches
  messages by domain (navigation, input, file operations, archives, search,
  media, terminal, tools), each handler passing on what it does not own.
  `view/` builds the widget tree, one file per region (header, sidebar, list,
  preview, terminal panel, status bar, overlays, preferences). `operations/`
  and `navigation/` hold the logic those two share.
- `ui::theme` maps the active Colony palette to the surfaces Xion draws.

### cli and registry

`src/cli.rs` handles the arguments: `--register` and `--unregister` (Windows
context-menu entries), `--help`, and the start path, which may arrive as a
`file://` URI from a desktop entry's `%U`. `src/registry.rs` writes and removes
the entries under `HKEY_CURRENT_USER\Software\Classes` by running `reg.exe` from
its absolute path. Nothing is ever written under `HKEY_LOCAL_MACHINE`.

## Data flow

```
UiMessage -> XionApp::update -> services (history, loader, search, thumbnails)
                             -> filesystem (listing, operations)
                                   ^
                                   | core::config
```

Blocking work (copying, deleting, indexing, thumbnails, the network scan) runs
in `tokio::task::spawn_blocking` and comes back to the iced loop as a message.

Refresh flows the other way: a subscription sends `FileWatchTick` every 750 ms
(`WATCHER_POLL_INTERVAL` in `src/ui/theme.rs`, subscribed in
`src/ui/app/windowing.rs`). The tick drains the watcher, and an event in the
displayed folder triggers a new listing of it.

## The shared Colony layer

Xion takes its look and its plumbing from
[colony-ui](https://crates.io/crates/colony-ui) 0.1.6, the crate every Project
Colony program shares:

- **Palette.** Colours come from the shared theme catalogue: a family, a
  variant, a high-contrast switch and an accent. `UiTokens::for_theme` resolves
  them and `UiColors::from_colony` derives the surfaces Xion names. Xion
  writes no colour of its own, and a family added to the catalogue appears
  without rebuilding Xion.
- **Typography.** The fonts are the kit's: JetBrains Mono Nerd Font in Regular,
  Medium and Bold, OpenDyslexic, and Font Awesome, registered at startup. Text
  sizes go through `sz(base)`, which multiplies by the user's text size
  settings, and the dyslexia switch swaps the whole interface at once.
- **Motion.** Every animation (the preview and terminal panels sliding, animated
  image previews) is gated on `colony_ui::effects_enabled()`, which is false
  under reduced motion.
- **Preferences.** The Preferences page is the kit's layout and building blocks,
  with Xion's own categories after the standard ones.
- **Paths.** `colony_ui::paths` gives `Colony/Xion/` under the platform's
  configuration and data folders.

## The terminal panel

The panel embeds [`iced_term`](https://github.com/Harzu/iced_term), pinned to a
git revision in `Cargo.toml`, which runs `alacritty_terminal` underneath: a
real pseudo-terminal (ConPTY on Windows, `openpty` elsewhere), a full cell grid,
the alternate screen, so `vim`, `htop` and `less` work. Keystrokes go to the
shell untouched; the shell does its own completion.

Xion only decides when an emulator starts and stops and which tab receives what
it produces (`src/ui/app/update/terminal.rs`, drawn by
`src/ui/app/view/terminal_panel.rs`). Each terminal tab has its own emulator,
identified by a number that is never reused, so a closed tab cannot receive
another one's output. Closing the panel stops the emulators and their shells.
The shell comes from the `terminal_shell` setting (`src/ui/app/shell.rs`).

## Configuration storage

What the user sees of it is in
[../guide/configuration.md](../guide/configuration.md). The code:

| File in `src/core/config/manager/` | What it holds |
|---|---|
| `mod.rs` | `ConfigManager`: the path, loading, the atomic write, the migration from the old folder, and their tests |
| `file_format.rs` | the on-disk shapes, `AppConfigFileV0` and `AppConfigFileV1`, kept apart from `AppConfig` so the runtime type can change without breaking old files |
| `migrate.rs` | `merge_from_v0`, `merge_from_v1` (file to `AppConfig`) and `config_to_file` (the reverse) |
| `validate.rs` | the `validated_*` functions, which clamp a value or fall back with a `ConfigWarning` |
| `limits.rs` | `CURRENT_CONFIG_VERSION` and the `MIN_*` / `MAX_*` bounds |
| `shortcuts_io.rs` | parsing and writing the `[shortcuts]` table |

Points that are easy to get wrong:

- **The path comes from `colony_ui::paths::locate::config_dir`, not
  `paths::config_dir`.** The second creates the folder, and a folder created
  before the migration runs makes a configuration look already in place: the
  migration is skipped and defaults get written next to the real file.
  `write_atomic` creates the parent itself when it saves.
- **The migration from `~/.config/xion/`** (`migrate_legacy_config_dir`)
  copies every file into a staging folder, `Colony/Xion.migrating`, and renames
  it into place only once every copy succeeded. A sub-directory counts as a
  failure, since Xion never creates one. On failure the staging folder is
  removed and that run uses the old path. A staging folder older than a minute
  was left by a crash and is replaced; a younger one may belong to a start
  running at the same moment and is left alone. The old folder gets a
  `.migrated` marker and is never moved or deleted.
- **Tests use `ConfigManager::with_path`** on a temporary folder.
  `ConfigManager::new()` runs the migration against the real home directory.
- **The session** (`SessionStore`, `session.rs`) lives in the data folder,
  `colony_ui::paths::locate::data_dir("Xion")`. When `session.toml` is missing
  it reads the tabs once from `config.toml` (or from `config.toml.bak` when the
  main file does not parse) through `ConfigManager::legacy_session`.

## Windows build resources

`build.rs` embeds two resources into the Windows executable: the application
manifest, `packaging/windows/xion.manifest`, and a version resource generated
from `Cargo.toml`, so the `ProductVersion` is always the one release-please
wrote. The manifest requests `asInvoker` (Xion never runs elevated, so neither
does anything it starts, and drag and drop from Explorer is not blocked),
per-monitor DPI awareness, and UTF-8 as the process code page. `build.rs` reads
the target from `CARGO_CFG_TARGET_OS`, so cross-compiling from Linux embeds
the resources too.

## Safety rules the code keeps

- **Archive extraction** refuses any entry that would land outside the
  destination, including through a symbolic link (`src/ui/app/archive.rs`,
  tests `zip_entry_with_backslash_traversal_is_rejected`,
  `sevenz_entry_with_absolute_path_is_rejected`,
  `create_parent_within_rejects_a_symlink_escape`).
- **Deleting** moves to the trash (`trash::delete`,
  `src/ui/app/operations/delete.rs`). Shift+Delete deletes permanently, and
  always asks first (tests `permanent_delete_asks_before_acting`,
  `cancelling_the_confirmation_deletes_nothing`).
- **External tools** (`ffmpeg`, `mutool`, `pdftoppm`) are only given absolute
  paths, which none of them can mistake for an option or a URL.
- **No `panic = "abort"`** in the release profile: the parsers for archives,
  images, PDFs and syntax run inside `spawn_blocking`, and unwinding turns a
  malformed file into a failed preview instead of a closed window.

## Building and testing

The toolchain is pinned in `rust-toolchain.toml`; `rust-version` in
`Cargo.toml` (1.95) is the oldest compiler CI proves the crate still builds
with. Beyond Rust, a build needs only a C compiler, for the bundled C code of
libgit2, zlib and zstd; the windowing libraries are loaded at run time.

```sh
cargo build --release
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Tests live in three places:

- unit tests next to the code they test, in `#[cfg(test)]` modules;
- `tests/integration_tests.rs`, which drives the crate from outside. Test-only
  hooks such as `XionApp::new_for_test` exist only under `cfg(test)` or the
  `test-util` feature, which the crate's dev-dependency on itself turns on for
  `tests/` alone, so they never reach the shipped binary;
- `tests/benches.rs`, measurements kept as ignored tests so CI keeps compiling
  them. Run them in release, one at a time:

  ```sh
  cargo test --release --test benches -- --ignored --test-threads=1 --nocapture
  ```

  `XION_BENCH_ENTRIES` changes the number of directory entries they use.

On Linux, `scripts/install.sh` builds a release binary and installs it under
`~/.local` with its icons (`assets/brand/`) and its desktop entry
(`packaging/xion.desktop`), without `sudo`; `--uninstall` removes exactly what
it installed.

CI (`.github/workflows/ci.yml`) runs on every pull request and every push to
`main`, and weekly so that a newly published advisory surfaces without a push:

| Job | What it runs |
|---|---|
| `ubuntu-latest`, `windows-latest` | format check, clippy with warnings as errors, every test, and a check that no source file starts with a UTF-8 byte order mark |
| `msrv` | `cargo check` and `cargo test` on Rust 1.95 |
| `advisories` | `cargo deny check advisories bans licenses sources` |

All four are required before anything merges into `main`. Releases are built,
signed and published by `.github/workflows/release.yml` through the shared
Project Colony workflow; the README's
[Code signing policy](../../README.md#code-signing-policy) says how to check
one.

## Where the code falls short

These are real debts, not choices. They are written down so that nobody
trusts a rule the code does not keep. Each comes with the command that measures
it today.

1. **The interface touches the disk directly.** `src/ui/` calls `std::fs` in
   many places, most of them in `app/archive.rs`, instead of going through
   `filesystem/`.
   `git grep -c 'std::fs::' -- src/ui`
2. **`core` depends on `ui`.** `AppConfig` stores `crate::ui::FileLabel`, and
   `dark_mode` is derived with `crate::ui::theme::resolves_dark`.
   `git grep -n 'crate::ui::' -- src/core`
3. **Some files are long.** The target is no file above 800 lines.
   `find src -name '*.rs' -exec wc -l {} + | sort -rn | head`
4. **Many files have no unit test**, among them `services/search.rs` and most
   of `ui/app/view/`. Some are covered from `tests/integration_tests.rs`.
   `grep -rL --include='*.rs' '#\[cfg(test)\]' src`
5. **The watcher triggers a full listing** of the folder on any relevant event,
   not a targeted update of the entries that changed.
