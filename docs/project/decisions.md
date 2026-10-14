# Goals and decisions

## What Xion is for

A file explorer for Linux and Windows in the spirit of Windows File Explorer,
Files and Dolphin: tabs, a sidebar, list and grid views, previews, archives, a
terminal next to the files. It is part of Project Colony, so it shares the
ecosystem's look, settings layout and release process with the other programs
instead of inventing its own.

## Rules the project holds itself to

- **Light and responsive.** Folders load lazily, a page at a time; lists and
  the tree render only the visible rows; metadata is read in batches; every
  cache (thumbnails, folders, search index) is bounded in size and
  in age; blocking work never runs on the render loop.
- **Split by responsibility.** The file system layer knows nothing of the
  interface, which is what makes virtualisation and most tests possible
  without a window. Where the code breaks this rule, the gap is written down in
  [../internals/architecture.md](../internals/architecture.md#where-the-code-falls-short).
- **Configuration over hard-coding.** Behaviour that a user could reasonably
  want different is a setting, versioned, migrated and validated
  ([../guide/configuration.md](../guide/configuration.md)).
- **Current Rust, current dependencies.** Rust edition 2024, with
  `Cargo.toml` and `Cargo.lock` kept up to date.
- **Keyboard and mouse alike.** The whole interface is meant to be usable with
  either one.
- **Measured, not assumed.** Performance claims in the code come with the
  measurement that backs them; the measurements live in `tests/benches.rs`.

## Decisions

| Decision | Why |
|---|---|
| **iced 0.14 on wgpu** for the interface | a native GPU-drawn interface in Rust, the toolkit every Colony program uses |
| **colony-ui** for colours, fonts, text sizes, motion, Preferences and paths | Xion looks and behaves like the rest of the ecosystem, and a theme added to the shared catalogue reaches it without a rebuild |
| **Theme as a catalogue choice** (family, variant, high contrast, accent), not a closed list of names | an unknown or removed theme falls back to the default palette instead of stopping Xion from starting |
| **Files under `Colony/Xion/`** | the ecosystem layout: settings in the configuration folder, the session in the data folder |
| **English interface with a French translation**, following the session's language unless chosen | English is the base language of the ecosystem; French is the second locale every Colony program ships |
| **`iced_term` (alacritty_terminal) for the terminal panel** | a real terminal: full screen programs, colours, Ctrl+C and resizing work, where an earlier home-made line model could not run `vim` or `htop` |
| **`notify` for the file watcher** | the native mechanism on each platform, with a no-op fallback when it cannot start |
| **Targets: `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`** | the two platforms CI builds and tests, and the two release assets |
| **`asInvoker` on Windows**, context-menu entries under `HKEY_CURRENT_USER` only | a file explorer has no reason to run elevated, and everything it starts would inherit the elevation |
| **Deleting goes to the trash** by default | a permanent delete is a separate action, behind a confirmation |
| **Releases signed in CI only**, through the shared Project Colony workflow | the signing key never reaches a developer machine or a build job; see the README's [Code signing policy](../../README.md#code-signing-policy) |

## Documentation still to write

Open items, carried over from the documentation roadmap of the earlier French
`docs/docs.md`. The module map, the caches, search and the navigation model are
written; these are not:

- [ ] Interaction diagrams: the modules and the data flowing between them.
  [../internals/architecture.md](../internals/architecture.md#data-flow) has a
  short sketch, not a diagram.
- [ ] The iced render cycle and the state model, in detail.
- [ ] Tie the code examples in `docs/` to the crate, so that
  `cargo test --doc` compiles them and they can no longer rot.
- [ ] Give each page the date it was last checked against the code (see
  [How these pages stay true](../README.md#how-these-pages-stay-true)).
