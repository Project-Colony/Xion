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
        .map_err(|error| XionError::Watcher(error.to_string()))?;

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
            .map_err(|error| XionError::Watcher(error.to_string()))?;
        self.watched.insert(path.to_path_buf());
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> AppResult<()> {
        if !self.watched.remove(path) {
            return Ok(());
        }
        self.watcher
            .unwatch(path)
            .map_err(|error| XionError::Watcher(error.to_string()))?;
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
