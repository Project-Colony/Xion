# Configuration Xion

Ce document décrit le format de configuration versionné de Xion, les valeurs par
défaut et le comportement en cas d'erreur. Il a été relu ligne à ligne contre
`src/core/config/manager.rs` et `src/core/config/types.rs` le 2026-08-24.

> Les références sont données par nom de symbole plutôt que par numéro de
> ligne : un numéro devient faux au commit suivant. Chaque référence se contrôle
> par un `grep`.

## Emplacement

Le chemin est calculé par `default_config_path()`
(`src/core/config/manager.rs`) :

```rust
ProjectDirs::from("io", "xion", "Xion")
    .map(|dirs| dirs.config_dir().join("config.toml"))
    .unwrap_or_else(|| PathBuf::from("config.toml"))
```

Avec `directories` 5.0.1 (cf. `Cargo.lock`), cela donne :

- **Linux** : `$XDG_CONFIG_HOME/xion/config.toml`, ou à défaut
  `~/.config/xion/config.toml`.
  Le nom du dossier est **en minuscules** : `directories` applique
  `trim_and_lowercase_then_replace_spaces` au seul champ « application ».
  Sur un système de fichiers sensible à la casse, `~/.config/Xion/` ne sera
  jamais lu.
- **Windows** : `%APPDATA%\xion\Xion\config\config.toml`.
  Le chemin comprend bien trois segments : l'organisation en minuscules,
  l'application, puis un sous-dossier `config` ajouté par `directories`.
- **macOS** : `~/Library/Application Support/io.xion.Xion/config.toml`
  (identifiant de bundle `qualifier.organization.application`).

Si `ProjectDirs::from` échoue, Xion retombe sur le chemin **relatif**
`config.toml`, résolu depuis le répertoire courant du processus. C'est un défaut
connu : selon l'endroit d'où l'exécutable est lancé, un `config.toml` peut être
créé ailleurs que prévu.

Le chemin résolu n'est aujourd'hui affiché nulle part dans l'interface.
`ConfigManager::path()` existe (`src/core/config/manager.rs`) mais n'a
aucun appelant côté UI.

En l'absence de fichier, `load_from_path` renvoie `AppConfig::default()` avec
`ConfigSource::Default` et aucun avertissement
(`src/core/config/manager.rs`) : un fichier écrit au mauvais endroit
n'entraîne donc **aucun message**.

## Format (version 1)

Le fichier est un TOML versionné par la clé `version`. Toutes les sections et
toutes les clés sont optionnelles ; ce qui manque prend la valeur par défaut.

Le bloc ci-dessous liste l'intégralité de ce que Xion écrit et relit
(`AppConfigFileV1` dans `src/core/config/manager.rs`, sérialisé par
`config_to_file`).

```toml
# ── Clés de premier niveau ────────────────────────────────────────────────────
# En TOML elles doivent toutes précéder la première section [...].

version = 1

# `theme` est la seule source de vérité. `dark_mode` est RECALCULÉ à partir de
# lui au chargement (`config.dark_mode = config.theme.is_dark()`) ; s'il est
# écrit et incohérent, un avertissement le signale et la valeur du thème gagne.
# `dark_mode = true` n'est lu que dans un fichier antérieur à la clé `theme`.
theme = "Light"              # Light | Dark | Nord | Solarized | HighContrast
dark_mode = false

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

Un fichier sans clé `version` est traité comme une V0 : seuls `start_path`,
`show_hidden`, `thumbnail_size`, `thumbnail_cache_entries` et
`thumbnail_cache_ttl_seconds` sont repris (`AppConfigFileV0`,
`src/core/config/manager.rs`, puis `merge_from_v0`). Le résultat est
signalé par `ConfigSource::Migrated`.

Une version supérieure à `CURRENT_CONFIG_VERSION` (= 1) est refusée avec un
message explicite (`src/core/config/manager.rs`) ; la configuration par
défaut est utilisée.

La migration est couverte par les tests `migrates_v0_file_without_version_key`,
`v0_migration_clamps_out_of_range_values`,
`v0_dark_mode_key_is_ignored_but_v1_theme_wins` et
`unsupported_future_version_is_rejected_without_losing_the_file`
(`src/core/config/manager.rs`).

## Validation et repli

Chaque champ numérique est borné (constantes `MIN_*` / `MAX_*`,
`src/core/config/manager.rs`). En cas de valeur hors bornes :

- la valeur fautive est ignorée,
- la valeur par défaut correspondante est réappliquée,
- un `ConfigWarning` est ajouté et remonté à l'UI.

Un thème inconnu déclenche un avertissement et retombe sur `Light`
(`src/core/config/manager.rs`). Une valeur de `terminal_shell` non
reconnue n'est pas une erreur : elle est prise telle quelle comme
`ShellConfig::Custom` (`src/core/config/manager.rs`).

## Rechargement à chaud

`UiMessage::Refresh` (`Ctrl+R` ou `F5`) appelle `reload_config`
(`src/ui/app/navigation.rs`) avant de relister le dossier
(`src/ui/app/update.rs`). Le fichier est donc relu sans redémarrage.

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
- [x] Rechargement à chaud — `src/ui/app/navigation.rs`.
- [x] Persistance des onglets, du thème, du shell et des favoris —
      `config_to_file`.
- [x] Tests de la migration, de la validation, de l'écriture atomique et de la
      récupération — 11 tests unitaires dans `src/core/config/manager.rs`.
- [ ] Affichage du chemin de configuration résolu dans l'interface.
- [ ] Remplacement du repli relatif `config.toml` par un chemin absolu sûr.
- [ ] Éditeur de configuration intégré à l'interface.
- [ ] Presets de configuration.
