//! Mise à jour automatique depuis les Releases publiques du dépôt GitHub `GITHUB_REPO`, sans serveur intermédiaire.
//! Chaque Release porte un manifeste (`MANIFEST_ASSET`) : pour chaque plateforme, nom du fichier, taille, empreinte
//! SHA-256 et signature Ed25519 de (version, empreinte, taille) par la clé de publication (`assets/publication.pub`).
//! Le fichier téléchargé est vérifié (taille et empreinte) avant de remplacer l'application : GitHub seul ne peut pas
//! imposer un exécutable.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau : aucune indexation qui panique
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Retour arrière : après une mise à jour, l'ancien exécutable (`.exe.old`) reste sur le disque tant que
/// la nouvelle version n'a pas tourné `TRIAL_CONFIRM_AFTER` (ou n'a pas été fermée normalement). Si elle plante avant,
/// le démarrage suivant remet l'ancienne version.
pub const TRIAL_CONFIRM_AFTER: Duration = Duration::from_secs(10);

/// Un essai de démarrage est en cours dans ce processus.
static TRIAL_PENDING: AtomicBool = AtomicBool::new(false);
/// Instant du démarrage mis à l'essai.
static TRIAL_SINCE: OnceLock<Instant> = OnceLock::new();

/// Dépôt GitHub public dont les Releases portent les mises à jour.
pub const GITHUB_REPO: &str = "Keeyzn/velka-app";

/// Manifeste joint à chaque Release (fabriqué et signé par le workflow de publication).
pub const MANIFEST_ASSET: &str = "velka-maj.json";

/// Version proposée par la dernière Release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub version: String,
    pub size: u64,
    pub sha256: String,
    /// Adresse du fichier de cette plateforme dans la Release.
    pub url: String,
}

/// Page des Releases, ouverte par le bandeau quand l'application ne se remplace pas elle-même.
pub fn releases_page() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/latest")
}

/// Adresse de l'API GitHub pour la dernière Release.
fn latest_api() -> String {
    format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
}

/// Début obligatoire de toute adresse téléchargée : les fichiers des Releases du dépôt, en HTTPS.
fn download_prefix() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/download/")
}

/// Clé de cette plateforme dans le manifeste.
fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "windows"
    }
}

/// L'adresse commence par `prefix` et ne remonte pas dans l'arborescence.
fn url_ok(url: &str, prefix: &str) -> bool {
    url.strip_prefix(prefix).is_some_and(|rest| {
        !rest.is_empty() && !rest.contains("..") && !rest.contains('?') && !rest.contains('#')
    })
}

/// L'application sait se remplacer elle-même : exécutable sous Windows, paquet `Velka.app` sur Mac, fichier AppImage
/// sous Linux. Ailleurs, le bandeau ouvre `releases_page()`.
pub const fn replaces_itself() -> bool {
    cfg!(any(windows, target_os = "macos", target_os = "linux"))
}

/// Version de l'application en cours.
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.trim().trim_start_matches('v').split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

/// `true` si `remote` est strictement plus récente que `local`.
pub fn is_newer(remote: &str, local: &str) -> bool {
    matches!((parse_version(remote), parse_version(local)), (Some(r), Some(l)) if r > l)
}

fn agent(secs: u64) -> ureq::Agent {
    crate::net::agent_status(Duration::from_secs(secs))
}

/// Interroge GitHub. `Ok(None)` : déjà à jour (ou aucune Release). Appel réseau : hors du fil d'interface.
pub fn check() -> Result<Option<Info>, String> {
    let info = check_from(
        &latest_api(),
        &download_prefix(),
        &crate::publication::public_key(),
    )?;
    // Une version qui n'a pas démarré sur ce PC (retour arrière) n'est plus proposée ; la suivante le sera.
    Ok(info.filter(|i| {
        let refused = is_refused(&i.version, &refused_file());
        if refused {
            crate::logging::log(&format!(
                "Mise à jour {} ignorée : son démarrage a échoué sur ce PC.",
                i.version
            ));
        }
        !refused
    }))
}

/// Adresse d'un fichier de la Release (`assets[].browser_download_url`), si elle est sous `prefix`.
fn asset_url<'a>(release: &'a serde_json::Value, name: &str, prefix: &str) -> Option<&'a str> {
    release["assets"]
        .as_array()?
        .iter()
        .find(|a| a["name"].as_str() == Some(name))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| url_ok(u, prefix))
}

/// Release publiée : champs `draft` et `prerelease` présents et faux.
fn is_published(release: &serde_json::Value) -> bool {
    release.get("draft").and_then(serde_json::Value::as_bool) == Some(false)
        && release
            .get("prerelease")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
}

fn check_from(api_url: &str, prefix: &str, public_key: &[u8; 32]) -> Result<Option<Info>, String> {
    let agent = agent(15);
    let mut resp = agent
        .get(api_url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("réseau : {e}"))?;
    match resp.status().as_u16() {
        200 => {}
        404 => return Ok(None), // aucune Release publiée
        code => return Err(format!("GitHub : statut {code}")),
    }
    let release: serde_json::Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| format!("réponse illisible : {e}"))?;
    // Seule une Release publiée compte : ni brouillon, ni pré-version (l'API « latest » les écarte déjà ; contrôle en
    // plus, au cas où la réponse en contiendrait une).
    if !is_published(&release) {
        return Ok(None);
    }
    let Some(manifest_url) = asset_url(&release, MANIFEST_ASSET, prefix) else {
        return Ok(None); // Release sans manifeste : rien à proposer
    };
    let mut resp = agent
        .get(manifest_url)
        .call()
        .map_err(|e| format!("réseau : {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("manifeste : statut {}", resp.status().as_u16()));
    }
    let manifest: serde_json::Value = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json()
        .map_err(|e| format!("manifeste illisible : {e}"))?;
    let Some(entry) = manifest.get(platform()).filter(|e| !e.is_null()) else {
        return Ok(None); // pas de fichier pour cette plateforme dans cette Release
    };
    let signed = serde_json::json!({
        "version": manifest.get("version"),
        "size": entry["size"],
        "sha256": entry["sha256"],
        "sig": entry["sig"],
    });
    let mut info = parse_info(&signed, public_key)?;
    let asset = entry["asset"].as_str().unwrap_or_default();
    info.url = asset_url(&release, asset, prefix)
        .ok_or("fichier de la mise à jour absent de la Release")?
        .to_string();
    Ok(is_newer(&info.version, current()).then_some(info))
}

/// Lit et vérifie une entrée du manifeste : champs bornés et signature de l'éditeur sur (version, empreinte, taille).
fn parse_info(v: &serde_json::Value, public_key: &[u8; 32]) -> Result<Info, String> {
    let info = Info {
        version: v["version"].as_str().unwrap_or_default().to_string(),
        size: v["size"].as_u64().unwrap_or(0),
        sha256: v["sha256"].as_str().unwrap_or_default().to_lowercase(),
        url: String::new(),
    };
    let valid = parse_version(&info.version).is_some()
        && info.sha256.len() == 64
        && info.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        && info.size > 1_000_000
        && info.size < 100_000_000;
    if !valid {
        return Err("manifeste de mise à jour invalide".into());
    }
    let text = commun::update_text(&info.version, &info.sha256, info.size);
    if !commun::publication_verify(
        public_key,
        commun::Publication::MiseAJour,
        &text,
        v["sig"].as_str().unwrap_or_default(),
    ) {
        return Err("mise à jour non signée par l'éditeur : refusée".into());
    }
    Ok(info)
}

/// Dossier où la nouvelle version est préparée.
fn staging_dir() -> PathBuf {
    crate::logging::data_dir().join("update")
}

/// Télécharge la nouvelle version, vérifie taille et empreinte, et renvoie le chemin du fichier prêt.
pub fn download(info: &Info, progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
    if !url_ok(&info.url, &download_prefix()) {
        return Err(crate::i18n::tr("Téléchargement refusé par le serveur.").into());
    }
    download_from(&staging_dir(), info, progress)
}

fn download_from(dir: &Path, info: &Info, progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Écriture impossible :")))?;
    let ext = if cfg!(target_os = "macos") {
        "zip"
    } else if cfg!(target_os = "linux") {
        "AppImage"
    } else {
        "exe"
    };
    let dest = dir.join(format!("velka-{}.{ext}", info.version));
    let mut resp = agent(600)
        .get(&info.url)
        .call()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Serveur injoignable :")))?;
    if resp.status().as_u16() != 200 {
        return Err(crate::i18n::tr("Téléchargement refusé par le serveur.").into());
    }
    let mut reader = resp.body_mut().with_config().limit(100_000_000).reader();
    let mut file = std::fs::File::create(&dest)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Écriture impossible :")))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("{} {e}", crate::i18n::tr("Téléchargement interrompu :")))?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).unwrap_or_default();
        hasher.update(chunk);
        file.write_all(chunk)
            .map_err(|e| format!("{} {e}", crate::i18n::tr("Écriture impossible :")))?;
        done += n as u64;
        progress((done as f32 / info.size as f32).clamp(0.0, 1.0));
    }
    drop(file);
    let sha: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if done != info.size || sha != info.sha256 {
        let _ = std::fs::remove_file(&dest);
        return Err(
            crate::i18n::tr("Fichier reçu invalide (taille ou empreinte différente).").into(),
        );
    }
    Ok(dest)
}

/// Remplace l'exécutable en cours par `new`, puis relance l'application une fois celle-ci fermée.
/// Windows autorise à renommer un exécutable en cours d'utilisation : l'ancien devient `velka.exe.old`.
#[cfg(windows)]
pub fn apply(new: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Remplacement impossible :")))?;
    let old = exe.with_extension("exe.old");
    // Cette version tourne : l'essai en cours (s'il y en a un) est fini, sinon le prochain démarrage la prendrait pour un échec.
    confirm();
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Remplacement impossible :")))?;
    if let Err(e) = std::fs::copy(new, &exe) {
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::rename(&old, &exe);
        return Err(format!(
            "{} {e}",
            crate::i18n::tr("Remplacement impossible :")
        ));
    }
    let _ = std::fs::remove_file(new);
    launch_after_exit(&exe)
}

/// Mac : `new` est le zip de `Velka.app` (reçu, vérifié et signé). Un petit script `sh` détaché attend la fin de ce
/// processus, extrait le zip à côté du paquet, échange les deux dossiers, puis rouvre l'application. Aucun retour
/// arrière automatique (pas d'essai de démarrage comme sous Windows) : si l'échange échoue, l'ancien paquet est
/// remis et rouvert.
#[cfg(target_os = "macos")]
pub fn apply(new: &Path) -> Result<(), String> {
    let fail = |e: String| format!("{} {e}", crate::i18n::tr("Remplacement impossible :"));
    // Les versions de développement et l'essai de démarrage ne touchent jamais à leur paquet.
    if cfg!(debug_assertions) || crate::smoke::enabled() {
        return Err("mise à jour non prise en charge ici".into());
    }
    let exe = std::env::current_exe().map_err(|e| fail(e.to_string()))?;
    let bundle = crate::uninstall::bundle_of(&exe)
        .ok_or_else(|| fail("Velka n'est pas dans un paquet Velka.app.".into()))?;
    let script = mac_script(std::process::id(), new, &bundle, true)
        .ok_or_else(|| fail("chemin non pris en charge.".into()))?;
    confirm();
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .spawn()
        .map(|_| ())
        .map_err(|e| fail(e.to_string()))
}

/// Script de remplacement du paquet (Mac). `reopen` : rouvre `bundle` à la fin (faux seulement dans les essais).
#[cfg(target_os = "macos")]
fn mac_script(pid: u32, zip: &Path, bundle: &Path, reopen: bool) -> Option<String> {
    use crate::uninstall::sh_quoted;
    let parent = bundle.parent()?;
    // Même volume que le paquet : `mv` ne copie pas, l'échange est quasi instantané.
    let stage = sh_quoted(&parent.join(".Velka-maj"))?;
    let old = sh_quoted(&parent.join(".Velka-ancienne.app"))?;
    let (zip, bundle) = (sh_quoted(zip)?, sh_quoted(bundle)?);
    let open = if reopen {
        format!("/usr/bin/open {bundle}; ")
    } else {
        String::new()
    };
    let stage_app = sh_quoted(&parent.join(".Velka-maj").join("Velka.app"))?;
    let stage_exe = sh_quoted(
        &parent
            .join(".Velka-maj")
            .join("Velka.app/Contents/MacOS/velka"),
    )?;
    Some(format!(
        "n=0; while /bin/kill -0 {pid} 2>/dev/null && [ $n -lt 100 ]; do /bin/sleep 0.3; n=$((n+1)); done;          /bin/rm -rf {stage} {old}; /bin/mkdir {stage};          if /usr/bin/ditto -x -k {zip} {stage} && [ -x {stage_exe} ] && /bin/mv {bundle} {old}; then          /bin/mv {stage_app} {bundle} || /bin/mv {old} {bundle}; fi;          /bin/rm -rf {stage} {old}; /bin/rm -f {zip};          /usr/bin/xattr -dr com.apple.quarantine {bundle} 2>/dev/null; {open}true"
    ))
}

/// Linux : `new` est la nouvelle AppImage (reçue, vérifiée et signée). Un petit script `sh` détaché attend la fin de ce
/// processus, copie le fichier à côté de l'AppImage en cours puis le renomme par-dessus (même dossier : l'échange est
/// atomique), et rouvre l'application. Seule une AppImage nommée `Velka….AppImage` est remplacée. Aucun retour arrière
/// automatique : si la copie échoue, l'ancienne AppImage reste et est rouverte.
#[cfg(target_os = "linux")]
pub fn apply(new: &Path) -> Result<(), String> {
    let fail = |e: String| format!("{} {e}", crate::i18n::tr("Remplacement impossible :"));
    // Les versions de développement et l'essai de démarrage ne touchent jamais à leur fichier.
    if cfg!(debug_assertions) || crate::smoke::enabled() {
        return Err("mise à jour non prise en charge ici".into());
    }
    let target = std::env::var_os("APPIMAGE")
        .and_then(|p| crate::uninstall::appimage_of(Path::new(&p)))
        .ok_or_else(|| fail("Velka n'est pas lancée depuis son fichier AppImage.".into()))?;
    let script = linux_script(std::process::id(), new, &target, true)
        .ok_or_else(|| fail("chemin non pris en charge.".into()))?;
    confirm();
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .spawn()
        .map(|_| ())
        .map_err(|e| fail(e.to_string()))
}

/// Script de remplacement de l'AppImage (Linux). `reopen` : rouvre `target` à la fin (faux seulement dans les essais).
#[cfg(target_os = "linux")]
fn linux_script(pid: u32, new: &Path, target: &Path, reopen: bool) -> Option<String> {
    use crate::uninstall::sh_quoted_linux as q;
    let parent = target.parent()?;
    let tmp = q(&parent.join(".Velka-maj.AppImage"))?;
    let (new, target) = (q(new)?, q(target)?);
    let open = if reopen {
        format!("nohup {target} >/dev/null 2>&1 & ")
    } else {
        String::new()
    };
    Some(format!(
        "n=0; while kill -0 {pid} 2>/dev/null && [ $n -lt 100 ]; do sleep 0.3; n=$((n+1)); done; \
         if cp {new} {tmp} && chmod 755 {tmp} && mv -f {tmp} {target}; then :; else rm -f {tmp}; fi; \
         rm -f {new}; {open}true"
    ))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn apply(_new: &Path) -> Result<(), String> {
    Err("mise à jour non prise en charge ici".into())
}

/// Relance l'application une fois celle-ci fermée (petit processus caché qui attend la fin de l'instance unique).
/// Sert à mettre à jour libmpv, qu'on ne peut pas remplacer tant qu'elle est chargée, et à « Redémarrer l'application ».
pub fn relaunch() -> Result<(), String> {
    // Linux : le fichier AppImage, pas l'exécutable de l'image montée qui disparaît à la fermeture.
    let exe = crate::procs::relaunch_exe()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Relance impossible :")))?;
    launch_after_exit(&exe)
}

/// Lance `exe` tout de suite avec `--wait-pid` (numéro de ce processus) : la nouvelle instance attend la fermeture de
/// celle-ci (`main.rs`) avant de prendre l'instance unique. plus de `cmd.exe /C ping … & start`, que les
/// antivirus jugent suspect, et plus de délai fixe de 3 s (une fermeture lente ne fait plus échouer la relance).
fn launch_after_exit(exe: &Path) -> Result<(), String> {
    std::process::Command::new(exe)
        .arg("--wait-pid")
        .arg(std::process::id().to_string())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Relance impossible :")))
}

/// Fichier d'essai : présent tant qu'une version venue d'une mise à jour n'a pas prouvé qu'elle démarre.
/// Ligne 1 : version mise à l'essai ; ligne 2 : chemin de son exécutable (deux installations ne se mélangent pas).
fn trial_file() -> PathBuf {
    crate::logging::data_dir().join("update-trial.txt")
}

/// Version dont le démarrage a échoué sur ce PC : elle n'est plus proposée.
fn refused_file() -> PathBuf {
    crate::logging::data_dir().join("update-refused.txt")
}

fn is_refused(version: &str, file: &Path) -> bool {
    std::fs::read_to_string(file).is_ok_and(|t| t.trim() == version)
}

/// Verdict du contrôle fait au démarrage.
#[derive(Debug, PartialEq, Eq)]
enum Trial {
    /// Pas de mise à jour récente : rien à surveiller.
    None,
    /// Premier démarrage après une mise à jour : l'ancien exécutable est gardé jusqu'à la confirmation.
    Started,
    /// Le démarrage précédent de cette version n'a jamais été confirmé : retour à l'ancienne version.
    Failed,
}

/// Compare l'exécutable, son `.exe.old` et le fichier d'essai (chemins passés pour pouvoir tester).
fn trial_check(exe: &Path, marker: &Path, version: &str) -> Trial {
    let old = exe.with_extension("exe.old");
    let mine = exe.display().to_string();
    let ours =
        std::fs::read_to_string(marker).is_ok_and(|t| t.lines().nth(1) == Some(mine.as_str()));
    if !old.exists() {
        if ours {
            let _ = std::fs::remove_file(marker);
        }
        return Trial::None;
    }
    if ours {
        return Trial::Failed;
    }
    if std::fs::write(marker, format!("{version}\n{mine}\n")).is_err() {
        // Sans fichier d'essai, aucun retour arrière possible : on revient au comportement d'avant (ancien supprimé).
        let _ = std::fs::remove_file(&old);
        return Trial::None;
    }
    Trial::Started
}

/// Remet l'ancienne version : l'exécutable en cours (qui n'a jamais été confirmé) devient `.exe.bad`, `.exe.old`
/// reprend sa place. Windows autorise ces renommages pendant l'exécution. En cas d'échec, rien ne bouge.
fn roll_back(exe: &Path, marker: &Path, refused: &Path, version: &str) -> Result<(), String> {
    let (old, bad) = (exe.with_extension("exe.old"), exe.with_extension("exe.bad"));
    let _ = std::fs::remove_file(&bad);
    std::fs::rename(exe, &bad).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&old, exe) {
        let _ = std::fs::rename(&bad, exe);
        return Err(e.to_string());
    }
    let _ = std::fs::write(refused, format!("{version}\n"));
    let _ = std::fs::remove_file(marker);
    Ok(())
}

/// Démarrage jugé réussi : l'ancien exécutable et le fichier d'essai disparaissent.
fn confirm_in(exe: &Path, marker: &Path) {
    let _ = std::fs::remove_file(exe.with_extension("exe.old"));
    let _ = std::fs::remove_file(marker);
}

/// Confirme le démarrage de la version en cours (sans effet s'il n'y a pas d'essai).
pub fn confirm() {
    if TRIAL_PENDING.swap(false, Ordering::Relaxed) {
        if let Ok(exe) = std::env::current_exe() {
            confirm_in(&exe, &trial_file());
        }
        crate::logging::log("Mise à jour : démarrage confirmé, ancienne version supprimée.");
    }
}

/// À appeler à chaque image : confirme le démarrage une fois `TRIAL_CONFIRM_AFTER` écoulé. Renvoie le temps restant
/// tant que l'essai dure, pour demander une image à ce moment-là même si la fenêtre est au repos.
pub fn trial_tick() -> Option<Duration> {
    if !TRIAL_PENDING.load(Ordering::Relaxed) {
        return None;
    }
    let left = TRIAL_CONFIRM_AFTER.saturating_sub(TRIAL_SINCE.get()?.elapsed());
    if left.is_zero() {
        confirm();
        return None;
    }
    Some(left)
}

/// Au démarrage : supprime les fichiers de préparation d'une mise à jour précédente, puis juge l'essai d'une version
/// fraîchement installée. Renvoie `true` si l'ancienne version vient d'être remise et relancée : l'appelant doit alors
/// quitter tout de suite.
pub fn startup() -> bool {
    let _ = std::fs::remove_dir_all(staging_dir());
    if crate::smoke::enabled() {
        return false; // l'essai de démarrage de la publication n'a ni mise à jour ni retour arrière
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let _ = std::fs::remove_file(exe.with_extension("exe.bad"));
    let (marker, version) = (trial_file(), current());
    match trial_check(&exe, &marker, version) {
        Trial::None => false,
        Trial::Started => {
            TRIAL_PENDING.store(true, Ordering::Relaxed);
            let _ = TRIAL_SINCE.set(Instant::now());
            crate::logging::log(&format!("Mise à jour : première ouverture de la {version}, ancienne version gardée jusqu'au démarrage réussi."));
            false
        }
        Trial::Failed => match roll_back(&exe, &marker, &refused_file(), version) {
            Ok(()) => {
                crate::logging::log(&format!("Mise à jour : la {version} n'a pas démarré correctement, retour à l'ancienne version."));
                if let Err(e) = launch_after_exit(&exe) {
                    crate::logging::log(&format!("Relance de l'ancienne version impossible : {e}"));
                }
                true
            }
            Err(e) => {
                // Pas de retour arrière possible : on garde cette version et on ne réessaie pas à chaque démarrage.
                crate::logging::log(&format!(
                    "Mise à jour : retour à l'ancienne version impossible : {e}"
                ));
                confirm_in(&exe, &marker);
                false
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une mise à jour n'est acceptée que signée par la clé de publication, pour cette version, taille et empreinte.
    #[test]
    fn mise_a_jour_signee_exigee() {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[2; 32]);
        let pk = sk.verifying_key().to_bytes();
        let sha = "ab".repeat(32);
        let sig = commun::publication_sign(
            &sk,
            commun::Publication::MiseAJour,
            &commun::update_text("9.0.0", &sha, 5_000_000),
        );
        let rep = |version: &str, size: u64, sha: &str, sig: &str| serde_json::json!({ "version": version, "size": size, "sha256": sha, "sig": sig });
        let ok = parse_info(&rep("9.0.0", 5_000_000, &sha, &sig), &pk).unwrap();
        assert_eq!(
            (ok.version.as_str(), ok.size, ok.sha256.as_str()),
            ("9.0.0", 5_000_000, sha.as_str())
        );
        assert!(
            parse_info(&rep("9.0.0", 5_000_000, &sha.to_uppercase(), &sig), &pk).is_ok(),
            "empreinte en majuscules acceptée"
        );
        assert!(
            parse_info(&rep("9.0.1", 5_000_000, &sha, &sig), &pk).is_err(),
            "autre version"
        );
        assert!(
            parse_info(&rep("9.0.0", 5_000_001, &sha, &sig), &pk).is_err(),
            "autre taille"
        );
        assert!(
            parse_info(&rep("9.0.0", 5_000_000, &"cd".repeat(32), &sig), &pk).is_err(),
            "autre empreinte"
        );
        assert!(
            parse_info(&rep("9.0.0", 5_000_000, &sha, ""), &pk).is_err(),
            "sans signature"
        );
        assert!(
            parse_info(&rep("9.0.0", 5_000_000, &sha, &sig), &[0; 32]).is_err(),
            "clé de publication absente"
        );
    }

    /// Petit serveur HTTP local : répond tour à tour `(statut, corps)` à chaque connexion, après avoir lu la requête.
    fn serve(responses: Vec<(u16, Vec<u8>)>) -> String {
        serve_with(|_| responses)
    }

    /// Comme `serve`, avec des réponses qui dépendent de l'adresse du serveur (liens vers lui-même).
    fn serve_with(make: impl FnOnce(&str) -> Vec<(u16, Vec<u8>)>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let responses = make(&format!("http://127.0.0.1:{port}"));
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut s, _)) = listener.accept() else {
                    return;
                };
                // Requête lue en entier (en-têtes puis corps) pour que le client n'obtienne pas de coupure.
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    let n = s.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    fn sha_hex(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Fichier reçu : accepté seulement si taille et empreinte (celles de la réponse signée) correspondent.
    #[test]
    fn telechargement_verifie_taille_et_empreinte() {
        let guard = crate::testutil::TempDir::new("maj-dl");
        let dir = guard.path().to_path_buf();
        let contenu: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let info = |base: &str| Info {
            version: "9.0.0".into(),
            size: contenu.len() as u64,
            sha256: sha_hex(&contenu),
            url: format!("{base}/dl/velka.exe"),
        };
        let progress = std::cell::Cell::new(0.0f32);
        let suivi = |f: f32| progress.set(f);

        // Bon fichier : écrit, avancement jusqu'à 100 %.
        let base = serve(vec![(200, contenu.clone())]);
        let path = download_from(&dir, &info(&base), &suivi).expect("fichier valable");
        assert_eq!(std::fs::read(&path).unwrap(), contenu);
        assert!((progress.get() - 1.0).abs() < f32::EPSILON);

        // Empreinte fausse (contenu modifié, même taille) : refusé et fichier supprimé.
        let mut piege = contenu.clone();
        piege[1000] ^= 0xff;
        let base = serve(vec![(200, piege)]);
        assert!(download_from(&dir, &info(&base), &suivi).is_err());
        assert!(
            !dir.join("velka-9.0.0.exe").exists(),
            "le fichier refusé ne reste pas sur le disque"
        );

        // Taille fausse (fichier tronqué).
        let base = serve(vec![(200, contenu[..contenu.len() - 10].to_vec())]);
        assert!(download_from(&dir, &info(&base), &suivi).is_err());
        assert!(!dir.join("velka-9.0.0.exe").exists());

        // Refus du serveur (fichier retiré de la Release…).
        let base = serve(vec![(403, b"{}".to_vec())]);
        assert!(download_from(&dir, &info(&base), &suivi).is_err());
    }

    /// Recherche : Release lue par l'API GitHub, puis son manifeste ; seule une version signée par l'éditeur est
    /// proposée, avec l'adresse du fichier de cette plateforme dans la même Release.
    #[test]
    fn recherche_de_mise_a_jour_signee() {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[2; 32]);
        let pk = sk.verifying_key().to_bytes();
        let sha = "ab".repeat(32);
        let sig = commun::publication_sign(
            &sk,
            commun::Publication::MiseAJour,
            &commun::update_text("999.0.0", &sha, 5_000_000),
        );
        let manifeste = |sig: &str| {
            let mut m = serde_json::json!({ "version": "999.0.0" });
            m[platform()] = serde_json::json!({ "asset": "velka-app", "size": 5_000_000, "sha256": sha, "sig": sig });
            m.to_string().into_bytes()
        };
        let release = |base: &str| {
            serde_json::json!({ "draft": false, "prerelease": false, "assets": [
                { "name": MANIFEST_ASSET, "browser_download_url": format!("{base}/dl/v999/{MANIFEST_ASSET}") },
                { "name": "velka-app", "browser_download_url": format!("{base}/dl/v999/velka-app") },
            ] })
            .to_string()
            .into_bytes()
        };
        let verifier = |m: Vec<u8>| {
            let mut prefix = String::new();
            let base = serve_with(|b| {
                prefix = format!("{b}/dl/");
                vec![(200, release(b)), (200, m)]
            });
            check_from(&format!("{base}/api"), &prefix, &pk)
        };

        let info = verifier(manifeste(&sig))
            .unwrap()
            .expect("version plus récente proposée");
        assert_eq!((info.version.as_str(), info.size), ("999.0.0", 5_000_000));
        assert!(info.url.ends_with("/dl/v999/velka-app"), "{}", info.url);
        // Signature d'une autre clé (dépôt compromis).
        let autre = ed25519_dalek::SigningKey::from_bytes(&[3; 32]);
        let fausse = commun::publication_sign(
            &autre,
            commun::Publication::MiseAJour,
            &commun::update_text("999.0.0", &sha, 5_000_000),
        );
        assert!(
            verifier(manifeste(&fausse)).is_err(),
            "signature d'une autre clé"
        );
        assert!(verifier(manifeste("")).is_err(), "sans signature");
        assert!(
            verifier(manifeste(&sig.replace(&sig[..4], "0000"))).is_err(),
            "signature abîmée"
        );
        // Aucune Release (404) : rien à proposer, sans erreur.
        assert_eq!(
            check_from(
                &format!("{}/api", serve(vec![(404, b"{}".to_vec())])),
                "http://127.0.0.1:1/dl/",
                &pk
            )
            .unwrap(),
            None
        );
        // Fichier hors du dépôt attendu : refusé.
        let base = serve(vec![(200, release("https://evil.example"))]);
        assert_eq!(
            check_from(&format!("{base}/api"), &format!("{base}/dl/"), &pk).unwrap(),
            None,
            "manifeste hors du dépôt ignoré"
        );
    }

    /// Brouillon ou pré-version : jamais proposés, même avec un manifeste correctement signé.
    #[test]
    fn brouillon_et_pre_version_ignores() {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[2; 32]);
        let pk = sk.verifying_key().to_bytes();
        let sha = "ab".repeat(32);
        let sig = commun::publication_sign(
            &sk,
            commun::Publication::MiseAJour,
            &commun::update_text("999.0.0", &sha, 5_000_000),
        );
        let mut m = serde_json::json!({ "version": "999.0.0" });
        m[platform()] = serde_json::json!({ "asset": "velka-app", "size": 5_000_000, "sha256": sha, "sig": sig });
        let essai = |draft: serde_json::Value, prerelease: serde_json::Value| {
            let mut prefix = String::new();
            let base = serve_with(|b| {
                prefix = format!("{b}/dl/");
                let mut r = serde_json::json!({ "assets": [
                    { "name": MANIFEST_ASSET, "browser_download_url": format!("{b}/dl/v999/{MANIFEST_ASSET}") },
                    { "name": "velka-app", "browser_download_url": format!("{b}/dl/v999/velka-app") },
                ] });
                if !draft.is_null() {
                    r["draft"] = draft.clone();
                }
                if !prerelease.is_null() {
                    r["prerelease"] = prerelease.clone();
                }
                vec![
                    (200, r.to_string().into_bytes()),
                    (200, m.to_string().into_bytes()),
                ]
            });
            check_from(&format!("{base}/api"), &prefix, &pk).unwrap()
        };
        assert!(
            essai(false.into(), false.into()).is_some(),
            "Release publiée : proposée"
        );
        assert_eq!(essai(true.into(), false.into()), None, "brouillon");
        assert_eq!(essai(false.into(), true.into()), None, "pré-version");
        assert_eq!(
            essai(serde_json::Value::Null, serde_json::Value::Null),
            None,
            "état inconnu"
        );
    }

    /// La clé embarquée dans l'application (`assets/publication.pub`) refuse un manifeste sans signature valide ou
    /// signé par une autre clé.
    #[test]
    fn cle_embarquee_refuse_signature_invalide_ou_etrangere() {
        let pk = crate::publication::public_key();
        let sha = "cd".repeat(32);
        let text = commun::update_text("999.0.0", &sha, 5_000_000);
        let autre = ed25519_dalek::SigningKey::from_bytes(&[0x5a; 32]);
        assert_ne!(
            autre.verifying_key().to_bytes(),
            pk,
            "clé d'essai différente de la clé embarquée"
        );
        let etrangere = commun::publication_sign(&autre, commun::Publication::MiseAJour, &text);
        let entree = |sig: &str| serde_json::json!({ "version": "999.0.0", "size": 5_000_000, "sha256": sha, "sig": sig });
        assert!(
            parse_info(&entree(&etrangere), &pk).is_err(),
            "signée par une autre clé"
        );
        assert!(parse_info(&entree(""), &pk).is_err(), "sans signature");
        assert!(
            parse_info(&entree(&"0".repeat(128)), &pk).is_err(),
            "signature nulle"
        );
    }

    #[test]
    fn adresses_des_releases() {
        let p = download_prefix();
        assert!(url_ok(&format!("{p}v1.0.0/velka.exe"), &p));
        assert!(!url_ok(&format!("{p}../x"), &p));
        assert!(!url_ok(&format!("{p}v1/a?b"), &p));
        assert!(!url_ok(&p, &p));
        assert!(!url_ok(
            "https://github.com.evil.example/Keeyzn/velka-app/releases/download/x1/a",
            &p
        ));
        assert!(releases_page().starts_with("https://github.com/"));
    }

    /// Dossier d'essai avec un exécutable « nouveau » et, si demandé, son `.exe.old` « ancien ».
    fn installation(name: &str, avec_ancien: bool) -> (crate::testutil::TempDir, PathBuf, PathBuf) {
        let guard = crate::testutil::TempDir::new(name);
        let exe = guard.path().join("velka.exe");
        std::fs::write(&exe, b"nouveau").unwrap();
        if avec_ancien {
            std::fs::write(exe.with_extension("exe.old"), b"ancien").unwrap();
        }
        let marker = guard.path().join("update-trial.txt");
        (guard, exe, marker)
    }

    /// Premier démarrage après une mise à jour : essai ouvert, ancien exécutable gardé ; sans confirmation, le
    /// démarrage suivant est un échec ; avec confirmation, tout est nettoyé.
    #[test]
    fn essai_de_demarrage_apres_mise_a_jour() {
        let (_guard, exe, marker) = installation("maj-essai", true);
        let old = exe.with_extension("exe.old");

        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::Started);
        assert!(
            old.exists(),
            "l'ancien exécutable est gardé pendant l'essai"
        );
        let text = std::fs::read_to_string(&marker).unwrap();
        assert_eq!(text.lines().next(), Some("9.0.0"));
        assert_eq!(
            text.lines().nth(1),
            Some(exe.display().to_string().as_str())
        );

        // Plantage avant la confirmation : le démarrage suivant le constate.
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::Failed);

        // Version confirmée : plus d'ancien exécutable ni de fichier d'essai, et plus rien à surveiller.
        confirm_in(&exe, &marker);
        assert!(!old.exists() && !marker.exists());
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::None);
    }

    /// Sans `.exe.old` (démarrage ordinaire) : rien à surveiller ; un fichier d'essai laissé par cette
    /// installation est supprimé, celui d'une autre installation reste.
    #[test]
    fn demarrage_ordinaire_sans_ancien_executable() {
        let (_guard, exe, marker) = installation("maj-ordinaire", false);
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::None);
        assert!(!marker.exists(), "aucun fichier d'essai créé");

        std::fs::write(&marker, format!("8.0.0\n{}\n", exe.display())).unwrap();
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::None);
        assert!(!marker.exists(), "fichier d'essai périmé supprimé");

        std::fs::write(&marker, "8.0.0\nC:\\ailleurs\\velka.exe\n").unwrap();
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::None);
        assert!(
            marker.exists(),
            "l'essai d'une autre installation n'est pas touché"
        );
    }

    /// Un fichier d'essai d'une autre installation ne fait pas passer celle-ci pour un échec.
    #[test]
    fn essai_d_une_autre_installation_ignore() {
        let (_guard, exe, marker) = installation("maj-autre", true);
        std::fs::write(&marker, "8.0.0\nC:\\ailleurs\\velka.exe\n").unwrap();
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::Started);
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap().lines().next(),
            Some("9.0.0"),
            "essai réécrit pour cette installation"
        );
    }

    /// Retour arrière : l'ancienne version reprend sa place, la nouvelle est mise de côté et ne sera plus proposée.
    #[test]
    fn retour_arriere_remet_l_ancienne_version() {
        let (guard, exe, marker) = installation("maj-retour", true);
        let refused = guard.path().join("update-refused.txt");
        assert_eq!(trial_check(&exe, &marker, "9.0.0"), Trial::Started);

        roll_back(&exe, &marker, &refused, "9.0.0").expect("retour arrière");
        assert_eq!(std::fs::read(&exe).unwrap(), b"ancien");
        assert_eq!(
            std::fs::read(exe.with_extension("exe.bad")).unwrap(),
            b"nouveau"
        );
        assert!(!exe.with_extension("exe.old").exists() && !marker.exists());
        assert!(is_refused("9.0.0", &refused));
        assert!(
            !is_refused("9.0.1", &refused),
            "une version plus récente reste proposée"
        );
        assert!(!is_refused("9.0.0", &guard.path().join("absent.txt")));
        // L'ancienne version, une fois relancée, démarre normalement.
        assert_eq!(trial_check(&exe, &marker, "8.0.0"), Trial::None);
    }

    /// Retour arrière impossible (ancien exécutable disparu) : la version en cours reste en place.
    #[test]
    fn retour_arriere_impossible_ne_casse_rien() {
        let (guard, exe, marker) = installation("maj-retour-ko", false);
        let refused = guard.path().join("update-refused.txt");

        assert!(roll_back(&exe, &marker, &refused, "9.0.0").is_err());
        assert_eq!(
            std::fs::read(&exe).unwrap(),
            b"nouveau",
            "l'exécutable est remis en place"
        );
        assert!(!exe.with_extension("exe.bad").exists());
        assert!(
            !refused.exists(),
            "version non refusée : rien n'a été remis"
        );
    }

    #[test]
    fn comparaison_des_versions() {
        assert!(is_newer("1.1.1", "1.1.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(!is_newer("abc", "1.1.0"));
        assert!(!is_newer("1.1", "1.0.0"));
        assert!(!is_newer("1.1.0.1", "1.1.0"));
    }

    /// Mac : le script remplace vraiment le paquet (zip réel fabriqué avec `ditto`) et garde l'ancien si le zip est
    /// invalide. Le paquet n'est pas rouvert (`reopen` faux).
    #[cfg(target_os = "macos")]
    #[test]
    fn remplacement_du_paquet_mac() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::testutil::TempDir::new("maj-mac");
        let root = dir.path();
        let paquet = |base: &Path, contenu: &str| {
            let exe = base.join("Velka.app/Contents/MacOS/velka");
            std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
            std::fs::write(&exe, contenu).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
            exe
        };
        let installe = paquet(&root.join("Applications"), "ancien");
        let bundle = root.join("Applications/Velka.app");
        paquet(&root.join("nouveau"), "nouveau");
        let zip = root.join("velka-9.9.9.zip");
        let ok = std::process::Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(root.join("nouveau/Velka.app"))
            .arg(&zip)
            .status()
            .unwrap();
        assert!(ok.success());
        // Processus déjà terminé : le script n'attend rien.
        let mut fini = std::process::Command::new("/usr/bin/true").spawn().unwrap();
        fini.wait().unwrap();
        let lance = |zip: &Path| {
            let script = mac_script(fini.id(), zip, &bundle, false).unwrap();
            assert!(std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .status()
                .unwrap()
                .success());
        };
        // Zip invalide : l'ancien paquet reste, rien ne traîne.
        let faux = root.join("faux.zip");
        std::fs::write(&faux, "pas un zip").unwrap();
        lance(&faux);
        assert_eq!(std::fs::read_to_string(&installe).unwrap(), "ancien");
        assert!(!faux.exists(), "le zip reçu est supprimé");
        assert!(
            !root.join("Applications/.Velka-maj").exists()
                && !root.join("Applications/.Velka-ancienne.app").exists()
        );
        // Zip réel : le paquet est remplacé et reste exécutable.
        lance(&zip);
        assert_eq!(std::fs::read_to_string(&installe).unwrap(), "nouveau");
        assert_eq!(
            std::fs::metadata(&installe).unwrap().permissions().mode() & 0o111,
            0o111
        );
        assert!(!zip.exists());
        assert!(
            !root.join("Applications/.Velka-maj").exists()
                && !root.join("Applications/.Velka-ancienne.app").exists()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn script_mac_refuse_un_chemin_avec_apostrophe() {
        assert!(mac_script(
            1,
            Path::new("/tmp/a'b.zip"),
            Path::new("/Applications/Velka.app"),
            true
        )
        .is_none());
        let s = mac_script(
            42,
            Path::new("/tmp/maj.zip"),
            Path::new("/Applications/Velka.app"),
            true,
        )
        .unwrap();
        assert!(s.contains("kill -0 42") && s.contains("/usr/bin/open '/Applications/Velka.app'"));
    }

    /// Linux : le script remplace vraiment l'AppImage (droits d'exécution compris) et garde l'ancienne si le fichier
    /// reçu manque. L'application n'est pas rouverte (`reopen` faux).
    #[cfg(target_os = "linux")]
    #[test]
    fn remplacement_de_l_appimage_linux() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::testutil::TempDir::new("maj-linux");
        let root = dir.path();
        let cible = root.join("Velka-x86_64.AppImage");
        std::fs::write(&cible, "ancien").unwrap();
        std::fs::set_permissions(&cible, std::fs::Permissions::from_mode(0o755)).unwrap();
        let nouveau = root.join("velka-9.9.9.AppImage");
        std::fs::write(&nouveau, "nouveau").unwrap();
        // Processus déjà terminé : le script n'attend rien.
        let mut fini = std::process::Command::new("true").spawn().unwrap();
        fini.wait().unwrap();
        let lance = |nouveau: &Path| {
            let script = linux_script(fini.id(), nouveau, &cible, false).unwrap();
            assert!(std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .status()
                .unwrap()
                .success());
        };
        // Fichier reçu absent : l'ancienne AppImage reste, rien ne traîne.
        lance(&root.join("absent.AppImage"));
        assert_eq!(std::fs::read_to_string(&cible).unwrap(), "ancien");
        assert!(!root.join(".Velka-maj.AppImage").exists());
        // Fichier reçu présent : remplacé, exécutable, fichier reçu supprimé.
        lance(&nouveau);
        assert_eq!(std::fs::read_to_string(&cible).unwrap(), "nouveau");
        assert_eq!(
            std::fs::metadata(&cible).unwrap().permissions().mode() & 0o111,
            0o111
        );
        assert!(!nouveau.exists() && !root.join(".Velka-maj.AppImage").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn script_linux_refuse_un_chemin_avec_apostrophe() {
        assert!(linux_script(
            1,
            Path::new("/tmp/a'b.AppImage"),
            Path::new("/home/x/Velka.AppImage"),
            true
        )
        .is_none());
        let s = linux_script(
            42,
            Path::new("/tmp/maj"),
            Path::new("/home/x/Velka.AppImage"),
            true,
        )
        .unwrap();
        assert!(s.contains("kill -0 42") && s.contains("nohup '/home/x/Velka.AppImage'"));
    }
}
