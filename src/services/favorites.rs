use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FavoritesService {
    favorites: Vec<PathBuf>,
}

impl FavoritesService {
    pub fn add(&mut self, path: PathBuf) -> bool {
        if self.contains(&path) {
            return false;
        }
        self.favorites.push(path);
        true
    }

    pub fn remove(&mut self, path: &Path) -> bool {
        let original_len = self.favorites.len();
        self.favorites.retain(|favorite| favorite != path);
        original_len != self.favorites.len()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.favorites.iter().any(|favorite| favorite == path)
    }

    pub fn list(&self) -> &[PathBuf] {
        &self.favorites
    }
}
