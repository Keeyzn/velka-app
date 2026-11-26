//! Détection, téléchargement et extraction des outils (Streamlink, libmpv).
//! Les versions validées (étiquette, archive, empreinte) sont inscrites dans `commun::outils` ; l'adresse de
//! téléchargement vient toujours de l'API des releases GitHub et doit pointer vers le dépôt attendu.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Streamlink,
    Libmpv,
}

impl Tool {
    pub const ALL: [Tool; 2] = [Tool::Streamlink, Tool::Libmpv];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Streamlink => "Streamlink",
            Tool::Libmpv => "libmpv",
        }
    }

    /// Nom court utilisé dans le manifeste des versions validées.
    fn key(self) -> &'static str {
        match self {
            Tool::Streamlink if cfg!(target_os = "macos") => commun::outils::STREAMLINK_MACOS,
            Tool::Libmpv if cfg!(target_os = "macos") => commun::outils::LIBMPV_MACOS,
            Tool::Streamlink if cfg!(target_os = "linux") => commun::outils::STREAMLINK_LINUX,
            Tool::Libmpv if cfg!(target_os = "linux") => commun::outils::LIBMPV_LINUX,
            Tool::Streamlink => commun::outils::STREAMLINK,
            Tool::Libmpv => commun::outils::LIBMPV,
        }
    }

    fn repo(self) -> &'static str {
        commun::outils::repo(self.key()).unwrap_or_default()
    }

    /// Nom du fichier de l'outil sur ce système (`streamlink` et `libmpv.2.dylib` sur Mac, `libmpv.so.2` sous Linux).
    pub fn file_name(self) -> &'static str {
        match self {
            Tool::Streamlink if !cfg!(windows) => "streamlink",
            Tool::Libmpv if cfg!(target_os = "macos") => "libmpv.2.dylib",
            Tool::Libmpv if cfg!(target_os = "linux") => "libmpv.so.2",
            Tool::Streamlink => "streamlink.exe",
            Tool::Libmpv => "libmpv-2.dll",
        }
    }
}

pub fn tools_dir() -> PathBuf {
    crate::logging::data_dir().join("tools")
}

/// Cherche l'outil : chemin des réglages, dossier `tools\`, installations connues, `PATH`.
pub fn detect(tool: Tool, custom: &str) -> Option<PathBuf> {
    let name = tool.file_name();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        candidates.push(if p.is_dir() { p.join(name) } else { p });
    }
    if let Some(p) = find_in_tools_dir(tool, &tools_dir()) {
        candidates.push(p);
    }
    if tool == Tool::Streamlink {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                candidates.push(Path::new(&pf).join("Streamlink").join("bin").join(name));
            }
        }
    }
    // Le `PATH` ne sert que pour Streamlink (un programme lancé tel quel). libmpv est une DLL chargée dans le
    // processus de l'application : on ne la prend jamais dans un dossier du `PATH`, seulement dans `tools\` ou au
    // chemin choisi dans les réglages.
    if searches_path(tool) {
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|d| d.join(name)));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Mac et Linux : un outil trouvé hors du dossier `tools` (Streamlink d'un dossier du `PATH`) est géré par l'utilisateur,
/// comme un chemin personnalisé : jamais remplacé par l'application.
fn managed_elsewhere(path: &Path) -> bool {
    !cfg!(windows) && !path.starts_with(tools_dir())
}

/// L'outil peut-il être cherché dans les dossiers du `PATH` ?
fn searches_path(tool: Tool) -> bool {
    tool == Tool::Streamlink
}

/// Cherche l'outil installé par l'application. L'archive de Streamlink place l'exécutable
/// dans un sous-dossier (`streamlink\bin\streamlink.exe`) : on cherche donc aussi en profondeur.
pub fn find_in_tools_dir(tool: Tool, tools: &Path) -> Option<PathBuf> {
    let base = tools.join(tool_subdir(tool));
    let name = tool.file_name();
    let direct = [base.join(name), base.join("bin").join(name)];
    direct
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| find_file(&base, &[name]))
}

fn tool_subdir(tool: Tool) -> &'static str {
    match tool {
        Tool::Streamlink => "streamlink",
        Tool::Libmpv => "libmpv",
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Clone)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn agent() -> ureq::Agent {
    crate::net::builder(Duration::from_secs(600))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build()
        .into()
}

/// Version validée de ce composant (inscrite dans `commun::outils`). Sans version validée, rien n'est installé.
pub fn validated(tool: Tool) -> Result<commun::outils::Pinned, String> {
    commun::outils::pinned(tool.key()).ok_or_else(|| {
        crate::i18n::tr("Versions des composants non validées par l'éditeur.").to_string()
    })
}

/// Release GitHub d'une étiquette validée, et son archive au nom exact du manifeste. L'adresse vient toujours de
/// l'API GitHub ; elle doit pointer vers les releases du dépôt attendu.
fn fetch_release(tool: Tool, pinned: &commun::outils::Pinned) -> Result<Asset, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        tool.repo(),
        pinned.tag
    );
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Impossible de joindre GitHub :")))?;
    let release: Release = resp
        .body_mut()
        .read_json()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Réponse GitHub illisible :")))?;
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name == pinned.asset)
        .ok_or_else(|| crate::i18n::tr("Archive validée introuvable sur GitHub.").to_string())?;
    if !commun::outils::download_url_ok(tool.key(), &asset.browser_download_url) {
        return Err(crate::i18n::tr("Adresse de téléchargement inattendue.").into());
    }
    Ok(asset)
}

/// Télécharge vers `dest` et renvoie l'empreinte SHA-256 (hexadécimal minuscule) du fichier reçu.
fn download(url: &str, dest: &Path, progress: &dyn Fn(f32)) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Téléchargement impossible :")))?;
    let total = resp.body().content_length().unwrap_or(0);
    let mut reader = resp.body_mut().as_reader();
    let mut file = File::create(dest)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Écriture impossible :")))?;
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut hasher = Sha256::new();
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
        if total > 0 {
            progress(done as f32 / total as f32);
        }
    }
    if done == 0 {
        return Err(crate::i18n::tr("Le fichier téléchargé est vide.").into());
    }
    Ok(commun::to_hex(&hasher.finalize()))
}

/// Recherche récursive d'un fichier par nom (insensible à la casse).
fn find_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(f) = find_file(&p, names) {
                return Some(f);
            }
        } else if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
            if names.iter().any(|w| w.eq_ignore_ascii_case(n)) {
                return Some(p);
            }
        }
    }
    None
}

// ---------- Versions et mises à jour ----------

/// Fichier posé dans le dossier de l'outil installé par l'application : il contient la version (étiquette de la
/// release GitHub) réellement installée, pour savoir plus tard si l'outil est à jour.
pub const VERSION_FILE: &str = "velka-version.txt";

/// Nom de l'archive libmpv réellement installée. Sert à remplacer un libmpv GPL déjà installé par le build
/// LGPL même quand l'étiquette de la release est la même (les deux builds sortent dans la même release GitHub).
pub const ASSET_FILE: &str = "velka-asset.txt";

/// Le version validée actuellement servi désigne-t-il un build libmpv LGPL ? Vrai seulement après une lecture réussie
/// du manifeste (`latest_tag`) ; tant qu'il est faux (manifeste illisible, ou encore GPL), rien ne change.
static MANIFEST_LIBMPV_LGPL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Nom d'archive d'un build libmpv LGPL (voir `commun::outils::pick_libmpv_lgpl`).
fn is_lgpl_asset(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    // Windows : `mpv-dev-lgpl-…` (zhongfly/mpv-winbuild) ; Mac et Linux : `libmpv-lgpl-…` (Keeyzn/velka-composants-mac,
    // Keeyzn/velka-composants-linux).
    n.starts_with("mpv-dev-lgpl-") || n.starts_with("libmpv-lgpl-")
}

/// Version affichée et comparée pour libmpv : l'étiquette, suivie de « (GPL) » quand le manifeste demande le build
/// LGPL et que le libmpv installé n'est pas (ou n'est pas connu comme) un build LGPL. Le texte différent suffit à
/// `is_update_available` pour proposer la mise à jour (mêmes composants numériques, texte différent).
fn libmpv_version_label(tag: &str, manifest_lgpl: bool, installed_asset: Option<&str>) -> String {
    if manifest_lgpl && !installed_asset.is_some_and(is_lgpl_asset) {
        format!("{tag} (GPL)")
    } else {
        tag.to_string()
    }
}

/// Composants numériques d'une version : `8.6.3-1` -> [8, 6, 3, 1], `2026-11-20-d11ffba` -> [2026, 11, 20],
/// `streamlink 8.0.0` -> [8, 0, 0]. Lit à partir du premier chiffre ; s'arrête devant une lettre (hachage de commit).
pub fn parse_version(s: &str) -> Option<Vec<u64>> {
    let b = s.as_bytes();
    let at = |k: usize| b.get(k).copied().unwrap_or(0);
    let mut i = s.find(|c: char| c.is_ascii_digit())?;
    let mut out = Vec::new();
    loop {
        let j = i + b
            .get(i..)
            .unwrap_or_default()
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if at(j).is_ascii_alphabetic() {
            break;
        }
        out.push(
            s.get(i..j)
                .and_then(|d| d.parse::<u64>().ok())
                .unwrap_or(u64::MAX),
        );
        if (at(j) == b'.' || at(j) == b'-') && at(j + 1).is_ascii_digit() {
            i = j + 1;
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn cmp_parts(x: &[u64], y: &[u64]) -> std::cmp::Ordering {
    for i in 0..x.len().max(y.len()) {
        let o = x.get(i).unwrap_or(&0).cmp(y.get(i).unwrap_or(&0));
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    std::cmp::Ordering::Equal
}

/// Compare deux versions composant par composant (jamais comme du texte : 10.0 est plus récent que 9.0).
#[cfg(test)]
pub fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    Some(cmp_parts(&parse_version(a)?, &parse_version(b)?))
}

/// La version `latest` est-elle plus récente que `installed` ? Une version installée sans numéro de build
/// (`streamlink --version` donne `8.6.3`, l'étiquette est `8.6.3-1`) n'est comparée que sur ses composants.
/// À composants égaux, une étiquette différente (autre build le même jour) compte comme une mise à jour.
pub fn is_update_available(installed: &str, latest: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches('v').to_string();
    let (Some(i), Some(l)) = (parse_version(installed), parse_version(latest)) else {
        // Illisible : on ne sait pas ordonner, on ne compare que l'égalité du texte.
        return norm(installed) != norm(latest);
    };
    let l_cmp = l.get(..i.len()).unwrap_or(&l);
    match cmp_parts(&i, l_cmp) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => i.len() == l.len() && norm(installed) != norm(latest),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    /// Pas installé.
    Missing,
    UpToDate,
    /// Version plus récente disponible, ou version installée inconnue (on propose alors la mise à jour).
    Outdated,
    /// Chemin personnalisé des Réglages : géré par l'utilisateur, jamais mis à jour par l'application.
    Custom,
}

impl UpdateState {
    pub fn needs_update(self) -> bool {
        matches!(self, UpdateState::Missing | UpdateState::Outdated)
    }
}

/// État d'un composant à partir de ce qu'on sait de lui.
pub fn evaluate(present: bool, custom: bool, installed: Option<&str>, latest: &str) -> UpdateState {
    if !present {
        UpdateState::Missing
    } else if custom {
        UpdateState::Custom
    } else {
        match installed {
            None => UpdateState::Outdated,
            Some(v) if is_update_available(v, latest) => UpdateState::Outdated,
            Some(_) => UpdateState::UpToDate,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolReport {
    pub tool: Tool,
    pub path: Option<PathBuf>,
    pub installed: Option<String>,
    pub latest: String,
    pub state: UpdateState,
}

/// Résultat de la dernière vérification des composants.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolsCheck {
    /// Pas encore vérifié (ou à refaire).
    Idle,
    Checking,
    /// Vérification impossible (réseau, GitHub) : le bouton reste actif pour réessayer.
    Failed(String),
    Done(Vec<ToolReport>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Vérification en cours : bouton désactivé.
    Checking,
    /// Mise à jour en cours : bouton désactivé (pas de double clic, pas de mises à jour simultanées).
    Updating,
    /// Au moins un composant à mettre à jour : bouton actif.
    Update,
    /// Tout est à jour : bouton désactivé, « À jour ».
    UpToDate,
    /// Vérification impossible : bouton actif, pour réessayer.
    Retry,
    /// Seule une mise à jour de libmpv reste, et libmpv est en cours d'utilisation : bouton actif « Redémarrage
    /// nécessaire » qui redémarre l'application et met à jour au démarrage suivant.
    RestartRequired,
}

/// Composants à mettre à jour (tous ceux qui en ont besoin) ; libmpv est ignoré tant qu'il est chargé (Windows ne
/// permet pas de remplacer une DLL en cours d'utilisation).
pub fn update_targets(check: &ToolsCheck, libmpv_locked: bool) -> Vec<Tool> {
    match check {
        ToolsCheck::Done(reports) => reports
            .iter()
            .filter(|r| r.state.needs_update() && !(r.tool == Tool::Libmpv && libmpv_locked))
            .map(|r| r.tool)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn button_state(check: &ToolsCheck, busy: bool, libmpv_locked: bool) -> ButtonState {
    if busy {
        return ButtonState::Updating;
    }
    match check {
        ToolsCheck::Idle | ToolsCheck::Checking => ButtonState::Checking,
        ToolsCheck::Failed(_) => ButtonState::Retry,
        ToolsCheck::Done(reports) => {
            if !update_targets(check, libmpv_locked).is_empty() {
                ButtonState::Update
            } else if reports.iter().any(|r| r.state.needs_update()) {
                ButtonState::RestartRequired
            } else {
                ButtonState::UpToDate
            }
        }
    }
}

/// Dernière version publiée pour l'outil (étiquette de la release GitHub).
pub fn latest_tag(tool: Tool) -> Result<String, String> {
    let p = validated(tool)?;
    if tool == Tool::Libmpv {
        MANIFEST_LIBMPV_LGPL.store(
            is_lgpl_asset(&p.asset),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
    Ok(p.tag)
}

fn custom_candidate(tool: Tool, custom: &str) -> Option<PathBuf> {
    if custom.trim().is_empty() {
        return None;
    }
    let p = PathBuf::from(custom.trim());
    Some(if p.is_dir() {
        p.join(tool.file_name())
    } else {
        p
    })
}

/// Version réellement installée : lue dans le fichier posé à l'installation ; à défaut (Streamlink venu d'ailleurs ou
/// installé par une ancienne version de l'application) en interrogeant `streamlink --version`.
pub fn installed_version(tool: Tool, path: &Path) -> Option<String> {
    let managed = tools_dir().join(tool_subdir(tool));
    if path.starts_with(&managed) {
        if let Ok(v) = fs::read_to_string(managed.join(VERSION_FILE)) {
            let v = v.trim();
            if !v.is_empty() {
                if tool == Tool::Libmpv {
                    let asset = fs::read_to_string(managed.join(ASSET_FILE)).ok();
                    let manifest_lgpl =
                        MANIFEST_LIBMPV_LGPL.load(std::sync::atomic::Ordering::Relaxed);
                    return Some(libmpv_version_label(v, manifest_lgpl, asset.as_deref()));
                }
                return Some(v.to_string());
            }
        }
    }
    match tool {
        Tool::Streamlink => streamlink_version(path),
        Tool::Libmpv => None,
    }
}

fn streamlink_version(exe: &Path) -> Option<String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v = text.split_whitespace().last()?.to_string();
    parse_version(&v).map(|_| v)
}

/// État d'un composant installé ou non, face à la dernière version `latest`.
pub fn report(tool: Tool, custom: &str, latest: &str) -> ToolReport {
    let path = detect(tool, custom);
    let present = path.is_some();
    let custom_used = match (&path, custom_candidate(tool, custom)) {
        (Some(p), Some(c)) => *p == c,
        _ => false,
    } || path.as_deref().is_some_and(managed_elsewhere);
    let installed = path.as_deref().and_then(|p| installed_version(tool, p));
    let state = evaluate(present, custom_used, installed.as_deref(), latest);
    ToolReport {
        tool,
        path,
        installed,
        latest: latest.to_string(),
        state,
    }
}

/// Ce que le diagnostic dit d'un composant.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub found: bool,
    /// Le chemin personnalisé des Réglages est celui qui sert.
    pub custom: bool,
    pub version: Option<String>,
    /// « LGPL » ou « GPL » pour un libmpv installé par l'application.
    pub build: Option<&'static str>,
}

/// Présence, origine et version d'un composant, sans jamais donner son chemin.
pub fn component_info(tool: Tool, custom: &str) -> Component {
    let path = detect(tool, custom);
    let custom_used = match (&path, custom_candidate(tool, custom)) {
        (Some(p), Some(c)) => *p == c,
        _ => false,
    };
    let version = path.as_deref().and_then(|p| installed_version(tool, p));
    let build = match (tool, path.as_deref()) {
        (Tool::Libmpv, Some(p)) => libmpv_build_in(&tools_dir().join(tool_subdir(Tool::Libmpv)), p),
        _ => None,
    };
    Component {
        found: path.is_some(),
        custom: custom_used,
        version,
        build,
    }
}

/// Build d'un libmpv installé par l'application : LGPL si l'archive posée à l'installation (`velka-asset.txt`) en est
/// une, GPL sinon (ancienne installation : pas de fichier). `None` pour un libmpv venu d'ailleurs.
fn libmpv_build_in(managed: &Path, dll: &Path) -> Option<&'static str> {
    if !dll.starts_with(managed) {
        return None;
    }
    let lgpl = fs::read_to_string(managed.join(ASSET_FILE))
        .ok()
        .is_some_and(|a| is_lgpl_asset(&a));
    Some(if lgpl { "LGPL" } else { "GPL" })
}

/// Vérifie tous les composants. `known_latest` : dernières versions déjà connues (récentes) pour éviter d'interroger
/// GitHub à chaque fois. Échoue (sans rien griser) si une dernière version ne peut pas être obtenue.
pub fn check_all(
    customs: &[(Tool, String)],
    known_latest: &[(Tool, String)],
    fetch: &dyn Fn(Tool) -> Result<String, String>,
) -> Result<Vec<ToolReport>, String> {
    let mut out = Vec::new();
    for tool in Tool::ALL {
        let latest = match known_latest.iter().find(|(t, _)| *t == tool) {
            Some((_, v)) => v.clone(),
            None => fetch(tool).map_err(|e| {
                format!(
                    "{} {e}",
                    crate::i18n::tr("Impossible de vérifier les mises à jour :")
                )
            })?,
        };
        let custom = customs
            .iter()
            .find(|(t, _)| *t == tool)
            .map_or("", |(_, c)| c.as_str());
        out.push(report(tool, custom, &latest));
    }
    Ok(out)
}

/// Marque posée avant un redémarrage demandé par l'utilisateur : au prochain démarrage (libmpv n'est alors plus
/// chargée), l'application met à jour d'elle-même tous les composants qui en ont besoin.
const PENDING_FILE: &str = "tools-update.pending";

pub fn mark_update_pending_in(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(PENDING_FILE), b"1").map_err(|e| e.to_string())
}

/// Lit et efface la marque : `true` une seule fois (jamais de redémarrage en boucle).
pub fn take_update_pending_in(dir: &Path) -> bool {
    fs::remove_file(dir.join(PENDING_FILE)).is_ok()
}

pub fn mark_update_pending() -> Result<(), String> {
    mark_update_pending_in(&crate::logging::data_dir())
}

pub fn take_update_pending() -> bool {
    take_update_pending_in(&crate::logging::data_dir())
}

/// Remet en place une ancienne version si une mise à jour a été interrompue entre le retrait et la mise en place.
fn recover_interrupted(root: &Path, tool: Tool) {
    let dest = root.join(tool_subdir(tool));
    let backup = root.join(format!("old-{}", tool_subdir(tool)));
    if !dest.exists() && backup.exists() {
        let _ = fs::rename(&backup, &dest);
    }
}

/// Télécharge et installe l'outil ; renvoie le chemin de l'exécutable ou de la DLL.
/// `progress` reçoit (fraction 0..1, message).
pub fn install(tool: Tool, progress: &dyn Fn(f32, &str)) -> Result<PathBuf, String> {
    progress(0.0, crate::i18n::tr("Recherche de la dernière version…"));
    recover_interrupted(&tools_dir(), tool);
    let pinned = validated(tool)?;
    let asset = fetch_release(tool, &pinned)?;
    crate::logging::log(&format!("{:?} : {} ({})", tool, pinned.tag, asset.name));

    let root = tools_dir();
    let work = root.join(format!("tmp-{}", tool_subdir(tool)));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Dossier inaccessible :")))?;
    let result = install_inner(tool, &asset, &pinned, &root, &work, progress);
    let _ = fs::remove_dir_all(&work);
    result
}

fn install_inner(
    tool: Tool,
    asset: &Asset,
    pinned: &commun::outils::Pinned,
    root: &Path,
    work: &Path,
    progress: &dyn Fn(f32, &str),
) -> Result<PathBuf, String> {
    let archive = work.join(&asset.name);
    let sha = download(&asset.browser_download_url, &archive, &|f| {
        progress(f * 0.8, crate::i18n::tr("Téléchargement…"))
    })?;
    // Seule l'archive dont l'empreinte a été validée par l'éditeur est extraite.
    if sha != pinned.sha256 {
        crate::logging::log(&format!(
            "{tool:?} : empreinte {sha} au lieu de {}",
            pinned.sha256
        ));
        return Err(crate::i18n::tr("Empreinte du fichier téléchargé incorrecte.").into());
    }
    progress(0.8, crate::i18n::tr("Extraction…"));
    let extracted = work.join("out");
    fs::create_dir_all(&extracted)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Extraction impossible :")))?;

    let dest_dir = root.join(tool_subdir(tool));
    let staging = root.join(format!("new-{}", tool_subdir(tool)));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|e| format!("{} {e}", crate::i18n::tr("Dossier inaccessible :")))?;

    let built = build_staging(tool, &archive, &extracted, &staging);
    if let Err(e) = built {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    // La version installée est notée dans le dossier même de l'outil (elle suit donc l'échange ci-dessous).
    if let Err(e) = fs::write(staging.join(VERSION_FILE), &pinned.tag) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("{} {e}", crate::i18n::tr("Écriture impossible :")));
    }

    // libmpv : on note aussi quelle archive (GPL ou LGPL) vient d'être installée.
    if tool == Tool::Libmpv {
        if let Err(e) = fs::write(staging.join(ASSET_FILE), &asset.name) {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("{} {e}", crate::i18n::tr("Écriture impossible :")));
        }
    }

    // Échange sûr : l'ancienne version est mise de côté, puis remise en place si quoi que ce soit échoue
    // (fichier verrouillé, disque plein, archive vide…) : la version précédente qui fonctionne est conservée.
    let backup = root.join(format!("old-{}", tool_subdir(tool)));
    let _ = fs::remove_dir_all(&backup);
    let had_old = dest_dir.exists();
    if had_old {
        if let Err(e) = fs::rename(&dest_dir, &backup) {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!(
                "{} {e}",
                crate::i18n::tr("Installation impossible :")
            ));
        }
    }
    let restore = || {
        let _ = fs::remove_dir_all(&dest_dir);
        if had_old {
            let _ = fs::rename(&backup, &dest_dir);
        }
    };
    if let Err(e) = fs::rename(&staging, &dest_dir) {
        let _ = fs::remove_dir_all(&staging);
        restore();
        return Err(format!(
            "{} {e}",
            crate::i18n::tr("Installation impossible :")
        ));
    }

    let final_path = match tool {
        Tool::Streamlink => find_file(&dest_dir, &[Tool::Streamlink.file_name()]),
        Tool::Libmpv => Some(dest_dir.join(Tool::Libmpv.file_name())),
    }
    .filter(|p| p.is_file() && fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false));
    match final_path {
        Some(p) => {
            let _ = fs::remove_dir_all(&backup);
            progress(1.0, crate::i18n::tr("Installé"));
            Ok(p)
        }
        None => {
            restore();
            Err(crate::i18n::tr("Fichier installé introuvable ou vide.").to_string())
        }
    }
}

/// Entrée de l'archive libmpv à garder : seul le nom de fichier compte, jamais le dossier indiqué dans l'archive.
fn is_libmpv_entry(name: &str) -> bool {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.eq_ignore_ascii_case("libmpv-2.dll") || base.eq_ignore_ascii_case("mpv-2.dll")
}

/// Extrait de l'archive 7z la seule DLL de libmpv, écrite au chemin fixe `dest`. Aucun chemin venant de l'archive
/// n'est utilisé : une entrée `..\x` ou un chemin absolu ne peut rien écrire ailleurs. Les entrées précédentes sont
/// lues sans être écrites (archive « solide ») et la lecture s'arrête dès que la DLL est extraite.
fn extract_libmpv(archive: &Path, dest: &Path) -> Result<(), String> {
    use sevenz_rust2::{ArchiveReader, EntryDecision, Error, Password};
    let mut found = false;
    // `ArchiveReader` seul, sans l'aide d'extraction « dossier » du paquet : aucun chemin de l'archive n'est calculé
    // ni utilisé, une entrée `..\x` est simplement lue puis ignorée.
    let mut extract = || -> Result<(), Error> {
        let mut reader = ArchiveReader::open(archive, Password::empty())?;
        reader.for_each_entries_with_decision(|entry, data| {
            if entry.is_directory() || !is_libmpv_entry(entry.name()) {
                // Archive « solide » : les entrées précédentes sont lues (et vérifiées) sans être écrites.
                return Ok(EntryDecision::Continue);
            }
            let mut f = File::create(dest)?;
            std::io::copy(data, &mut f)?;
            found = true;
            Ok(EntryDecision::Stop)
        })
    };
    extract().map_err(|e| format!("{} {e}", crate::i18n::tr("Extraction impossible :")))?;
    if found {
        Ok(())
    } else {
        let _ = fs::remove_file(dest);
        Err(crate::i18n::tr("libmpv-2.dll introuvable dans l'archive.").to_string())
    }
}

/// Prépare le dossier `staging` de l'outil à partir de l'archive vérifiée (`extracted` : dossier de travail).
/// Windows : Streamlink portable (zip, dossier parent de `bin\`) et la seule DLL de libmpv (7z). Mac :
/// archives zip de `Keeyzn/velka-composants-mac`, Streamlink (`streamlink/bin/streamlink` + Python autonome)
/// et libmpv (dossier `libmpv/` : `libmpv.2.dylib` et ses bibliothèques, qui se cherchent dans leur propre dossier).
/// Linux : mêmes dispositions, archives de `Keeyzn/velka-composants-linux` (`libmpv.so.2` seule,
/// ses bibliothèques sont liées dedans).
fn build_staging(
    tool: Tool,
    archive: &Path,
    extracted: &Path,
    staging: &Path,
) -> Result<(), String> {
    let unzip = || -> Result<(), String> {
        let f = File::open(archive).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(f)
            .map_err(|e| format!("{} {e}", crate::i18n::tr("Archive invalide :")))?;
        // `extract` n'écrit qu'à l'intérieur de `extracted` (chemins `..` ou absolus refusés) et garde les droits
        // d'exécution enregistrés dans l'archive (lanceur et Python sur Mac).
        zip.extract(extracted)
            .map_err(|e| format!("{} {e}", crate::i18n::tr("Extraction impossible :")))
    };
    match tool {
        Tool::Streamlink => {
            unzip()?;
            let exe = find_file(extracted, &[Tool::Streamlink.file_name()]).ok_or(
                crate::i18n::tr("streamlink.exe introuvable dans l'archive."),
            )?;
            // On garde tout le dossier parent de bin\ (Python embarqué, modules).
            let app_root = exe
                .parent()
                .and_then(|bin| bin.parent())
                .ok_or(crate::i18n::tr("Structure d'archive inattendue."))?;
            copy_dir(app_root, staging)
        }
        Tool::Libmpv if !cfg!(windows) => {
            unzip()?;
            let lib = find_file(extracted, &[Tool::Libmpv.file_name()])
                .ok_or(crate::i18n::tr("libmpv-2.dll introuvable dans l'archive."))?;
            let dir = lib
                .parent()
                .ok_or(crate::i18n::tr("Structure d'archive inattendue."))?;
            copy_dir(dir, staging)
        }
        Tool::Libmpv => extract_libmpv(archive, &staging.join(Tool::Libmpv.file_name())),
    }
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// libmpv (DLL chargée dans l'application) n'est jamais cherchée dans les dossiers du `PATH`.
    #[test]
    fn libmpv_jamais_cherchee_dans_le_path() {
        assert!(!searches_path(Tool::Libmpv));
        assert!(searches_path(Tool::Streamlink));
    }

    #[test]
    fn versions_numeriques_pas_textuelles() {
        use std::cmp::Ordering::*;
        // « 10.0 » est plus récent que « 9.0 » (en texte, "10.0" < "9.0").
        assert_eq!(compare_versions("10.0", "9.0"), Some(Greater));
        assert_eq!(compare_versions("9.0", "10.0"), Some(Less));
        assert_eq!(compare_versions("8.6.3-1", "8.6.3-1"), Some(Equal));
        assert_eq!(compare_versions("8.6.10-1", "8.6.9-1"), Some(Greater));
        assert_eq!(
            compare_versions("8.6", "8.6.0"),
            Some(Equal),
            "composants manquants = 0"
        );
        assert_eq!(compare_versions("v8.6.3", "8.6.3"), Some(Equal));
        assert_eq!(
            compare_versions("2026-11-20-d11ffba253", "2026-11-19-ffffff"),
            Some(Greater)
        );
        assert_eq!(
            compare_versions("2026-12-01-abc", "2026-11-30-abc"),
            Some(Greater)
        );
        assert_eq!(compare_versions("abc", "1.0"), None);
        assert_eq!(parse_version("streamlink 8.0.0"), Some(vec![8, 0, 0]));
        assert_eq!(
            parse_version("2026-11-20-1a2b3c"),
            Some(vec![2026, 11, 20]),
            "un hachage n'est pas un numéro"
        );
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn mise_a_jour_disponible() {
        assert!(is_update_available("8.6.2-1", "8.6.3-1"));
        assert!(is_update_available("9.0.0-1", "10.0.0-1"));
        assert!(is_update_available("8.6.3-1", "8.6.3-2"), "autre build");
        assert!(!is_update_available("8.6.3-1", "8.6.3-1"));
        assert!(
            !is_update_available("10.0.0-1", "9.0.0-1"),
            "installée plus récente"
        );
        // `streamlink --version` n'a pas de numéro de build.
        assert!(!is_update_available("8.6.3", "8.6.3-1"));
        assert!(is_update_available("8.6.2", "8.6.3-1"));
        assert!(!is_update_available("8.7.0", "8.6.3-1"));
        // libmpv : étiquettes datées.
        assert!(!is_update_available(
            "2026-11-20-d11ffba253",
            "2026-11-20-d11ffba253"
        ));
        assert!(is_update_available(
            "2026-11-19-aaaaaaaa",
            "2026-11-20-d11ffba253"
        ));
        assert!(
            is_update_available("2026-11-20-aaaaaaaa", "2026-11-20-bbbbbbbb"),
            "autre build le même jour"
        );
        // Illisible : seulement l'égalité du texte.
        assert!(!is_update_available("abc", "abc"));
        assert!(is_update_available("abc", "def"));
    }

    #[test]
    fn etat_d_un_composant() {
        use UpdateState::*;
        assert_eq!(evaluate(false, false, None, "8.6.3-1"), Missing);
        assert_eq!(
            evaluate(true, true, Some("1.0"), "8.6.3-1"),
            Custom,
            "chemin personnalisé : jamais mis à jour"
        );
        assert_eq!(evaluate(true, false, Some("8.6.3-1"), "8.6.3-1"), UpToDate);
        assert_eq!(evaluate(true, false, Some("8.6.2-1"), "8.6.3-1"), Outdated);
        assert_eq!(
            evaluate(true, false, None, "8.6.3-1"),
            Outdated,
            "version inconnue : on propose la mise à jour"
        );
        assert!(
            Missing.needs_update()
                && Outdated.needs_update()
                && !UpToDate.needs_update()
                && !Custom.needs_update()
        );
    }

    fn rapport(tool: Tool, state: UpdateState) -> ToolReport {
        ToolReport {
            tool,
            path: None,
            installed: None,
            latest: "1".into(),
            state,
        }
    }

    /// État du bouton : actif seulement si au moins un composant a une mise à jour ; jamais grisé sur une erreur.
    #[test]
    fn etat_du_bouton() {
        use UpdateState::*;
        let done =
            |a, b| ToolsCheck::Done(vec![rapport(Tool::Streamlink, a), rapport(Tool::Libmpv, b)]);
        // 1. Tout à jour : désactivé, « À jour ».
        assert_eq!(
            button_state(&done(UpToDate, UpToDate), false, false),
            ButtonState::UpToDate
        );
        assert_eq!(
            button_state(&done(UpToDate, Custom), false, false),
            ButtonState::UpToDate
        );
        // 2. Un seul à mettre à jour : actif, et la cible est celui-là seulement.
        assert_eq!(
            button_state(&done(Outdated, UpToDate), false, false),
            ButtonState::Update
        );
        assert_eq!(
            update_targets(&done(Outdated, UpToDate), false),
            vec![Tool::Streamlink]
        );
        assert_eq!(
            update_targets(&done(UpToDate, Outdated), false),
            vec![Tool::Libmpv]
        );
        // 3. Les deux : actif, les deux sont ciblés (pas seulement le premier).
        assert_eq!(
            button_state(&done(Outdated, Missing), false, false),
            ButtonState::Update
        );
        assert_eq!(
            update_targets(&done(Outdated, Missing), false),
            vec![Tool::Streamlink, Tool::Libmpv]
        );
        // Pendant une opération : désactivé (pas de double clic).
        assert_eq!(
            button_state(&done(Outdated, Outdated), true, false),
            ButtonState::Updating
        );
        assert_eq!(
            button_state(&ToolsCheck::Checking, false, false),
            ButtonState::Checking
        );
        assert_eq!(
            button_state(&ToolsCheck::Idle, false, false),
            ButtonState::Checking
        );
        // 4. Erreur de vérification : le bouton reste actif pour réessayer.
        assert_eq!(
            button_state(&ToolsCheck::Failed("réseau".into()), false, false),
            ButtonState::Retry
        );
        assert!(update_targets(&ToolsCheck::Failed("réseau".into()), false).is_empty());
        // libmpv en cours d'utilisation : ignoré ; s'il reste le seul, redémarrage nécessaire (jamais « À jour »).
        assert_eq!(
            update_targets(&done(Outdated, Outdated), true),
            vec![Tool::Streamlink]
        );
        assert_eq!(
            button_state(&done(Outdated, Outdated), false, true),
            ButtonState::Update
        );
        assert_eq!(
            button_state(&done(UpToDate, Outdated), false, true),
            ButtonState::RestartRequired
        );
    }

    /// Vérification : une erreur de réseau donne une erreur (jamais « à jour ») ; les versions déjà connues évitent
    /// l'appel réseau.
    #[test]
    fn marque_de_mise_a_jour_au_demarrage() {
        let dir = std::env::temp_dir().join(format!("velka-pending-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(!take_update_pending_in(&dir), "rien à faire sans marque");
        mark_update_pending_in(&dir).unwrap();
        assert!(take_update_pending_in(&dir), "la marque est lue une fois");
        assert!(
            !take_update_pending_in(&dir),
            "puis effacée : pas de redémarrage en boucle"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verification_reseau() {
        let echec = |_t: Tool| Err::<String, String>("réseau coupé".to_string());
        let e = check_all(&[], &[], &echec).unwrap_err();
        assert!(e.contains("réseau coupé"), "{e}");
        let connues = [
            (Tool::Streamlink, "8.6.3-1".to_string()),
            (Tool::Libmpv, "2026-11-20-x".to_string()),
        ];
        let r = check_all(&[], &connues, &echec).expect("pas d'appel réseau quand tout est connu");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].latest, "8.6.3-1");
    }

    /// L'installation écrit la version ; un échec garde l'ancienne version ; une installation interrompue est réparée.
    #[test]
    fn echange_sur_et_version_notee() {
        let root = std::env::temp_dir().join(format!("velka-swap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dest = root.join("libmpv");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("libmpv-2.dll"), b"ancienne").unwrap();
        fs::write(dest.join(VERSION_FILE), "2026-01-01-old").unwrap();
        // Mise à jour interrompue juste après la mise de côté : l'ancienne version revient.
        let backup = root.join("old-libmpv");
        fs::rename(&dest, &backup).unwrap();
        assert!(!dest.exists());
        recover_interrupted(&root, Tool::Libmpv);
        assert_eq!(fs::read(dest.join("libmpv-2.dll")).unwrap(), b"ancienne");
        assert!(!backup.exists());
        // Rien à réparer quand tout est en place.
        recover_interrupted(&root, Tool::Libmpv);
        assert_eq!(
            fs::read_to_string(dest.join(VERSION_FILE)).unwrap(),
            "2026-01-01-old"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// le diagnostic dit si le libmpv installé par l'application est LGPL ou GPL ; un libmpv venu d'ailleurs
    /// (chemin personnalisé, PATH) n'a pas de build connu.
    #[test]
    fn build_libmpv_pour_le_diagnostic() {
        let base = std::env::temp_dir().join(format!("velka-test-{}-build", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let managed = base.join("tools").join("libmpv");
        fs::create_dir_all(&managed).unwrap();
        let dll = managed.join("libmpv-2.dll");
        let ailleurs = base.join("autre").join("libmpv-2.dll");
        assert_eq!(
            libmpv_build_in(&managed, &dll),
            Some("GPL"),
            "installé avant L2 : pas de fichier, donc GPL"
        );
        fs::write(
            managed.join(ASSET_FILE),
            "mpv-dev-lgpl-x86_64-20261124-git-35efcf53d5.7z",
        )
        .unwrap();
        assert_eq!(libmpv_build_in(&managed, &dll), Some("LGPL"));
        fs::write(
            managed.join(ASSET_FILE),
            "mpv-dev-x86_64-20261120-git-d11ffba253.7z",
        )
        .unwrap();
        assert_eq!(libmpv_build_in(&managed, &dll), Some("GPL"));
        assert_eq!(
            libmpv_build_in(&managed, &ailleurs),
            None,
            "libmpv hors du dossier de l'application"
        );
        let _ = fs::remove_dir_all(&base);
    }

    /// un libmpv GPL déjà installé est remplacé par le build LGPL même à étiquette identique, sans jamais
    /// proposer de mise à jour en boucle ni changer le comportement tant que le manifeste n'est pas LGPL.
    #[test]
    fn remplacement_gpl_par_lgpl() {
        let tag = "2026-11-20-d11ffba253";
        let gpl = "mpv-dev-x86_64-20261120-git-d11ffba253.7z";
        let lgpl = "mpv-dev-lgpl-x86_64-20261120-git-d11ffba253.7z";
        assert!(is_lgpl_asset(lgpl) && !is_lgpl_asset(gpl));
        // Manifeste encore GPL (ou illisible) : aucune différence avec le comportement d'avant.
        for marque in [None, Some(gpl), Some(lgpl)] {
            assert_eq!(libmpv_version_label(tag, false, marque), tag);
        }
        // Manifeste LGPL : ancienne installation (sans marque) ou installation GPL = à remplacer ; LGPL = à jour.
        let ancien = libmpv_version_label(tag, true, None);
        assert_eq!(ancien, format!("{tag} (GPL)"));
        assert_eq!(libmpv_version_label(tag, true, Some(gpl)), ancien);
        assert_eq!(libmpv_version_label(tag, true, Some(lgpl)), tag);
        assert_eq!(
            evaluate(true, false, Some(&ancien), tag),
            UpdateState::Outdated
        );
        assert_eq!(evaluate(true, false, Some(tag), tag), UpdateState::UpToDate);
        // Une étiquette plus récente reste une mise à jour, comme avant.
        assert_eq!(
            evaluate(true, false, Some(tag), "2026-11-25-aaaaaaa"),
            UpdateState::Outdated
        );
        // Chemin personnalisé : jamais touché.
        assert_eq!(
            evaluate(true, true, Some(&ancien), tag),
            UpdateState::Custom
        );
    }

    #[test]
    fn nom_de_la_dll_seul() {
        assert!(is_libmpv_entry("libmpv-2.dll"));
        assert!(is_libmpv_entry("bin/LIBMPV-2.DLL"));
        assert!(is_libmpv_entry("..\\..\\mpv-2.dll"));
        assert!(!is_libmpv_entry("libmpv-2.dll.a"));
        assert!(!is_libmpv_entry("include/mpv/client.h"));
        assert!(!is_libmpv_entry("libmpv-2.dll/"));
    }

    fn archive_7z(path: &Path, src: &Path, entrees: &[(&str, &[u8])]) {
        let mut w = sevenz_rust2::ArchiveWriter::create(path).unwrap();
        for (nom, contenu) in entrees {
            fs::write(src, contenu).unwrap();
            let e = sevenz_rust2::ArchiveEntry::from_path(src, nom.to_string());
            w.push_archive_entry(e, Some(File::open(src).unwrap()))
                .unwrap();
        }
        w.finish().unwrap();
    }

    /// Une entrée `../x` n'est jamais écrite : seule la DLL l'est, au chemin choisi par l'app.
    #[test]
    fn extraction_libmpv_sans_echappement() {
        let guard = crate::testutil::TempDir::new("7z");
        let root = guard.path().to_path_buf();
        let out = root.join("work").join("staging");
        fs::create_dir_all(&out).unwrap();
        let src = root.join("src.bin");
        let archive = root.join("piege.7z");
        // Entrées piégées : `../x`, `..\x` (séparateur Windows), chemin absolu, lecteur. Toutes sont ignorées.
        archive_7z(
            &archive,
            &src,
            &[
                ("../echappe.txt", b"piege"),
                (r"..\echappe2.txt", b"piege"),
                (r"..\..\echappe3.txt", b"piege"),
                (r"C:\echappe4.txt", b"piege"),
                ("/echappe5.txt", b"piege"),
                ("include/client.h", b"h"),
                ("sub/libmpv-2.dll", b"dll"),
            ],
        );
        let dest = out.join("libmpv-2.dll");
        extract_libmpv(&archive, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"dll");
        for nom in [
            "echappe.txt",
            "echappe2.txt",
            "echappe3.txt",
            "echappe4.txt",
            "echappe5.txt",
        ] {
            assert!(
                !root.join("work").join(nom).exists()
                    && !root.join(nom).exists()
                    && !out.join(nom).exists(),
                "{nom} écrit"
            );
        }
        assert!(!Path::new(r"C:\echappe4.txt").exists());
        assert_eq!(
            fs::read_dir(&out).unwrap().count(),
            1,
            "seule la DLL est extraite"
        );
        // Archive sans DLL : erreur, aucun fichier laissé.
        let vide = root.join("vide.7z");
        archive_7z(&vide, &src, &[("../echappe.txt", b"piege")]);
        fs::remove_file(&dest).unwrap();
        assert!(extract_libmpv(&vide, &dest).is_err());
        assert!(!dest.exists() && !root.join("work").join("echappe.txt").exists());
    }

    /// Essai manuel avec la vraie archive libmpv : `$env:VELKA_ARCHIVE_7Z="<fichier .7z>"` puis
    /// `cargo test extraction_archive_reelle -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn extraction_archive_reelle() {
        let Some(archive) = std::env::var_os("VELKA_ARCHIVE_7Z") else {
            panic!("VELKA_ARCHIVE_7Z non défini");
        };
        let guard = crate::testutil::TempDir::new("7z-reelle");
        let dest = guard.path().join("libmpv-2.dll");
        extract_libmpv(Path::new(&archive), &dest).unwrap();
        let taille = fs::metadata(&dest).unwrap().len();
        println!("libmpv-2.dll extraite : {taille} octets");
        assert!(taille > 1_000_000);
        assert_eq!(&fs::read(&dest).unwrap()[..2], b"MZ");
        // Facultatif : `VELKA_COPIE_DLL=<fichier>` garde une copie de la DLL extraite (essai à la main avec un
        // « chemin personnalisé » des Réglages, sans toucher au dossier `tools\` de l'application).
        if let Some(copie) = std::env::var_os("VELKA_COPIE_DLL") {
            fs::copy(&dest, copie).unwrap();
        }
    }

    /// Chaque composant de ce système a une version validée, au nom de son dépôt.
    #[test]
    fn versions_validees_presentes() {
        for t in [Tool::Streamlink, Tool::Libmpv] {
            let p = validated(t).expect("version validée");
            assert_eq!(p.outil, t.key());
            assert_eq!(p.sha256.len(), 64);
        }
    }

    #[test]
    fn streamlink_installe_dans_bin() {
        let root = std::env::temp_dir().join(format!("velka-tools-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin = root.join("streamlink").join("bin");
        fs::create_dir_all(&bin).unwrap();
        assert_eq!(find_in_tools_dir(Tool::Streamlink, &root), None);
        fs::write(bin.join(Tool::Streamlink.file_name()), b"x").unwrap();
        assert_eq!(
            find_in_tools_dir(Tool::Streamlink, &root),
            Some(bin.join(Tool::Streamlink.file_name()))
        );
        // libmpv à plat
        fs::create_dir_all(root.join("libmpv")).unwrap();
        fs::write(root.join("libmpv").join(Tool::Libmpv.file_name()), b"x").unwrap();
        assert!(find_in_tools_dir(Tool::Libmpv, &root).is_some());
        let _ = fs::remove_dir_all(&root);
    }

    /// Mac : installation à partir des vraies archives Mac (chemins donnés par `VELKA_ESSAI_SL_ZIP` et
    /// `VELKA_ESSAI_MPV_ZIP`, fabriquées par `tools/mac/*.sh`) : extraction, droits d'exécution gardés, Streamlink qui
    /// répond, libmpv qui se charge avec ses bibliothèques. Lancé par la CI Mac.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "demande les archives Mac (VELKA_ESSAI_SL_ZIP, VELKA_ESSAI_MPV_ZIP)"]
    fn installation_mac_depuis_les_archives() {
        let base = std::env::temp_dir().join(format!("velka-mac-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for (tool, var) in [
            (Tool::Streamlink, "VELKA_ESSAI_SL_ZIP"),
            (Tool::Libmpv, "VELKA_ESSAI_MPV_ZIP"),
        ] {
            let archive = PathBuf::from(std::env::var(var).unwrap());
            let (extracted, staging) = (
                base.join(format!("x-{tool:?}")),
                base.join(tool_subdir(tool)),
            );
            fs::create_dir_all(&extracted).unwrap();
            fs::create_dir_all(&staging).unwrap();
            build_staging(tool, &archive, &extracted, &staging).unwrap();
        }
        let sl = find_in_tools_dir(Tool::Streamlink, &base).expect("lanceur streamlink");
        assert_eq!(sl, base.join("streamlink").join("bin").join("streamlink"));
        let v = streamlink_version(&sl).expect("streamlink --version");
        println!("Streamlink installé : {v}");
        let mpv = find_in_tools_dir(Tool::Libmpv, &base).expect("libmpv");
        // SAFETY : essai de chargement de la libmpv qui vient d'être installée.
        let lib = unsafe { libloading::Library::new(&mpv) }
            .expect("libmpv se charge avec ses bibliothèques");
        // SAFETY : fonction de libmpv sans paramètre, qui renvoie un entier.
        let api = unsafe {
            lib.get::<unsafe extern "C" fn() -> u64>(b"mpv_client_api_version\0")
                .unwrap()()
        };
        println!("libmpv installée, API {}.{}", api >> 16, api & 0xffff);
        assert!(api >> 16 >= 2);
        let _ = fs::remove_dir_all(&base);
    }

    /// Mac, de bout en bout : version validée (`commun::outils`), téléchargement depuis
    /// Keeyzn/velka-composants-mac, empreinte, installation dans le dossier `tools`, puis état « à jour ». Réseau réel :
    /// lancé par la CI Mac seulement.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "réseau réel : installe les composants Mac publiés"]
    fn installation_reelle_mac() {
        installation_reelle();
    }

    /// Linux, de bout en bout : version validée (`commun::outils`), téléchargement depuis
    /// Keeyzn/velka-composants-linux, empreinte, installation dans le dossier `tools`, puis état « à jour ». Réseau réel.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "réseau réel : installe les composants Linux publiés"]
    fn installation_reelle_linux() {
        installation_reelle();
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn installation_reelle() {
        for tool in Tool::ALL {
            let p = install(tool, &|_, _| {}).unwrap_or_else(|e| panic!("{tool:?} : {e}"));
            println!("{tool:?} installé : {}", p.display());
            assert!(p.starts_with(tools_dir()));
            let latest = latest_tag(tool).unwrap();
            let r = report(tool, "", &latest);
            assert_eq!(r.state, UpdateState::UpToDate, "{r:?}");
        }
        let sl = detect(Tool::Streamlink, "").unwrap();
        println!(
            "Streamlink : {}",
            streamlink_version(&sl).expect("streamlink --version")
        );
        assert_eq!(component_info(Tool::Libmpv, "").build, Some("LGPL"));
    }

    /// Linux : installation à partir des vraies archives Linux (chemins donnés par `VELKA_ESSAI_SL_ZIP` et
    /// `VELKA_ESSAI_MPV_ZIP`, fabriquées par le dépôt `Keeyzn/velka-composants-linux`) : extraction, droits d'exécution
    /// gardés, Streamlink qui répond, libmpv qui se charge.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "demande les archives Linux (VELKA_ESSAI_SL_ZIP, VELKA_ESSAI_MPV_ZIP)"]
    fn installation_linux_depuis_les_archives() {
        let base = std::env::temp_dir().join(format!("velka-linux-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for (tool, var) in [
            (Tool::Streamlink, "VELKA_ESSAI_SL_ZIP"),
            (Tool::Libmpv, "VELKA_ESSAI_MPV_ZIP"),
        ] {
            let archive = PathBuf::from(std::env::var(var).unwrap());
            let (extracted, staging) = (
                base.join(format!("x-{tool:?}")),
                base.join(tool_subdir(tool)),
            );
            fs::create_dir_all(&extracted).unwrap();
            fs::create_dir_all(&staging).unwrap();
            build_staging(tool, &archive, &extracted, &staging).unwrap();
        }
        let sl = find_in_tools_dir(Tool::Streamlink, &base).expect("lanceur streamlink");
        assert_eq!(sl, base.join("streamlink").join("bin").join("streamlink"));
        let v = streamlink_version(&sl).expect("streamlink --version");
        println!("Streamlink installé : {v}");
        let mpv = find_in_tools_dir(Tool::Libmpv, &base).expect("libmpv");
        assert_eq!(mpv, base.join("libmpv").join("libmpv.so.2"));
        // SAFETY : essai de chargement de la libmpv qui vient d'être installée.
        let lib = unsafe { libloading::Library::new(&mpv) }.expect("libmpv se charge");
        // SAFETY : fonction de libmpv sans paramètre, qui renvoie un entier.
        let api = unsafe {
            lib.get::<unsafe extern "C" fn() -> u64>(b"mpv_client_api_version\0")
                .unwrap()()
        };
        println!("libmpv installée, API {}.{}", api >> 16, api & 0xffff);
        assert!(api >> 16 >= 2);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn chemin_personnalise_inexistant() {
        assert_ne!(
            detect(Tool::Libmpv, "Z:\\nexiste\\pas\\libmpv-2.dll"),
            Some(PathBuf::from("Z:\\nexiste\\pas\\libmpv-2.dll"))
        );
    }
}

#[cfg(test)]
mod reseau {
    use super::*;

    /// Test manuel : `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn releases_github() {
        for t in Tool::ALL {
            let p = validated(t).expect("manifeste");
            let a = fetch_release(t, &p).expect("release");
            println!("{t:?} {} -> {}", p.tag, a.browser_download_url);
        }
    }
}
