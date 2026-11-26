//! Rapport de plantage local. Une panique Rust ou une exception native non gérée (libmpv, pilote OpenGL) écrit
//! `crash.txt` dans le dossier de données (`%LOCALAPPDATA%\Velka`, à côté de `log.txt`) : version, emplacement de
//! l'erreur (nom du fichier source seul, jamais de chemin), pile d'appels (adresses relatives à l'exécutable), sans
//! donnée personnelle ni jeton. Rien n'est envoyé :
//! Réglages > Avancé et Système > Journaux et diagnostic propose « Copier le rapport de plantage » pour le support.
use std::path::PathBuf;

const FILE: &str = "crash.txt";
/// Taille maximale du rapport (la pile d'appels peut être longue).
const MAX_LEN: usize = 32 * 1024;

pub fn path() -> PathBuf {
    crate::logging::data_dir().join(FILE)
}

/// Rapport du dernier plantage, s'il y en a un.
pub fn read() -> Option<String> {
    std::fs::read_to_string(path())
        .ok()
        .filter(|t| !t.trim().is_empty())
}

/// Un rapport de plus de 30 jours est effacé au démarrage (il ne concerne plus la version utilisée).
pub fn prune() {
    let p = path();
    let old = std::fs::metadata(&p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > std::time::Duration::from_secs(30 * 86_400));
    if old {
        let _ = std::fs::remove_file(p);
    }
}

/// Installe le crochet de panique et le filtre d'exceptions natives. À appeler au tout début de `main`.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", file_only(l.file()), l.line(), l.column()))
            .unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        write(&report(
            "panique",
            &format!("{message}\nemplacement : {location}\nfil : {thread}"),
            &bt,
        ));
        crate::logging::log("Plantage : rapport écrit dans crash.txt");
        previous(info);
    }));
    native::install();
}

/// Nom du fichier seul, sans dossier (un chemin de compilation ne figure jamais dans le rapport).
fn file_only(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Pile d'appels : chaque ligne « at <chemin>:ligne:colonne » ne garde que le nom du fichier.
fn strip_paths(backtrace: &str) -> String {
    backtrace
        .lines()
        .map(|line| {
            let body = line.trim_start();
            match body.strip_prefix("at ") {
                Some(rest) => format!(
                    "{}at {}",
                    " ".repeat(line.len() - body.len()),
                    file_only(rest)
                ),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn write(text: &str) {
    let _ = std::fs::create_dir_all(crate::logging::data_dir());
    let _ = std::fs::write(path(), text);
}

/// Texte du rapport, nettoyé (`sanitize`) et borné.
fn report(kind: &str, detail: &str, backtrace: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let backtrace = strip_paths(backtrace);
    let mut out = format!(
        "Velka {} ({}, {})\nheure Unix : {secs}\ntype : {kind}\nbase de l'exécutable : {:#x}\n\n{detail}\n\npile d'appels :\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        native::image_base(),
    );
    out = sanitize(&out, &personal_values());
    if out.len() > MAX_LEN {
        let mut cut = MAX_LEN;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push_str("\n[tronqué]\n");
    }
    out
}

/// Valeurs personnelles à masquer : dossier du profil, nom d'utilisateur, nom du PC.
fn personal_values() -> Vec<(String, &'static str)> {
    let mut v = Vec::new();
    for (var, label) in [
        ("USERPROFILE", "%USERPROFILE%"),
        ("LOCALAPPDATA", "%LOCALAPPDATA%"),
        ("APPDATA", "%APPDATA%"),
        ("USERNAME", "<utilisateur>"),
        ("COMPUTERNAME", "<pc>"),
    ] {
        if let Ok(val) = std::env::var(var) {
            if val.len() >= 3 {
                v.push((val, label));
            }
        }
    }
    // Les chemins les plus longs d'abord (le profil contient le nom d'utilisateur).
    v.sort_by_key(|(val, _)| std::cmp::Reverse(val.len()));
    v
}

/// Texte nettoyé (valeurs personnelles et jetons masqués), pour tout ce qui est copié vers le support (diagnostic, B2).
pub fn clean(text: &str) -> String {
    sanitize(text, &personal_values())
}

/// Masque les valeurs personnelles (sans tenir compte de la casse) et tout ce qui ressemble à un jeton ou une clé :
/// suite d'au moins 24 lettres et chiffres mêlés, jeton `oauth:…`.
pub fn sanitize(text: &str, personal: &[(String, &'static str)]) -> String {
    let mut s = text.to_string();
    for (val, label) in personal {
        let lower = s.to_lowercase();
        let needle = val.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        if lower.len() != s.len() {
            // Casse qui change la longueur en octets (rare) : remplacement exact seulement.
            s = s.replace(val.as_str(), label);
            continue;
        }
        let mut out = String::with_capacity(s.len());
        let mut last = 0;
        for (i, _) in lower.match_indices(&needle) {
            out.push_str(&s[last..i]);
            out.push_str(label);
            last = i + needle.len();
        }
        out.push_str(&s[last..]);
        s = out;
    }
    let mut out = String::with_capacity(s.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let digits = word.chars().filter(char::is_ascii_digit).count();
        let letters = word.chars().filter(char::is_ascii_alphabetic).count();
        let token_like = word.len() >= 24 && digits > 0 && letters > 0 && !word.starts_with("0x");
        let key_like = word.to_ascii_lowercase().starts_with("oauth:");
        out.push_str(if token_like || key_like {
            "<masqué>"
        } else {
            word
        });
        word.clear();
    };
    for c in s.chars() {
        if c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'
            || c == ':' && word.eq_ignore_ascii_case("oauth")
        {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

#[cfg(windows)]
mod native {
    use windows::Win32::System::Diagnostics::Debug::{
        SetUnhandledExceptionFilter, EXCEPTION_POINTERS,
    };

    /// Adresse de chargement de l'exécutable : les adresses de la pile s'en déduisent (symbolisation avec le PDB).
    pub fn image_base() -> usize {
        // SAFETY : GetModuleHandleW(None) rend le module du processus, sans effet de bord.
        unsafe {
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .map_or(0, |h| h.0 as usize)
        }
    }

    /// Exception native non gérée (accès mémoire invalide dans libmpv, pilote…) : rapport court puis comportement
    /// habituel de Windows (le filtre rend « continuer la recherche »).
    unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
        // SAFETY : Windows passe des pointeurs valides pendant l'appel du filtre ; ils sont seulement lus.
        let (code, addr) = unsafe {
            info.as_ref()
                .and_then(|p| p.ExceptionRecord.as_ref())
                .map_or((0, 0), |r| {
                    (r.ExceptionCode.0 as u32, r.ExceptionAddress as usize)
                })
        };
        let base = image_base();
        let detail = format!(
            "code : {code:#010x}\nadresse : {addr:#x} (exe + {:#x})",
            addr.wrapping_sub(base)
        );
        super::write(&super::report(
            "exception native",
            &detail,
            "(non disponible pour une exception native)",
        ));
        0 // EXCEPTION_CONTINUE_SEARCH
    }

    pub fn install() {
        // SAFETY : enregistre une fonction `extern "system"` valide pour toute la durée du processus.
        unsafe {
            let _ = SetUnhandledExceptionFilter(Some(filter));
        }
    }
}

#[cfg(not(windows))]
mod native {
    pub fn image_base() -> usize {
        0
    }
    pub fn install() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapport_sans_donnee_personnelle_ni_jeton() {
        let perso = vec![
            ("D:\\Profils\\Jean Dupont".to_string(), "%USERPROFILE%"),
            ("Jean Dupont".to_string(), "<utilisateur>"),
            ("PC-JEAN".to_string(), "<pc>"),
        ];
        let t = "panic at d:\\profils\\jean dupont\\AppData\\x.rs on PC-JEAN token abcdefghij0123456789klmnop oauth:abc123 \
                 user Jean Dupont at 0x7ff6a1b2c3d4 src/app/mod.rs:42:7";
        let s = sanitize(t, &perso);
        assert!(!s.to_lowercase().contains("jean"), "{s}");
        assert!(!s.contains("PC-JEAN"));
        assert!(!s.contains("abcdefghij0123456789klmnop"));
        assert!(!s.contains("abc123"));
        assert!(s.contains("%USERPROFILE%\\AppData\\x.rs"));
        assert!(
            s.contains("0x7ff6a1b2c3d4"),
            "les adresses restent lisibles"
        );
        assert!(s.contains("src/app/mod.rs:42:7"));
    }

    #[test]
    fn rapport_sans_chemin_de_source() {
        assert_eq!(file_only("D:\\a\\b\\src\\main.rs"), "main.rs");
        assert_eq!(file_only("/opt/x/.cargo/registry/src/foo/lib.rs"), "lib.rs");
        assert_eq!(file_only("main.rs"), "main.rs");
        let bt = "   0: velka::f\n             at /opt/build/src/app/mod.rs:42:7\n   1: inconnu";
        let r = report("panique", "boum", bt);
        assert!(r.contains("             at mod.rs:42:7"), "{r}");
        assert!(!r.contains("/opt/build"), "{r}");
    }

    #[test]
    fn rapport_borne_et_versionne() {
        let r = report("panique", "boum", &"x ".repeat(40_000));
        assert!(r.len() <= MAX_LEN + 20);
        assert!(r.starts_with(&format!("Velka {}", env!("CARGO_PKG_VERSION"))));
        assert!(r.contains("boum") && r.contains("base de l'exécutable"));
    }
}
