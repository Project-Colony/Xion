# Xion

A file explorer for Linux and Windows, written in Rust with [iced](https://iced.rs).
Xion is part of [Project Colony](https://github.com/Project-Colony): its settings
live under `Colony/Xion/` in the user configuration directory, and its look comes
from the shared Colony theme catalogue.

> **Status:** in development, with no release yet. On Linux it builds and its
> tests pass in CI. The Windows build is currently broken on `main`, and
> `cargo deny` reports open security advisories in some dependencies. The
> interface is in French only for now.

## What it does

- Tabs, back and forward history, and an editable address bar with suggestions.
- A sidebar with drives, favourites, quick access and a folder tree.
- List and grid views with configurable, resizable and sortable columns.
- Copy, move, rename and delete, with progress and undo of the last action.
  Deleting goes to the trash; Shift+Delete deletes permanently after a
  confirmation.
- Archives: browse, extract and create `.zip`, `.tar.gz` and `.7z`. Extraction
  refuses any entry that would land outside the destination folder.
- Search by name over an in-memory index, plus an optional recursive full-text
  search that respects `.gitignore`.
- Previews: metadata, images, syntax-highlighted text, a hex view and a diff of
  two files.
- An embedded terminal panel built on `iced_term` (`alacritty_terminal`).
- Locations mounted by gvfs (SMB, SFTP, phones, Google Drive) appear in the
  sidebar.
- A versioned TOML configuration, reloaded on change, at
  `~/.config/Colony/Xion/config.toml` on Linux.

## Build from source

The Rust toolchain is pinned in `rust-toolchain.toml`; rustup installs it on the
first build. On Debian or Ubuntu, install the system libraries first:

```sh
sudo apt-get install libgtk-3-dev libxkbcommon-dev pkg-config
```

Then:

```sh
git clone https://github.com/Project-Colony/Xion.git
cd Xion
cargo build --release
./target/release/xion
```

On Linux, `scripts/install.sh` builds a release binary and installs it with its
icons and desktop entry under `~/.local`, without `sudo`.
`scripts/install.sh --uninstall` removes exactly what it installed.

## Documentation

Technical notes are in `docs/` (in French for now): architecture, configuration,
search and UI routing.

## License

Xion is licensed under the GNU General Public License v3.0 or later. See
[LICENSE](LICENSE). The bundled JetBrains Mono Nerd Font files are under the SIL
Open Font License 1.1 (`ui/Assets/Fonts/OFL.txt`).
