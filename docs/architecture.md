# Architecture modulaire Xion

Document relu contre le code le 2026-08-25. Les affirmations ci-dessous sont
appuyées par un fichier plus un symbole, ou par un nom de test ; celles qui ne
l'étaient pas ont été soit corrigées, soit déplacées dans la section « Écarts
constatés ».

> Les preuves sont données par nom de symbole plutôt que par numéro de ligne :
> un numéro devient faux au commit suivant. Chaque référence se contrôle par un
> `grep`. Les rares chiffres (lignes, décomptes) sont datés et accompagnés de la
> commande qui les recalcule.

## Objectifs

- Séparer les responsabilités par domaines.
- Garder un backend FS testable, indépendant de l'UI.
- Rendre les comportements configurables et extensibles.

## Vue d'ensemble des modules

`src/lib.rs` déclare six modules publics, plus un septième réservé à
Windows :

```
core/        -> types partagés, configuration, erreurs
filesystem/  -> accès FS, métadonnées, caches, opérations, observateur
services/    -> logique métier (historique, favoris, recherche, miniatures…)
platform/    -> intégration OS, un backend par cible
terminal     -> session shell sur pseudo-terminal (fichier unique)
ui/          -> état UI, routing, composants Iced
registry     -> menu contextuel Windows, sous #[cfg(windows)]
```

Le commentaire de `src/lib.rs` parle de « cinq couches » et range
`terminal` sous `services` : c'est inexact, `terminal` est un module de premier
niveau (`src/lib.rs`).

## core/

**Responsabilité** : types stables et partagés.

- `XionError` / `AppResult` : erreurs unifiées (`src/core/mod.rs`).
  La variante `Rejected` distingue un refus délibéré (copie d'un dossier dans
  lui-même, entrée d'archive qui s'échappe) d'une erreur d'E/S.
- `core::uri` : décodage pour-cent, partagé par les noms de montages gvfs et
  les URI (`src/core/uri.rs`).
- `core::config` : `AppConfig` et sa persistance, découpé en
  `types.rs` (structures), `shortcuts.rs` (`KeyChord`, `ShortcutBindings`) et
  `manager/` — lui-même en six fichiers : `mod.rs` (chemin, chargement,
  écriture atomique, tests), `file_format.rs` (les formes sur disque V0/V1),
  `migrate.rs` (les deux `merge_from_v*` et `config_to_file`), `validate.rs`
  (les `validated_*`), `limits.rs` (les bornes) et `shortcuts_io.rs`.
- Le chemin du fichier vient du socle partagé, `colony_ui::paths::locate`, et
  non d'un calcul local : `<racine>/Colony/Xion/config.toml` sur les trois
  plateformes, avec déménagement automatique depuis l'ancien `~/.config/xion/`.
  Détail dans `docs/config.md`.

> Règle visée : aucune dépendance sur `filesystem/`, `services/`, `ui/`.
> Elle est **violée** aujourd'hui, voir « Écarts constatés ».

## filesystem/

**Responsabilité** : accès au système de fichiers et normalisation.

- `FileSystem` / `LocalFileSystem` : trait et implémentation locale
  (`src/filesystem/access.rs`).
- `FsEntry`, `ListOptions`, `EntryFilter`, `SortKey`, `SortOrder`.
- `Page` / `PageRequest` : pagination (`src/filesystem/paging.rs`).
- `DirectoryCache` / `MetadataCache` / `TimedCache` : caches à TTL
  (`src/filesystem/cache.rs`).
- `LocalFileOperations` : `copy_items`, `move_items`, `rename_item`,
  `delete_items` (`src/filesystem/operations.rs`).
- `FileWatcher` : trait (`src/filesystem/watcher.rs`), avec deux
  implémentations :
  - `NativeFileWatcher`, bâti sur `notify::RecommendedWatcher`, en surveillance
    non récursive (`RecursiveMode::NonRecursive`), traduction des évènements par
    `map_kind` et file d'attente bornée par `MAX_QUEUED_EVENTS = 1_000` ;
  - `NoopFileWatcher`, utilisé uniquement quand la création du watcher natif
    échoue — le repli est dans le constructeur de `XionApp`
    (`src/ui/app/construction.rs`).

**Invariants**

- Les listes retournées sont triées et filtrées selon `ListOptions`.
- Les opérations de copie refusent une destination occupée et une copie dans un
  sous-dossier d'elle-même ; c'est couvert par
  `copy_into_own_subdirectory_is_rejected`,
  `copy_into_deep_own_subdirectory_is_rejected`,
  `copy_onto_itself_is_rejected`, `copy_rejects_an_occupied_destination`
  (`src/filesystem/operations.rs`).

## services/

**Responsabilité** : logique métier transverse (`src/services/mod.rs`).

- `HistoryService` : historique back/forward.
- `FavoritesService` : favoris.
- `DirectoryLoader` : orchestration pages + caches.
- `SearchService` : index en mémoire et filtres (voir `docs/search.md`).
- `ThumbnailService` et fonctions de génération (image, vidéo via ffmpeg,
  PDF via mutool/pdftoppm).
- `NetworkDiscoveryService` : découverte SMB et mDNS
  (`src/services/network/`, découpé en `wnet.rs`, `net_view.rs`, `mdns.rs`,
  `credentials.rs` et `ftp.rs`).
- `gvfs` : les emplacements montés par gvfs — partages SMB, SFTP, téléphones,
  Google Drive — retrouvés sous `$XDG_RUNTIME_DIR/gvfs`, où `gvfsd-fuse` les
  expose comme des dossiers ordinaires. Aucune liaison GIO : `LocalFileSystem`
  les lit tels quels. Branché dans le panneau latéral
  (`src/ui/app/view/sidebar_sections.rs`).
- `VirtualList` : fenêtre visible pour le rendu virtualisé.
- `highlight` : coloration syntaxique via syntect.

## platform/

**Responsabilité** : tout ce qui n'existe que sur une cible, derrière une API
uniforme, pour que le reste du crate n'écrive pas de `#[cfg]` en ligne
(`src/platform/mod.rs`). Le backend `unix` dégrade explicitement au lieu de
ne rien faire silencieusement.

## terminal

Session shell adossée à un vrai pseudo-terminal (`src/terminal.rs`). Voir la
section dédiée plus bas.

## ui/

**Responsabilité** : état, routing et rendu.

- `AppState` : `config`, `route`, `navigation` (`src/ui/mod.rs`).
- `Route` : `pane` + `kind` (`src/ui/mod.rs`) — voir
  `docs/ui-routing.md`.
- `PaneKind` : `Tree` / `List` / `Preview` (`src/ui/mod.rs`).
- `UiMessage` : plus de 150 variantes (`src/ui/mod.rs`). Les énumérer dans un
  document serait périmé au commit suivant ; l'enum est sa propre référence.
- `XionApp` : l'application Iced, découpée en fichiers simples — `mod.rs`,
  `state.rs`, `types.rs`, `helpers.rs`, `construction.rs`, `archive.rs`,
  `media.rs`, `paging.rs`, `permissions.rs`, `shell.rs`, `windowing.rs` — et en
  quatre sous-modules : `update/` (9 fichiers), `view/` (21), `operations/` (8)
  et `navigation/` (5).
- Les couleurs ne sont plus écrites dans Xion. `UiTokens::for_theme`
  (`src/ui/theme.rs`) résout un couple famille/variante dans le catalogue
  partagé de `colony-ui` — 25 familles, 57 variantes, 8 accents — et
  `UiColors::from_colony` en dérive les dix-neuf surfaces que Xion nomme. La
  palette arrive en argument plutôt que par `active_palette()` : `for_theme`
  tourne à chaque reconstruction de l'arbre de widgets, donc une lecture d'état
  global coûterait un verrou par image. Couvert par `colony_palette_tests`
  (`src/ui/theme.rs`), qui vérifie la lisibilité sur **tout** le catalogue.

## Flux de données

```
UI (UiMessage) -> XionApp::update -> services (history, loader, search)
                                  -> filesystem (listage, opérations)
                                        ^
                                        | core::config
```

Le rafraîchissement automatique remonte dans l'autre sens : une souscription
Iced émet `FileWatchTick` toutes les 750 ms
(`src/ui/app/windowing.rs`, `WATCHER_POLL_INTERVAL`, `src/ui/theme.rs`), qui
appelle `poll_watcher` (`src/ui/app/state.rs`) et déclenche un
relistage si un évènement concerne le dossier affiché.

## Décisions clés

- `filesystem` reste découplé de l'UI, ce qui rend la virtualisation et les
  tests possibles sans fenêtre.
- `services` coordonne la logique de navigation, de cache et de recherche.
- Les opérations bloquantes passent par `tokio::task::spawn_blocking` avant
  d'être renvoyées dans la boucle Iced (copie, suppression, indexation,
  scan réseau).

## Écarts constatés entre ce document et le code

Ces points sont des dettes réelles, pas des choix. Ils sont écrits ici pour que
personne ne réimplémente ce qui existe, ni ne fasse confiance à une règle qui
n'est pas tenue.

1. **L'UI accède directement au disque.** Au 2026-08-25, `src/ui/` compte 47
   usages de `std::fs::` (31 hors modules de test), dont 27 dans le seul
   `app/archive.rs`, puis `app/helpers.rs` (6) et `app/update/tools.rs` (5).
   L'ancienne formulation « `ui` n'a pas accès direct au disque : elle demande
   au `filesystem` » était simplement fausse.
   Contrôle : `grep -rho 'std::fs::[A-Za-z_]*' src/ui --include='*.rs' | wc -l`
2. **`core` dépend de `ui`.** `src/core/config/types.rs` stocke des
   `crate::ui::FileLabel`, et `merge_from_v1` / `config_to_file`
   (`src/core/config/manager/migrate.rs`) les convertissent depuis et vers le
   TOML. `types.rs` appelle en plus `crate::ui::theme::resolves_dark` pour
   dériver `dark_mode` du thème. La règle « `core` ne dépend de rien » n'est
   donc pas tenue.
   Contrôle : `grep -rn 'crate::ui::' src/core/`
3. **Aucun fichier de plus de 1000 lignes ; trois au-dessus de 800.** Au
   2026-08-25, dans cet ordre : `ui/app/types.rs`, `ui/app/helpers.rs`,
   `services/thumbnails.rs` — tous entre 800 et 1000. Le décompte exact bouge à
   chaque commit ; ce qui compte est lesquels, pas combien. La cible « aucun
   fichier > 800 lignes » n'est donc pas encore atteinte, mais le point noir
   historique a disparu :
   `ui/app/view/mod.rs` est passé de 3174 à ~360 lignes, sa fonction `view`
   déléguant à vingt sous-modules, et `ui/app/update.rs` a été remplacé par
   `ui/app/update/`, neuf fichiers.
   Contrôle : `find src -name '*.rs' -exec wc -l {} + | sort -rn | head`
4. **75 des 99 fichiers `.rs` de `src/` n'ont aucun test unitaire** (relevé du
   2026-08-25), dont `services/search.rs`, `core/config/shortcuts.rs`,
   `ui/app/state.rs` et l'ensemble de `ui/app/view/`. `filesystem/watcher.rs`,
   `ui/theme.rs` et `core/config/manager/mod.rs` en ont désormais.
   Contrôle : `grep -rL --include='*.rs' '#\[cfg(test)\]' src | wc -l`

## État d'implémentation

- [x] Modules `core/`, `filesystem/`, `services/`, `platform/`, `ui/` présents
      et séparés — `src/lib.rs`.
- [x] Config centralisée utilisée côté UI — `src/ui/app/construction.rs`.
- [x] Services branchés dans la boucle UI (history, loader, thumbnails).
- [x] Virtualisation des listes branchée dans l'UI —
      `src/services/virtualization.rs`.
- [x] Observateur FS natif — `NativeFileWatcher`,
      `src/filesystem/watcher.rs`, instancié `src/ui/app/construction.rs`,
      interrogé `src/ui/app/state.rs`.
- [x] Découper `view/mod.rs` et `update.rs` — `src/ui/app/view/` (21 fichiers)
      et `src/ui/app/update/` (9 fichiers).
- [x] Thème issu du catalogue partagé `colony-ui` — `UiTokens::for_theme`,
      `src/ui/theme.rs`.
- [ ] Invalidation fine des caches sur évènement : le watcher déclenche
      aujourd'hui un relistage complet du dossier, pas une mise à jour ciblée.
- [ ] Sortir les appels `std::fs` de `src/ui/` vers `filesystem/`.
- [ ] Retirer la dépendance de `core` vers `ui` (`FileLabel`,
      `theme::resolves_dark`).
- [ ] Ramener les trois derniers fichiers au-dessus de 800 lignes sous la
      cible : `ui/app/types.rs`, `ui/app/helpers.rs`,
      `services/thumbnails.rs`.

## Terminal intégré

Réécrit sur un vrai pseudo-terminal (`src/terminal.rs`). L'implémentation
précédente branchait des tubes sur `cmd.exe` : sans tty, pas de Ctrl-C, pas de
taille de fenêtre, pas de séquences VT, et aucune sortie tant qu'aucun `\n`
n'arrivait.

- `portable-pty` ouvre le pty dans `TerminalProcess::spawn` : ConPTY sous
  Windows, `openpty` sous Unix. Le terminal est donc **multiplateforme**.
- Les octets passent par un analyseur `vte`, donc les séquences d'échappement
  sont interprétées au lieu d'être affichées : couleurs, effacement de ligne,
  `\r` qui réécrit en place (les barres de progression fonctionnent).
- `TERM=xterm-256color` est positionné dans `TerminalProcess::spawn`, sinon
  beaucoup d'outils désactivent la couleur.
- Ctrl-C envoie un ETX (`CTRL_C = 0x03`) dans le pty :
  `TerminalProcess::interrupt`.
- Le redimensionnement est propagé : `TerminalProcess::resize`.
- Scrollback de 10 000 lignes (`SCROLLBACK_MAX`).

**Limite assumée** : le modèle d'écran est un historique de lignes avec un
curseur dans la ligne courante, pas une grille plein écran
(`src/terminal.rs`). Les applications plein écran — `vim`, `htop`,
`less` en mode interactif — ne s'affichent pas correctement. C'est hors
périmètre, pas un bug à signaler.

Tests : 16 tests unitaires dans `src/terminal.rs`, dont
`carriage_return_overwrites_in_place`, `sgr_sequences_are_consumed_not_printed`,
`utf8_survives_a_chunk_boundary`, `a_real_shell_echoes_through_the_pty`,
`interrupt_stops_a_running_command`.

## Sécurité

- Le manifeste Windows demande `asInvoker`, plus `requireAdministrator`
  (`xion.manifest`). Aucune écriture `HKEY_LOCAL_MACHINE` n'existe dans le
  dépôt ; l'élévation permanente n'était donc justifiée par rien, et elle
  contaminait tous les processus lancés depuis Xion tout en faisant bloquer le
  glisser-déposer depuis Explorer par UIPI.
- L'extraction d'archive refuse toute entrée qui sortirait du dossier de
  destination (zip-slip), y compris via un lien symbolique ; couvert par
  `zip_entry_with_backslash_traversal_is_rejected`,
  `sevenz_entry_with_absolute_path_is_rejected`,
  `create_parent_within_rejects_a_symlink_escape`
  (`src/ui/app/archive.rs`).
- La suppression passe par la corbeille (`trash::delete`,
  `src/ui/app/operations/delete.rs`). `Maj+Suppr` déclenche la suppression
  définitive, toujours derrière une confirmation ; tests
  `permanent_delete_asks_before_acting`,
  `cancelling_the_confirmation_deletes_nothing`
  (`src/ui/app/operations/mod.rs`).

## Intégration continue

`.github/workflows/ci.yml` : matrice `ubuntu-latest` + `windows-latest`, avec
`cargo fmt --all --check`, `cargo clippy --all-targets --all-features
-- -D warnings`, `cargo test --all-targets --all-features`, un contrôle
d'absence de BOM UTF-8, et un job séparé `advisories` qui lance
`cargo deny check advisories bans licenses sources`. Le tout est aussi rejoué
chaque lundi par un déclencheur `schedule`, pour qu'une advisory publiée entre
deux pushs remonte quand même.
