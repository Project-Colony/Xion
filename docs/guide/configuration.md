# Configuration

Everything Xion remembers about your choices is in one TOML file,
`config.toml`. You never have to open it: every setting has a control in the
interface, and each change is saved the moment you make it. This page is for
when you want to read it, edit it by hand, or understand what happened to it.

## Where the files are

Xion keeps its files under `Colony/Xion/`, like every Project Colony program.

| File | Linux | Windows |
|---|---|---|
| `config.toml`, your settings | `~/.config/Colony/Xion/` | `%LOCALAPPDATA%\Colony\Xion\` |
| `session.toml`, the tabs to reopen | `~/.local/share/Colony/Xion/` | `%LOCALAPPDATA%\Colony\Xion\` |

On Linux, `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME` replace `~/.config` and
`~/.local/share` when they are set. On Windows it is `AppData\Local`, never
`Roaming`.

Next to `config.toml` you may also find:

- `config.toml.bak`, the previous version, kept by every save;
- `config.toml.corrupt-<timestamp>`, a file Xion could not read and moved
  aside (see [When the file is unreadable](#when-the-file-is-unreadable)).

### Coming from an older Xion

Versions before the move to `Colony/Xion/` wrote to `~/.config/xion/` on Linux.
The first start of a newer version copies that whole folder, backup included,
into `Colony/Xion/`, then leaves a `.migrated` marker in the old folder so the
copy happens once. The old folder itself is kept, untouched, in case you need
it.

The copy is all or nothing. If any file cannot be copied, nothing is created
under `Colony/Xion/` and Xion keeps using `~/.config/xion/` for that run; it
tries again at the next start. If `Colony/Xion/` already holds a `config.toml`,
it wins and nothing is copied.

## Editing the file by hand

You can edit `config.toml` while Xion runs, then press **Ctrl+R** or **F5**:
the refresh reads the file again before listing the folder.

Two things to know:

- **Comments are not kept.** The next setting changed from the interface
  rewrites the whole file from what Xion understood of it.
- **Every key is optional.** A missing key, or a whole missing section, takes
  its default. An empty file is a valid configuration.

## Every key

The block below is everything Xion reads and writes, with its default value.
Ranges are inclusive; a value outside its range is replaced by the default, and
Xion says so in the status bar.

```toml
# Top-level keys. In TOML they all come before the first [section].

version = 1

# Appearance. The theme is picked from the Colony catalogue that Xion shares
# with the other Project Colony programs: a family, then a variant in it.
theme_family = "gruvbox"     # gruvbox, nord, catppuccin, solarized...
theme_variant = "dark"       # dark, light, mocha...: the names depend on the family
high_contrast = false        # strengthens whichever palette is active
# accent = "blue"            # red | orange | yellow | green | blue | indigo |
                             # violet | amber. Absent: the theme's own accent.

# Interface language: auto | en | fr. `auto` follows the session: LC_ALL,
# LC_MESSAGES, then LANG on Linux (empty values are skipped), the display
# language on Windows.
language = "auto"

# Folder opened in a new tab. Default: the folder Xion was started from.
# start_path = "/home/alice"

# true also forces [view] row_height to 22.0.
compact_mode = false

# Shell of the terminal panel. System is your login shell ($SHELL on Linux, cmd
# on Windows); Bash is Git Bash on Windows. Any other value is the absolute
# path of the shell to run. The old names Cmd and GitBash are still read.
terminal_shell = "System"    # System | PowerShell | Bash | /absolute/path

# Whether the recursive full-text search skips what .gitignore files exclude.
respect_gitignore = false

# Your favourites, in order. Written only when there is at least one.
# user_favorites = ["/home/alice/work"]

[list]
show_hidden = false
sort_key = "name"            # name | modified | size
sort_order = "asc"           # asc | desc
directories_first = true
filter = "all"               # all | only-directories | only-files

[cache]
thumbnail_entries = 256      # 32..8192
thumbnail_ttl_seconds = 300  # 30..86400
directory_entries = 256      # 32..8192
directory_ttl_seconds = 45   # 30..86400

[filesystem]
metadata_batch_size = 256    # 16..4096
metadata_parallelism = 4     # 1..32

[view]
mode = "list"                # list | grid
thumbnail_size = 48          # 24..256
row_height = 32.0            # 20.0..72.0
grid_columns = 4             # 1..12
grid_row_height = 140.0      # 72.0..240.0
overscan = 6                 # 0..128, rows rendered beyond the visible ones
columns = ["name", "type", "size", "modified"]

[paging]
page_size = 120              # 24..2048, entries loaded at a time

# Colour labels, by path: Red | Orange | Yellow | Green | Blue | Purple | Gray.
# Written only when at least one label exists.
[labels]
"/home/alice/projects" = "Blue"

# Widths of the Details view columns, in points. The keys are the column
# identifiers, the same in every interface language.
[column_widths]
Name = 300.0
Type = 80.0
Size = 100.0
Modified = 150.0

[shortcuts]
move_up = "ArrowUp"
move_down = "ArrowDown"
move_home = "Home"
move_end = "End"
activate = "Enter"
clear_selection = "Escape"
cycle_pane_focus = "Tab"
back = "Alt+ArrowLeft"
forward = "Alt+ArrowRight"
refresh = "Ctrl+R"
select_all = "Ctrl+A"
toggle_context_menu = "Ctrl+M"
rename = "F2"
delete = "Delete"
new_folder = "Ctrl+Shift+N"
focus_search = "Ctrl+E"
```

An unknown theme family or variant is not an error: Xion starts on the
catalogue's fallback palette, so a theme removed from the catalogue never stops
it from starting.

### Files written by older versions

- **No `version` key.** The file is read as the original format. Only
  `start_path`, `show_hidden`, `thumbnail_size`, `thumbnail_cache_entries` and
  `thumbnail_cache_ttl_seconds` are carried over; everything else takes its
  default.
- **A `version` above 1** was written by a newer Xion. This one refuses it,
  says so, and handles it like an unreadable file (see below): it is moved
  aside, not overwritten.
- **A `theme` key** (`Light`, `Dark`, `Nord`, `Solarized` or `HighContrast`)
  comes from before the Colony catalogue. It is still read, only when
  `theme_family` or `theme_variant` is missing, and never written again:

  | old `theme` | becomes |
  |---|---|
  | `Light` | `gruvbox` / `light` |
  | `Dark` | `gruvbox` / `dark` |
  | `Nord` | `nord` / `dark` |
  | `Solarized` | `solarized` / `dark` |
  | `HighContrast` | `gruvbox` / `dark` with `high_contrast = true` |
  | anything else | `gruvbox` / `dark`, with a warning |

- **`dark_mode`** is no longer a setting. Xion works it out from the theme, and
  a written value that disagrees with the theme is ignored with a warning.
- **`[[tabs]]` and `active_tab_index`** moved to `session.toml` (below). They
  are read from `config.toml` once, when `session.toml` does not exist yet,
  and dropped from `config.toml` at its next save.

## When the file is unreadable

If `config.toml` is not valid TOML, Xion does not overwrite it. It renames it to
`config.toml.corrupt-<timestamp>`, loads `config.toml.bak` instead when there is
one, and tells you both things in the status bar. Your broken file is still
there to fix or compare.

Saves are atomic: Xion writes a temporary file next to `config.toml`, flushes
it to disk, copies the old file to `config.toml.bak`, then renames the new one
into place. A crash or a power cut during a save leaves either the old file or
the new one, never half of it.

## The session file

`session.toml` holds the tabs Xion reopens at the next start. It is something
Xion produced rather than a choice you made, so it lives in the data folder,
apart from your settings.

```toml
active_tab_index = 0     # clamped to the tabs below

[[tabs]]
path = "/home/alice"
```

It is rewritten on every navigation and tab change. If it cannot be read, Xion
starts with a single tab and replaces it at the next save. Deleting it is
harmless.

## Keyboard shortcuts

The sixteen actions in `[shortcuts]` can be rebound. A value is zero or more
modifiers joined by `+`, then a key:

- modifiers: `Ctrl` (or `Control`), `Alt`, `Shift`;
- named keys: `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`,
  `Enter`, `Escape` (or `Esc`), `Tab`, `Space`, `F2`, `F3`, `F5`, `Delete` (or
  `Del`);
- any other single character, taken literally.

Anything else is rejected with a warning, and that action keeps its default.

These are fixed and cannot be rebound:

| Keys | Action |
|---|---|
| Shift+Delete | delete permanently, after a confirmation |
| F5 | refresh, like the `refresh` shortcut |
| Ctrl+1 to Ctrl+9 | open the favourite at that position |
| Ctrl+Shift+C | copy the path of the selection |
| a printable key on its own | type into the quick filter |

The last one is why a shortcut cannot be a bare character without a modifier:
typing it filters the list instead.

## Known limits

- The interface does not show where `config.toml` is. The table at the top of
  this page is the answer.
- If the system gives no configuration folder at all (no home directory, for
  instance), Xion falls back to a `config.toml` in the folder it was started
  from.
