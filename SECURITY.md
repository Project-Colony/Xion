# Security policy

Xion is a file explorer: it opens whatever files are on the disk, including
ones that came from somewhere else. That is its attack surface, and this page
says what it covers and how to report a problem.

## Supported versions

Only the **latest release** receives security fixes. There is no backport
branch: a fix ships in the next release, and Colony delivers it as an update.

| Version | Supported |
|---|---|
| latest release | yes |
| anything older | no |

## Reporting a vulnerability

**Do not open a public issue.** Report it privately through GitHub:

<https://github.com/Project-Colony/Xion/security/advisories/new>

Include what you have: what an attacker controls and what they get, the Xion
version and operating system, and the steps or the file that triggers it. A
sample file is the most useful thing you can send.

This is a small volunteer project, so there is no response-time guarantee. The
aim is an acknowledgement within 7 days, and a fixed release before anything is
disclosed, coordinated with you. Credit in the release notes if you want it.

## Scope

Reports of particular interest:

- **Untrusted files.** Xion parses files it did not create: ZIP, 7z and
  `.tar.gz` archives (listing, extraction, creation), images for thumbnails and
  previews, PDF text for previews, and any text file for syntax highlighting.
  Extraction refuses entries that would land outside the destination folder,
  including through a symbolic link. A path that escapes that check, or a file
  that crashes or hangs Xion, is in scope. Video and PDF thumbnails are made by
  running `ffmpeg`, `mutool` or `pdftoppm` when they are installed; a way to
  make Xion pass a file name to them as an option, or run a different program,
  is in scope.
- **Local network discovery.** Opening *Network* in the sidebar browses mDNS
  for SMB, FTP, HTTP and printer services, and on Windows also asks the system
  for the SMB shares it can see. Any machine on the network can announce any
  name, so the results are shown as unverified. A crafted announcement that
  does more than show a misleading name is in scope.
- **Windows shell integration.** `xion --register` writes context-menu entries
  under `HKEY_CURRENT_USER\Software\Classes`, and `xion --unregister` removes
  them. Xion never writes `HKEY_LOCAL_MACHINE` and never asks for elevation
  (its manifest requests `asInvoker`). A way to make those entries run
  something other than the Xion binary is in scope.
- **The embedded terminal.** The terminal panel runs your own shell as your
  user. Escape sequences in a file printed there that make Xion itself run a
  command, or write outside the terminal, are in scope.
- **Release signing.** Every release asset is signed in CI with the Project
  Colony ed25519 release key; the
  [Code signing policy](README.md#code-signing-policy) explains how to check a
  download. A way to publish an asset that verifies without that key is
  critical.

Out of scope: what a program you run in the terminal panel does, bugs in
`ffmpeg`, `mutool` or `pdftoppm` themselves (report those upstream), and
anything that needs an attacker who can already write to your configuration
directory.
