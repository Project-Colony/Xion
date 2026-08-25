//! Config file loading, saving, validation, and the V0/V1 file format structures.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

mod file_format;
mod limits;
mod migrate;
mod shortcuts_io;
mod validate;

use file_format::*;
use limits::*;
use migrate::*;
use shortcuts_io::*;
use validate::*;

use super::types::{AppConfig, AppConfigLoad, ConfigSource, ConfigWarning};

// ── Validation constants ──────────────────────────────────────────────────────

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Manages loading and saving the application config file.
#[derive(Debug, Clone)]
pub struct ConfigManager {
    path: PathBuf,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    /// Creates a new configuration manager with the default config path.
    pub fn new() -> Self {
        Self::with_path(default_config_path())
    }

    /// Creates a manager bound to an explicit file.
    ///
    /// The only constructor used to hardcode the user's config directory, so
    /// anything that saved during a test overwrote the developer's real
    /// `config.toml`. Tests point this at a temporary directory instead.
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Path of the copy kept by [`Self::save`] before each rewrite.
    pub fn backup_path(&self) -> PathBuf {
        sibling_path(&self.path, ".bak")
    }

    pub fn load(&self) -> AppConfigLoad {
        match load_from_path(&self.path) {
            Ok(load) => load,
            Err(error) => self.recover_unreadable_file(error),
        }
    }

    /// Saves the current config to disk. Errors are logged via tracing.
    pub fn save(&self, config: &AppConfig) {
        let file = config_to_file(config);
        let contents = match toml::to_string_pretty(&file) {
            Ok(contents) => contents,
            Err(error) => {
                tracing::warn!("Config: erreur sérialisation TOML: {error}");
                return;
            }
        };
        if let Err(error) = write_atomic(&self.path, &contents) {
            tracing::warn!(
                "Config: impossible d'écrire {}: {error}",
                self.path.display()
            );
        }
    }

    /// A config that fails to parse used to be left in place and silently
    /// flattened by the next `save()`. Move it aside and try the backup copy,
    /// so a truncated or hand-broken file never costs the whole configuration.
    fn recover_unreadable_file(&self, error: ConfigError) -> AppConfigLoad {
        let mut warnings = vec![ConfigWarning {
            message: error.to_string(),
        }];

        let quarantine = sibling_path(&self.path, &format!(".corrupt-{}", unix_timestamp()));
        match fs::rename(&self.path, &quarantine) {
            Ok(()) => warnings.push(ConfigWarning {
                message: format!(
                    "Config illisible mise de côté dans {}",
                    quarantine.display()
                ),
            }),
            Err(error) => warnings.push(ConfigWarning {
                message: format!("Config illisible, mise de côté impossible: {error}"),
            }),
        }

        let backup = self.backup_path();
        if let Ok(mut restored) = load_from_path(&backup)
            && !matches!(restored.source, ConfigSource::Default)
        {
            warnings.push(ConfigWarning {
                message: format!("Configuration restaurée depuis {}", backup.display()),
            });
            warnings.append(&mut restored.warnings);
            return AppConfigLoad {
                config: restored.config,
                source: restored.source,
                warnings,
            };
        }

        AppConfigLoad {
            config: AppConfig::default(),
            source: ConfigSource::Default,
            warnings,
        }
    }
}

// ── Atomic file publication ───────────────────────────────────────────────────

/// Appends `suffix` to the file name of `path`, keeping the same directory.
fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("config.toml"))
        .to_os_string();
    name.push(suffix);
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn remove_stale_temp(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        tracing::debug!(
            "Config: nettoyage de {} impossible: {error}",
            path.display()
        );
    }
}

/// Publishes `contents` at `path` without ever leaving a truncated file behind.
///
/// The previous `fs::write` truncated the live config first, so a crash or a
/// power cut during any of the saves triggered on every navigation left an
/// empty file and reset theme, tabs, favourites and column widths.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    // The temporary file must live in the target directory: `fs::rename` is
    // only atomic within one filesystem. The pid keeps two Xion instances from
    // fighting over the same temporary name.
    let temp_path = sibling_path(path, &format!(".tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temp_path)?;
    let written = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written {
        remove_stale_temp(&temp_path);
        return Err(error);
    }

    // Keep the last published copy: `load()` falls back to it when the live
    // file turns out to be unreadable.
    if path.exists()
        && let Err(error) = fs::copy(path, sibling_path(path, ".bak"))
    {
        tracing::debug!("Config: sauvegarde .bak impossible: {error}");
    }

    if let Err(error) = fs::rename(&temp_path, path) {
        remove_stale_temp(&temp_path);
        return Err(error);
    }

    Ok(())
}

// ── Loading logic ─────────────────────────────────────────────────────────────

fn default_config_path() -> PathBuf {
    // `locate::` et non `paths::config_dir` : ce dernier **crée** le dossier,
    // c'est la raison de son `io::Result`. Créé avant la migration, il faisait
    // croire à une configuration déjà en place, la migration était sautée, et
    // Xion écrivait des valeurs par défaut à côté d'une configuration
    // existante — constaté : deux onglets et un dossier de démarrage perdus.
    //
    // La documentation de `colony-ui` le dit : « afficher où les préférences
    // vivraient ne doit pas faire exister le dossier ». `save` crée son parent
    // lui-même, donc personne n'a besoin de le devancer.
    let Ok(dir) = colony_ui::paths::locate::config_dir(COLONY_PROGRAM) else {
        return PathBuf::from("config.toml");
    };

    if let Some(legacy) = legacy_config_dir() {
        migrate_legacy_config_dir(&legacy, &dir);
    }

    dir.join("config.toml")
}

/// Le nom du programme tel que l'écosystème l'écrit.
///
/// Capitalisé, pas un identifiant en minuscules : la convention est
/// `<racine>/Colony/<Programme>/`, avec `<Programme>` orthographié comme le
/// programme s'écrit — voir `design/filesystem.md` de Project-Colony-Resources.
const COLONY_PROGRAM: &str = "Xion";

/// Là où Xion écrivait avant d'adopter la disposition Colony.
///
/// `ProjectDirs::from("io", "xion", "Xion")` produisait `~/.config/xion/` sous
/// Linux. Les programmes Colony se rangent désormais côte à côte sous un même
/// dossier, pour qu'une sauvegarde de celui-ci les emporte tous.
fn legacy_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("io", "xion", "Xion").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Déplace une configuration antérieure vers son emplacement Colony.
///
/// Le dossier entier voyage, pas seulement `config.toml` : le `.bak` posé à côté
/// est le chemin de secours quand le fichier principal est illisible, et le
/// laisser derrière reviendrait à s'en priver sans le dire.
///
/// Ne fait rien si la destination existe déjà — une configuration récente ne
/// doit jamais être écrasée par une ancienne — et rien non plus si l'ancienne
/// est absente, ce qui est le cas de toute installation neuve.
fn migrate_legacy_config_dir(legacy: &Path, target: &Path) -> bool {
    if target.exists() || !legacy.is_dir() || legacy == target {
        return false;
    }

    if let Some(parent) = target.parent() {
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
    }

    // Un renommage suffit tant que les deux vivent sur le même système de
    // fichiers, ce qui est le cas ordinaire — les deux sont sous `~/.config`.
    if fs::rename(legacy, target).is_ok() {
        return true;
    }

    // Sinon on copie, et on ne supprime l'original que si tout est passé :
    // perdre la configuration de quelqu'un pour un déménagement de dossier
    // serait un très mauvais échange.
    if fs::create_dir_all(target).is_err() {
        return false;
    }
    let Ok(entries) = fs::read_dir(legacy) else {
        return false;
    };
    let mut copied_everything = true;
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        if fs::copy(entry.path(), target.join(entry.file_name())).is_err() {
            copied_everything = false;
        }
    }
    if copied_everything {
        let _ = fs::remove_dir_all(legacy);
    }
    copied_everything
}

fn load_from_path(path: &Path) -> Result<AppConfigLoad, ConfigError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppConfigLoad {
                config: AppConfig::default(),
                source: ConfigSource::Default,
                warnings: Vec::new(),
            });
        }
        Err(error) => {
            return Err(ConfigError {
                message: format!("Lecture config impossible: {error}"),
            });
        }
    };

    // An empty file used to be parsed as a valid V0 document and migrated
    // silently, which reset every setting without a single warning.
    if contents.trim().is_empty() {
        return Ok(AppConfigLoad {
            config: AppConfig::default(),
            source: ConfigSource::Default,
            warnings: vec![ConfigWarning {
                message: format!(
                    "Config vide ({}), valeurs par défaut utilisées",
                    path.display()
                ),
            }],
        });
    }

    let header: VersionHeader = toml::from_str(&contents).map_err(|error| ConfigError {
        message: format!("Header de config invalide: {error}"),
    })?;
    let version = header.version.unwrap_or(0);

    let mut warnings = Vec::new();
    if header.version.is_none() {
        warnings.push(ConfigWarning {
            message: "Clé 'version' absente: fichier relu au format V0, \
                      les réglages plus récents seront réinitialisés"
                .to_string(),
        });
    }
    let config = match version {
        0 => {
            let file: AppConfigFileV0 = toml::from_str(&contents).map_err(|error| ConfigError {
                message: format!("Config V0 invalide: {error}"),
            })?;
            let config = merge_from_v0(file, &mut warnings);
            return Ok(AppConfigLoad {
                config,
                source: ConfigSource::Migrated(path.to_path_buf()),
                warnings,
            });
        }
        CURRENT_CONFIG_VERSION => {
            let file: AppConfigFileV1 = toml::from_str(&contents).map_err(|error| ConfigError {
                message: format!("Config V{CURRENT_CONFIG_VERSION} invalide: {error}"),
            })?;
            merge_from_v1(file, &mut warnings)
        }
        other => {
            return Err(ConfigError {
                message: format!(
                    "Version de config {other} non supportée (max: {CURRENT_CONFIG_VERSION}). \
                     Mettez à jour Xion ou supprimez le fichier de config pour le recréer."
                ),
            });
        }
    };

    Ok(AppConfigLoad {
        config,
        source: ConfigSource::File(path.to_path_buf()),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::types::{ListConfig, SortKeyConfig, TabPersistConfig, ThemeConfig};

    /// Every test writes through `with_path`, never through `new()`: the
    /// default path is the developer's real `config.toml`.
    fn manager_in(dir: &std::path::Path) -> ConfigManager {
        ConfigManager::with_path(dir.join("config.toml"))
    }

    /// Le cas de tout utilisateur existant : la configuration vit encore dans
    /// l'ancien dossier au premier lancement de la version alignée.
    #[test]
    fn a_pre_colony_configuration_moves_with_its_backup() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("xion");
        let target = root.path().join("Colony").join("Xion");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("config.toml"), "theme = \"Nord\"").unwrap();
        std::fs::write(legacy.join("config.toml.bak"), "theme = \"Dark\"").unwrap();

        assert!(super::migrate_legacy_config_dir(&legacy, &target));

        assert_eq!(
            std::fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Nord\""
        );
        assert!(
            target.join("config.toml.bak").exists(),
            "la sauvegarde est le recours quand le fichier principal est illisible"
        );
        assert!(!legacy.exists(), "l'ancien dossier ne doit pas subsister");
    }

    /// Une configuration déjà écrite à l'emplacement Colony ne doit jamais être
    /// écrasée par une ancienne restée là.
    #[test]
    fn an_existing_colony_configuration_wins() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("xion");
        let target = root.path().join("Colony").join("Xion");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("config.toml"), "theme = \"Ancien\"").unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("config.toml"), "theme = \"Actuel\"").unwrap();

        assert!(!super::migrate_legacy_config_dir(&legacy, &target));
        assert_eq!(
            std::fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Actuel\""
        );
    }

    /// Une installation neuve n'a rien à migrer et ne doit rien créer.
    #[test]
    fn a_fresh_installation_migrates_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("absent");
        let target = root.path().join("Colony").join("Xion");
        assert!(!super::migrate_legacy_config_dir(&legacy, &target));
        assert!(!target.exists());
    }

    #[test]
    fn with_path_never_uses_the_user_config_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        assert_eq!(manager.path(), dir.path().join("config.toml"));
        assert_ne!(manager.path(), ConfigManager::new().path());
    }

    #[test]
    fn save_then_load_roundtrips_theme_and_tabs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());

        let config = AppConfig {
            start_path: dir.path().to_path_buf(),
            theme: ThemeConfig::Nord,
            dark_mode: true,
            list: ListConfig {
                sort_key: SortKeyConfig::Size,
                ..ListConfig::default()
            },
            tabs: vec![TabPersistConfig {
                path: dir.path().to_path_buf(),
            }],
            active_tab_index: 0,
            user_favorites: vec![dir.path().to_path_buf()],
            ..AppConfig::default()
        };
        manager.save(&config);

        let loaded = manager.load();
        assert!(
            loaded.warnings.is_empty(),
            "warnings: {:?}",
            loaded.warnings
        );
        assert_eq!(loaded.config.theme, ThemeConfig::Nord);
        assert_eq!(loaded.config.list.sort_key, SortKeyConfig::Size);
        assert_eq!(loaded.config.tabs.len(), 1);
        assert_eq!(loaded.config.user_favorites, vec![dir.path().to_path_buf()]);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        manager.save(&AppConfig::default());

        let leftovers: Vec<String> = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "fichiers temporaires restants: {leftovers:?}"
        );
        assert!(manager.path().exists());
    }

    #[test]
    fn save_keeps_the_previous_version_as_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());

        let first = AppConfig {
            theme: ThemeConfig::Nord,
            ..AppConfig::default()
        };
        manager.save(&first);
        // No previous file existed, so nothing to back up yet.
        assert!(!manager.backup_path().exists());

        let second = AppConfig {
            theme: ThemeConfig::Light,
            ..AppConfig::default()
        };
        manager.save(&second);

        let backup = fs::read_to_string(manager.backup_path()).expect("read backup");
        assert!(
            backup.contains("Nord"),
            "le .bak doit contenir la version précédente"
        );
        assert_eq!(manager.load().config.theme, ThemeConfig::Light);
    }

    #[test]
    fn unreadable_config_is_quarantined_and_restored_from_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());

        let good = AppConfig {
            theme: ThemeConfig::Solarized,
            ..AppConfig::default()
        };
        manager.save(&good);
        // Second save publishes the .bak holding the Solarized config.
        manager.save(&good);

        fs::write(manager.path(), "version = 1\nthis is not toml <<<").expect("corrupt");
        let loaded = manager.load();

        assert_eq!(loaded.config.theme, ThemeConfig::Solarized);
        assert!(
            !manager.path().exists(),
            "le fichier fautif doit être déplacé"
        );
        let quarantined = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"));
        assert!(
            quarantined,
            "le fichier fautif doit être conservé sous .corrupt-*"
        );
    }

    #[test]
    fn empty_config_falls_back_to_defaults_with_a_warning() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        fs::write(manager.path(), "   \n\n").expect("write empty");

        let loaded = manager.load();
        assert!(matches!(loaded.source, ConfigSource::Default));
        assert_eq!(loaded.warnings.len(), 1);
        assert!(
            manager.path().exists(),
            "un fichier vide n'est pas corrompu"
        );
    }

    #[test]
    fn migrates_v0_file_without_version_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        fs::write(
            manager.path(),
            format!(
                "start_path = {:?}\nshow_hidden = true\nthumbnail_size = 64\n\
                 thumbnail_cache_entries = 512\nthumbnail_cache_ttl_seconds = 120\n",
                dir.path().to_string_lossy()
            ),
        )
        .expect("write v0");

        let loaded = manager.load();
        assert!(matches!(loaded.source, ConfigSource::Migrated(_)));
        assert_eq!(loaded.config.start_path, dir.path());
        assert!(loaded.config.list.show_hidden);
        assert_eq!(loaded.config.view.thumbnail_size, 64);
        assert_eq!(loaded.config.cache.thumbnail_entries, 512);
        assert_eq!(loaded.config.cache.thumbnail_ttl_seconds, 120);
        // The missing `version` key must be reported, not silently assumed.
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.message.contains("version"))
        );
    }

    #[test]
    fn v0_migration_clamps_out_of_range_values() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        fs::write(
            manager.path(),
            "thumbnail_size = 4096\nthumbnail_cache_entries = 1\n",
        )
        .expect("write v0");

        let loaded = manager.load();
        let defaults = AppConfig::default();
        assert_eq!(
            loaded.config.view.thumbnail_size,
            defaults.view.thumbnail_size
        );
        assert_eq!(
            loaded.config.cache.thumbnail_entries,
            defaults.cache.thumbnail_entries
        );
        assert!(loaded.warnings.len() >= 2);
    }

    #[test]
    fn v0_dark_mode_key_is_ignored_but_v1_theme_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        // Legacy V1 file carrying only the boolean.
        fs::write(manager.path(), "version = 1\ndark_mode = true\n").expect("write v1");
        let loaded = manager.load();
        assert_eq!(loaded.config.theme, ThemeConfig::Dark);
        assert!(loaded.config.dark_mode);
    }

    #[test]
    fn dark_mode_is_recomputed_from_the_theme() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        fs::write(
            manager.path(),
            "version = 1\ndark_mode = false\ntheme = \"Nord\"\n",
        )
        .expect("write v1");

        let loaded = manager.load();
        assert_eq!(loaded.config.theme, ThemeConfig::Nord);
        assert!(loaded.config.dark_mode, "dark_mode doit suivre le thème");
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.message.contains("dark_mode"))
        );
    }

    #[test]
    fn unsupported_future_version_is_rejected_without_losing_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        fs::write(manager.path(), "version = 99\n").expect("write v99");

        let loaded = manager.load();
        assert!(matches!(loaded.source, ConfigSource::Default));
        assert!(!manager.path().exists());
        // The rejected file is preserved under .corrupt-* rather than dropped.
        let kept = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"));
        assert!(kept);
    }
}
