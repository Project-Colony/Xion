# Routing UI : modèle de navigation

Document réécrit le 2026-08-24 à partir de `src/ui/mod.rs` et
`src/ui/app/state.rs`. La version précédente décrivait des structures qui
n'existent plus (`Route.path`, `NavigationState.selection: Option<PathBuf>`,
`SelectEntry(path)`) : du code écrit contre elle ne compilait pas.

> Les références sont données par nom de symbole plutôt que par numéro de
> ligne : un numéro devient faux au commit suivant. Chaque référence se contrôle
> par un `grep`.

## Panneaux

```rust
pub enum PaneKind { Tree, List, Preview }   // src/ui/mod.rs
```

- **Tree** : arborescence latérale (racines, lecteurs, favoris).
- **List** : contenu du dossier courant.
- **Preview** : aperçu de l'entrée focalisée.

`cycle_focus` fait tourner le focus dans l'ordre Tree → List → Preview → Tree
(`src/ui/app/navigation.rs`).

## Route

Une route n'est plus un chemin : c'est un panneau plus un **genre** de
destination.

```rust
pub enum RouteKind {                        // src/ui/mod.rs
    Local(PathBuf),
    Network,
    Recent,
}

pub struct Route {                          // src/ui/mod.rs
    pub pane: PaneKind,
    pub kind: RouteKind,
}
```

Le champ `path` n'existe plus. Pour obtenir un chemin, on passe par les
accesseurs de `Route` (`src/ui/mod.rs`) :

| Méthode           | Rôle                                                        |
| ----------------- | ----------------------------------------------------------- |
| `local_path()`    | `Some(&PathBuf)` seulement pour `RouteKind::Local`           |
| `key()`           | clé de cache : le chemin, ou `network://`, ou `recent://`    |
| `address_label()` | ce qu'affiche la barre d'adresse                             |
| `display_label()` | libellé humain (« Réseau », « Récents »)                     |
| `is_network()` / `is_recent()` | tests de genre                               |

Les deux pseudo-chemins sont des constantes : `NETWORK_ROUTE = "network://"`
(`src/ui/mod.rs`) et `RECENT_ROUTE = "recent://"` (`src/ui/mod.rs`).
`route_kind_from_path` (`src/ui/app/types.rs`) reconstruit un
`RouteKind` à partir d'un chemin en testant ces préfixes.

## État UI

```rust
pub struct AppState {                       // src/ui/mod.rs
    pub config: AppConfig,
    pub route: Route,
    pub navigation: NavigationState,
}

pub struct NavigationState {                // src/ui/mod.rs
    pub focused_pane: PaneKind,
    pub selection: SelectionState,
}

pub struct SelectionState {                 // src/ui/mod.rs
    pub selected: HashSet<PathBuf>,
    pub focused: Option<PathBuf>,
    pub anchor: Option<PathBuf>,
}
```

La sélection est donc un **ensemble**, pas une option. Les trois champs sont
distincts et servent chacun à autre chose :

- `selected` : tout ce qui est sélectionné, ce sur quoi portent les actions ;
- `focused` : l'entrée sous le curseur clavier, celle qu'affiche l'aperçu ;
- `anchor` : le point de départ d'une sélection par plage (Maj+clic,
  Maj+flèches).

### La route vit dans l'onglet actif

`AppState.route` n'est pas la source de vérité : c'est une projection de
l'onglet actif. `update_active_tab_path` (`src/ui/app/state.rs`) écrit
le chemin dans `TabManager`, en dérive `route.kind`, met à jour la barre
d'adresse et resynchronise l'observateur de fichiers. Changer d'onglet change
donc de route.

## Messages

`UiMessage` compte plus de 150 variantes (`src/ui/mod.rs`). Les énumérer ici
serait périmé au commit suivant : la référence est l'enum lui-même. Les quatre
qui portent la navigation :

```rust
NavigateTo(PathBuf),                        // src/ui/mod.rs
FocusPane(PaneKind),                        // src/ui/mod.rs
SelectEntry { path: PathBuf, kind: SelectionKind },   // src/ui/mod.rs
ActivateEntry(PathBuf),                     // src/ui/mod.rs
```

`SelectEntry` n'est plus un tuple : il porte un `SelectionKind`
(`src/ui/mod.rs`) qui décide du comportement.

```rust
pub enum SelectionKind { Single, Toggle, Range }
```

- `Single` : remplace la sélection.
- `Toggle` : ajoute ou retire (Ctrl+clic).
- `Range` : étend depuis `anchor` (Maj+clic).

`selection_kind_from_modifiers` (`src/ui/app/navigation.rs`) déduit le
genre des modificateurs enfoncés.

Les commandes clavier passent par un enum séparé, `KeyboardCommand`
(`src/ui/mod.rs`), traduit depuis les touches par
`command_for_key` dans `src/ui/app/helpers.rs`.

## Règles de navigation

1. `NavigateTo` appelle `navigate_to` (`src/ui/app/state.rs`) :
   met à jour l'onglet actif, enregistre le chemin dans `HistoryService`,
   persiste les onglets, puis relance `refresh_entries`.
2. `refresh_entries` (`src/ui/app/state.rs`) remet à zéro la sélection,
   le filtre rapide, la recherche, l'état de glisser-déposer et le défilement.
   La sélection ne survit donc pas à un changement de dossier.
3. `FocusPane` ne modifie que `navigation.focused_pane`
   (`src/ui/app/update.rs`).
4. `SelectEntry` ne change pas de dossier ; elle met à jour `selected`,
   `focused` et éventuellement `anchor` via `apply_selection`
   (`src/ui/app/navigation.rs`).

### Sélection et filtre

`select_all_entries` (`src/ui/app/navigation.rs`) et la sélection par plage
ne portent que sur les entrées **visibles** : si un filtre rapide ou une
recherche est actif, les entrées masquées ne sont jamais sélectionnées. C'est
couvert par `quick_filter_select_all_ignores_hidden_entries`,
`search_query_select_all_ignores_hidden_entries`,
`shift_click_range_respects_filter` et `arrow_keys_walk_visible_rows_only`
(`src/ui/app/navigation.rs`).

## Historique

- Géré par `services::HistoryService`.
- Chaque `navigate_to` appelle `history.record` (`src/ui/app/state.rs`),
  ce qui tronque l'historique « avant ».
- `Back` / `Forward` (`src/ui/app/update.rs`) réappliquent le chemin via
  `update_active_tab_path` puis `refresh_entries` — sans repasser par
  `record`, donc sans écraser l'historique.
- La barre d'adresse propose les entrées d'historique en suggestions
  (`address_suggestions`, `src/ui/app/state.rs`, six au maximum).

## Rafraîchissement automatique

Une souscription émet `FileWatchTick` toutes les 750 ms
(`src/ui/app/mod.rs`). Les évènements de `NativeFileWatcher` sont filtrés
par `is_event_relevant` (`src/ui/app/navigation.rs`) : seuls ceux qui
concernent le dossier surveillé provoquent un relistage.

## État d'implémentation

- [x] Messages UI branchés sur une boucle Iced.
- [x] Commandes clavier/souris standard (Back/Forward, sélection, F5, Ctrl+R).
- [x] Virtualisation de la liste — `services::VirtualList`.
- [x] Arbre latéral connecté au routing — `build_tree_nodes`
      (`src/ui/app/helpers.rs`), cache `cached_tree_nodes`
      (`src/ui/app/mod.rs`), reconstruction `src/ui/app/state.rs`.
- [x] Barre d'adresse éditable + historique — champs `address_input`
      et `address_editing` de `XionApp` (`src/ui/app/mod.rs`), suggestions par
      `address_suggestions` (`src/ui/app/state.rs`).
- [x] Glisser-déposer entre panneaux + multi-sélection.
- [x] Prévisualisation enrichie (métadonnées, image, texte).
- [ ] `RouteKind::Recent` et `RouteKind::Network` n'ont pas d'équivalent pour la
      corbeille : `NavigateToTrash` liste les éléments, mais `TrashListLoaded`
      se contente d'écrire un compteur dans la barre d'état
      (`src/ui/app/update.rs`).
- [ ] Aucun test ne couvre `Route` ni `SelectionState` directement ; ils ne sont
      testés qu'indirectement, via `tests/integration_tests.rs` (mod `ui_update`).
