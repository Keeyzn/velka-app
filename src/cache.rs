//! Purge du cache disque des images : photos de profil (`cache\avatars`) et emotes (`cache\emotes`).
//! Au démarrage, en arrière-plan : les fichiers de plus de 30 jours sont supprimés, puis les plus anciens tant que le
//! total dépasse 100 Mo. Rien d'autre n'est touché (ni `tools\`, ni les réglages, ni les journaux) ; un fichier
//! supprimé est simplement retéléchargé quand l'image revient dans le chat ou sur l'accueil.
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Un fichier plus vieux que cela (date de dernière écriture) est supprimé.
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);
/// Taille totale des deux dossiers au-delà de laquelle les plus anciens fichiers sont supprimés.
const MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Dossiers de cache d'images : photos de profil et emotes (les outils ne sont pas touchés).
pub fn dirs() -> [PathBuf; 2] {
    let base = crate::logging::data_dir().join("cache");
    [base.join("avatars"), base.join("emotes")]
}

/// Purge les deux dossiers (voir le haut du fichier) et note le résultat dans le journal s'il y a eu des suppressions.
pub fn prune() {
    let (count, bytes) = prune_dirs(&dirs(), SystemTime::now(), MAX_AGE, MAX_BYTES);
    if count > 0 {
        crate::logging::log(&format!(
            "Cache des images : {count} fichiers supprimés ({} Ko)",
            bytes.div_ceil(1024)
        ));
    }
}

/// Supprime les fichiers de plus de `max_age`, puis les plus anciens tant que le total dépasse `max_bytes`.
/// Renvoie le nombre de fichiers supprimés et les octets libérés. Seuls les fichiers ordinaires placés directement
/// dans `dirs` sont considérés : sous-dossiers et liens sont ignorés, un dossier absent n'est pas une erreur.
fn prune_dirs(
    dirs: &[PathBuf],
    now: SystemTime,
    max_age: Duration,
    max_bytes: u64,
) -> (usize, u64) {
    let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
    for dir in dirs {
        collect(dir, &mut files);
    }
    files.sort_by_key(|f| f.1); // les plus anciens d'abord
    let mut total: u64 = files.iter().map(|f| f.2).sum();
    let (mut removed, mut freed) = (0, 0);
    for (path, time, size) in files {
        // Une date dans le futur (horloge déréglée) ne rend pas un fichier « vieux ».
        let old = now.duration_since(time).is_ok_and(|age| age > max_age);
        if !old && total <= max_bytes {
            break; // les suivants sont plus récents encore
        }
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
            freed += size;
            total = total.saturating_sub(size);
        }
    }
    (removed, freed)
}

fn collect(dir: &Path, out: &mut Vec<(PathBuf, SystemTime, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        // `DirEntry::metadata` ne suit pas les liens : un lien n'est pas un fichier ordinaire ici.
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        if let Ok(modified) = meta.modified() {
            out.push((e.path(), modified, meta.len()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOUR: Duration = Duration::from_secs(24 * 3600);

    /// Fichier de `taille` octets daté d'il y a `jours` jours (par rapport à `now`).
    fn fichier(dir: &Path, nom: &str, taille: usize, jours: u64, now: SystemTime) -> PathBuf {
        let p = dir.join(nom);
        std::fs::write(&p, vec![b'x'; taille]).unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
        f.set_modified(now - JOUR * jours as u32).unwrap();
        p
    }

    /// Plus de 30 jours : supprimé. Sinon gardé, ainsi que les sous-dossiers et leur contenu.
    #[test]
    fn fichiers_de_plus_de_30_jours_supprimes() {
        let guard = crate::testutil::TempDir::new("cache-age");
        let (dir, now) = (guard.path().to_path_buf(), SystemTime::now());
        let vieux = fichier(&dir, "vieux.png", 10, 31, now);
        let limite = fichier(&dir, "limite.png", 10, 29, now);
        let neuf = fichier(&dir, "neuf.png", 10, 0, now);
        std::fs::create_dir(dir.join("sous")).unwrap();
        let dedans = fichier(&dir.join("sous"), "vieux.png", 10, 90, now);
        // Date dans le futur (horloge déréglée) : gardé.
        let futur = dir.join("futur.png");
        std::fs::write(&futur, b"x").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&futur)
            .unwrap()
            .set_modified(now + JOUR * 5)
            .unwrap();

        let (n, octets) = prune_dirs(std::slice::from_ref(&dir), now, MAX_AGE, MAX_BYTES);
        assert_eq!((n, octets), (1, 10));
        assert!(!vieux.exists());
        assert!(limite.exists() && neuf.exists() && futur.exists() && dedans.exists());
    }

    /// Au-delà du plafond, les plus anciens partent d'abord, tous dossiers confondus, jusqu'à repasser dessous.
    #[test]
    fn plafond_de_taille_les_plus_anciens_d_abord() {
        let guard = crate::testutil::TempDir::new("cache-taille");
        let now = SystemTime::now();
        let (a, b) = (guard.path().join("avatars"), guard.path().join("emotes"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        // 5 fichiers de 40 octets (200 au total), plafond 100 : les trois plus anciens partent (200 -> 80).
        let f1 = fichier(&a, "a1.png", 40, 20, now);
        let f2 = fichier(&b, "e2.png", 40, 15, now);
        let f3 = fichier(&a, "a3.png", 40, 10, now);
        let f4 = fichier(&b, "e4.png", 40, 5, now);
        let f5 = fichier(&a, "a5.png", 40, 1, now);

        let (n, octets) = prune_dirs(&[a, b], now, MAX_AGE, 100);
        assert_eq!((n, octets), (3, 120));
        assert!(!f1.exists() && !f2.exists() && !f3.exists());
        assert!(f4.exists() && f5.exists());
    }

    /// Sous les deux seuils : rien ne bouge. Dossier absent : pas d'erreur.
    #[test]
    fn rien_a_supprimer_sous_les_seuils() {
        let guard = crate::testutil::TempDir::new("cache-rien");
        let (dir, now) = (guard.path().to_path_buf(), SystemTime::now());
        let p = fichier(&dir, "a.png", 50, 10, now);
        assert_eq!(
            prune_dirs(&[dir.clone(), dir.join("absent")], now, MAX_AGE, 100),
            (0, 0)
        );
        assert!(p.exists());
    }
}
