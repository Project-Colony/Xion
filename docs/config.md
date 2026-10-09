# Configuration Xion

Ce document décrit le format de configuration versionné de Xion, les valeurs par
défaut et le comportement en cas d'erreur. Il a été relu ligne à ligne contre
`src/core/config/types.rs` et les six fichiers de `src/core/config/manager/`
le 2026-08-25.

> Les références sont données par nom de symbole plutôt que par numéro de
> ligne : un numéro devient faux au commit suivant. Chaque référence se contrôle
> par un `grep`.

`manager.rs` n'existe plus en tant que fichier unique. Le module est découpé, et
les références ci-dessous nomment le bon fichier :

| Fichier | Contenu |
| --- | --- |
| `manager/mod.rs` | `ConfigManager`, `default_config_path`, `load_from_path`, écriture atomique, tests |
| `manager/file_format.rs` | `AppConfigFileV0` / `AppConfigFileV1`, les formes sur disque |
| `manager/migrate.rs` | `merge_from_v0`, `merge_from_v1`, `config_to_file` |
| `manager/validate.rs` | les fonctions `validated_*` |
| `manager/limits.rs` | `CURRENT_CONFIG_VERSION` et les bornes `MIN_*` / `MAX_*` |
| `manager/shortcuts_io.rs` | `merge_shortcuts`, `parse_shortcut`, `chord_to_string` |

## Emplacement

Xion suit la disposition de l'écosystème Colony, `<racine>/Colony/<Programme>/`,
et le chemin vient du socle partagé plutôt que d'un calcul local
(`default_config_path`, `manager/mod.rs`) :

```rust
colony_ui::paths::locate::config_dir("Xion")?.join("config.toml")
```

Le nom du programme est **capitalisé** — `Xion`, pas `xion` : la convention est
`<racine>/Colony/<Programme>/` avec le programme orthographié comme il s'écrit
(`COLONY_PROGRAM`, `manager/mod.rs` ; voir `design/filesystem.md` de
Project-Colony-Resources). Cela donne :

- **Linux** : `~/.config/Colony/Xion/config.toml`
  (`$XDG_CONFIG_HOME/Colony/Xion/config.toml` si la variable est posée).
- **Windows** : `%LOCALAPPDATA%\Colony\Xion\config.toml`.
  `AppData\Local`, jamais `Roaming` : `colony_ui` appelle
  `dirs::config_local_dir()`.
- **macOS** : `~/Library/Application Support/Colony/Xion/config.toml`.

C'est `locate::config_dir` qui est appelé, et non `paths::config_dir` : le
second **crée** le dossier. Créé avant la migration, il ferait croire à une
configuration déjà en place, la migration serait sautée et Xion écrirait des
valeurs par défaut à côté d'une configuration existante. `save` crée son parent
lui-même (`write_atomic`), donc personne n'a besoin de le devancer.

### Migration from the old location

Xion used to write under `ProjectDirs::from("io", "xion", "Xion")`, that is
`~/.config/xion/` on Linux. That path is now only a **migration source**
(`legacy_config_dir`, `manager/mod.rs`).

When the path is first computed, `migrate_legacy_config_dir` (`manager/mod.rs`)
copies the **whole directory**, not only `config.toml`: the `.bak` next to it is
the fallback when the main file is unreadable, and leaving it behind would
silently lose it. It follows rule FS-7 of Project-Colony-Resources
(`design/filesystem.md`):

1. Every file is copied (and synced) into a temporary sibling of the target,
   `Colony/Xion.migrating`. Only once **every** file made it is that directory
   renamed to `Colony/Xion`, so a half-copied profile is never picked up. A
   `Colony/Xion.migrating` older than a minute was left by a start that
   crashed and is replaced; a younger one may be another start copying at the
   same moment, so it is left alone and this run uses the old directory.
2. A `.migrated` marker is then written into the old directory. Later starts see
   it and skip the migration.
3. The old directory is **never moved or deleted** in this release. If the
   migration turns out to be wrong, the files are still where they were. Its
   removal is left to a later release, for directories that carry the marker.
4. If anything fails (an unreadable file, a sub-directory, a full disk), the
   temporary directory is removed, no `Colony/Xion` is created, and
   `default_config_path` returns `~/.config/xion/config.toml` for this run: the
   user keeps their profile instead of getting an empty one. The next start
   tries again.

On Windows and macOS the data directory is the same folder as the
configuration (see the session file below), so a run that fell back to the old
directory still creates `Colony/Xion` to save `session.toml` there. That is why
only a `config.toml` counts as a configuration already in place: the next
successful copy moves its files into the existing folder one by one,
`config.toml` last, and keeps `session.toml`. Right before that move, the
destination is checked for a `config.toml` again: if another start published
its copy in the meantime, that copy wins and this one is dropped.

The copy only handles **files**. A sub-directory counts as a failure rather
than something to skip: Xion creates none (`config.toml`, its `.bak` and the
`.corrupt-*` files are all flat), so one is unexpected, and skipping it while
reporting success is how an earlier version lost data. Symlinked files are
copied as the file they point to.

The migration does nothing when:

- the marker is present;
- the destination already holds a `config.toml`: a newer configuration never
  loses to an older one left behind;
- the old directory is absent, which is the case of every fresh install;
- the old and new paths are the same.

Tests: `a_pre_colony_configuration_is_copied_with_its_backup`,
`a_second_start_does_not_migrate_again`, `an_existing_colony_configuration_wins`,
`a_sub_directory_fails_the_copy_and_keeps_the_old_profile`,
`an_unreadable_file_fails_the_copy_and_keeps_the_old_profile`,
`a_folder_holding_only_the_session_is_not_a_migrated_profile`,
`a_recent_staging_folder_is_left_to_its_owner`,
`a_staging_folder_left_by_a_crash_is_replaced`,
`a_copy_published_meanwhile_by_another_start_wins`,
`a_fresh_installation_migrates_nothing` (`manager/mod.rs`).

`ConfigManager::new()` runs this migration against the real home directory, so
tests build their manager with `ConfigManager::with_path` on a temporary
directory instead.

### Quand le chemin ne peut pas être calculé

Si `locate::config_dir` échoue, Xion retombe sur le chemin **relatif**
`config.toml`, résolu depuis le répertoire courant du processus. C'est un défaut
connu : selon l'endroit d'où l'exécutable est lancé, un `config.toml` peut être
créé ailleurs que prévu.

Le chemin résolu n'est aujourd'hui affiché nulle part dans l'interface.
`ConfigManager::path()` existe (`manager/mod.rs`) mais n'a aucun appelant côté
UI.

En l'absence de fichier, `load_from_path` renvoie `AppConfig::default()` avec
`ConfigSource::Default` et aucun avertissement (`manager/mod.rs`) : un fichier
écrit au mauvais endroit n'entraîne donc **aucun message**.

## Format (version 1)

Le fichier est un TOML versionné par la clé `version`. Toutes les sections et
toutes les clés sont optionnelles ; ce qui manque prend la valeur par défaut.

Le bloc ci-dessous liste l'intégralité de ce que Xion écrit et relit
(`AppConfigFileV1` dans `manager/file_format.rs`, sérialisé par
`config_to_file`, `manager/migrate.rs`).

```toml
# ── Clés de premier niveau ────────────────────────────────────────────────────
# En TOML elles doivent toutes précéder la première section [...].

version = 1

# ── Apparence ─────────────────────────────────────────────────────────────────
# Le thème est une sélection dans le catalogue partagé de `colony-ui` :
# 25 familles, 57 variantes, 8 accents. Les clés sont des chaînes libres et non
# un ensemble fermé, à dessein : le socle ajoute des familles sans que Xion soit
# recompilé, et `colony_ui::resolve` retombe sur sa palette de repli pour tout
# couple inconnu — une famille retirée en amont dégrade au lieu d'empêcher le
# démarrage.
theme_family = "gruvbox"     # clé de famille : gruvbox, nord, catppuccin…
theme_variant = "dark"       # clé de variante dans cette famille : dark, light,
                             # mocha… ; les noms varient selon la famille
high_contrast = false        # modificateur applicable à N'IMPORTE quelle
                             # palette, pas un thème à part
accent = "blue"              # red | orange | yellow | green | blue | indigo |
                             # violet | amber. Absent = l'accent du thème.

# `dark_mode` est RECALCULÉ au chargement à partir de la palette résolue
# (`resolves_dark`, `src/ui/theme.rs`, qui mesure la luminance du fond) ; s'il
# est écrit et incohérent, un avertissement le signale et le thème gagne.
dark_mode = false

# `theme` (ancien nom : Light | Dark | Nord | Solarized | HighContrast) est
# encore LU, uniquement pour migrer un fichier antérieur au catalogue Colony.
# Il n'est plus jamais écrit, et `theme_family` / `theme_variant` l'emportent
# dès qu'ils sont présents tous les deux. Voir « Migration ».

# Dossier d'ouverture. Défaut : le répertoire courant du processus.
start_path = "/home/utilisateur"

# true force aussi view.row_height à 22.0, dans `merge_from_v1`.
compact_mode = false

# Cmd | PowerShell | GitBash, ou n'importe quelle autre chaîne, prise telle
# quelle comme commande personnalisée.
terminal_shell = "Cmd"

respect_gitignore = false

# Écrit seulement si la liste n'est pas vide.
user_favorites = ["/home/utilisateur/travail"]

# ── Sections ──────────────────────────────────────────────────────────────────

[list]
show_hidden = false
sort_key = "name"            # name | modified | size
sort_order = "asc"           # asc | desc
directories_first = true
filter = "all"               # all | only-directories | only-files

[cache]
thumbnail_entries = 256      # 32..8192
thumbnail_ttl_seconds = 300  # 30..86400
directory_entries = 256      # 32..8192
directory_ttl_seconds = 45   # 30..86400

[filesystem]
metadata_batch_size = 256    # 16..4096
metadata_parallelism = 4     # 1..32

[view]
mode = "list"                # list | grid
thumbnail_size = 48          # 24..256
row_height = 32.0            # 20.0..72.0
grid_columns = 4             # 1..12
grid_row_height = 140.0      # 72.0..240.0
overscan = 6                 # 0..128
columns = ["name", "type", "size", "modified"]

[paging]
page_size = 120              # 24..2048

# Étiquettes de couleur, indexées par chemin.
# Valeurs : Red | Orange | Yellow | Green | Blue | Purple | Gray
[labels]
"/home/utilisateur/projets" = "Blue"

# Largeur des colonnes de la vue Détails, en points.
#
# Les clés sont les identifiants stables des colonnes, indépendants de la
# langue de l'interface — pas les libellés affichés. Xion écrivait autrefois
# ces largeurs sous les libellés français (« Nom », « Taille », « Modifié »),
# si bien que trois des quatre valeurs par défaut ci-dessous n'étaient jamais
# relues, et que les largeurs enregistrées n'étaient de toute façon jamais
# appliquées au rendu.
[column_widths]
Name = 300.0
Type = 80.0
Size = 100.0
Modified = 150.0

[shortcuts]
move_up = "ArrowUp"
move_down = "ArrowDown"
move_home = "Home"
move_end = "End"
activate = "Enter"
clear_selection = "Escape"
cycle_pane_focus = "Tab"
back = "Alt+ArrowLeft"
forward = "Alt+ArrowRight"
refresh = "Ctrl+R"
select_all = "Ctrl+A"
toggle_context_menu = "Ctrl+M"
rename = "F2"
delete = "Delete"
new_folder = "Ctrl+Shift+N"
focus_search = "Ctrl+E"
```

`[labels]`, `[column_widths]` et `user_favorites` ne sont écrits que lorsqu'ils
ne sont pas vides (`config_to_file` les met à `None` sinon) : leur absence d'un
fichier existant est normale.

The restored tabs are no longer part of this file: see the next section.

## Session file (`session.toml`)

The tabs Xion reopens at the next start are something Xion produced, not a
choice the user made, so they live in the **data** directory rather than next to
the preferences (rule FS-5 of Project-Colony-Resources, `design/filesystem.md`).
`SessionStore` (`src/core/config/session.rs`) reads and writes:

```rust
colony_ui::paths::locate::data_dir("Xion")?.join("session.toml")
```

- **Linux**: `~/.local/share/Colony/Xion/session.toml`
  (`$XDG_DATA_HOME/Colony/Xion/session.toml` when the variable is set).
- **Windows**: `%LOCALAPPDATA%\Colony\Xion\session.toml`.
- **macOS**: `~/Library/Application Support/Colony/Xion/session.toml`.

On Windows and macOS this is the same directory as `config.toml`; the two file
names keep them apart.

```toml
# Active tab at startup. Clamped to the tabs below when they are rebuilt
# (`initial_tabs`, `src/ui/app/construction.rs`), since the file can be edited.
active_tab_index = 0

# Tabs restored at startup. An empty list opens a single tab on `start_path`.
[[tabs]]
path = "/home/utilisateur"
```

The file is rewritten on every tab change and navigation (`save_session`,
`src/ui/app/state.rs`), atomically like `config.toml` (temporary file, then
rename) but without a `.bak`. An unreadable file is logged and Xion starts from
a single tab; the next save replaces it.

### Tabs carried over from `config.toml`

Before this file existed, the tabs were stored in `config.toml` as `[[tabs]]`
and `active_tab_index`. When `session.toml` is absent at startup,
`SessionStore::load` reads them once from `config.toml`
(`ConfigManager::legacy_session`, `manager/mod.rs`) and writes them to
`session.toml`. The V1 reader still accepts both keys without a warning, and the
next config save drops them, since `config_to_file` no longer writes them.
When `config.toml` is missing or does not parse (the loader has just moved an
unreadable one aside and restored the settings from `config.toml.bak`), the tabs
are read from `config.toml.bak` instead.
Once `session.toml` exists, `config.toml` is never consulted for tabs again.

Tests: `a_session_round_trips`,
`tabs_move_from_an_older_config_to_the_session_file`,
`tabs_come_back_from_the_backup_of_an_unreadable_config`,
`an_existing_session_file_wins_over_an_older_config`,
`a_missing_session_and_no_legacy_tabs_load_empty_and_write_nothing`
(`session.rs`).

## Ce qui écrit les quatre clés d'apparence

Aucune n'a besoin d'être éditée à la main : l'écran **Apparence** les écrit
(`render_appearance_layer`, `src/ui/app/view/appearance.rs`, ouvert par
`UiMessage::ToggleAppearance` depuis le menu `⋯`). Les deux sélecteurs sont
ceux de `colony-ui` et se rendent seuls depuis le catalogue ; Xion n'écrit ni
les vignettes, ni les noms, ni la liste.

| Message | Effet sur le fichier |
| --- | --- |
| `SetThemeVariant(family, variant)` | écrit `theme_family` et `theme_variant` |
| `SetAccent(key)` | écrit `accent`, ou l'efface si on rechoisit le même |
| `ToggleHighContrast` | bascule `high_contrast` |
| `ToggleDarkMode` | bascule `theme_variant` entre `dark` et `light` |

Chacun sauvegarde immédiatement (`update/tools.rs`).

`ToggleDarkMode` ne fait plus tourner cinq thèmes : il bascule la variante
claire/sombre **de la famille courante**. Une famille dont les variantes ne
s'appellent ni `dark` ni `light` — `catppuccin` et ses quatre saveurs — n'a pas
de contraire évident ; dans ce cas rien ne change et la barre d'état invite à
choisir dans le menu (`update/tools.rs`).

## Raccourcis

Les 16 raccourcis configurables correspondent exactement aux champs de
`ShortcutBindings` (`src/core/config/shortcuts.rs`).

Format d'une valeur : des modificateurs séparés par `+`, puis la touche.
Modificateurs reconnus : `Ctrl` (ou `Control`), `Alt`, `Shift`
(`KeyChord::parse`, `src/core/config/shortcuts.rs`).

Touches nommées reconnues : `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`,
`Home`, `End`, `Enter`, `Escape` (ou `Esc`), `Tab`, `Space`, `F2`, `F3`, `F5`,
`Delete` (ou `Del`). Toute autre valeur d'un seul caractère est prise comme
caractère littéral ; au-delà, le parseur renvoie « Touche inconnue ».

### Raccourcis non configurables

Ils sont câblés dans `src/ui/app/helpers.rs` et ne passent pas par le fichier :

- `Maj+Suppr` : suppression définitive, après confirmation
  (`src/ui/app/helpers.rs`).
- `F5` : équivalent de `refresh` (`src/ui/app/helpers.rs`).
- `Ctrl+1` à `Ctrl+9` : saut vers le favori numéroté
  (`src/ui/app/helpers.rs`).
- `Ctrl+Maj+C` : copier le chemin (`src/ui/app/helpers.rs`).
- Une touche imprimable seule alimente le filtre rapide
  (`src/ui/app/helpers.rs`), ce qui empêche de lier un raccourci à un
  caractère nu.

## Migration

### Depuis une V0 (fichier sans clé `version`)

Un fichier sans clé `version` est traité comme une V0 : seuls `start_path`,
`show_hidden`, `thumbnail_size`, `thumbnail_cache_entries` et
`thumbnail_cache_ttl_seconds` sont repris (`AppConfigFileV0`,
`manager/file_format.rs`, puis `merge_from_v0`, `manager/migrate.rs`). Le
résultat est signalé par `ConfigSource::Migrated`.

Une version supérieure à `CURRENT_CONFIG_VERSION` (= 1, `manager/limits.rs`)
est refusée avec un message explicite (`load_from_path`, `manager/mod.rs`) ; la
configuration par défaut est utilisée.

### Depuis l'ancienne clé `theme`

Xion portait cinq palettes écrites à la main derrière un `enum`. Elles ont
disparu au profit du catalogue `colony-ui`, et `merge_from_v1`
(`manager/migrate.rs`) traduit l'ancien nom vers un couple famille/variante :

| ancien `theme` | `theme_family` | `theme_variant` | `high_contrast` |
| --- | --- | --- | --- |
| `"Light"` | `gruvbox` | `light` | `false` |
| `"Dark"` | `gruvbox` | `dark` | `false` |
| `"Nord"` | `nord` | `dark` | `false` |
| `"Solarized"` | `solarized` | `dark` | `false` |
| `"HighContrast"` | `gruvbox` | `dark` | **`true`** |
| absent | `gruvbox` | `dark` (ou `light` si `dark_mode = false`) | `false` |
| autre chaîne | `gruvbox` | `dark` | `false`, **avec avertissement** |

`Light` et `Dark` étaient des noms génériques sans famille derrière eux : ils
prennent celle du repli du catalogue. `HighContrast` n'était pas une palette
mais un rehaussement, et Colony en fait un modificateur applicable à n'importe
quelle palette — d'où la case `high_contrast` plutôt qu'une famille dédiée.

Cette traduction n'a lieu **que** si `theme_family` et `theme_variant` sont
absents (ou l'un des deux) ; dès que les deux sont là, l'ancienne clé est
ignorée. Un `high_contrast` explicite dans le fichier gagne dans tous les cas.

### Tests de migration

`migrates_v0_file_without_version_key`,
`v0_migration_clamps_out_of_range_values`,
`v0_dark_mode_key_is_ignored_but_v1_theme_wins`,
`dark_mode_is_recomputed_from_the_theme` et
`unsupported_future_version_is_rejected_without_losing_the_file`
(`manager/mod.rs`).

## Validation et repli

Chaque champ numérique est borné (constantes `MIN_*` / `MAX_*`,
`manager/limits.rs`, appliquées par les fonctions `validated_*`,
`manager/validate.rs`). En cas de valeur hors bornes :

- la valeur fautive est ignorée,
- la valeur par défaut correspondante est réappliquée,
- un `ConfigWarning` est ajouté et remonté à l'UI.

Le thème, lui, n'est pas validé et n'a pas à l'être : une famille ou une
variante inconnue **ne produit aucun avertissement**. `colony_ui::resolve`
retombe sur sa palette de repli, et Xion démarre sur cette palette. Seule
l'**ancienne** clé `theme`, avec une chaîne hors des cinq noms historiques,
déclenche encore un avertissement (`merge_from_v1`, `manager/migrate.rs`).

Une valeur de `terminal_shell` non reconnue n'est pas une erreur non plus :
elle est prise telle quelle comme `ShellConfig::Custom`
(`manager/migrate.rs`).

## Rechargement à chaud

`UiMessage::Refresh` (`Ctrl+R` ou `F5`) appelle `reload_config`
(`src/ui/app/navigation/buffers.rs`) avant de relister le dossier
(`src/ui/app/update/navigation.rs`). Le fichier est donc relu sans redémarrage.

The file is written by the actions that change a setting (theme, compact mode,
favourites, shell, sort order, view mode...). The open tabs go to
`session.toml` instead. `ConfigManager::save` serialises the whole
`AppConfigFileV1` and replaces the file. Two consequences:

- **Comments in a hand-edited file are lost** at the first setting change. The
  file is rewritten as a whole, not merged.
- The write is **atomic**: `write_atomic` writes a temporary sibling file
  (`.tmp-<pid>`, in the same directory so that `rename` stays atomic), calls
  `sync_all` on it, copies the previous file to `.bak`, then renames. A power
  cut during the save never leaves a truncated configuration.
  Tests: `save_is_atomic_and_leaves_no_temporary_file`,
  `save_keeps_the_previous_version_as_backup`.

## Récupération d'un fichier illisible

Un fichier qui ne parse plus n'est plus laissé en place puis écrasé
silencieusement à la sauvegarde suivante. `ConfigManager::load` le déplace en
`config.toml.corrupt-<horodatage>`, tente de recharger `config.toml.bak`, et
remonte le tout en avertissements. Test :
`unreadable_config_is_quarantined_and_restored_from_backup`.

## Avancement

- [x] Format TOML versionné (v1) — `AppConfigFileV1`.
- [x] Migration depuis une version non versionnée — `merge_from_v0`.
- [x] Validation des champs avec avertissements — constantes `MIN_*`/`MAX_*` et
      fonctions `validated_*`, appelées depuis `merge_from_v1`.
- [x] Rechargement à chaud — `src/ui/app/navigation/buffers.rs`.
- [x] Theme, shell and favourites saved: `config_to_file`.
- [x] Restored tabs kept in `session.toml` under the data directory:
      `SessionStore`.
- [x] Location aligned on `<root>/Colony/Xion/`, with the old directory copied,
      marked and kept: `migrate_legacy_config_dir`.
- [x] Thème choisi dans le catalogue partagé `colony-ui` (famille, variante,
      contraste élevé, accent) — `ThemeChoice`, `src/core/config/types.rs`.
- [x] Tests for the migration, validation, atomic write and recovery: 20 unit
      tests in `manager/mod.rs` (count of 2026-10-09;
      `grep -c '#\[test\]' src/core/config/manager/mod.rs`).
- [ ] Affichage du chemin de configuration résolu dans l'interface.
- [ ] Remplacement du repli relatif `config.toml` par un chemin absolu sûr.
- [ ] Éditeur de configuration intégré à l'interface.
- [ ] Presets de configuration.
