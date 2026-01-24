# Recherche avancée et indexation

Ce document décrit la recherche avancée introduite dans Xion, basée sur une indexation locale
et des filtres composables.

## Objectifs

- Proposer une indexation réutilisable pour les recherches (même hors UI).
- Permettre des filtres avancés : type d'entrée, extensions, taille, dates.
- Garder l'API indépendante de l'UI pour rester testable.

## Composants principaux

### `SearchService`

Le service central pour construire un index et exécuter des recherches filtrées.

Principales capacités :

- Construire un index réutilisable (`build_index`).
- Filtrer l'index via une requête (`search_index`).
- Appliquer une pagination (`search_index_paged`).

### `SearchIndex`

Un index en mémoire qui conserve les entrées et leurs métadonnées, ainsi que des
champs normalisés pour des comparaisons rapides.

- `root` : racine indexée.
- `entries()` : itérateur sur les entrées indexées.

### `SearchIndexOptions`

Options appliquées lors de l'indexation :

- `include_hidden` : inclure les fichiers cachés.
- `recursive` : indexer récursivement les sous-dossiers.

### `SearchQuery`

Filtres et paramètres de la recherche :

- `text` : texte à rechercher dans les noms (insensible à la casse par défaut).
- `extensions` : liste d'extensions autorisées (ex. `"rs"`, `"png"`).
- `entry_filter` : type d'entrée (tout, dossiers uniquement, fichiers uniquement).
- `min_size` / `max_size` : filtres de taille en octets.
- `modified_after` / `modified_before` : filtres temporels via `SystemTime`.
- `case_sensitive` : activer la casse stricte.

## Exemple d'utilisation (service)

```rust
use std::time::SystemTime;
use xion::filesystem::{EntryFilter, LocalFileSystem};
use xion::services::{SearchIndexOptions, SearchQuery, SearchService};

let filesystem = LocalFileSystem::new();
let service = SearchService::default();

let index = service.build_index(
    &filesystem,
    std::path::Path::new("/tmp"),
    SearchIndexOptions {
        include_hidden: false,
        recursive: true,
    },
)?;

let query = SearchQuery {
    text: Some("rapport".into()),
    extensions: vec!["pdf".into()],
    entry_filter: EntryFilter::OnlyFiles,
    min_size: Some(10_000),
    max_size: None,
    modified_after: Some(SystemTime::UNIX_EPOCH),
    modified_before: None,
    case_sensitive: false,
};

let results = service.search_index(&index, &query);
```

## Notes d'intégration

- L'indexation est synchrone pour le moment : l'intégration UI devra encapsuler
  les appels dans un loader ou un worker pour éviter les blocages.
- Les filtres sont cumulés (ET logique).
- Les entrées sans `modified` sont exclues si un filtre temporel est défini.
