# Configuration Xion

Ce document décrit le format de configuration versionné de Xion ainsi que les
valeurs par défaut et le comportement en cas d'erreur.

## Emplacement

Par défaut, la configuration est lue depuis :

- **Windows** : `%APPDATA%\\Xion\\config.toml`
- **Linux** : `~/.config/Xion/config.toml`
- **macOS** : `~/Library/Application Support/Xion/config.toml`

Si le dossier de configuration n'est pas disponible, Xion utilise `./config.toml`.

## Format (version 1)

Le fichier est un TOML versionné via la clé `version`. Les sections sont optionnelles ;
les valeurs absentes utilisent les défauts.

```toml
version = 1
start_path = "C:/"

[list]
show_hidden = false
sort_key = "name"           # name | modified | size
sort_order = "asc"          # asc | desc
directories_first = true
filter = "all"              # all | only-directories | only-files

[cache]
thumbnail_entries = 256
thumbnail_ttl_seconds = 300
directory_entries = 256
directory_ttl_seconds = 45

[filesystem]
metadata_batch_size = 256
metadata_parallelism = 4

[view]
mode = "list"              # list | grid
thumbnail_size = 48
row_height = 32.0
grid_columns = 4
grid_row_height = 140.0
overscan = 6
columns = ["name", "type", "size", "modified"]

[paging]
page_size = 120

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
```

## Migration

Si une configuration plus ancienne (sans `version`) est détectée, elle est
migrée automatiquement vers la version actuelle en conservant les champs connus
(`start_path`, `show_hidden`, `thumbnail_*`).

## Validation & fallback

Chaque champ est validé (plages min/max). En cas de valeur invalide :

- la valeur fautive est ignorée,
- la valeur par défaut correspondante est réappliquée,
- un avertissement est remonté à l'UI (statut d'action).

## Rechargement à chaud

La configuration est rechargée à chaque action de rafraîchissement (par défaut
`Ctrl+R`). Les caches dépendants sont réinitialisés si nécessaire.

## Avancement (checklist)

- [x] Format TOML versionné (v1) documenté.
- [x] Migration depuis une version non versionnée (V0).
- [x] Validation des champs + warnings UI.
- [x] Rechargement à chaud via `Ctrl+R`.
- [ ] Éditeur de configuration intégré à l'UI.
- [ ] Presets de configuration (Explorer-like, performance, minimal).
