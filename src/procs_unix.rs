//! Fonctions système communes au Mac et à Linux (pistes Mac et Linux) : instance unique par verrou de fichier,
//! attente de la fin d'un processus, heure locale. Seule dépendance : `libc`.

/// Verrou de l'instance unique, gardé ouvert jusqu'à la fin du processus.
static INSTANCE_LOCK: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();

/// Vrai pour la première instance ; faux si Velka tourne déjà (verrou exclusif sur `instance.lock` du dossier de
/// données, rendu par le système à la fin du processus, même après un plantage).
pub fn acquire_single_instance() -> bool {
    let dir = crate::logging::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("instance.lock"))
    else {
        return true; // en cas de doute, on laisse démarrer (comme sous Windows)
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = INSTANCE_LOCK.set(file);
            true
        }
        Err(std::fs::TryLockError::WouldBlock) => false,
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}

/// Attend la fin du processus `pid`, au plus `max` (relance après une mise à jour d'outil ou un redémarrage).
pub fn wait_for_exit(pid: u32, max: std::time::Duration) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    let start = std::time::Instant::now();
    // SAFETY : `kill` avec le signal 0 n'envoie rien : il dit seulement si le processus existe encore.
    while unsafe { libc::kill(pid, 0) } == 0 && start.elapsed() < max {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Heure locale du système.
fn local_tm() -> libc::tm {
    // SAFETY : `time` et `localtime_r` écrivent dans des variables locales valides ; `tm` est une structure C simple
    // que `localtime_r` remplit entièrement.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    }
}

/// Heure locale « HH:MM » (horodatage du chat).
pub fn local_hhmm() -> String {
    let t = local_tm();
    format!("{:02}:{:02}", t.tm_hour, t.tm_min)
}

/// Date et heure locales pour un nom de fichier : `2026-11-20_14-03-22`.
pub fn local_stamp() -> String {
    let t = local_tm();
    format!(
        "{}-{:02}-{:02}_{:02}-{:02}-{:02}",
        t.tm_year + 1900,
        t.tm_mon + 1,
        t.tm_mday,
        t.tm_hour,
        t.tm_min,
        t.tm_sec
    )
}

/// Arrêt immédiat du processus, sans les fonctions de fin.
pub fn terminate_now() {
    // SAFETY : `_exit` arrête le processus sans lancer les fonctions de fin ; rien n'est utilisé après.
    unsafe { libc::_exit(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heure_locale_bien_formee() {
        let h = local_hhmm();
        assert_eq!(h.len(), 5, "{h}");
        assert_eq!(local_stamp().len(), 19, "{}", local_stamp());
    }

    #[test]
    fn instance_unique_et_attente() {
        // Ce processus de test est vivant : l'attente va jusqu'au bout du délai (court) ; un numéro libre rend la main.
        let t = std::time::Instant::now();
        wait_for_exit(std::process::id(), std::time::Duration::from_millis(120));
        assert!(t.elapsed() >= std::time::Duration::from_millis(100));
        let t = std::time::Instant::now();
        wait_for_exit(u32::MAX, std::time::Duration::from_secs(5));
        assert!(t.elapsed() < std::time::Duration::from_secs(1));
    }
}
