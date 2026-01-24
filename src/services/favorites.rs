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

#[cfg(test)]
mod tests {
    use super::FavoritesService;
    use std::path::PathBuf;

    #[test]
    fn add_and_remove_favorites() {
        let mut favorites = FavoritesService::default();

        assert!(favorites.add(PathBuf::from("/alpha")));
        assert!(!favorites.add(PathBuf::from("/alpha")));
        assert!(favorites.add(PathBuf::from("/beta")));
        assert_eq!(favorites.list().len(), 2);

        assert!(favorites.contains(PathBuf::from("/alpha").as_path()));
        assert!(favorites.remove(PathBuf::from("/alpha").as_path()));
        assert!(!favorites.contains(PathBuf::from("/alpha").as_path()));
        assert_eq!(favorites.list().len(), 1);
    }

    #[test]
    fn remove_missing_favorite_returns_false() {
        let mut favorites = FavoritesService::default();
        favorites.add(PathBuf::from("/alpha"));

        assert!(!favorites.remove(PathBuf::from("/beta").as_path()));
        assert_eq!(favorites.list().len(), 1);
    }
}
