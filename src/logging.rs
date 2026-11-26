//! Journal `log.txt` avec rotation à 200 Ko.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MAX_BYTES: u64 = 200 * 1024;
static PATH: OnceLock<PathBuf> = OnceLock::new();

/// Dossier de données `%LOCALAPPDATA%\Velka`.
pub fn data_dir() -> PathBuf {
    if crate::smoke::enabled() {
        return crate::smoke::data_dir();
    }
    // Versions de développement seulement : `VELKA_DATA_DIR=<dossier>` lance une instance avec ses propres réglages,
    // favoris et composants (essais, captures d'écran), sans toucher aux données habituelles. Sans effet en release.
    if let Some(dir) = isolated_dir() {
        return dir;
    }
    directories::BaseDirs::new()
        .map(|d| d.data_local_dir().join("Velka"))
        .unwrap_or_else(|| PathBuf::from("Velka"))
}

/// Dossier de données d'une instance isolée (`VELKA_DATA_DIR`, versions de développement seulement).
pub fn isolated_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var_os("VELKA_DATA_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
}

pub fn init() {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("log.txt");
    if fs::metadata(&path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, dir.join("log.old.txt"));
    }
    let _ = PATH.set(path);
    log("Démarrage de Velka");
}

/// Fichier du journal détaillé du lecteur (`mpv.log`). Il contient le chemin du dossier utilisateur et les
/// chaînes regardées : il n'est écrit qu'en mode diagnostic (`--diag`). Sans `--diag`, on renvoie `None` et le fichier
/// d'une session précédente est supprimé.
pub fn mpv_log(diag: bool) -> Option<PathBuf> {
    mpv_log_in(&data_dir(), diag)
}

fn mpv_log_in(dir: &Path, diag: bool) -> Option<PathBuf> {
    let path = dir.join("mpv.log");
    if diag {
        return Some(path);
    }
    let _ = fs::remove_file(&path);
    None
}

pub fn log(msg: &str) {
    if let Some(p) = PATH.get() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // Une seule écriture par ligne : plusieurs `write!` de fils différents s'entremêlaient dans le fichier.
            let _ = f.write_all(format_line(secs, msg).as_bytes());
        }
    }
}

/// Ligne complète du journal, composée d'avance pour être écrite en un seul appel.
fn format_line(secs: u64, msg: &str) -> String {
    format!(
        "[{secs}] {msg}
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligne_complete_en_un_morceau() {
        assert_eq!(
            format_line(1795201907, "Démarrage"),
            "[1795201907] Démarrage
"
        );
    }

    /// Journal du lecteur : en mode diagnostic il est gardé, sans mode diagnostic il n'est pas écrit et
    /// celui d'une session précédente est supprimé.
    #[test]
    fn journal_du_lecteur_seulement_en_diagnostic() {
        let dir = std::env::temp_dir().join(format!("velka-test-{}-mpvlog", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mpv.log");
        fs::write(&file, "ancien journal").unwrap();
        assert_eq!(mpv_log_in(&dir, true), Some(file.clone()));
        assert!(file.exists(), "le mode diagnostic ne supprime rien");
        assert_eq!(mpv_log_in(&dir, false), None);
        assert!(
            !file.exists(),
            "sans diagnostic, l'ancien journal est supprimé"
        );
        assert_eq!(
            mpv_log_in(&dir, false),
            None,
            "rien à supprimer : pas d'erreur"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Des fils qui journalisent en même temps ne mélangent pas leurs lignes.
    #[test]
    fn lignes_non_entremelees() {
        let path = std::env::temp_dir().join(format!("velka-test-{}-log.txt", std::process::id()));
        let _ = fs::remove_file(&path);
        let handles: Vec<_> = (0..8u64)
            .map(|t| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..200u64 {
                        let mut f = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                            .unwrap();
                        f.write_all(
                            format_line(
                                1_795_201_907 + t,
                                &format!("fil {t} message {i} {}", "x".repeat(60)),
                            )
                            .as_bytes(),
                        )
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let text = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(text.lines().count(), 8 * 200);
        for l in text.lines() {
            assert!(
                l.starts_with("[17952019") && l.contains("] fil ") && l.ends_with('x'),
                "ligne abîmée : {l}"
            );
        }
    }
}
