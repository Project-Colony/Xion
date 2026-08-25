use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use crate::core::{AppResult, XionError};
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const MAX_QUEUED_EVENTS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Removed,
    Renamed,
}

#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: WatchEventKind,
}

pub trait FileWatcher {
    fn watch(&mut self, path: &Path) -> AppResult<()>;
    fn unwatch(&mut self, path: &Path) -> AppResult<()>;
    fn poll(&mut self) -> AppResult<Vec<WatchEvent>>;
}

/// Traduit une erreur de `notify` en gardant son type d'entrée-sortie.
///
/// L'aplatir en texte — ce que faisait `XionError::Watcher(error.to_string())`
/// — perdait le `io::ErrorKind`, donc l'appelant ne pouvait plus distinguer une
/// permission refusée, qui est normale et sur laquelle l'utilisateur ne peut
/// rien, d'une panne réelle comme une limite d'inotify atteinte, qui se corrige.
///
/// Le texte de `notify` cite en outre les chemins concernés au format `Debug`,
/// d'où le `about ["/boot"]` que la barre d'état affichait à côté du chemin déjà
/// écrit en clair.
fn watcher_error(error: notify::Error) -> XionError {
    match error.kind {
        notify::ErrorKind::Io(io) => XionError::Io(io),
        other => XionError::Watcher(format!("{other:?}")),
    }
}

#[derive(Debug)]
pub struct NativeFileWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    watched: HashSet<PathBuf>,
    queued: VecDeque<WatchEvent>,
}

impl NativeFileWatcher {
    pub fn new() -> AppResult<Self> {
        let (sender, receiver) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(watcher_error)?;

        Ok(Self {
            watcher,
            receiver,
            watched: HashSet::new(),
            queued: VecDeque::new(),
        })
    }

    fn enqueue_event(&mut self, event: Event) {
        let Some(kind) = Self::map_kind(&event.kind) else {
            return;
        };

        for path in event.paths {
            if self.queued.len() < MAX_QUEUED_EVENTS {
                self.queued.push_back(WatchEvent { path, kind });
            } else {
                tracing::warn!("FileWatcher: queue pleine ({MAX_QUEUED_EVENTS}), events ignorés");
                break;
            }
        }
    }

    fn map_kind(kind: &EventKind) -> Option<WatchEventKind> {
        match kind {
            EventKind::Create(_) => Some(WatchEventKind::Created),
            EventKind::Modify(modify) => match modify {
                ModifyKind::Name(RenameMode::Any | RenameMode::From | RenameMode::To) => {
                    Some(WatchEventKind::Renamed)
                }
                _ => Some(WatchEventKind::Modified),
            },
            EventKind::Remove(_) => Some(WatchEventKind::Removed),
            _ => None,
        }
    }
}

impl FileWatcher for NativeFileWatcher {
    fn watch(&mut self, path: &Path) -> AppResult<()> {
        if self.watched.contains(path) {
            return Ok(());
        }
        self.watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(watcher_error)?;
        self.watched.insert(path.to_path_buf());
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> AppResult<()> {
        if !self.watched.remove(path) {
            return Ok(());
        }
        self.watcher.unwatch(path).map_err(watcher_error)?;
        Ok(())
    }

    fn poll(&mut self) -> AppResult<Vec<WatchEvent>> {
        let events: Vec<_> = self.receiver.try_iter().collect();
        for event in events {
            match event {
                Ok(event) => self.enqueue_event(event),
                Err(error) => {
                    return Err(XionError::Watcher(error.to_string()));
                }
            }
        }

        Ok(self.queued.drain(..).collect())
    }
}

#[derive(Debug, Default)]
pub struct NoopFileWatcher {
    queued: VecDeque<WatchEvent>,
}

impl NoopFileWatcher {
    pub fn new() -> Self {
        Self {
            queued: VecDeque::new(),
        }
    }

    pub fn push_event(&mut self, event: WatchEvent) {
        self.queued.push_back(event);
    }
}

impl FileWatcher for NoopFileWatcher {
    fn watch(&mut self, _path: &Path) -> AppResult<()> {
        Ok(())
    }

    fn unwatch(&mut self, _path: &Path) -> AppResult<()> {
        Ok(())
    }

    fn poll(&mut self) -> AppResult<Vec<WatchEvent>> {
        Ok(self.queued.drain(..).collect())
    }
}

#[cfg(test)]
mod watcher_error_tests {
    use super::watcher_error;
    use crate::core::XionError;
    use std::io;

    /// Le cas rencontré : ouvrir `/boot`, qui est en `drwx------ root:root`.
    /// L'appelant doit pouvoir reconnaître un refus pour se taire.
    #[test]
    fn a_permission_refusal_keeps_its_io_kind() {
        let error = notify::Error {
            kind: notify::ErrorKind::Io(io::Error::from(io::ErrorKind::PermissionDenied)),
            paths: vec!["/boot".into()],
        };
        match watcher_error(error) {
            XionError::Io(io) => assert_eq!(io.kind(), io::ErrorKind::PermissionDenied),
            other => panic!("type d'erreur perdu : {other}"),
        }
    }

    /// Une limite d'inotify atteinte se corrige, elle
    /// (`fs.inotify.max_user_watches`) : elle doit rester signalée, donc
    /// arriver distincte d'un refus de permission.
    #[test]
    fn a_real_failure_is_not_mistaken_for_a_refusal() {
        let error = notify::Error {
            kind: notify::ErrorKind::MaxFilesWatch,
            paths: Vec::new(),
        };
        assert!(matches!(watcher_error(error), XionError::Watcher(_)));
    }

    /// Le message ne doit plus porter la liste de chemins que `notify` ajoute
    /// au format `Debug` : c'est elle qui produisait `about ["/boot"]` dans la
    /// barre d'état, à côté du chemin déjà écrit en clair.
    #[test]
    fn the_message_no_longer_carries_a_debug_path_list() {
        let error = notify::Error {
            kind: notify::ErrorKind::WatchNotFound,
            paths: vec!["/boot".into()],
        };
        let rendered = watcher_error(error).to_string();
        assert!(!rendered.contains("about"), "message : {rendered}");
        assert!(!rendered.contains("/boot"), "message : {rendered}");
    }
}
