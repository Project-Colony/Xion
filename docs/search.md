# Recherche et indexation

Document relu contre `src/services/search.rs` et `src/ui/app/state.rs` le
2026-08-24.

> Les numéros de ligne cités sont un relevé de cette date. En cas de
> divergence, c'est le nom du symbole ou du test qui fait foi.

## Objectifs

- Fournir une indexation réutilisable, utilisable hors UI.
- Permettre des filtres composables : type d'entrée, extensions, taille, dates.
- Garder l'API indépendante de l'UI pour rester testable.

## `SearchService`

Structure sans état (`#[derive(Debug, Default)] pub struct SearchService;`),
donc utilisable comme `SearchService` ou `SearchService::default()`.

Huit méthodes publiques, réparties en deux familles.

**Recherche directe, sans index** — relit le dossier à chaque appel :

| Méthode                | Rôle                                              |
| ---------------------- | ------------------------------------------------- |
| `search_in_dir`        | filtre par nom dans un dossier                    |
| `search_in_dir_paged`  | idem, paginé                                      |
| `search_files_only`    | idem, restreint aux fichiers                      |

**Recherche sur index** — construit une fois, interrogé plusieurs fois :

| Méthode                    | Rôle                                          |
| -------------------------- | --------------------------------------------- |
| `build_index`              | indexe une racine avec des `ListOptions` par défaut |
| `build_index_with_options` | idem, en imposant les `ListOptions`           |
| `search_index`             | filtre l'index, renvoie un `Vec<FsEntry>`     |
| `search_index_paged`       | filtre et pagine en une seule passe           |
| `count_index_matches`      | compte sans matérialiser les résultats        |

La récursion est bornée par `MAX_SEARCH_DEPTH = 32`
(`src/services/search.rs`), pour éviter un débordement de pile sur une
arborescence profonde ou cyclique.

## `SearchIndex`

Index en mémoire conservant les entrées et des champs normalisés pour comparer
vite.

- `root: PathBuf` : racine indexée (champ public).
- `len()` / `is_empty()` : taille de l'index.
- `entries()` : itérateur sur les `&FsEntry`.

## `SearchIndexOptions`

Trois champs — le troisième est souvent oublié :

- `include_hidden: bool` — inclure les fichiers cachés. Défaut : `false`.
- `recursive: bool` — descendre dans les sous-dossiers. Défaut : `true`.
- `max_entries: usize` — plafond du nombre d'entrées indexées, pour borner la
  mémoire. Défaut : **50 000**. La valeur `0` signifie « sans limite ».

`SearchIndexOptions` n'est pas `Default`-dérivé : les valeurs par défaut sont
écrites à la main (`impl Default for SearchIndexOptions`). Construire la
structure sans `..SearchIndexOptions::default()` provoque une erreur de
compilation `E0063: missing field max_entries`.

## `SearchQuery`

- `text: Option<String>` — texte cherché dans les noms.
- `extensions: Vec<String>` — extensions autorisées (`"rs"`, `"png"`…).
- `entry_filter: EntryFilter` — tout, dossiers seulement, fichiers seulement.
- `min_size` / `max_size: Option<u64>` — bornes en octets.
- `modified_after` / `modified_before: Option<SystemTime>` — bornes temporelles.
- `case_sensitive: bool` — défaut `false`.

Les filtres se cumulent en ET logique. Une entrée sans date de modification est
exclue dès qu'un filtre temporel est posé.

## Exemple

```rust
use std::time::SystemTime;
use xion::filesystem::{EntryFilter, LocalFileSystem};
use xion::services::{SearchIndexOptions, SearchQuery, SearchService};

let filesystem = LocalFileSystem::new();
let service = SearchService;

let index = service.build_index(
    &filesystem,
    std::path::Path::new("/tmp"),
    SearchIndexOptions {
        include_hidden: false,
        recursive: true,
        // Sans cette ligne : E0063, le champ `max_entries` manque.
        ..SearchIndexOptions::default()
    },
)?;

let query = SearchQuery {
    text: Some("rapport".into()),
    extensions: vec!["pdf".into()],
    entry_filter: EntryFilter::OnlyFiles,
    min_size: Some(10_000),
    ..SearchQuery::default()
};

let results = service.search_index(&index, &query);
```

Ce bloc n'est compilé par aucun test : `cargo test --doc` ne voit pas les
fichiers de `docs/`, faute d'un `#[doc = include_str!(...)]` qui les rattache au
crate. C'est une faiblesse connue de ce document.

## Intégration UI

L'indexation est **asynchrone**, contrairement à ce qu'affirmait la version
précédente de ce document.

`XionApp::start_search_indexing` (`src/ui/app/state.rs`) :

1. cherche d'abord la racine dans un cache LRU de 8 index
   (`search_index_cache`, éviction dans `src/ui/app/update.rs`,
   `MAX_SEARCH_CACHE = 8`) ; en cas de succès, aucun travail n'est fait ;
2. sinon, lance un `Task::perform` qui appelle `build_index_with_options` hors
   de la boucle de rendu, et signale l'état via le drapeau `search.indexing` ;
3. le résultat revient par `UiMessage::SearchIndexBuilt { path, result }`, qui
   n'est appliqué que si la route n'a pas changé entre-temps.

Deux points à connaître :

- l'UI force `recursive: false` : l'index couvre le dossier affiché, pas
  l'arborescence. La recherche récursive passe par un autre chemin,
  `UiMessage::FullTextSearchSubmit`, qui parcourt avec `ignore::WalkBuilder` et
  respecte éventuellement le `.gitignore` ;
- `include_hidden` est repris de la configuration de liste, pas fixé en dur.

Il n'y a donc plus rien à écrire côté worker : le code existe.

## Couverture de tests

`src/services/search.rs` ne contient **aucun** `#[cfg(test)]`. La seule
couverture est `search_service_text_filter` dans `tests/integration_tests.rs`,
qui exerce `search_in_dir` et pas les chemins d'index.

Non couvert aujourd'hui : `max_entries`, `MAX_SEARCH_DEPTH`, le cumul des
filtres, la pagination de `search_index_paged` et la cohérence entre
`count_index_matches` et `search_index`.

## Avancement

- [x] Index réutilisable en mémoire — `build_index`, `build_index_with_options`.
- [x] Filtres composables — `SearchQuery`.
- [x] Pagination — `search_index_paged`.
- [x] Indexation asynchrone côté UI — `XionApp::start_search_indexing`.
- [x] Cache LRU des index — `MAX_SEARCH_CACHE = 8`.
- [x] Recherche plein texte récursive — `UiMessage::FullTextSearchSubmit`.
- [ ] Tests unitaires de `SearchService`.
- [ ] Compilation de l'exemple ci-dessus par `cargo test --doc`.
- [ ] Indexation récursive dans l'index principal (aujourd'hui `recursive: false`).
