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

### Déménagement depuis l'ancien emplacement

Xion écrivait auparavant sous `ProjectDirs::from("io", "xion", "Xion")`, soit
`~/.config/xion/` sous Linux. Ce chemin n'est plus qu'une **source de
migration** (`legacy_config_dir`, `manager/mod.rs`).

Au premier calcul du chemin, `migrate_legacy_config_dir` (`manager/mod.rs`)
déplace le **dossier entier**, pas seulement `config.toml` : le `.bak` posé à
côté est le recours quand le fichier principal est illisible, et le laisser
derrière reviendrait à s'en priver sans le dire. Un `fs::rename` suffit quand
les deux vivent sur le même système de fichiers, ce qui est le cas ordinaire ;
sinon les fichiers sont copiés un à un, et l'ancien dossier n'est supprimé que
si **tout** est passé.

Trois cas où la migration ne fait rien :

- la destination existe déjà — une configuration récente n'est jamais écrasée
  par une ancienne restée là ;
- l'ancien dossier est absent, ce qui est le cas de toute installation neuve ;
- l'ancien et le nouveau chemin sont identiques.

Tests : `a_pre_colony_configuration_moves_with_its_backup`,
`an_existing_colony_configuration_wins`, `a_fresh_installation_migrates_nothing`
(`manager/mod.rs`).

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

# Onglet actif au démarrage, ramené dans les bornes de [[tabs]] au chargement
# par `merge_from_v1`.
active_tab_index = 0

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

# Onglets restaurés au démarrage. Une liste vide est ignorée.
[[tabs]]
path = "/home/utilisateur"

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

L'écriture est déclenchée par les actions qui modifient un réglage (thème, mode
compact, onglets, favoris, shell…). `ConfigManager::save` sérialise
l'intégralité de `AppConfigFileV1` et remplace le fichier. Deux conséquences à
connaître :

- **Les commentaires d'un fichier édité à la main sont perdus** au premier
  changement de réglage. Le fichier est réécrit en entier, pas fusionné.
- L'écriture est **atomique** : `write_atomic` écrit dans un fichier temporaire
  voisin (`.tmp-<pid>`, dans le même dossier pour que `rename` reste atomique),
  le `sync_all`, copie l'ancien fichier en `.bak`, puis renomme. Une coupure
  pendant la sauvegarde ne laisse donc pas de configuration tronquée.
  Tests : `save_is_atomic_and_leaves_no_temporary_file`,
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
- [x] Persistance des onglets, du thème, du shell et des favoris —
      `config_to_file`.
- [x] Emplacement aligné sur `<racine>/Colony/Xion/`, avec déménagement
      automatique de l'ancien dossier — `migrate_legacy_config_dir`.
- [x] Thème choisi dans le catalogue partagé `colony-ui` (famille, variante,
      contraste élevé, accent) — `ThemeChoice`, `src/core/config/types.rs`.
- [x] Tests de la migration, de la validation, de l'écriture atomique et de la
      récupération — 14 tests unitaires dans `manager/mod.rs` (relevé du
      2026-08-25 ; `grep -c '#\[test\]' src/core/config/manager/mod.rs`).
- [ ] Affichage du chemin de configuration résolu dans l'interface.
- [ ] Remplacement du repli relatif `config.toml` par un chemin absolu sûr.
- [ ] Éditeur de configuration intégré à l'interface.
- [ ] Presets de configuration.
