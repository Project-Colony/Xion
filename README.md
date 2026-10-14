<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/png/xion-logo-512.png">
  <img src="assets/brand/png/xion-logo-light-1024.png" alt="Xion" width="360">
</picture>

**A tabbed file explorer with a real terminal panel, the same on Linux and Windows.**

</div>

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Colony app](https://img.shields.io/badge/Colony-utility-purple)](https://github.com/Project-Colony/Colony)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows-lightgrey)](#installation)

Every desktop ships its own file manager: Nautilus on GNOME, Dolphin on KDE,
Explorer on Windows. Each one belongs to its platform, so changing machines
means changing tools, shortcuts and habits. Xion is one explorer for Linux and
Windows, with tabs, previews, archives and a terminal panel, built in Rust on
the toolkit every [Project Colony](https://github.com/Project-Colony) program
shares.

> **Status:** early. Proven: every change is built and tested on Linux
> and Windows in CI, and on the minimum Rust version; `cargo deny` reports no
> open advisory; releases are published signed, and the signatures verify.
> Wired but untested: the Windows context-menu integration (`xion --register`)
> has only been tried by hand, and the single-instance handover exists on Linux
> only (over D-Bus), so on Windows every launch opens a new window.

## Why Xion

- **One program on both systems.** The same interface, the same shortcuts and
  the same settings file on Linux and on Windows, instead of a different file
  manager on each.
- **A real terminal next to the files.** The panel at the bottom opens in the
  folder you are looking at, has its own tabs, and is a full terminal emulator:
  `vim`, `htop` and Ctrl+C behave as they do anywhere else.
- **Archives without another tool.** ZIP, `.tar.gz` and 7z archives are browsed
  like folders, extracted and created in place, and extraction refuses any
  entry that would land outside the destination.
- **Part of an ecosystem.** Themes, accents, text size, the dyslexia-friendly
  font and reduced motion come from the same catalogue and the same
  Preferences layout as every other Colony program.

## What it does

- Tabs, back and forward history, and an editable address bar with
  suggestions.
- A sidebar with drives, favourites, quick access, recent locations and a
  folder tree, and an optional second pane.
- List and grid views with resizable, sortable columns, a quick filter, and
  colour labels on files.
- Copy, move, rename (one file or many at once) and delete, with progress and
  undo of the last action. Deleting goes to the trash; Shift+Delete deletes
  permanently after a confirmation.
- Archives: browse, extract and create `.zip`, `.tar.gz` and `.7z`.
- Search by name over an in-memory index of the folder, and a recursive
  full-text search that can respect `.gitignore`.
- Previews: metadata, images, syntax-highlighted text, a hex view, and a diff
  of two files. Properties show a file's SHA-256 and let you edit its
  permissions.
- In a git repository, modified, new, staged and conflicting files are marked
  in the list.
- An embedded terminal panel built on `iced_term` (`alacritty_terminal`),
  running your own shell.
- Locations mounted by gvfs (SMB, SFTP, phones, Google Drive) in the sidebar,
  and the SMB, FTP, HTTP and printer services your local network announces.
- An English and a French interface. Preferences > General > Language picks
  one, or follows the session's language by default.
- Appearance and accessibility preferences shared with the Colony ecosystem:
  every theme and accent of the Colony catalogue, high contrast, two text
  sizes that multiply, the OpenDyslexic font, and reduced motion, which turns
  every animation off. The **Xion** button in the top-left corner opens
  Preferences.
- Settings in one versioned TOML file under `Colony/Xion/`, saved as you change
  them and reloaded with Ctrl+R.

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

```bash
chmod +x xion-linux && ./xion-linux
```

To check the download first, see [Code signing policy](#code-signing-policy).

### Build from source

```bash
git clone https://github.com/Project-Colony/Xion
cd Xion
cargo build --release
./target/release/xion
```

Requires Rust 1.95 or newer; `rust-toolchain.toml` pins the toolchain CI uses,
and rustup installs it on the first build. Beyond Rust, the build needs only a
C compiler (`build-essential` on Debian or Ubuntu) for the bundled C code of
libgit2, zlib and zstd. The windowing libraries are loaded at run time, so no
development package is required.

On Linux, `scripts/install.sh` builds a release binary and installs it with its
icons and desktop entry under `~/.local`, without `sudo`.
`scripts/install.sh --uninstall` removes exactly what it installed.

## Documentation

Full documentation is in [docs/](docs/). Start at
[docs/README.md](docs/README.md). To report a vulnerability, see
[SECURITY.md](SECURITY.md).

## Code signing policy

Every release asset is signed by the Project Colony organisation in CI, never on
a developer machine. Next to each asset on the release page:

| File | What it is |
|---|---|
| `<asset>.sig` | an ed25519 signature over the asset, made with the organisation's release key |
| `<asset>.meta` | three lines binding the asset to its file name, its sha256 and the release version |
| `<asset>.meta.sig` | an ed25519 signature over the `.meta` |

The private key is an organisation secret, used only by the shared
[sign-and-publish workflow](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/.github/workflows/sign-and-publish.yml)
in a job that builds nothing; the jobs that compile Xion never see it.
Colony checks all three files before it installs or updates Xion, and refuses a
release older than the one installed. To check a download yourself with
OpenSSL 3 (the same commands work for every asset):

```bash
cat > colony-release.pub <<'EOF'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEARNjg3Nn8H6/aBg1unwGjkUTcrdTxERNefVaqU8cFu0s=
-----END PUBLIC KEY-----
EOF
a=xion-linux
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a" -sigfile "$a.sig"
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a.meta" -sigfile "$a.meta.sig"
cat "$a.meta"     # version=<tag>, asset=<file name>, sha256=<digest>
sha256sum "$a"    # the digest must equal the sha256 line
```

How releases are built, signed and published:
[design/releases.md](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/design/releases.md#5-signing).

## Privacy

Xion sends no telemetry, no analytics and no crash reports. It has no account
and no update check; Colony handles updates.

| Data | Stored or sent | Where, and why |
|---|---|---|
| Settings | stored | `Colony/Xion/config.toml` in this machine's config directory: your preferences, favourites, colour labels and column widths |
| Open tabs | stored | `Colony/Xion/session.toml` in this machine's data directory, to reopen them at the next start |
| Network scan | sent, on your local network only | when you open or refresh *Network* in the sidebar, at most every 30 seconds: the standard mDNS queries for SMB, FTP, HTTP and printer services, for a few seconds. On Windows, Xion also asks the system for the SMB shares it can see. What answers is listed, marked as unverified |
| Windows context menu | stored | entries under `HKEY_CURRENT_USER\Software\Classes`, only after you run `xion --register`; `xion --unregister` removes them |

Xion connects to no server, and touches your local network only for the scan
above. It asks for and stores no password. Thumbnails and the search index stay
in memory. It runs local programs on your behalf: your shell in the terminal
panel, `ffmpeg`, `mutool` or `pdftoppm` for video and PDF thumbnails when they
are installed, and the system's opener for files you open. Opening a network
share or a gvfs location is ordinary file access through the system.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Xion's fonts (JetBrains Mono Nerd
Font, OpenDyslexic and Font Awesome 6 Free) come embedded in
[colony-ui](https://crates.io/crates/colony-ui), which ships them with their
licence texts.
