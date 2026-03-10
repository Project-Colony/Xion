use std::collections::VecDeque;
use std::path::PathBuf;

const MAX_HISTORY: usize = 200;

#[derive(Debug, Default)]
pub struct HistoryService {
    entries: VecDeque<PathBuf>,
    cursor: Option<usize>,
}

impl HistoryService {
    pub fn record(&mut self, path: PathBuf) {
        match self.cursor {
            Some(index) if index + 1 < self.entries.len() => {
                self.entries.truncate(index + 1);
            }
            _ => {}
        }

        self.entries.push_back(path);

        if self.entries.len() > MAX_HISTORY {
            self.entries.pop_front();
        }

        self.cursor = Some(self.entries.len() - 1);
    }

    pub fn back(&mut self) -> Option<PathBuf> {
        let index = self.cursor?;
        if index == 0 {
            return None;
        }
        let new_index = index - 1;
        self.cursor = Some(new_index);
        self.entries.get(new_index).cloned()
    }

    pub fn forward(&mut self) -> Option<PathBuf> {
        let index = self.cursor?;
        if index + 1 >= self.entries.len() {
            return None;
        }
        let new_index = index + 1;
        self.cursor = Some(new_index);
        self.entries.get(new_index).cloned()
    }

    pub fn current(&self) -> Option<&PathBuf> {
        self.cursor.and_then(|index| self.entries.get(index))
    }

    pub fn can_back(&self) -> bool {
        matches!(self.cursor, Some(index) if index > 0)
    }

    pub fn can_forward(&self) -> bool {
        matches!(self.cursor, Some(index) if index + 1 < self.entries.len())
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = &PathBuf> + DoubleEndedIterator {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::HistoryService;
    use std::path::PathBuf;

    #[test]
    fn record_sets_current_and_truncates_forward_history() {
        let mut history = HistoryService::default();
        history.record(PathBuf::from("/one"));
        history.record(PathBuf::from("/two"));
        history.record(PathBuf::from("/three"));

        assert!(history.can_back());
        assert!(!history.can_forward());
        assert_eq!(history.current(), Some(&PathBuf::from("/three")));

        assert_eq!(history.back(), Some(PathBuf::from("/two")));
        assert_eq!(history.current(), Some(&PathBuf::from("/two")));

        history.record(PathBuf::from("/four"));
        assert_eq!(history.entries().len(), 3);
        assert_eq!(history.current(), Some(&PathBuf::from("/four")));
        assert!(!history.can_forward());
    }

    #[test]
    fn back_and_forward_navigate_history() {
        let mut history = HistoryService::default();
        history.record(PathBuf::from("/one"));
        history.record(PathBuf::from("/two"));
        history.record(PathBuf::from("/three"));

        assert_eq!(history.back(), Some(PathBuf::from("/two")));
        assert_eq!(history.back(), Some(PathBuf::from("/one")));
        assert_eq!(history.back(), None);

        assert_eq!(history.forward(), Some(PathBuf::from("/two")));
        assert_eq!(history.forward(), Some(PathBuf::from("/three")));
        assert_eq!(history.forward(), None);
    }
}
