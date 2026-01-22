# Routing UI : modèle de navigation

## Objectif

Définir une navigation cohérente et testable pour un explorateur moderne : arbre latéral,
contenu de dossier et panneau de prévisualisation.

## Panes et layout

- **Tree** : navigation par arborescence (racines, lecteurs, favoris).
- **List** : contenu du dossier courant (tri, colonnes, sélection).
- **Preview** : aperçu des fichiers (métadonnées, miniatures, détails).

Chaque pane est adressable via `PaneKind` et peut devenir le focus principal.

## Route

```
Route {
  pane: PaneKind,
  path: PathBuf
}
```

- `pane` : indique le panneau actif.
- `path` : dossier courant pour la liste et la preview.

## État UI minimal

- `AppState` contient :
  - `config` : options runtime.
  - `route` : contexte de navigation.
  - `navigation` : focus et sélection.

```
NavigationState {
  focused_pane: PaneKind,
  selection: Option<PathBuf>
}
```

## Messages UI (intention)

- `NavigateTo(path)` : ouverture d'un nouveau dossier.
- `FocusPane(pane)` : déplacer le focus vers un panneau.
- `SelectEntry(path)` : sélectionner une entrée.

## Règles de navigation

1. `NavigateTo` met à jour `route.path` et réinitialise la sélection.
2. `FocusPane` ne modifie pas la route, seulement le focus.
3. `SelectEntry` ne change pas de dossier, elle prépare la preview.

## Historique

- L'historique est géré côté `services::HistoryService`.
- Chaque `NavigateTo` enregistre un nouveau chemin.
- `Back` et `Forward` restaurent un chemin et réhydratent l'UI.

## Prochaines étapes

- Brancher les messages UI à une boucle Iced.
- Ajouter des commandes clavier/souris standard (Back/Forward, Ctrl+L, etc.).
- Introduire la virtualisation pour la liste lorsque le volume est important.
