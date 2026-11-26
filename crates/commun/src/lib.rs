//! Outils communs de Velka : vérification des signatures Ed25519 de publication (mises à jour de l'application),
//! composants tiers téléchargés (`outils`), chiffrement des secrets enregistrés sur le disque (`dpapi`), dates et
//! conversions hexadécimales.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

pub mod dpapi;
pub mod outils;

const SIG_LEN: usize = 64;

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| {
            s.get(2 * i..2 * i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        })
        .collect()
}

/// Secondes Unix de l'horloge locale.
pub fn now_secs() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

// ---------- Dates ----------

/// Jours depuis 1970 → (année, mois, jour).
pub fn civil_from_days(days: u32) -> (i64, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `31/01/2027`
pub fn format_date(days: u32) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{d:02}/{m:02}/{y}")
}

/// Jours depuis 1970 pour l'heure système actuelle.
pub fn today_days() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as u32)
        .unwrap_or(0)
}

// ---------- Publication (signature des mises à jour) ----------

/// Ce que signe la clé de publication. Chaque usage a son propre domaine : une signature ne vaut que pour lui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
    /// Version de l'application proposée en mise à jour (version, empreinte, taille).
    MiseAJour,
}

impl Publication {
    fn domain(self) -> &'static [u8] {
        match self {
            Publication::MiseAJour => b"velka-maj-v1",
        }
    }
}

fn publication_message(kind: Publication, text: &str) -> Vec<u8> {
    let mut m = kind.domain().to_vec();
    m.push(0);
    m.extend_from_slice(text.as_bytes());
    m
}

/// Signe `text` (hexadécimal, 128 caractères).
pub fn publication_sign(sk: &SigningKey, kind: Publication, text: &str) -> String {
    to_hex(&sk.sign(&publication_message(kind, text)).to_bytes())
}

/// Vérifie une signature de publication. Une clé publique nulle (pas encore créée) refuse tout.
pub fn publication_verify(
    public_key: &[u8; 32],
    kind: Publication,
    text: &str,
    sig_hex: &str,
) -> bool {
    if public_key == &[0u8; 32] {
        return false;
    }
    let Some(sig) = from_hex(sig_hex).and_then(|b| <[u8; SIG_LEN]>::try_from(b).ok()) else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    vk.verify_strict(
        &publication_message(kind, text),
        &Signature::from_bytes(&sig),
    )
    .is_ok()
}

/// Texte signé d'une mise à jour de l'application.
pub fn update_text(version: &str, sha256: &str, size: u64) -> String {
    format!("{version}\n{}\n{size}", sha256.to_ascii_lowercase())
}

/// Lit une clé publique écrite en hexadécimal (64 caractères) ; `None` si vide ou invalide.
pub fn public_key_from_hex(s: &str) -> Option<[u8; 32]> {
    from_hex(s)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .filter(|k| k != &[0u8; 32])
}

/// Lit une clé privée de publication (graine Ed25519 de 32 octets en hexadécimal) ; `None` si invalide.
pub fn signing_key_from_hex(s: &str) -> Option<SigningKey> {
    from_hex(s)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .map(|seed| SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sk(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }
    fn pk(seed: u8) -> [u8; 32] {
        sk(seed).verifying_key().to_bytes()
    }

    #[test]
    fn signature_de_publication() {
        let t = update_text("1.5.0", "ab", 12);
        let s = publication_sign(&sk(9), Publication::MiseAJour, &t);
        assert!(publication_verify(&pk(9), Publication::MiseAJour, &t, &s));
        assert!(
            !publication_verify(&pk(1), Publication::MiseAJour, &t, &s),
            "autre clé : refusé"
        );
        assert!(
            !publication_verify(&pk(9), Publication::MiseAJour, &format!("{t} "), &s),
            "texte modifié : refusé"
        );
        assert!(
            !publication_verify(&[0; 32], Publication::MiseAJour, &t, &s),
            "clé absente : refusé"
        );
        assert!(!publication_verify(&pk(9), Publication::MiseAJour, &t, ""));
        assert!(!publication_verify(
            &pk(9),
            Publication::MiseAJour,
            &t,
            &s[..126]
        ));
        assert_eq!(public_key_from_hex(&to_hex(&pk(9))), Some(pk(9)));
        assert_eq!(public_key_from_hex(""), None);
        assert_eq!(public_key_from_hex(&"0".repeat(64)), None);
        assert_eq!(update_text("1.5.0", "AB", 12), "1.5.0\nab\n12");
        let k = signing_key_from_hex(&to_hex(&[9; 32])).map(|k| k.verifying_key().to_bytes());
        assert_eq!(k, Some(pk(9)));
        assert!(signing_key_from_hex("zz").is_none());
    }

    #[test]
    fn hexadecimal() {
        assert_eq!(to_hex(&[0, 15, 255]), "000fff");
        assert_eq!(from_hex("000fff"), Some(vec![0, 15, 255]));
        assert_eq!(from_hex("0"), None);
        assert_eq!(from_hex("zz"), None);
        assert_eq!(from_hex("é1"), None);
    }

    #[test]
    fn dates() {
        assert_eq!(format_date(0), "01/01/1970");
        assert_eq!(format_date(19_723), "01/01/2024");
        assert_eq!(format_date(19_783), "01/03/2024");
        assert!(today_days() > 20_000);
    }
}
