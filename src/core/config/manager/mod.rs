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

use super::session::Session;
use super::types::{AppConfig, AppConfigLoad, ConfigSource, ConfigWarning, TabPersistConfig};

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
    ///
    /// Computing that path runs the one-time copy of the pre-Colony folder
    /// (`migrate_legacy_config_dir`) against the real home directory, so tests
    /// use [`Self::with_path`] instead.
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
        if let Err(error) = write_atomic(&self.path, &contents, true) {
            tracing::warn!(
                "Config: impossible d'écrire {}: {error}",
                self.path.display()
            );
        }
    }

    /// The tabs a `config.toml` written before `session.toml` existed still
    /// carries, or `None` when it has none.
    ///
    /// [`SessionStore::load`](super::SessionStore::load) reads them once, when
    /// there is no session file yet. [`Self::save`] never writes them back.
    pub fn legacy_session(&self) -> Option<Session> {
        let contents = fs::read_to_string(&self.path).ok()?;
        let file: LegacySessionFile = toml::from_str(&contents).ok()?;
        let tabs: Vec<TabPersistConfig> = file
            .tabs?
            .into_iter()
            .filter_map(|tab| tab.path.map(|path| TabPersistConfig { path }))
            .collect();
        let last = tabs.len().checked_sub(1)?;
        Some(Session {
            active_tab_index: file.active_tab_index.unwrap_or(0).min(last),
            tabs,
        })
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
///
/// `keep_backup` also keeps the previous copy as `<name>.bak`, which only the
/// config loader reads back.
pub(in crate::core::config) fn write_atomic(
    path: &Path,
    contents: &str,
    keep_backup: bool,
) -> std::io::Result<()> {
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
    if keep_backup
        && path.exists()
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
        return PathBuf::from(CONFIG_FILE);
    };

    match legacy_config_dir() {
        Some(legacy) => migrate_legacy_config_dir(&legacy, &dir).join(CONFIG_FILE),
        None => dir.join(CONFIG_FILE),
    }
}

/// Le nom du programme tel que l'écosystème l'écrit.
///
/// Capitalisé, pas un identifiant en minuscules : la convention est
/// `<racine>/Colony/<Programme>/`, avec `<Programme>` orthographié comme le
/// programme s'écrit — voir `design/filesystem.md` de Project-Colony-Resources.
pub(in crate::core::config) const COLONY_PROGRAM: &str = "Xion";

/// Là où Xion écrivait avant d'adopter la disposition Colony.
///
/// `ProjectDirs::from("io", "xion", "Xion")` produisait `~/.config/xion/` sous
/// Linux. Les programmes Colony se rangent désormais côte à côte sous un même
/// dossier, pour qu'une sauvegarde de celui-ci les emporte tous.
fn legacy_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("io", "xion", "Xion").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Written into the legacy directory once its contents reached the Colony
/// location, so later starts skip the migration and a later release knows the
/// old directory is safe to remove.
const MIGRATED_MARKER: &str = ".migrated";

const CONFIG_FILE: &str = "config.toml";

/// Copies a pre-Colony configuration to its Colony location and returns the
/// directory to use for this run.
///
/// The whole directory travels, not only `config.toml`: the `.bak` next to it
/// is the fallback when the main file is unreadable, and leaving it behind
/// would silently lose it.
///
/// The legacy directory is never moved or deleted in this release (rule FS-7 of
/// Project-Colony-Resources `design/filesystem.md`): if the migration turns out
/// to be wrong, the user's files are still where they were. Files are copied
/// into a temporary sibling of `target`, which only becomes `target` once every
/// file made it, so a half-copied profile is never picked up.
///
/// Returns `target` when there is nothing to do: the marker is present, the
/// target already holds a `config.toml` (a newer configuration never loses to
/// an older one), or there is no legacy directory, which is the case of every
/// fresh install. Returns `legacy` when the copy failed, so the user keeps their
/// profile for this run instead of getting an empty one; the next start tries
/// again.
fn migrate_legacy_config_dir<'a>(legacy: &'a Path, target: &'a Path) -> &'a Path {
    // `config.toml`, not the folder itself: on Windows and macOS the data
    // directory is this same folder, so a run that fell back to `legacy` still
    // creates it to save `session.toml`. Taking that folder for a migrated
    // profile would skip the migration for good and start on defaults.
    if legacy == target
        || target.join(CONFIG_FILE).exists()
        || !legacy.is_dir()
        || legacy.join(MIGRATED_MARKER).exists()
    {
        return target;
    }

    let staging = sibling_path(target, ".migrating");
    let claimed = target
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| claim_staging(&staging));
    if let Err(error) = claimed {
        // The staging folder may belong to another start: leave it alone.
        tracing::warn!(
            "Config: cannot prepare {}, using the old location for now: {error}",
            staging.display()
        );
        return legacy;
    }

    let copied =
        copy_flat_directory(legacy, &staging).and_then(|()| publish_staging(&staging, target));
    if let Err(error) = copied {
        tracing::warn!(
            "Config: copy of {} to {} failed, using the old location for now: {error}",
            legacy.display(),
            target.display()
        );
        if staging.exists()
            && let Err(error) = fs::remove_dir_all(&staging)
        {
            tracing::debug!("Config: cleanup of {} failed: {error}", staging.display());
        }
        return legacy;
    }

    // `target` now holds `config.toml`, which already stops the next start
    // from migrating again: a missing marker costs nothing but the hint for a
    // later release.
    if let Err(error) = fs::write(
        legacy.join(MIGRATED_MARKER),
        format!("Copied to {}\n", target.display()),
    ) {
        tracing::warn!("Config: could not write the migration marker: {error}");
    }
    target
}

/// A staging folder older than this was left by a start that crashed, and is
/// removed. A younger one may belong to another Xion starting at the same
/// moment: removing it could publish that start's half-done copy.
const STALE_STAGING: std::time::Duration = std::time::Duration::from_secs(60);

/// Creates the staging folder, replacing one a crashed start left behind.
///
/// The name is fixed, so a leftover is found again rather than piling up in the
/// shared `Colony/` folder. `create_dir` fails when the folder exists, which is
/// what keeps two starts from copying into the same one.
fn claim_staging(staging: &Path) -> std::io::Result<()> {
    match fs::create_dir(staging) {
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let stale = fs::metadata(staging)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > STALE_STAGING);
            if !stale {
                return Err(error);
            }
            fs::remove_dir_all(staging)?;
            fs::create_dir(staging)
        }
        result => result,
    }
}

/// Puts the complete copy in `staging` at `target`.
fn publish_staging(staging: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        return fs::rename(staging, target);
    }
    // `target` exists without a `config.toml` (see `migrate_legacy_config_dir`):
    // its files stay, and the copies are moved in one by one. `config.toml`
    // goes last, since it is what tells a later start the migration is done.
    let mut names = fs::read_dir(staging)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort_by_key(|name| name.as_os_str() == CONFIG_FILE);
    for name in &names {
        fs::rename(staging.join(name), target.join(name))?;
    }
    fs::remove_dir(staging)
}

/// Copies every file of `source` into the empty directory `destination`.
///
/// Only knows files, so a sub-directory is an error rather than something to
/// skip: skipping it while reporting success is how an earlier version deleted
/// a folder it had never copied. Xion itself writes no sub-directory.
///
/// Each copy is synced before returning: the caller renames `destination` into
/// place next, and a power cut must not leave a published profile of empty
/// files.
fn copy_flat_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        // `metadata` follows symlinks: a config file linked in by a dotfile
        // manager is copied as the file it points to.
        if !fs::metadata(entry.path())?.is_file() {
            return Err(std::io::Error::other(format!(
                "{} is not a regular file",
                entry.path().display()
            )));
        }
        // Not `fs::copy`: it carries a read-only attribute over, and Windows
        // refuses to flush a handle that cannot write. Writing through a
        // handle of our own is what lets the copy be synced.
        let mut copy = fs::File::create(destination.join(entry.file_name()))?;
        std::io::copy(&mut fs::File::open(entry.path())?, &mut copy)?;
        copy.sync_all()?;
    }
    Ok(())
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
    use crate::core::config::types::{ListConfig, SortKeyConfig, ThemeChoice};

    /// Every test writes through `with_path`, never through `new()`: the
    /// default path is the developer's real `config.toml`.
    fn manager_in(dir: &std::path::Path) -> ConfigManager {
        ConfigManager::with_path(dir.join("config.toml"))
    }

    /// A legacy directory and its Colony target, side by side in a tempdir.
    fn legacy_and_target(root: &Path) -> (PathBuf, PathBuf) {
        let legacy = root.join("xion");
        let target = root.join("Colony").join("Xion");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("config.toml"), "theme = \"Nord\"").unwrap();
        fs::write(legacy.join("config.toml.bak"), "theme = \"Dark\"").unwrap();
        (legacy, target)
    }

    /// Every existing user's case: on the first start of the aligned version,
    /// the configuration still lives in the old directory.
    #[test]
    fn a_pre_colony_configuration_is_copied_with_its_backup() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());

        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);

        assert_eq!(
            fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Nord\""
        );
        assert!(
            target.join("config.toml.bak").exists(),
            "the backup is the fallback when the main file is unreadable"
        );
        assert!(
            legacy.join("config.toml").exists(),
            "the old directory stays until a later release"
        );
        assert!(legacy.join(MIGRATED_MARKER).exists());
        assert!(
            !target.join(MIGRATED_MARKER).exists(),
            "the marker belongs to the old directory only"
        );
    }

    /// The marker is what a second start checks: even with the Colony copy
    /// gone, the old directory is not copied again.
    #[test]
    fn a_second_start_does_not_migrate_again() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);

        fs::remove_dir_all(&target).unwrap();
        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);
        assert!(!target.exists());
    }

    /// A configuration already written at the Colony location must never be
    /// overwritten by an older one left behind.
    #[test]
    fn an_existing_colony_configuration_wins() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("config.toml"), "theme = \"Actuel\"").unwrap();

        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);
        assert_eq!(
            fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Actuel\""
        );
    }

    /// A copy that cannot take everything publishes nothing, deletes nothing,
    /// and points this run at the old directory instead of an empty profile.
    fn assert_failed_copy_falls_back(legacy: &Path, target: &Path) {
        assert_eq!(
            super::migrate_legacy_config_dir(legacy, target),
            legacy,
            "this run must keep using the old directory"
        );
        assert!(!target.exists(), "no half-copied profile is published");
        assert!(legacy.join("config.toml").exists());
        assert!(!legacy.join(MIGRATED_MARKER).exists());
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect();
        assert!(leftovers.is_empty(), "staging left behind: {leftovers:?}");
    }

    #[test]
    fn a_sub_directory_fails_the_copy_and_keeps_the_old_profile() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        fs::create_dir_all(legacy.join("sous-dossier")).unwrap();
        fs::write(legacy.join("sous-dossier").join("precieux"), "à garder").unwrap();

        assert_failed_copy_falls_back(&legacy, &target);
        assert!(legacy.join("sous-dossier").join("precieux").exists());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_file_fails_the_copy_and_keeps_the_old_profile() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        let locked = legacy.join("config.toml.bak");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&locked).is_ok() {
            // Running as root: permissions do not stop the read, so there is
            // no failure to observe.
            return;
        }

        assert_failed_copy_falls_back(&legacy, &target);
    }

    /// On Windows and macOS the data directory is the Colony config folder
    /// itself, so a run that fell back to the old location still creates that
    /// folder to save `session.toml`. The folder alone must not pass for a
    /// migrated profile, or every later start would open on defaults.
    #[test]
    fn a_folder_holding_only_the_session_is_not_a_migrated_profile() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        let blocker = legacy.join("sous-dossier");
        fs::create_dir_all(&blocker).unwrap();
        assert_failed_copy_falls_back(&legacy, &target);

        // What that run saved, where the data directory is off Linux.
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("session.toml"), "active_tab_index = 0\n").unwrap();

        assert_eq!(
            super::migrate_legacy_config_dir(&legacy, &target),
            legacy,
            "the copy still fails, so the old profile is still the one used"
        );
        assert!(!target.join("config.toml").exists());

        fs::remove_dir(&blocker).unwrap();
        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);
        assert_eq!(
            fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Nord\""
        );
        assert!(target.join("config.toml.bak").exists());
        assert!(
            target.join("session.toml").exists(),
            "the session saved meanwhile is kept"
        );
        assert!(legacy.join(MIGRATED_MARKER).exists());
        assert!(!sibling_path(&target, ".migrating").exists());
    }

    /// A staging folder this recent may be another start copying right now:
    /// it is left alone, and this run keeps the old profile.
    #[test]
    fn a_recent_staging_folder_is_left_to_its_owner() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        let staging = sibling_path(&target, ".migrating");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("config.toml"), "partial").unwrap();

        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), legacy);
        assert!(staging.join("config.toml").exists());
        assert!(!target.exists());
        assert!(!legacy.join(MIGRATED_MARKER).exists());
    }

    /// A staging folder left by a start that crashed is replaced, so it does
    /// not stay in the shared `Colony/` folder forever.
    #[cfg(unix)]
    #[test]
    fn a_staging_folder_left_by_a_crash_is_replaced() {
        let root = tempfile::tempdir().expect("tempdir");
        let (legacy, target) = legacy_and_target(root.path());
        let staging = sibling_path(&target, ".migrating");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("config.toml"), "partial").unwrap();
        let long_ago = std::time::SystemTime::now() - 2 * STALE_STAGING;
        fs::File::open(&staging)
            .and_then(|dir| dir.set_modified(long_ago))
            .unwrap();

        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);
        assert_eq!(
            fs::read_to_string(target.join("config.toml")).unwrap(),
            "theme = \"Nord\""
        );
        assert!(!staging.exists());
    }

    /// A fresh install has nothing to migrate and must create nothing.
    #[test]
    fn a_fresh_installation_migrates_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("absent");
        let target = root.path().join("Colony").join("Xion");
        assert_eq!(super::migrate_legacy_config_dir(&legacy, &target), target);
        assert!(!target.exists());
    }

    #[test]
    fn with_path_never_uses_the_user_config_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());
        assert_eq!(manager.path(), dir.path().join("config.toml"));
        // Not `ConfigManager::new()`: it runs the migration against the real
        // home directory.
        let user_config = colony_ui::paths::locate::config_dir(COLONY_PROGRAM)
            .map(|dir| dir.join(CONFIG_FILE))
            .ok();
        assert_ne!(Some(manager.path().to_path_buf()), user_config);
    }

    #[test]
    fn save_then_load_roundtrips_theme_and_favorites() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());

        let config = AppConfig {
            start_path: dir.path().to_path_buf(),
            theme: ThemeChoice {
                family: "nord".to_string(),
                variant: "dark".to_string(),
                high_contrast: false,
                accent: None,
            },
            dark_mode: true,
            list: ListConfig {
                sort_key: SortKeyConfig::Size,
                ..ListConfig::default()
            },
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
        assert_eq!(loaded.config.theme.keys(), ("nord", "dark"));
        assert_eq!(loaded.config.list.sort_key, SortKeyConfig::Size);
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
            theme: ThemeChoice {
                family: "nord".to_string(),
                variant: "dark".to_string(),
                high_contrast: false,
                accent: None,
            },
            ..AppConfig::default()
        };
        manager.save(&first);
        // No previous file existed, so nothing to back up yet.
        assert!(!manager.backup_path().exists());

        let second = AppConfig {
            theme: ThemeChoice {
                family: "gruvbox".to_string(),
                variant: "light".to_string(),
                high_contrast: false,
                accent: None,
            },
            ..AppConfig::default()
        };
        manager.save(&second);

        let backup = fs::read_to_string(manager.backup_path()).expect("read backup");
        assert!(
            backup.contains("nord"),
            "le .bak doit contenir la version précédente"
        );
        assert_eq!(manager.load().config.theme.keys(), ("gruvbox", "light"));
    }

    #[test]
    fn unreadable_config_is_quarantined_and_restored_from_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager_in(dir.path());

        let good = AppConfig {
            theme: ThemeChoice {
                family: "solarized".to_string(),
                variant: "dark".to_string(),
                high_contrast: false,
                accent: None,
            },
            ..AppConfig::default()
        };
        manager.save(&good);
        // Second save publishes the .bak holding the Solarized config.
        manager.save(&good);

        fs::write(manager.path(), "version = 1\nthis is not toml <<<").expect("corrupt");
        let loaded = manager.load();

        assert_eq!(loaded.config.theme.keys(), ("solarized", "dark"));
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
        assert_eq!(loaded.config.theme.keys(), ("gruvbox", "dark"));
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
        assert_eq!(loaded.config.theme.keys(), ("nord", "dark"));
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
