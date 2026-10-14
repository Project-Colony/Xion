# Xion documentation

Sorted by who is reading, not by subject.

## Using Xion

| | |
|---|---|
| [guide/configuration.md](guide/configuration.md) | where the settings live, every key in `config.toml`, and the keyboard shortcuts |

## Reading the code

| | |
|---|---|
| [internals/architecture.md](internals/architecture.md) | how the crate is split, how it is built and tested, and where the code falls short of its own rules |
| [internals/ui-routing.md](internals/ui-routing.md) | what a route, a selection and a tab are, and how navigation moves between them |
| [internals/search.md](internals/search.md) | how the search index is built, what it filters on, and how the interface drives it |

## Why it exists

| | |
|---|---|
| [project/decisions.md](project/decisions.md) | what Xion is for, the technical choices that follow from it, and the documentation still to write |

## How these pages stay true

No page carries a "reviewed" checkbox. A ticked box proves no review took
place: an earlier index certified three pages as reviewed and aligned with the
code while two of them described structures and paths that no longer existed.

Instead, each page cites its sources: a file and a symbol, or a test name. A
line number is a reading taken on one day; when it disagrees with the code, the
symbol it names is what counts.

Security policy: [../SECURITY.md](../SECURITY.md).
