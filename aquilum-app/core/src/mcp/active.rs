use crate::search::paths::same_path;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Default)]
pub struct ActiveNote {
    path: RwLock<Option<PathBuf>>,
}

impl ActiveNote {
    pub fn get(&self) -> Option<PathBuf> {
        self.path.read().ok().and_then(|path| path.clone())
    }

    pub fn set(&self, path: Option<PathBuf>) {
        if let Ok(mut current) = self.path.write() {
            *current = path;
        }
    }

    pub fn is(&self, path: &std::path::Path) -> bool {
        self.get().is_some_and(|active| same_path(&active, path))
    }
}
