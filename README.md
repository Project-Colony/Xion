# Xion Explorer

Xion est un explorateur de fichiers écrit en Rust, inspiré de Windows File
Explorer, Files et Dolphin.

## Objectifs

- Construire un explorateur de fichiers moderne en Rust.
- Prioriser une empreinte **RAM minimale**, une **consommation CPU faible** et
  une **réactivité maximale**.
- Garder une architecture **modulaire** (modules séparés par domaine).
- Ne rien coder en dur : les comportements passent par la configuration.

## Exigences techniques

- Rust **édition 2024**, chaîne d'outils épinglée à 1.98.0
  (`rust-toolchain.toml`).
- Cibles suivies : `x86_64-unknown-linux-gnu` et `x86_64-pc-windows-msvc`.

## Stack

- **UI** : Iced 0.14
- **Fenêtrage/événements** : winit
- **Rendu** : wgpu
- **Terminal intégré** : portable-pty + vte
- **Observateur de fichiers** : notify

## Structure du dépôt

- `src/` : le code, découpé en `core`, `filesystem`, `services`, `platform`,
  `terminal` et `ui`.
- `docs/` : documentation technique.
- `tasks/` : suivi des tâches et de la roadmap.
- `.github/workflows/ci.yml` : intégration continue.

`tasks/roadmap.md` est la **source unique** de l'état d'avancement. Cette page
n'en donne qu'un résumé ; en cas de divergence, c'est la roadmap qui fait foi,
et la roadmap est elle-même tenue à jour d'après le code.

## Ce qui fonctionne

- Navigation par onglets, historique back/forward, barre d'adresse éditable avec
  suggestions.
- Panneau latéral : lecteurs, favoris, quick access, arborescence.
- Vue Liste et vue Grille, colonnes configurables et redimensionnables, tri par
  colonne.
- Opérations de fichiers complètes : copier, coller, déplacer, renommer,
  supprimer, avec barre de progression et annulation de la dernière action.
- Suppression via la **corbeille** ; `Maj+Suppr` supprime définitivement, après
  confirmation.
- Archives : ouverture, extraction (complète ou d'une seule entrée), création de
  `.zip`, `.tar.gz` et `.7z`. L'extraction refuse toute entrée qui sortirait du
  dossier de destination.
- Recherche : filtre par nom sur index en mémoire (asynchrone, avec cache), et
  recherche plein texte récursive respectant `.gitignore` en option.
- Prévisualisation : métadonnées, image, texte coloré syntaxiquement, vue
  hexadécimale, diff entre deux fichiers.
- Terminal intégré sur pseudo-terminal réel.
- Configuration TOML versionnée, migrée, validée et rechargeable à chaud.
- Thèmes (Light, Dark, Nord, Solarized, HighContrast), mode compact, étiquettes
  de couleur, statut Git par entrée.
- Rafraîchissement automatique par observateur de fichiers natif.

## Terminal intégré

Le terminal tourne sur un **vrai pseudo-terminal** (`portable-pty` : ConPTY sous
Windows, `openpty` sous Unix), et les octets passent par un analyseur `vte`.
Concrètement :

- il est multiplateforme ;
- Ctrl-C interrompt réellement la commande au premier plan ;
- le redimensionnement du panneau est propagé au shell ;
- les couleurs et les barres de progression (`\r`) s'affichent correctement.

**Limite assumée** : le modèle d'écran est un historique de lignes, pas une
grille plein écran. Les applications plein écran comme `vim`, `htop` ou `less`
en mode interactif ne s'affichent pas correctement. Ce n'est pas un bug.

## Sécurité

- Le manifeste Windows demande `asInvoker` : Xion ne s'élève pas en
  administrateur. Aucune écriture `HKEY_LOCAL_MACHINE` n'existe dans le code, et
  une élévation permanente contaminerait tous les processus lancés depuis
  l'application.
- L'intégration au menu contextuel (`xion --register` / `--unregister`) n'écrit
  que sous `HKEY_CURRENT_USER`.
- Les suppressions passent par la corbeille par défaut.

## Intégration continue

`.github/workflows/ci.yml` s'exécute sur `ubuntu-latest` et `windows-latest` :

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- contrôle d'absence de BOM UTF-8
- job séparé `cargo deny check advisories bans licenses sources`, rejoué chaque
  lundi pour faire remonter les advisories publiées entre deux pushs.

## Chantiers ouverts

Détail et justification dans `tasks/roadmap.md`.

- Corbeille navigable (elle est listée, pas parcourable).
- FTP et connexion aux partages authentifiés : les fonctions
  `list_ftp_directory` et `connect_authenticated` existent dans
  `services/network.rs` mais ne sont branchées sur aucune commande de
  l'interface. SFTP n'est pas supporté.
- Indicateur d'état réseau : l'enum `NetworkStatus` existe et n'est lu nulle
  part dans `src/ui/`.
- Découpage de `src/ui/app/view/mod.rs` (3174 lignes pour une seule fonction) et
  de `src/ui/app/update.rs`.
- Couverture de tests des modules purs non couverts : `services/search.rs`,
  `core/config/shortcuts.rs`, `filesystem/watcher.rs`.
- Mise à jour des dépendances et surveillance automatisée (pas de
  `dependabot.yml`).

## Documentation

- `docs/architecture.md` : modules, responsabilités, écarts connus.
- `docs/ui-routing.md` : modèle de navigation, route, sélection, messages.
- `docs/config.md` : format TOML, chemins réels, migration, validation.
- `docs/search.md` : indexation et filtres.
- `docs/docs.md` : règles persistantes et décisions techniques.
- `tasks/roadmap.md` : source unique de l'état d'avancement.
- `AUDIT.txt` : état vérifié des items d'audit, avec les preuves.
