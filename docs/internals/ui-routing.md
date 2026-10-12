# UI routing: panes, routes and selection

How the interface knows where it is, what is selected and where it goes next.
The types are in `src/ui/mod.rs`; the logic is in `src/ui/app/state.rs` and
`src/ui/app/navigation/`.

## Panes

```rust
pub enum PaneKind { Tree, List, Preview }
```

- **Tree**: the sidebar (drives, favourites, quick access, the folder tree).
- **List**: the content of the current folder.
- **Preview**: the preview of the focused entry.

`cycle_focus` (`src/ui/app/navigation/focus.rs`) moves the keyboard focus
Tree, List, Preview, then back to Tree.

## Routes

A route is a pane plus a *kind* of destination, not a path:

```rust
pub enum RouteKind {
    Local(PathBuf),
    Network,
    Recent,
}

pub struct Route {
    pub pane: PaneKind,
    pub kind: RouteKind,
}
```

To get a path, go through the accessors on `Route`:

| Method | Returns |
|---|---|
| `local_path()` | `Some(&PathBuf)` for `RouteKind::Local` only |
| `key()` | the cache key: the path, `network://` or `recent://` |
| `address_label()` | what the address bar shows |
| `display_label()` | the path, or the translated name of Network or Recent |
| `is_network()`, `is_recent()` | which kind it is |

The two pseudo-paths are the constants `NETWORK_ROUTE` (`network://`) and
`RECENT_ROUTE` (`recent://`). `route_kind_from_path` (`src/ui/app/types.rs`)
turns a path typed or stored somewhere back into a `RouteKind` by checking for
those prefixes.

## State

```rust
pub struct AppState {
    pub config: AppConfig,
    pub route: Route,
    pub navigation: NavigationState,
}

pub struct NavigationState {
    pub focused_pane: PaneKind,
    pub selection: SelectionState,
}

pub struct SelectionState {
    pub selected: HashSet<PathBuf>,
    pub focused: Option<PathBuf>,
    pub anchor: Option<PathBuf>,
}
```

The selection is a set, and its three fields do different jobs:

- `selected`: everything selected, what an action applies to;
- `focused`: the entry under the keyboard cursor, the one the preview shows;
- `anchor`: where a range selection starts (Shift+click, Shift+arrows).

### The route belongs to the active tab

`AppState.route` is not the source of truth; it mirrors the active tab.
`update_active_tab_path` (`src/ui/app/state.rs`) writes the path into the tab,
derives `route.kind` from it, updates the address bar and points the file
watcher at the new folder. Switching tabs therefore switches routes.

## Messages

Four `UiMessage` variants carry navigation:

```rust
NavigateTo(PathBuf),
FocusPane(PaneKind),
SelectEntry { path: PathBuf, kind: SelectionKind },
ActivateEntry(PathBuf),
```

`SelectionKind` decides what a selection does:

```rust
pub enum SelectionKind { Single, Toggle, Range }
```

- `Single` replaces the selection;
- `Toggle` adds or removes one entry (Ctrl+click);
- `Range` extends from `anchor` (Shift+click).

`selection_kind_from_modifiers` (`src/ui/app/navigation/rubber_band.rs`) picks
the kind from the modifier keys held.

Keyboard actions go through a separate enum, `KeyboardCommand`, built from a
key press and the configured shortcuts by
`command_from_key_press_with_shortcuts` (`src/ui/app/helpers.rs`).

## Navigation rules

1. `NavigateTo` calls `navigate_to` (`src/ui/app/state.rs`): it updates the
   active tab, records the path in `HistoryService`, saves the session, then
   calls `refresh_entries`.
2. `refresh_entries` clears the selection, the quick filter, the search, any
   drag in progress and the scroll position. A selection never survives a
   change of folder.
3. `FocusPane` only changes `navigation.focused_pane`.
4. `SelectEntry` never changes folder. It updates `selected`, `focused` and,
   depending on the kind, `anchor`, through `apply_selection`
   (`src/ui/app/navigation/mod.rs`).

### Selection and filters

Select all (`select_all_entries`) and range selection only ever reach
**visible** entries: with a quick filter or a search active, hidden entries are
never selected. Tests: `quick_filter_select_all_ignores_hidden_entries`,
`search_query_select_all_ignores_hidden_entries`,
`shift_click_range_respects_filter` and `arrow_keys_walk_visible_rows_only`
(`src/ui/app/navigation/mod.rs`).

## History

- `services::HistoryService` keeps it.
- Every `navigate_to` calls `history.record`, which drops the forward entries.
- Back and Forward (`src/ui/app/update/navigation.rs`) apply the path through
  `update_active_tab_path` and `refresh_entries` without calling `record`, so
  they move through the history instead of rewriting it.
- The address bar offers up to six history entries as suggestions
  (`address_suggestions`, `src/ui/app/state.rs`).

## Automatic refresh

A subscription sends `FileWatchTick` every 750 ms (`src/ui/app/windowing.rs`).
`is_event_relevant` (`src/ui/app/navigation/buffers.rs`) keeps only the watcher
events about the folder on screen, and those trigger a new listing.

## Gaps

- The trash has no route of its own. `NavigateToTrash` lists the trashed items,
  but `TrashListLoaded` only reports their number in the status bar
  (`src/ui/app/update/navigation.rs`).
- No test covers `Route` or `SelectionState` directly; they are only exercised
  through `tests/integration_tests.rs` (module `ui_update`).
