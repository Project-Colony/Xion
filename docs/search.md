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

Trois méthodes publiques, réparties en deux familles.

Il y en avait huit. Cinq — `search_in_dir_paged`, `search_files_only`,
`build_index`, `search_index`, `search_index_paged` — n'étaient appelées de
nulle part : ni par l'interface, ni par les tests, ni par les exemples. Elles
ne décrivaient donc pas une capacité du logiciel, seulement une intention. Les
trois qui restent sont celles qui tournent réellement.

**Recherche directe, sans index** — relit le dossier à chaque appel :

| Méthode          | Rôle                           |
| ---------------- | ------------------------------ |
| `search_in_dir`  | filtre par nom dans un dossier |

**Recherche sur index** — construit une fois, interrogé plusieurs fois :

| Méthode                    | Rôle                                   |
| -------------------------- | -------------------------------------- |
| `build_index_with_options` | indexe une racine, `ListOptions` imposées |
| `count_index_matches`      | compte sans matérialiser les résultats |

Le filtre lui-même vit dans `SearchService::matching`, une seule fois : les
trois interrogations d'index en portaient chacune leur copie.

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

let index = service.build_index_with_options(
    &filesystem,
    std::path::Path::new("/tmp"),
    SearchIndexOptions {
        include_hidden: false,
        recursive: true,
        // Sans cette ligne : E0063, le champ `max_entries` manque.
        ..SearchIndexOptions::default()
    },
    ListOptions::default(),
)?;

let query = SearchQuery {
    text: Some("rapport".into()),
    extensions: vec!["pdf".into()],
    entry_filter: EntryFilter::OnlyFiles,
    min_size: Some(10_000),
    ..SearchQuery::default()
};

let matches = service.count_index_matches(&index, &query);
```

Ce bloc n'est compilé par aucun test : `cargo test --doc` ne voit pas les
fichiers de `docs/`, faute d'un `#[doc = include_str!(...)]` qui les rattache au
crate. C'est une faiblesse connue de ce document.

## Intégration UI

L'indexation est **asynchrone**, contrairement à ce qu'affirmait la version
précédente de ce document.

`XionApp::start_search_indexing` (`src/ui/app/state.rs`) :

1. ne fait rien s'il n'y a pas de requête. L'index est une seconde copie du
   dossier — 12,4 Mo pour 50 000 entrées, mesuré, plus que le listage — et il
   était auparavant construit à chaque navigation, qu'on cherche ou non ;
2. ne fait rien non plus si un index du même dossier est déjà présent ou en
   cours de construction : l'indexation est déclenchée par la frappe, donc
   sans ce garde-fou taper « rapport » lançait sept parcours concurrents ;
3. sinon, lance un `Task::perform` qui appelle `build_index_with_options` hors
   de la boucle de rendu, et signale l'état via le drapeau `search.indexing` ;
4. le résultat revient par `UiMessage::SearchIndexBuilt { path, result }`, qui
   n'est appliqué que si la route n'a pas changé entre-temps.

Il n'y a plus de cache LRU. Il en gardait huit, soit huit copies complètes de
dossier, pour un cas d'usage étroit — chercher dans A, partir, revenir,
rechercher — et il pouvait servir un index périmé quand les fichiers avaient
changé entre-temps.

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

Non couvert aujourd'hui : `max_entries`, `MAX_SEARCH_DEPTH` et le cumul des
filtres.

## Avancement

- [x] Index réutilisable en mémoire — `build_index_with_options`.
- [x] Filtres composables — `SearchQuery`.
- [x] Indexation asynchrone côté UI — `XionApp::start_search_indexing`.
- [x] Indexation paresseuse : construite à la première requête, pas à chaque navigation.
- [x] Recherche plein texte récursive — `UiMessage::FullTextSearchSubmit`.
- [ ] Tests unitaires de `SearchService`.
- [ ] Compilation de l'exemple ci-dessus par `cargo test --doc`.
- [ ] Indexation récursive dans l'index principal (aujourd'hui `recursive: false`).
