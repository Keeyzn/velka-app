//! Aides pour les tests : un dossier temporaire supprimé à la fin du test, même s'il échoue.
use std::path::{Path, PathBuf};

pub struct TempDir(PathBuf);

impl TempDir {
    /// Dossier neuf `velka-test-<pid>-<nom>` dans le dossier temporaire du système.
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("velka-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        TempDir(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
