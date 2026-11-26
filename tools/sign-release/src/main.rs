//! Signature des mises à jour de Velka, sur le PC du mainteneur (jamais dans GitHub Actions).
//!
//!   cargo run -p sign-release -- keygen <chemin de la clé privée> [--remplacer]
//!     Crée une nouvelle paire Ed25519. La clé privée (graine hexadécimale) est écrite dans le fichier donné, qui doit
//!     être hors du dépôt et ne pas exister ; ses droits sont restreints à l'utilisateur. La clé publique remplace le
//!     contenu de `assets/publication.pub` (refusé si une clé y est déjà, sauf avec `--remplacer`).
//!
//!   cargo run -p sign-release -- sign <vX.Y.Z>
//!     Télécharge les fichiers de la Release brouillon (outil `gh`, connecté avec votre compte), vérifie leurs SHA-256
//!     avec `SHA256SUMS.txt`, écrit `velka-maj.json` signé avec la clé privée dont le chemin est dans la variable
//!     d'environnement `VELKA_SIGNING_KEY_FILE`, affiche un résumé, demande confirmation, puis envoie ce fichier dans la
//!     même Release. La Release reste un brouillon : elle se publie à la main sur GitHub.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Code de l'application : source unique du nom du dépôt et du nom du manifeste.
const UPDATE_RS: &str = include_str!("../../../src/update.rs");
/// Fichiers de mise à jour de chaque plateforme, tels que produits par `.github/workflows/release.yml`.
const FILES: [(&str, &str); 3] = [
    ("windows", "velka.exe"),
    ("macos", "Velka-mac.zip"),
    ("linux", "Velka-x86_64.AppImage"),
];
/// Empreintes produites par le workflow.
const SUMS: &str = "SHA256SUMS.txt";
/// Variable d'environnement qui donne le chemin de la clé privée.
const KEY_ENV: &str = "VELKA_SIGNING_KEY_FILE";

/// Valeur d'une constante `pub const NOM: &str = "…";` de `src/update.rs`.
fn const_str(src: &str, name: &str) -> Option<String> {
    let line = src.lines().find(|l| {
        l.trim_start()
            .starts_with(&format!("pub const {name}: &str = "))
    })?;
    let start = line.find('"')? + 1;
    let end = line.rfind('"')?;
    (end > start)
        .then(|| line.get(start..end).map(str::to_string))
        .flatten()
}

fn repo() -> Result<String, String> {
    const_str(UPDATE_RS, "GITHUB_REPO")
        .ok_or_else(|| "GITHUB_REPO introuvable dans src/update.rs".into())
}

fn manifest_name() -> Result<String, String> {
    const_str(UPDATE_RS, "MANIFEST_ASSET")
        .ok_or_else(|| "MANIFEST_ASSET introuvable dans src/update.rs".into())
}

/// Racine du dépôt (dossier de l'espace de travail Cargo).
fn repo_root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent()
        .and_then(Path::parent)
        .map_or_else(|| here.to_path_buf(), Path::to_path_buf)
}

fn publication_pub() -> PathBuf {
    repo_root().join("assets").join("publication.pub")
}

/// Le dossier `dir` (déjà résolu) est-il dans le dépôt de Velka, ou dans n'importe quel dépôt Git ?
fn inside_repository(dir: &Path, root: &Path) -> bool {
    if dir.starts_with(root) {
        return true;
    }
    dir.ancestors().any(|a| a.join(".git").exists())
}

fn valid_tag(tag: &str) -> Option<String> {
    let v = tag.strip_prefix('v')?;
    let parts: Vec<&str> = v.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.parse::<u32>().is_ok()))
    .then(|| v.to_string())
}

/// Lit `SHA256SUMS.txt` (format de `sha256sum` : « empreinte  nom » ou « empreinte *nom »).
fn parse_sums(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (sha, name) = line
            .split_once(char::is_whitespace)
            .ok_or(format!("ligne illisible : {line}"))?;
        let name = name.trim().trim_start_matches('*');
        let sha = sha.to_ascii_lowercase();
        if sha.len() != 64
            || !sha.bytes().all(|b| b.is_ascii_hexdigit())
            || name.is_empty()
            || name.contains(['/', '\\'])
        {
            return Err(format!("ligne invalide : {line}"));
        }
        if out.iter().any(|(_, n)| n == name) {
            return Err(format!("fichier en double : {name}"));
        }
        out.push((sha, name.to_string()));
    }
    Ok(out)
}

fn sha256_hex(data: &[u8]) -> String {
    commun::to_hex(&Sha256::digest(data))
}

/// Entrée signée du manifeste pour un fichier.
fn entry(sk: &SigningKey, version: &str, name: &str, data: &[u8]) -> serde_json::Value {
    let sha = sha256_hex(data);
    let size = data.len() as u64;
    let sig = commun::publication_sign(
        sk,
        commun::Publication::MiseAJour,
        &commun::update_text(version, &sha, size),
    );
    serde_json::json!({ "asset": name, "size": size, "sha256": sha, "sig": sig })
}

/// Manifeste complet, chaque signature revérifiée avec la clé publique de l'application.
fn build_manifest(
    sk: &SigningKey,
    public: &[u8; 32],
    version: &str,
    files: &[(&str, &str, Vec<u8>)],
) -> Result<serde_json::Value, String> {
    let mut m = serde_json::json!({ "version": version });
    for (platform, name, data) in files {
        let e = entry(sk, version, name, data);
        let text = commun::update_text(
            version,
            e["sha256"].as_str().unwrap_or_default(),
            data.len() as u64,
        );
        if !commun::publication_verify(
            public,
            commun::Publication::MiseAJour,
            &text,
            e["sig"].as_str().unwrap_or_default(),
        ) {
            return Err(format!("signature de {name} refusée par la clé publique"));
        }
        m[*platform] = e;
    }
    Ok(m)
}

fn gh(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| format!("outil gh introuvable ({e}) : installez GitHub CLI"))?;
    if !out.status.success() {
        return Err(format!(
            "gh {} : {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Restreint les droits du fichier de clé à l'utilisateur courant.
fn restrict(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").map_err(|_| "variable USERNAME absente")?;
        let who = match std::env::var("USERDOMAIN") {
            Ok(d) if !d.is_empty() => format!("{d}\\{user}"),
            _ => user,
        };
        let ok = Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &format!("{who}:(R,W)")])
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            return Err("droits du fichier non restreints (icacls a échoué)".into());
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn keygen(target: &str, replace: bool) -> Result<(), String> {
    let target = std::path::absolute(target).map_err(|e| format!("chemin invalide : {e}"))?;
    let parent = target.parent().ok_or("chemin invalide")?;
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("dossier introuvable : {e}"))?;
    let root = repo_root().canonicalize().map_err(|e| e.to_string())?;
    if inside_repository(&parent, &root) {
        return Err(
            "refusé : ce chemin est dans un dépôt Git. Choisissez un dossier hors du dépôt.".into(),
        );
    }
    let pub_path = publication_pub();
    let current = std::fs::read_to_string(&pub_path).unwrap_or_default();
    if commun::public_key_from_hex(&current).is_some() && !replace {
        return Err("assets/publication.pub contient déjà une clé : les installations existantes ne vérifieraient plus les \
                    mises à jour signées avec une nouvelle clé. Ajoutez --remplacer seulement si c'est voulu."
            .into());
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("générateur aléatoire indisponible : {e}"))?;
    let sk = SigningKey::from_bytes(&seed);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&target).map_err(|e| {
        format!("écriture de la clé privée impossible (le fichier existe déjà ?) : {e}")
    })?;
    f.write_all(format!("{}\n", commun::to_hex(&seed)).as_bytes())
        .map_err(|e| e.to_string())?;
    drop(f);
    if let Err(e) = restrict(&target) {
        eprintln!("ATTENTION : {e}. Restreignez les droits du fichier à la main.");
    }
    let public = commun::to_hex(&sk.verifying_key().to_bytes());
    std::fs::write(&pub_path, format!("{public}\n"))
        .map_err(|e| format!("écriture de assets/publication.pub impossible : {e}"))?;
    println!("Nouvelle paire de clés créée.");
    println!("  Clé privée : {}", target.display());
    println!("  Clé publique (assets/publication.pub) : {public}");
    println!("À faire : sauvegarder la clé privée à deux endroits hors de GitHub, puis commiter assets/publication.pub.");
    println!("Sans cette clé privée, plus aucune mise à jour ne pourra être signée pour les installations existantes.");
    Ok(())
}

fn load_key() -> Result<(SigningKey, [u8; 32]), String> {
    let path = std::env::var(KEY_ENV)
        .map_err(|_| format!("variable {KEY_ENV} absente (chemin du fichier de clé privée)"))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("lecture de la clé privée impossible : {e}"))?;
    let sk = commun::signing_key_from_hex(&text)
        .ok_or("clé privée invalide (64 caractères hexadécimaux attendus)")?;
    let expected = std::fs::read_to_string(publication_pub()).unwrap_or_default();
    let expected = commun::public_key_from_hex(&expected)
        .ok_or("assets/publication.pub est vide ou invalide : lancez d'abord « keygen »")?;
    if sk.verifying_key().to_bytes() != expected {
        return Err(
            "la clé privée ne correspond pas à assets/publication.pub : rien n'est signé".into(),
        );
    }
    Ok((sk, expected))
}

fn sign(tag: &str) -> Result<(), String> {
    let version = valid_tag(tag).ok_or("étiquette invalide (attendu : vX.Y.Z)")?;
    let (sk, public) = load_key()?;
    let repo = repo()?;
    let manifest = manifest_name()?;

    let view = gh(&[
        "release",
        "view",
        tag,
        "--repo",
        &repo,
        "--json",
        "isDraft,isPrerelease,assets",
    ])?;
    let view: serde_json::Value =
        serde_json::from_str(&view).map_err(|e| format!("réponse de gh illisible : {e}"))?;
    if view["isDraft"].as_bool() != Some(true) {
        return Err(format!(
            "la Release {tag} n'est pas un brouillon : rien n'est fait"
        ));
    }

    let dir = std::env::temp_dir().join(format!("velka-sign-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let result = (|| {
        let dir_s = dir.to_string_lossy().into_owned();
        let mut args = vec![
            "release",
            "download",
            tag,
            "--repo",
            &repo,
            "--dir",
            &dir_s,
            "--pattern",
            SUMS,
        ];
        for (_, name) in &FILES {
            args.extend(["--pattern", name]);
        }
        gh(&args)?;
        let sums = parse_sums(
            &std::fs::read_to_string(dir.join(SUMS)).map_err(|e| format!("{SUMS} absent : {e}"))?,
        )?;
        for (sha, name) in &sums {
            if FILES.iter().all(|(_, n)| n != name) {
                continue; // autres fichiers de la Release (installateur, .dmg) : vérifiés par le workflow, pas signés ici
            }
            let data = std::fs::read(dir.join(name)).map_err(|e| format!("{name} absent : {e}"))?;
            if sha256_hex(&data) != *sha {
                return Err(format!(
                    "{name} : empreinte différente de {SUMS} : rien n'est signé"
                ));
            }
        }
        let mut files = Vec::new();
        for (platform, name) in FILES {
            if sums.iter().all(|(_, n)| n != name) {
                return Err(format!("{name} absent de {SUMS}"));
            }
            files.push((
                platform,
                name,
                std::fs::read(dir.join(name)).map_err(|e| e.to_string())?,
            ));
        }
        let m = build_manifest(&sk, &public, &version, &files)?;
        let out = dir.join(&manifest);
        std::fs::write(
            &out,
            serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        println!("Release {tag} ({repo}), brouillon. Fichiers vérifiés et signés :");
        for (platform, name, data) in &files {
            println!(
                "  {platform:8} {name:24} {:>12} octets  {}",
                data.len(),
                sha256_hex(data)
            );
        }
        print!("Envoyer {manifest} dans cette Release brouillon ? Tapez « oui » pour confirmer : ");
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if answer.trim() != "oui" {
            println!("Annulé : rien n'a été envoyé.");
            return Ok(());
        }
        gh(&[
            "release",
            "upload",
            tag,
            &out.to_string_lossy(),
            "--repo",
            &repo,
            "--clobber",
        ])?;
        println!(
            "{manifest} envoyé. La Release reste un brouillon : publiez-la à la main sur GitHub."
        );
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["keygen", path] => keygen(path, false),
        ["keygen", path, "--remplacer"] => keygen(path, true),
        ["sign", tag] => sign(tag),
        _ => Err("utilisation : sign-release keygen <chemin hors du dépôt> [--remplacer] | sign-release sign <vX.Y.Z>".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("sign-release : {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constantes_lues_dans_le_code_de_l_application() {
        assert_eq!(repo().unwrap(), "Keeyzn/velka-app");
        assert_eq!(manifest_name().unwrap(), "velka-maj.json");
        assert_eq!(
            const_str("pub const X: &str = \"a\";", "X").as_deref(),
            Some("a")
        );
        assert_eq!(const_str("pub const Y: &str = \"a\";", "X"), None);
    }

    #[test]
    fn etiquettes() {
        assert_eq!(valid_tag("v1.2.3").as_deref(), Some("1.2.3"));
        assert!(valid_tag("1.2.3").is_none());
        assert!(valid_tag("v1.2").is_none());
        assert!(valid_tag("v1.2.x").is_none());
        assert!(valid_tag("v1..3").is_none());
    }

    #[test]
    fn empreintes() {
        let a = "ab".repeat(32);
        let b = "CD".repeat(32);
        let s = parse_sums(&format!("{a}  velka.exe\n{b} *Velka-mac.zip\n\n")).unwrap();
        assert_eq!(
            s,
            vec![
                (a.clone(), "velka.exe".to_string()),
                (b.to_lowercase(), "Velka-mac.zip".to_string())
            ]
        );
        assert!(parse_sums(&format!("{a}  ../x")).is_err(), "chemin refusé");
        assert!(parse_sums("abc  velka.exe").is_err(), "empreinte courte");
        assert!(parse_sums(&format!("{a}  a\n{a}  a")).is_err(), "doublon");
    }

    #[test]
    fn cle_privee_jamais_dans_un_depot() {
        let root = repo_root().canonicalize().unwrap();
        assert!(inside_repository(&root, &root));
        assert!(inside_repository(&root.join("assets"), &root));
        let tmp = std::env::temp_dir().join(format!("velka-sign-test-{}", std::process::id()));
        let depot = tmp.join("autre-depot");
        std::fs::create_dir_all(depot.join(".git")).unwrap();
        std::fs::create_dir_all(depot.join("cles")).unwrap();
        assert!(
            inside_repository(&depot.join("cles"), &root),
            "dossier d'un autre dépôt Git"
        );
        let libre = tmp.join("hors-depot");
        std::fs::create_dir_all(&libre).unwrap();
        let libre = libre.canonicalize().unwrap();
        // Le dossier temporaire du système n'est normalement pas dans un dépôt Git.
        if !libre.ancestors().any(|a| a.join(".git").exists()) {
            assert!(!inside_repository(&libre, &root));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Le manifeste signé est accepté par la clé publique correspondante, refusé par une autre.
    #[test]
    fn manifeste_signe_verifiable() {
        let sk = SigningKey::from_bytes(&[5; 32]);
        let pk = sk.verifying_key().to_bytes();
        let files = vec![
            ("windows", "velka.exe", b"exe".to_vec()),
            ("linux", "Velka-x86_64.AppImage", b"app".to_vec()),
        ];
        let m = build_manifest(&sk, &pk, "1.2.3", &files).unwrap();
        assert_eq!(m["version"], "1.2.3");
        let e = &m["windows"];
        assert_eq!(
            (e["asset"].as_str(), e["size"].as_u64()),
            (Some("velka.exe"), Some(3))
        );
        let text = commun::update_text("1.2.3", e["sha256"].as_str().unwrap(), 3);
        assert!(commun::publication_verify(
            &pk,
            commun::Publication::MiseAJour,
            &text,
            e["sig"].as_str().unwrap()
        ));
        let autre = SigningKey::from_bytes(&[6; 32]).verifying_key().to_bytes();
        assert!(
            build_manifest(&sk, &autre, "1.2.3", &files).is_err(),
            "clé publique différente : refusé"
        );
    }
}
