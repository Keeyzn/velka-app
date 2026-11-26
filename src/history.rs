//! Historique des chaînes (15 max, `history.txt`) et gestion des favoris.
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_HISTORY: usize = 15;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct History {
    /// Le plus récent en premier.
    pub items: Vec<String>,
}

impl History {
    pub fn load_from(path: &Path) -> Self {
        let items = fs::read_to_string(path)
            .map(|t| {
                t.lines()
                    .filter_map(crate::login::normalize)
                    .take(MAX_HISTORY)
                    .collect()
            })
            .unwrap_or_default();
        Self { items }
    }

    pub fn load() -> Self {
        Self::load_from(&history_path())
    }

    /// Ajoute une chaîne en tête, sans doublon, limité à `MAX_HISTORY`.
    pub fn push(&mut self, login: &str) {
        self.items.retain(|l| l != login);
        self.items.insert(0, login.to_string());
        self.items.truncate(MAX_HISTORY);
    }

    pub fn remove(&mut self, login: &str) {
        self.items.retain(|l| l != login);
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        crate::config::write_atomic(path, self.items.join("\n").as_bytes())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&history_path())
    }
}

pub fn history_path() -> PathBuf {
    crate::logging::data_dir().join("history.txt")
}

/// Ajoute ou retire un favori ; renvoie `true` s'il est désormais favori.
pub fn toggle_favorite(favorites: &mut Vec<String>, login: &str) -> bool {
    if let Some(i) = favorites.iter().position(|l| l == login) {
        favorites.remove(i);
        false
    } else {
        favorites.push(login.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_sans_doublon_et_limite() {
        let mut h = History::default();
        for i in 0..20 {
            h.push(&format!("chan{i}"));
        }
        assert_eq!(h.items.len(), MAX_HISTORY);
        h.push("chan10");
        assert_eq!(h.items[0], "chan10");
        assert_eq!(h.items.iter().filter(|l| *l == "chan10").count(), 1);
    }

    #[test]
    fn favoris() {
        let mut f = vec![];
        assert!(toggle_favorite(&mut f, "xqc"));
        assert!(!toggle_favorite(&mut f, "xqc"));
        assert!(f.is_empty());
    }

    #[test]
    fn sauvegarde_et_lecture() {
        let p = std::env::temp_dir().join(format!("velka-hist-{}.txt", std::process::id()));
        let mut h = History::default();
        h.push("a");
        h.push("b");
        h.save_to(&p).unwrap();
        assert_eq!(History::load_from(&p), h);
        let _ = fs::remove_file(&p);
    }
}
