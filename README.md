# Xion

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Colony app](https://img.shields.io/badge/Colony-utility-purple)](https://github.com/Project-Colony/Colony)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows-lightgrey)](#installation)

A file explorer for Linux and Windows, written in Rust with [iced](https://iced.rs).
Xion is part of [Project Colony](https://github.com/Project-Colony): its settings
live under `Colony/Xion/` in the user configuration directory, and its look comes
from the shared Colony theme catalogue.

> **Status:** in development. It builds and its tests pass on Linux and Windows
> in CI, and `cargo deny` reports no open advisory. The interface is in French
> only for now.

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

## Installation

### Via Colony (recommended)

Search for **Xion** in [Colony](https://github.com/Project-Colony/Colony) and
install it. Updates arrive through the launcher, which checks each release's
signature before installing it.

### Direct binary download

Grab the asset for your platform from the
[latest release](../../releases/latest):

| Platform | Asset |
|---|---|
| Linux | `xion-linux` |
| Windows | `xion-windows.exe` |

```sh
chmod +x xion-linux && ./xion-linux
```

### Build from source

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

## Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Windows builds are Authenticode-signed this way once the SignPath Foundation
has accepted the project; until then they ship without Authenticode. Every
release asset, on every platform, is always signed with the Project-Colony
ed25519 release key, and Colony verifies that signature before installing.

Team roles and members:

- Committers and reviewers: [MotherSphere](https://github.com/MotherSphere)
- Approvers: [MotherSphere](https://github.com/MotherSphere)

### Privacy policy

Xion itself never contacts the internet: there is no telemetry, no analytics,
no account and no update check. It touches the network in one place, your
local network, and only when you open it.

- **The Network location.** Opening or refreshing *Network* in the sidebar
  starts a scan, unless the previous one is less than 30 seconds old. Xion
  browses mDNS (DNS-SD) for SMB, FTP, HTTP and printer services for about
  three seconds: it sends the standard multicast queries on your local network
  and lists what answers, marked as unverified, since any machine on the
  network can announce any name. On Windows it also asks Windows for the SMB
  shares it can see (`WNetEnumResource`, then `net view`). Nothing else is
  sent during a scan, and Xion asks for and stores no network password.
  Opening a share found there is ordinary file access through the system.
- **gvfs locations.** On Linux, locations your desktop has already mounted
  through gvfs (SMB, SFTP, phones, Google Drive) appear in the sidebar. Xion
  only lists them; opening one is ordinary file access through gvfs.

What Xion keeps stays on your machine, in `config.toml` under `Colony/Xion/`
in your configuration directory: your preferences, the paths of your open tabs
and favourites, the labels you put on files, and column widths. Thumbnails and
the search index are held in memory only. Xion runs local programs on your
behalf: your shell in the terminal panel, `ffmpeg`, `mutool` or `pdftoppm` for
video and PDF thumbnails when they are installed, and the system's opener for
files you open. On Windows, `xion --register` adds context-menu entries under
`HKEY_CURRENT_USER`, and `xion --unregister` removes them.

## License

Xion is licensed under the GNU General Public License v3.0 or later. See
[LICENSE](LICENSE). The bundled JetBrains Mono Nerd Font files are under the SIL
Open Font License 1.1 (`ui/Assets/Fonts/OFL.txt`).
