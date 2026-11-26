//! Chiffrement des secrets enregistrés sur le disque avec DPAPI (`CryptProtectData`) : le fichier n'est lisible
//! que par la même session Windows sur le même PC. Une copie du fichier (sauvegarde, synchronisation, autre compte,
//! disque volé) ne donne rien. Limite : un programme qui tourne déjà sous la même session peut le déchiffrer.
//!
//! Format texte d'un fichier protégé : `DPAPI1:` suivi du bloc chiffré en hexadécimal.
//!
//! Sur Mac : même garantie que DPAPI. Une clé aléatoire de 32 octets est rangée dans le
//! Trousseau de session de l'utilisateur (élément « Velka », compte `secrets-v1`, accessible à ses programmes sans
//! demande de mot de passe, comme DPAPI) ; le bloc est chiffré par ChaCha20-Poly1305 avec cette clé. Une copie du
//! fichier sans le Trousseau déverrouillé de la session ne donne rien. Préfixe : `TROUSSEAU1:`.
//!
//! Sur Linux : même chiffrement que sur Mac. La clé de 32 octets est rangée dans le
//! trousseau du bureau (Secret Service : GNOME Keyring, KWallet…, par l'outil `secret-tool`, attributs
//! `service=Velka`, `account=secrets-v1`) ; sans trousseau (bureau minimal, `secret-tool` absent), dans
//! `~/.config/Velka/secrets.key`, lisible par le seul utilisateur (`0600`), hors du dossier des données. Le repli
//! protège moins qu'un trousseau (une copie de tout le dossier personnel suffit), mais mieux que les jetons en clair
//! dans `settings.json`. Préfixe : `CLE1:`.

/// Préfixe d'un texte protégé.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub const PREFIX: &str = "DPAPI1:";
#[cfg(target_os = "macos")]
pub const PREFIX: &str = "TROUSSEAU1:";
#[cfg(target_os = "linux")]
pub const PREFIX: &str = "CLE1:";

/// Gardée telle quelle : la changer rendrait illisibles les secrets déjà chiffrés.
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
const ENTROPY: &[u8] = b"velka-secrets-v1";

#[cfg(windows)]
fn blob(data: &[u8]) -> windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
    windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

#[cfg(windows)]
fn take(out: windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB) -> Vec<u8> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    if out.pbData.is_null() {
        return Vec::new();
    }
    // SAFETY : bloc alloué par DPAPI (LocalAlloc), de `cbData` octets ; copié puis effacé et libéré une seule fois.
    unsafe {
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        std::ptr::write_bytes(out.pbData, 0, out.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(out.pbData.cast())));
        v
    }
}

/// Chiffre pour la session Windows en cours. `None` si DPAPI est indisponible.
#[cfg(windows)]
pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = blob(plain);
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY : les blocs d'entrée pointent sur des tranches vivantes pendant l'appel ; `out` est rempli par DPAPI.
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
        .ok()?;
    }
    Some(take(out))
}

/// Déchiffre un bloc produit par `protect` (même session Windows). `None` sinon.
#[cfg(windows)]
pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = blob(sealed);
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY : voir `protect`.
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
        .ok()?;
    }
    Some(take(out))
}

/// Chiffre avec la clé du Trousseau de la session (créée au premier usage). `None` si le Trousseau est indisponible.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};
    let key = trousseau::key()?;
    let nonce = trousseau::random::<12>()?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: ENTROPY,
            },
        )
        .ok()?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Some(out)
}

/// Déchiffre un bloc produit par `protect` (même session, même Trousseau). `None` sinon.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};
    let (nonce, ct) = (sealed.get(..12)?, sealed.get(12..)?);
    let key = trousseau::existing_key()?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: ENTROPY,
            },
        )
        .ok()
}

/// Clé des secrets dans le Trousseau, par l'outil système `/usr/bin/security` (aucune dépendance en plus).
#[cfg(target_os = "macos")]
mod trousseau {
    use std::process::Command;
    use std::sync::Mutex;

    const SECURITY: &str = "/usr/bin/security";
    /// Nom et compte de l'élément du Trousseau (visible dans l'app « Trousseaux d'accès »).
    const SERVICE: &str = "Velka";
    const ACCOUNT: &str = "secrets-v1";

    /// Clé lue une fois par lancement (évite un processus `security` à chaque enregistrement des réglages).
    static CACHE: Mutex<Option<[u8; 32]>> = Mutex::new(None);

    pub fn random<const N: usize>() -> Option<[u8; N]> {
        use std::io::Read;
        let mut b = [0u8; N];
        std::fs::File::open("/dev/urandom")
            .ok()?
            .read_exact(&mut b)
            .ok()?;
        Some(b)
    }

    fn parse(hex: &str) -> Option<[u8; 32]> {
        crate::from_hex(hex.trim())?.try_into().ok()
    }

    fn read() -> Option<[u8; 32]> {
        let out = Command::new(SECURITY)
            .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
            .args(super::test_keychain())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        parse(&String::from_utf8(out.stdout).ok()?)
    }

    /// Clé existante seulement (lecture) : un secret illisible ne crée pas de nouvelle clé.
    pub fn existing_key() -> Option<[u8; 32]> {
        let mut cache = CACHE.lock().ok()?;
        if cache.is_none() {
            *cache = read();
        }
        *cache
    }

    /// Clé existante, ou nouvelle clé aléatoire ajoutée au Trousseau. `-A` : lisible par les programmes de la session
    /// sans demande (sinon macOS redemanderait le mot de passe après chaque mise à jour de l'appli non signée). On
    /// relit après l'ajout : si un autre lancement a créé la clé en même temps, c'est la sienne qui compte.
    pub fn key() -> Option<[u8; 32]> {
        let mut cache = CACHE.lock().ok()?;
        if cache.is_none() {
            *cache = read().or_else(|| {
                let hex = crate::to_hex(&random::<32>()?);
                let _ = Command::new(SECURITY)
                    .args([
                        "add-generic-password",
                        "-s",
                        SERVICE,
                        "-a",
                        ACCOUNT,
                        "-A",
                        "-w",
                        &hex,
                    ])
                    .args(super::test_keychain())
                    .output();
                read()
            });
        }
        *cache
    }
}

/// Clé des secrets sur Linux : trousseau du bureau par `secret-tool` (paquet `libsecret-tools`), sinon fichier `0600`.
/// Même interface que le module du Mac (`random`, `existing_key`, `key`).
#[cfg(target_os = "linux")]
mod trousseau {
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Attributs de l'élément du trousseau (visible dans « Mots de passe et clés » sous le nom « Velka »).
    const ATTRS: [&str; 4] = ["service", "Velka", "account", "secrets-v1"];
    /// Un trousseau qui ne répond pas peut faire attendre D-Bus longtemps : au-delà, on passe au fichier. Assez long
    /// pour qu'un trousseau verrouillé laisse taper son mot de passe dans la fenêtre du bureau.
    const TIMEOUT: Duration = Duration::from_secs(10);

    static CACHE: Mutex<Option<[u8; 32]>> = Mutex::new(None);

    pub fn random<const N: usize>() -> Option<[u8; N]> {
        let mut b = [0u8; N];
        std::fs::File::open("/dev/urandom")
            .ok()?
            .read_exact(&mut b)
            .ok()?;
        Some(b)
    }

    fn parse(hex: &str) -> Option<[u8; 32]> {
        crate::from_hex(hex.trim())?.try_into().ok()
    }

    /// Fichier de repli : `$XDG_CONFIG_HOME/Velka/secrets.key` (sinon `~/.config/Velka`) ; en version de
    /// développement, le fichier d'essai de `VELKA_TROUSSEAU` (le trousseau du bureau n'est alors pas touché).
    fn key_file() -> Option<PathBuf> {
        if let Some(p) = super::test_keychain().into_iter().next() {
            return Some(PathBuf::from(p));
        }
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|p| !p.is_empty())
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(config.join("Velka").join("secrets.key"))
    }

    /// Lance `secret-tool` (entrée `input`, sortie rendue), arrêté après `TIMEOUT`. `None` : outil absent, échec ou
    /// délai dépassé.
    fn secret_tool(args: &[&str], input: &str) -> Option<String> {
        let mut child = Command::new("secret-tool")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut out = String::new();
                    child.stdout.take()?.read_to_string(&mut out).ok()?;
                    return status.success().then_some(out);
                }
                Ok(None) if start.elapsed() < TIMEOUT => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    /// Trousseau du bureau utilisé, sauf pendant les essais (`VELKA_TROUSSEAU`).
    fn use_keyring() -> bool {
        super::test_keychain().is_empty()
    }

    fn read_keyring() -> Option<[u8; 32]> {
        if !use_keyring() {
            return None;
        }
        let mut args = vec!["lookup"];
        args.extend(ATTRS);
        parse(&secret_tool(&args, "")?)
    }

    fn store_keyring(hex: &str) -> bool {
        if !use_keyring() {
            return false;
        }
        let mut args = vec!["store", "--label=Velka"];
        args.extend(ATTRS);
        secret_tool(&args, hex).is_some()
    }

    fn read_file() -> Option<[u8; 32]> {
        parse(&std::fs::read_to_string(key_file()?).ok()?)
    }

    /// Fichier créé seulement s'il n'existe pas (`create_new`), en `0600` dès sa création.
    fn store_file(hex: &str) {
        use std::os::unix::fs::OpenOptionsExt;
        let Some(path) = key_file() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| f.write_all(hex.as_bytes()));
    }

    /// Fichier d'abord s'il existe, puis trousseau : une clé passée une fois au fichier (trousseau absent ou qui
    /// n'a pas répondu à temps) le reste, même si le trousseau répond plus tard ; on ne mélange jamais deux clés.
    fn read() -> Option<[u8; 32]> {
        read_file().or_else(read_keyring)
    }

    /// Clé existante seulement (lecture) : un secret illisible ne crée pas de nouvelle clé.
    pub fn existing_key() -> Option<[u8; 32]> {
        let mut cache = CACHE.lock().ok()?;
        if cache.is_none() {
            *cache = read();
        }
        *cache
    }

    /// Clé existante, ou nouvelle clé aléatoire rangée dans le trousseau, sinon dans le fichier. On relit après
    /// l'ajout : si un autre lancement a créé la clé en même temps, c'est la sienne qui compte.
    pub fn key() -> Option<[u8; 32]> {
        let mut cache = CACHE.lock().ok()?;
        if cache.is_none() {
            *cache = read().or_else(|| {
                let hex = crate::to_hex(&random::<32>()?);
                if !store_keyring(&hex) {
                    store_file(&hex);
                }
                read()
            });
        }
        *cache
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn protect(_plain: &[u8]) -> Option<Vec<u8>> {
    None
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn unprotect(_sealed: &[u8]) -> Option<Vec<u8>> {
    None
}

/// Mac : session sans accès au Trousseau déverrouillé (connexion SSH : macOS répond « User interaction is not
/// allowed »). Les essais lancés à distance sautent alors ceux du Trousseau.
pub fn session_without_keychain() -> bool {
    cfg!(target_os = "macos")
        && std::env::var_os("SSH_CONNECTION").is_some()
        && test_keychain().is_empty()
}

/// Trousseau d'essai (Mac, versions de développement seulement) : `VELKA_TROUSSEAU=<fichier .keychain-db>` fait
/// travailler les essais dans ce Trousseau à part (créé avec un mot de passe aléatoire),
/// au lieu du Trousseau de session (inaccessible en SSH). Sur Linux : `VELKA_TROUSSEAU=<fichier>` est le fichier de
/// clé d'essai, et le trousseau du bureau n'est pas touché. Sans effet dans l'appli publiée (build release).
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
fn test_keychain() -> Vec<std::ffi::OsString> {
    if !cfg!(debug_assertions) {
        return Vec::new();
    }
    std::env::var_os("VELKA_TROUSSEAU")
        .filter(|p| !p.is_empty())
        .into_iter()
        .collect()
}

/// Texte protégé (`DPAPI1:…`, `TROUSSEAU1:…` sur Mac, `CLE1:…` sur Linux) ; `None` si le chiffrement est indisponible.
pub fn seal_text(plain: &str) -> Option<String> {
    protect(plain.as_bytes()).map(|b| format!("{PREFIX}{}", crate::to_hex(&b)))
}

/// Texte lu sur le disque : déchiffré s'il est protégé, sinon rendu tel quel (ancien fichier en clair, ou secret
/// recollé à la main). Le booléen vaut `true` pour un texte en clair. `None` : texte protégé mais indéchiffrable
/// (autre session Windows, fichier abîmé).
pub fn open_text(stored: &str) -> Option<(String, bool)> {
    match stored.trim().strip_prefix(PREFIX) {
        Some(h) => {
            let plain = unprotect(&crate::from_hex(h)?)?;
            String::from_utf8(plain).ok().map(|s| (s, false))
        }
        None => Some((stored.to_string(), true)),
    }
}

#[cfg(all(test, any(windows, target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_dpapi() {
        if crate::dpapi::session_without_keychain() {
            eprintln!("session SSH sur Mac : Trousseau inaccessible, essai sauté");
            return;
        }
        let t = seal_text("secret-123").expect("chiffrement disponible");
        assert!(t.starts_with(PREFIX));
        assert!(
            !t.contains("secret-123"),
            "le texte n'apparaît pas en clair"
        );
        assert_eq!(open_text(&t), Some(("secret-123".to_string(), false)));
        assert_eq!(open_text("en clair"), Some(("en clair".to_string(), true)));
        // Bloc modifié : refusé.
        let mut b = t.into_bytes();
        let i = b.len() - 3;
        b[i] = if b[i] == b'0' { b'1' } else { b'0' };
        assert_eq!(open_text(&String::from_utf8(b).unwrap()), None);
        assert_eq!(open_text(&format!("{PREFIX}zz")), None);
        assert_eq!(
            seal_text("").and_then(|t| open_text(&t)),
            Some((String::new(), false))
        );
    }
}
