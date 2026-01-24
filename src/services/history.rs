use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct HistoryService {
    entries: Vec<PathBuf>,
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

        self.entries.push(path);
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

    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }
}
