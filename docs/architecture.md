# Architecture modulaire Xion

## Objectifs

- Séparer les responsabilités par domaines (core/filesystem/services/ui).
- Préparer un backend FS testable, indépendant de l'UI.
- Rendre les comportements configurables et extensibles.

## Vue d'ensemble des modules

```
core/        -> types partagés, config, erreurs
filesystem/  -> accès FS, métadonnées, indexation
services/    -> logique métier (historique, favoris, recherche, thumbnails)
ui/          -> état UI, routing, composants Iced
```

## core/

**Responsabilité** : types stables et partagés entre les modules.

- `AppConfig` : configuration runtime (chemin de départ, options d'affichage).
- `XionError` : erreurs unifiées pour les modules haut niveau.
- `AppResult` : alias pour la propagation d'erreurs.

> Règle : aucune dépendance sur `filesystem/`, `services/`, `ui/`.

## filesystem/

**Responsabilité** : accès au système de fichiers et normalisation des données.

- `FileSystem` : trait pour injecter une implémentation (locale, mock, future API).
- `LocalFileSystem` : implémentation OS locale.
- `FsEntry` : entrée unifiée (nom, chemin, type, métadonnées).
- `ListOptions` : options runtime (tri, filtrage, masquage des fichiers cachés).
- `Page`/`PageRequest` : pagination pour listes volumineuses.
- `DirectoryCache`/`MetadataCache` : caches TTL pour répertoires et métadonnées.
- `FileWatcher` : abstraction de surveillance (implémentation no-op pour le proto).

**Invariants**

- Les appels sont synchrones côté prototype, mais l'API doit rester extensible.
- Les listes retournées sont triées et filtrées (fichiers cachés via options).

## services/

**Responsabilité** : logique métier transverse.

- `HistoryService` : gestion de l'historique de navigation (back/forward).
- `FavoritesService` : favoris rapides.
- `SearchService` : recherche initiale (filtrage par nom).
- `DirectoryLoader` : orchestration des pages + caches (dossiers/métadonnées).
- `ThumbnailService` : cache et génération des miniatures (prépare la phase perf).

## ui/

**Responsabilité** : état UI, routing et messages.

- `AppState` : état racine, point d'entrée côté UI.
- `Route` : contexte de navigation (chemin + panneau actif).
- `PaneKind` : arbre / liste / preview.
- `UiMessage` : messages d'interaction (navigation, focus, sélection).

## Flux de données

```
UI (messages) -> AppState -> services (historique) -> filesystem (listes)
                              ^
                              | configuration / core
```

## Décisions clés

- `filesystem` est découplé de l'UI pour faciliter la virtualisation.
- `services` coordonne la logique (navigation, cache, recherche).
- `ui` n'a pas accès direct au disque : elle demande au `filesystem`.

## Prochaines itérations

1. Brancher les services dans `AppState`.
2. Introduire la virtualisation de listes dans l'UI.
3. Passer à une boucle Iced pour intégrer le routing réel.
