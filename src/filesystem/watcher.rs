use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::core::AppResult;

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
