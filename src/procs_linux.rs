//! Équivalents Linux des fonctions système de `procs.rs`. Aucune dépendance en plus : les
//! outils standard du bureau Linux sont appelés s'ils sont présents (`xdg-open`, `notify-send`, `zenity` ou `kdialog`,
//! `wl-paste`, `xclip` ou `xsel`) ; s'il en manque un, la fonction ne fait rien (comme une fonction absente). Les textes
//! sont passés en arguments, jamais dans une commande `sh` : aucun échappement, aucune injection possible.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Lance `program` avec `args` et rend sa sortie si le programme existe et réussit.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Le programme est-il dans un dossier du `PATH` ?
fn installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

/// Langue de la session (« fr-FR », « pt-BR »…), selon les variables lues par les programmes Linux.
pub fn user_locale() -> Option<String> {
    locale_from_env(|k| std::env::var(k).ok())
}

/// Ordre de gettext : `LC_ALL`, `LC_MESSAGES`, puis `LANG` ; `LANGUAGE` (liste « fr:en ») passe devant si la langue
/// n'est pas « C » / « POSIX ». « fr_FR.UTF-8 » donne « fr-FR ».
pub(crate) fn locale_from_env(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    let base = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| get(k).filter(|v| !v.is_empty()))?;
    let base = normalize_locale(&base)?;
    let first = get("LANGUAGE").and_then(|l| l.split(':').find_map(normalize_locale));
    Some(first.unwrap_or(base))
}

fn normalize_locale(v: &str) -> Option<String> {
    let v = v.split(['.', '@']).next()?.trim();
    (!v.is_empty() && v != "C" && v != "POSIX").then(|| v.replace('_', "-"))
}

/// Boîte d'alerte bloquante (fenêtre impossible à créer, avertissement OpenGL) : `zenity`, sinon `kdialog`, sinon le
/// terminal.
pub fn message_box(text: &str, warning: bool) {
    let shown = if installed("zenity") {
        let kind = if warning { "--warning" } else { "--error" };
        output(
            "zenity",
            &[kind, "--no-markup", "--title=Velka", "--text", text],
        )
        .is_some()
    } else if installed("kdialog") {
        let kind = if warning { "--sorry" } else { "--error" };
        output("kdialog", &["--title", "Velka", kind, text]).is_some()
    } else {
        false
    };
    if !shown {
        eprintln!("Velka : {text}");
    }
}

/// Notification du bureau (live qui commence, notification de test). Le clic n'ouvre pas encore la chaîne sous Linux :
/// `login` est ignoré.
pub fn notify(title: &str, text: &str, _login: &str) {
    let (title, text) = (title.to_string(), text.to_string());
    std::thread::spawn(move || {
        let _ = output("notify-send", &["--app-name=Velka", "--", &title, &text]);
    });
}

/// Fenêtre de choix d'un dossier (bloquante, à appeler depuis un fil dédié). `None` si l'utilisateur annule ou si ni
/// `zenity` ni `kdialog` n'est installé.
pub fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    let start = start
        .filter(|d| d.is_dir())
        .map(|d| format!("{}/", d.to_string_lossy().trim_end_matches('/')));
    let out = if installed("zenity") {
        let mut args = vec!["--file-selection", "--directory"];
        let filename;
        if let Some(dir) = &start {
            filename = format!("--filename={dir}");
            args.push(&filename);
        }
        output("zenity", &args)
    } else if installed("kdialog") {
        output(
            "kdialog",
            &["--getexistingdirectory", start.as_deref().unwrap_or("")],
        )
    } else {
        None
    }?;
    let path = out.trim_end_matches('\n');
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Ouvre un dossier dans le gestionnaire de fichiers.
pub fn open_folder(dir: &Path) {
    let _ = Command::new("xdg-open")
        .arg(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Lanceur de démarrage automatique (`~/.config/autostart/velka.desktop`, norme XDG, reconnue par GNOME, KDE, Xfce…).
pub fn autostart_file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".config")))?;
    Some(config.join("autostart").join("velka.desktop"))
}

pub fn startup_enabled() -> bool {
    autostart_file().is_some_and(|p| p.exists())
}

/// Programme à relancer : le fichier AppImage lui-même s'il y en a un (`APPIMAGE`, donné par l'AppImage), sinon
/// l'exécutable en cours (dans une AppImage, il est dans un dossier temporaire qui disparaît à la fermeture).
pub fn launcher_exe() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
}

/// Active ou retire le démarrage à l'ouverture de session. Pris en compte à la prochaine ouverture de session.
pub fn set_startup(on: bool) -> bool {
    let Some(file) = autostart_file() else {
        return false;
    };
    if !on {
        return match std::fs::remove_file(&file) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        };
    }
    let Some(entry) = launcher_exe().and_then(|exe| desktop_entry(&exe)) else {
        return false;
    };
    let Some(parent) = file.parent() else {
        return false;
    };
    std::fs::create_dir_all(parent).is_ok() && std::fs::write(&file, entry).is_ok()
}

/// Chemin entre guillemets pour la clé `Exec` d'un fichier `.desktop` : `"`, `` ` ``, `$` et `\` précédés d'une barre
/// oblique inverse (règle des arguments), elle-même doublée (règle des chaînes), et `%` doublé. `None` pour un chemin
/// sur plusieurs lignes.
fn exec_quoted(exe: &Path) -> Option<String> {
    let s = exe.to_string_lossy();
    if s.contains(['\n', '\r']) {
        return None;
    }
    let mut q = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                q.push_str("\\\\");
                q.push(c);
            }
            '%' => q.push_str("%%"),
            c => q.push(c),
        }
    }
    q.push('"');
    Some(q)
}

pub(crate) fn desktop_entry(exe: &Path) -> Option<String> {
    Some(format!(
        "[Desktop Entry]\nType=Application\nName=Velka\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_quoted(exe)?
    ))
}

/// Dossier des données partagées de l'utilisateur (`$XDG_DATA_HOME`, sinon `~/.local/share`).
fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".local").join("share")))
}

/// Lanceur du menu des applications (`~/.local/share/applications/velka.desktop`) et son icône
/// (`~/.local/share/icons/hicolor/256x256/apps/velka.png`). `velka` = identifiant de fenêtre.
pub fn menu_files() -> Option<(PathBuf, PathBuf)> {
    let share = data_home()?;
    Some((
        share.join("applications").join("velka.desktop"),
        share
            .join("icons")
            .join("hicolor")
            .join("256x256")
            .join("apps")
            .join("velka.png"),
    ))
}

pub(crate) fn menu_entry(exe: &Path) -> Option<String> {
    Some(format!(
        "[Desktop Entry]\nType=Application\nName=Velka\nExec={}\nIcon=velka\nTerminal=false\nCategories=AudioVideo;Video;Player;\nStartupWMClass=velka\n",
        exec_quoted(exe)?
    ))
}

/// Écrit `content` dans `path` seulement s'il est différent (dossiers créés au besoin). `Ok(true)` si écrit.
fn write_if_changed(path: &Path, content: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == content) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, content).map(|()| true)
}

/// Lancée depuis une AppImage (`APPIMAGE`) : lanceur dans le menu des applications et icône, mis à jour si l'AppImage
/// a été déplacée ou remplacée. Équivalent du raccourci du menu Démarrer de Windows. Hors AppImage (version de
/// développement) : rien.
pub fn ensure_start_menu_shortcut() {
    // Instance isolée (essais) : l'entrée du menu de l'installation habituelle n'est jamais touchée.
    if crate::logging::isolated_dir().is_some() {
        return;
    }
    let Some(appimage) = std::env::var_os("APPIMAGE")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
    else {
        return;
    };
    let (Some((desktop, icon)), Some(entry)) = (menu_files(), menu_entry(&appimage)) else {
        return;
    };
    let result = write_if_changed(&icon, crate::ui::logo::ICON_PNG)
        .and_then(|i| Ok(write_if_changed(&desktop, entry.as_bytes())? || i));
    match result {
        Ok(true) => crate::logging::log("Lanceur du menu des applications mis à jour"),
        Ok(false) => {}
        Err(e) => crate::logging::log(&format!("Lanceur du menu des applications : {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langue_de_la_session() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            locale_from_env(env(&[("LANG", "fr_FR.UTF-8")])).as_deref(),
            Some("fr-FR")
        );
        assert_eq!(
            locale_from_env(env(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "de_DE.UTF-8")])).as_deref(),
            Some("de-DE")
        );
        assert_eq!(
            locale_from_env(env(&[("LANG", "fr_FR.UTF-8"), ("LANGUAGE", "pt_BR:en")])).as_deref(),
            Some("pt-BR")
        );
        assert_eq!(
            locale_from_env(env(&[("LANG", "C.UTF-8"), ("LANGUAGE", "pt_BR")])),
            None
        );
        assert_eq!(
            locale_from_env(env(&[("LANG", "sr_RS@latin")])).as_deref(),
            Some("sr-RS")
        );
        assert_eq!(locale_from_env(env(&[])), None);
        assert_eq!(
            crate::i18n::index_for_locale("pt-BR"),
            crate::i18n::index_for_locale("pt")
        );
    }

    #[test]
    fn lanceur_de_demarrage_lisible() {
        let e = desktop_entry(Path::new("/home/moi/Applis/Velka 100% \"$x\".AppImage")).unwrap();
        assert!(
            e.contains("Exec=\"/home/moi/Applis/Velka 100%% \\\\\"\\\\$x\\\\\".AppImage\"\n"),
            "{e}"
        );
        assert!(e.starts_with("[Desktop Entry]\n") && e.contains("\nName=Velka\n"));
        assert_eq!(desktop_entry(Path::new("/a\nb")), None);
    }

    #[test]
    fn lanceur_du_menu() {
        let e = menu_entry(Path::new("/home/moi/Applis/Velka-x86_64.AppImage")).unwrap();
        assert!(
            e.contains("\nExec=\"/home/moi/Applis/Velka-x86_64.AppImage\"\n"),
            "{e}"
        );
        assert!(
            e.contains("\nIcon=velka\n") && e.contains("\nStartupWMClass=velka\n"),
            "{e}"
        );
        let dir = std::env::temp_dir().join(format!("velka-menu-{}", std::process::id()));
        let f = dir.join("a").join("velka.desktop");
        assert!(write_if_changed(&f, b"x").unwrap());
        assert!(
            !write_if_changed(&f, b"x").unwrap(),
            "contenu identique : rien n'est réécrit"
        );
        assert!(write_if_changed(&f, b"y").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
