//! The session: the tabs Xion reopens at the next start.
//!
//! Kept out of `config.toml` on purpose. The configuration holds what the user
//! chose; the open tabs are something Xion produced while it ran, so they live
//! in the data directory (rule FS-5 of Project-Colony-Resources
//! `design/filesystem.md`). It also stops every navigation from rewriting the
//! user's settings file and its `.bak`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::manager::{COLONY_PROGRAM, write_atomic};
use super::{ConfigManager, TabPersistConfig};

const SESSION_FILE: &str = "session.toml";

/// What `session.toml` holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    /// Index into `tabs`. Clamped where the tabs are rebuilt, since the file
    /// can be edited by hand.
    pub active_tab_index: usize,
    pub tabs: Vec<TabPersistConfig>,
}

/// Loads and saves `session.toml`.
#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    /// A store bound to `session.toml` in Xion's Colony data directory.
    ///
    /// `locate::` rather than `paths::data_dir`, which creates the directory:
    /// loading should not, and [`Self::save`] creates it when it writes.
    pub fn new() -> Self {
        let path = colony_ui::paths::locate::data_dir(COLONY_PROGRAM)
            .map(|dir| dir.join(SESSION_FILE))
            .unwrap_or_else(|_| PathBuf::from(SESSION_FILE));
        Self::with_path(path)
    }

    /// A store bound to an explicit file, for tests.
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The saved session, or an empty one.
    ///
    /// The first time there is no `session.toml`, the tabs an older
    /// `config.toml` still carries are taken over and written here, so they
    /// survive the next config save, which drops them.
    pub fn load(&self, config: &ConfigManager) -> Session {
        match fs::read_to_string(&self.path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|error| {
                tracing::warn!(
                    "Session: {} is unreadable, starting from one tab: {error}",
                    self.path.display()
                );
                Session::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(session) = config.legacy_session() else {
                    return Session::default();
                };
                self.save(&session);
                session
            }
            Err(error) => {
                tracing::warn!(
                    "Session: cannot read {}, starting from one tab: {error}",
                    self.path.display()
                );
                Session::default()
            }
        }
    }

    /// Writes the session atomically (temporary file, then rename). Errors are
    /// logged: losing the tab list is not worth interrupting the user.
    pub fn save(&self, session: &Session) {
        let written = toml::to_string_pretty(session)
            .map_err(io::Error::other)
            .and_then(|contents| write_atomic(&self.path, &contents, false));
        if let Err(error) = written {
            tracing::warn!("Session: cannot write {}: {error}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(paths: &[&str]) -> Vec<TabPersistConfig> {
        paths
            .iter()
            .map(|path| TabPersistConfig {
                path: PathBuf::from(path),
            })
            .collect()
    }

    #[test]
    fn a_session_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigManager::with_path(dir.path().join("config.toml"));
        let store = SessionStore::with_path(dir.path().join("data").join(SESSION_FILE));
        let session = Session {
            active_tab_index: 1,
            tabs: tabs(&["/a", "/b"]),
        };

        store.save(&session);

        assert_eq!(store.load(&config), session);
        assert!(
            !dir.path().join("data").join("session.toml.bak").exists(),
            "only the config loader reads a .bak back"
        );
    }

    #[test]
    fn tabs_move_from_an_older_config_to_the_session_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigManager::with_path(dir.path().join("config.toml"));
        let store = SessionStore::with_path(dir.path().join(SESSION_FILE));
        fs::write(
            config.path(),
            "version = 1\nactive_tab_index = 1\ntheme_family = \"nord\"\n\
             theme_variant = \"dark\"\n\n[[tabs]]\npath = \"/a\"\n\n[[tabs]]\npath = \"/b\"\n",
        )
        .expect("write config");

        // The config still loads cleanly with the old keys in it.
        let loaded = config.load();
        assert!(
            loaded.warnings.is_empty(),
            "warnings: {:?}",
            loaded.warnings
        );
        assert_eq!(loaded.config.theme.keys(), ("nord", "dark"));

        let expected = Session {
            active_tab_index: 1,
            tabs: tabs(&["/a", "/b"]),
        };
        assert_eq!(store.load(&config), expected);
        assert!(
            store.path().exists(),
            "the tabs are written to session.toml"
        );

        // The next config save drops them from config.toml...
        config.save(&loaded.config);
        let rewritten = fs::read_to_string(config.path()).expect("read config");
        assert!(!rewritten.contains("tabs"), "config.toml: {rewritten}");
        assert_eq!(config.legacy_session(), None);

        // ...and they are still restored, from session.toml this time.
        assert_eq!(store.load(&config), expected);
    }

    #[test]
    fn an_existing_session_file_wins_over_an_older_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigManager::with_path(dir.path().join("config.toml"));
        let store = SessionStore::with_path(dir.path().join(SESSION_FILE));
        fs::write(config.path(), "version = 1\n[[tabs]]\npath = \"/old\"\n").expect("write");
        let session = Session {
            active_tab_index: 0,
            tabs: tabs(&["/new"]),
        };
        store.save(&session);

        assert_eq!(store.load(&config), session);
    }

    #[test]
    fn a_missing_session_and_no_legacy_tabs_load_empty_and_write_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = ConfigManager::with_path(dir.path().join("config.toml"));
        let store = SessionStore::with_path(dir.path().join(SESSION_FILE));

        assert_eq!(store.load(&config), Session::default());
        assert!(!store.path().exists());
    }
}
