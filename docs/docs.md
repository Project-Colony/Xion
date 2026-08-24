# Documentation Xion

## But du projet

Un explorateur de fichiers en Rust, inspiré de Windows File Explorer, Files et
Dolphin.

## Règles persistantes

Elles sont énoncées ici et **suivies** dans `tasks/tasks.md`, qui indique pour
chacune si elle est tenue et par quel mécanisme.

- Rust édition 2024.
- Dépendances, `Cargo.toml` et `Cargo.lock` maintenus à jour.
- Projet léger en RAM et en CPU, et réactif.
- Architecture modulaire : séparer les modules par domaine et responsabilité.
- Éviter le hardcode : préférer la configuration, les options et les extensions.

## Décisions techniques

- **Pile UI** : Iced 0.14 + winit + wgpu.
- **Terminal intégré** : `portable-pty` (ConPTY sous Windows, `openpty` sous
  Unix) et `vte` pour l'interprétation des séquences d'échappement. Le modèle
  d'écran est un historique de lignes, pas une grille : les applications plein
  écran (`vim`, `htop`) sont hors périmètre.
- **Observateur de fichiers** : `notify`, via `NativeFileWatcher`.
- **Cibles** : `x86_64-unknown-linux-gnu` et `x86_64-pc-windows-msvc`.
- **Élévation Windows** : `asInvoker`. Xion ne s'élève pas ; l'intégration au
  menu contextuel n'écrit que sous `HKEY_CURRENT_USER`.
- **Style UI** : interface native moderne, navigation clavier et souris
  complète.

## Principes d'architecture

- Séparation par domaine :
  - `core/` : types partagés, erreurs, configuration.
  - `filesystem/` : accès FS, watch, métadonnées, opérations.
  - `services/` : recherche, miniatures, historique, favoris, réseau.
  - `platform/` : intégration OS, un backend par cible.
  - `terminal` : session shell sur pseudo-terminal.
  - `ui/` : composants Iced, état, routing, rendu.
- Chaque module expose une API claire et testable.

Les écarts entre ces principes et le code sont listés dans
`docs/architecture.md`, section « Écarts constatés ». Ils sont réels : l'UI
appelle `std::fs` directement, et `core` dépend de `ui`.

## Principes de performance

- Chargement paresseux des dossiers et des métadonnées.
- Virtualisation des listes et de l'arborescence.
- Caches ciblés (métadonnées, miniatures, dossiers, index de recherche), tous
  bornés en taille et en durée de vie.
- Batching des appels filesystem.
- Travail bloquant systématiquement déporté hors de la boucle de rendu, via
  `tokio::task::spawn_blocking`.

## Conventions de configuration

- Configuration versionnée en TOML, migrée et validée.
- Valeurs par défaut minimales, extensibles.
- Voir `docs/config.md` pour le format, les chemins réels et la validation.

## Qualité

Tenue par `.github/workflows/ci.yml`, en matrice ubuntu + windows :
`cargo fmt --all --check`, `cargo clippy --all-targets --all-features
-- -D warnings`, `cargo test --all-targets --all-features`, et un contrôle
d'absence de BOM UTF-8. Un job séparé lance
`cargo deny check advisories bans licenses sources`. Le tout est rejoué chaque
lundi par un déclencheur `schedule`.

## Roadmap documentaire

- [x] Architecture des modules : responsabilités et API principales.
- [x] Stratégies de cache (métadonnées, miniatures, dossiers, index).
- [x] Indexation et recherche.
- [x] Modèle de navigation et de sélection (`docs/ui-routing.md`).
- [ ] Diagrammes d'interaction (modules et flux de données).
- [ ] Cycle de rendu Iced et modèle d'état détaillé.
- [ ] Rattacher les exemples de code de `docs/` au crate, pour que
      `cargo test --doc` les compile et qu'ils ne puissent plus pourrir.

## Sur la relecture des documents

Il n'y a plus de section « vérification documentaire » avec des cases à cocher.
Une case cochée ne prouve pas qu'une relecture a eu lieu : la version
précédente de ce fichier certifiait que trois documents étaient « revus et
alignés sur l'implémentation actuelle » alors que deux d'entre eux décrivaient
des structures et des chemins qui n'existaient plus.

À la place, chaque document porte en tête la date à laquelle il a été confronté
au code, et cite ses sources (`fichier:ligne` ou nom de test). Les numéros de
ligne sont un relevé daté ; en cas de divergence, c'est le nom du symbole cité
qui fait foi.

## Références internes

- `docs/architecture.md` : modules, responsabilités, écarts connus, terminal,
  sécurité, CI.
- `docs/ui-routing.md` : route, sélection, messages, historique.
- `docs/config.md` : chemins de configuration, format TOML complet, migration,
  validation, raccourcis.
- `docs/search.md` : indexation, filtres, intégration UI asynchrone.
- `tasks/roadmap.md` : source unique de l'état d'avancement.
- `tasks/tasks.md` : obligations permanentes et suivi documentaire.
- `AUDIT.txt` : items d'audit avec leurs preuves.
