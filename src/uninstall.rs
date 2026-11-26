//! Désinstallation : supprime le dossier de données `%LOCALAPPDATA%\Velka` (outils, réglages,
//! historique, cache, journal) ET l'application elle-même (exécutable, raccourcis, entrée « Applications »
//! de Windows). Le tout est en partie verrouillé tant que l'application tourne (libmpv chargée) : la
//! suppression est donc confiée à un petit processus caché qui attend la fermeture de l'application.
use std::path::Path;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use std::path::PathBuf;

#[cfg(windows)]
/// Sécurité : on n'efface qu'un dossier nommé `Velka`, jamais une racine ni un dossier parent.
pub fn is_safe_target(dir: &Path) -> bool {
    dir.file_name().is_some_and(|n| n == "Velka") && dir.components().count() >= 3
}

#[cfg(windows)]
/// Fichiers de l'application à retirer en plus des données.
pub struct AppFiles {
    /// Exécutable en cours (`velka.exe`).
    pub exe: PathBuf,
    /// Désinstalleur de l'installateur (`unins000.exe`) s'il est à côté de l'exécutable.
    pub unins: Option<PathBuf>,
    /// Raccourci « Velka » du Bureau de l'utilisateur, s'il existe.
    pub desktop_lnk: Option<PathBuf>,
}

#[cfg(windows)]
fn quoted(p: &Path) -> Option<String> {
    let s = p.display().to_string();
    (!s.contains('"')).then(|| format!("\"{s}\""))
}

#[cfg(windows)]
/// Ligne de commande `cmd` qui attend, puis supprime l'application et le dossier de données (deux essais).
pub fn cleanup_command_line(dir: &Path, app: Option<&AppFiles>) -> Option<String> {
    if !is_safe_target(dir) {
        return None;
    }
    let data = quoted(dir)?;
    let mut line = String::from("/C ping -n 6 127.0.0.1 >nul");
    // Retraits répétés après une pause : un fichier encore verrouillé au premier passage part au second.
    let mut again: Vec<String> = Vec::new();
    if let Some(app) = app {
        let exe = quoted(&app.exe)?;
        let old = quoted(&app.exe.with_extension("exe.old"))?;
        if let Some(unins) = &app.unins {
            // Installé : le désinstalleur retire l'exécutable, les raccourcis (Bureau, menu Démarrer) et l'entrée Windows.
            line.push_str(&format!(
                " & {} /VERYSILENT /NORESTART /SUPPRESSMSGBOXES & ping -n 9 127.0.0.1 >nul",
                quoted(unins)?
            ));
        }
        again.push(format!("del /f /q {exe} {old}"));
        if let Some(lnk) = &app.desktop_lnk {
            again.push(format!("del /f /q {}", quoted(lnk)?));
        }
        // Dossier d'installation : seulement s'il porte le nom prévu par l'installateur, jamais un dossier quelconque
        // (version portable posée sur le Bureau ou ailleurs).
        if app.unins.is_some() {
            if let Some(install) = app
                .exe
                .parent()
                .filter(|d| d.file_name().is_some_and(|n| n == "Velka"))
            {
                again.push(format!("rmdir /s /q {}", quoted(install)?));
            }
        }
    }
    again.push(format!("rmdir /s /q {data}"));
    let pass = again.join(" & ");
    line.push_str(&format!(" & {pass} & ping -n 4 127.0.0.1 >nul & {pass}"));
    Some(line)
}

/// Lance la suppression différée. Renvoie `false` si le dossier n'est pas jugé sûr.
#[cfg(windows)]
pub fn spawn_cleanup(dir: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    // Les versions de développement et l'essai de démarrage ne touchent jamais à leur exécutable.
    let app = std::env::current_exe()
        .ok()
        .filter(|_| !cfg!(debug_assertions) && !crate::smoke::enabled())
        .map(|exe| {
            let unins = exe
                .parent()
                .map(|d| d.join("unins000.exe"))
                .filter(|u| u.is_file());
            let desktop_lnk = unins
                .as_ref()
                .and_then(|_| directories::UserDirs::new())
                .and_then(|u| u.desktop_dir().map(|d| d.join("Velka.lnk")))
                .filter(|l| l.is_file());
            AppFiles {
                exe,
                unins,
                desktop_lnk,
            }
        });
    let Some(line) = cleanup_command_line(dir, app.as_ref()) else {
        return false;
    };
    std::process::Command::new(crate::procs::system_exe("cmd.exe"))
        .raw_arg(line)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .is_ok()
}

/// Mac : mêmes garde-fous que sous Windows. On n'efface qu'un dossier nommé `Velka` et qu'un paquet nommé
/// `Velka.app` (jamais le dossier qui le contient, jamais l'exécutable d'une version de développement).
#[cfg(target_os = "macos")]
fn is_safe_target_mac(dir: &Path) -> bool {
    dir.file_name().is_some_and(|n| n == "Velka") && dir.components().count() >= 4
}

/// `.../Velka.app/Contents/MacOS/velka` donne `.../Velka.app`.
#[cfg(target_os = "macos")]
pub(crate) fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    let in_bundle =
        exe.parent()?.file_name()? == "MacOS" && exe.parent()?.parent()?.file_name()? == "Contents";
    (in_bundle && bundle.file_name()? == "Velka.app").then(|| bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
pub(crate) fn sh_quoted(p: &Path) -> Option<String> {
    let s = p.display().to_string();
    (!s.contains('\'') && !s.contains('\n')).then(|| format!("'{s}'"))
}

/// Script `sh` qui attend la fin du processus `pid`, puis supprime le dossier de données, le paquet (s'il y en a un)
/// et l'élément « Velka » du Trousseau (la clé des secrets ; sans les données elle ne sert plus). Deux passages.
#[cfg(target_os = "macos")]
fn cleanup_script(dir: &Path, bundle: Option<&Path>, pid: u32) -> Option<String> {
    if !is_safe_target_mac(dir) {
        return None;
    }
    let mut targets = vec![sh_quoted(dir)?];
    if let Some(b) = bundle {
        targets.push(sh_quoted(b)?);
    }
    let rm = format!("/bin/rm -rf {}", targets.join(" "));
    let keychain =
        "/usr/bin/security delete-generic-password -s Velka -a secrets-v1 >/dev/null 2>&1";
    Some(format!("while /bin/kill -0 {pid} 2>/dev/null; do /bin/sleep 0.3; done; {rm}; {keychain}; /bin/sleep 2; {rm}"))
}

#[cfg(target_os = "macos")]
pub fn spawn_cleanup(dir: &Path) -> bool {
    // Les versions de développement et l'essai de démarrage ne touchent jamais à leur exécutable.
    let bundle = std::env::current_exe()
        .ok()
        .filter(|_| !cfg!(debug_assertions) && !crate::smoke::enabled())
        .and_then(|e| bundle_of(&e));
    let Some(script) = cleanup_script(dir, bundle.as_deref(), std::process::id()) else {
        return false;
    };
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .spawn()
        .is_ok()
}

/// Linux : mêmes garde-fous. On n'efface qu'un dossier nommé `Velka` (données `~/.local/share/Velka`, clé
/// des secrets `~/.config/Velka`), les lanceurs `velka.desktop` (démarrage et menu) et l'icône `velka.png`, et le
/// fichier AppImage seulement s'il s'appelle `Velka….AppImage` (jamais l'exécutable d'une version de développement).
#[cfg(target_os = "linux")]
fn is_safe_target_linux(dir: &Path) -> bool {
    dir.file_name().is_some_and(|n| n == "Velka") && dir.components().count() >= 4
}

/// Fichier AppImage de l'application (`APPIMAGE`), s'il porte bien le nom de Velka.
#[cfg(target_os = "linux")]
pub(crate) fn appimage_of(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    (path.is_absolute() && name.starts_with("Velka") && name.ends_with(".AppImage"))
        .then(|| path.to_path_buf())
}

#[cfg(target_os = "linux")]
pub(crate) fn sh_quoted_linux(p: &Path) -> Option<String> {
    let s = p.to_str()?;
    (!s.contains('\'') && !s.contains('\n')).then(|| format!("'{s}'"))
}

/// Script `sh` qui attend la fin du processus `pid`, puis supprime les dossiers et fichiers de `targets`, et la clé
/// des secrets rangée dans le trousseau du bureau (`secret-tool`, sans effet s'il n'y en a pas). Deux passages.
#[cfg(target_os = "linux")]
fn cleanup_script_linux(
    dir: &Path,
    extra_dirs: &[PathBuf],
    files: &[PathBuf],
    pid: u32,
) -> Option<String> {
    if !is_safe_target_linux(dir) {
        return None;
    }
    let mut targets = vec![sh_quoted_linux(dir)?];
    for d in extra_dirs.iter().filter(|d| is_safe_target_linux(d)) {
        targets.push(sh_quoted_linux(d)?);
    }
    for f in files {
        targets.push(sh_quoted_linux(f)?);
    }
    let rm = format!("rm -rf {}", targets.join(" "));
    let keyring = "secret-tool clear service Velka account secrets-v1 >/dev/null 2>&1";
    Some(format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.3; done; {rm}; {keyring}; sleep 2; {rm}"
    ))
}

#[cfg(target_os = "linux")]
pub fn spawn_cleanup(dir: &Path) -> bool {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".config")));
    let extra_dirs: Vec<PathBuf> = config.map(|c| c.join("Velka")).into_iter().collect();
    let mut files: Vec<PathBuf> = crate::procs::autostart_file().into_iter().collect();
    // Lanceur du menu des applications et son icône (créés au lancement depuis l'AppImage).
    if let Some((desktop, icon)) = crate::procs::menu_files() {
        files.extend([desktop, icon]);
    }
    // Les versions de développement et l'essai de démarrage ne touchent jamais à leur exécutable.
    if !cfg!(debug_assertions) && !crate::smoke::enabled() {
        files.extend(std::env::var_os("APPIMAGE").and_then(|p| appimage_of(Path::new(&p))));
    }
    let Some(script) = cleanup_script_linux(dir, &extra_dirs, &files, std::process::id()) else {
        return false;
    };
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .spawn()
        .is_ok()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn spawn_cleanup(_dir: &Path) -> bool {
    false
}

#[cfg(all(test, target_os = "linux"))]
mod tests_linux {
    use super::*;

    #[test]
    fn dossiers_surs_uniquement_linux() {
        assert!(is_safe_target_linux(&PathBuf::from(
            "/home/moi/.local/share/Velka"
        )));
        assert!(is_safe_target_linux(&PathBuf::from(
            "/home/moi/.config/Velka"
        )));
        assert!(!is_safe_target_linux(&PathBuf::from(
            "/home/moi/.local/share"
        )));
        assert!(!is_safe_target_linux(&PathBuf::from("/Velka")));
        assert!(!is_safe_target_linux(&PathBuf::from("/home/Velka")));
    }

    #[test]
    fn appimage_reconnue_seulement_sous_son_nom() {
        assert!(appimage_of(Path::new("/home/moi/Applis/Velka-x86_64.AppImage")).is_some());
        assert!(appimage_of(Path::new("/home/moi/Applis/Autre.AppImage")).is_none());
        assert!(appimage_of(Path::new("/home/moi/velka-target/release/velka")).is_none());
        assert!(appimage_of(Path::new("Velka.AppImage")).is_none());
    }

    #[test]
    fn script_de_nettoyage_linux() {
        let data = PathBuf::from("/home/moi/.local/share/Velka");
        let extra = [
            PathBuf::from("/home/moi/.config/Velka"),
            PathBuf::from("/home/moi/.config"),
        ];
        let files = [
            PathBuf::from("/home/moi/.config/autostart/velka.desktop"),
            PathBuf::from("/home/moi/Velka.AppImage"),
        ];
        let s = cleanup_script_linux(&data, &extra, &files, 42).unwrap();
        assert!(s.starts_with("while kill -0 42"));
        let rm = "rm -rf '/home/moi/.local/share/Velka' '/home/moi/.config/Velka' '/home/moi/.config/autostart/velka.desktop' '/home/moi/Velka.AppImage'";
        assert_eq!(s.matches(rm).count(), 2, "{s}");
        assert!(
            !s.contains("'/home/moi/.config' "),
            "dossier parent jamais supprimé : {s}"
        );
        assert!(s.contains("secret-tool clear service Velka account secrets-v1"));
        assert!(
            cleanup_script_linux(&PathBuf::from("/home/moi/Documents"), &[], &[], 42).is_none()
        );
        assert!(cleanup_script_linux(
            &PathBuf::from("/home/m'oi/.local/share/Velka"),
            &[],
            &[],
            42
        )
        .is_none());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests_mac {
    use super::*;

    #[test]
    fn dossiers_surs_uniquement_mac() {
        assert!(is_safe_target_mac(&PathBuf::from(
            "/Volumes/Donnees/moi/Library/Application Support/Velka"
        )));
        assert!(!is_safe_target_mac(&PathBuf::from(
            "/Volumes/Donnees/moi/Library/Application Support"
        )));
        assert!(!is_safe_target_mac(&PathBuf::from("/Velka")));
        assert!(!is_safe_target_mac(&PathBuf::from(
            "/Volumes/Donnees/moi/Documents"
        )));
    }

    #[test]
    fn paquet_reconnu_seulement_sous_son_nom() {
        assert_eq!(
            bundle_of(&PathBuf::from(
                "/Applications/Velka.app/Contents/MacOS/velka"
            )),
            Some(PathBuf::from("/Applications/Velka.app"))
        );
        assert!(bundle_of(&PathBuf::from(
            "/Volumes/Donnees/moi/velka-target/release/velka"
        ))
        .is_none());
        assert!(bundle_of(&PathBuf::from(
            "/Applications/Autre.app/Contents/MacOS/velka"
        ))
        .is_none());
    }

    #[test]
    fn script_de_nettoyage() {
        let data = PathBuf::from("/Volumes/Donnees/moi/Library/Application Support/Velka");
        let app = PathBuf::from("/Applications/Velka.app");
        let s = cleanup_script(&data, Some(&app), 42).unwrap();
        assert!(s.starts_with("while /bin/kill -0 42"));
        assert_eq!(s.matches("/bin/rm -rf '/Volumes/Donnees/moi/Library/Application Support/Velka' '/Applications/Velka.app'").count(), 2);
        assert!(s.contains("delete-generic-password -s Velka -a secrets-v1"));
        let sans = cleanup_script(&data, None, 42).unwrap();
        assert!(!sans.contains("/Applications"));
        assert!(
            cleanup_script(&PathBuf::from("/Volumes/Donnees/moi/Documents"), None, 42).is_none()
        );
        assert!(cleanup_script(
            &PathBuf::from("/Volumes/Donnees/m'oi/Library/Application Support/Velka"),
            None,
            42
        )
        .is_none());
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn dossiers_surs_uniquement() {
        assert!(is_safe_target(&PathBuf::from(
            r"D:\Profils\moi\AppData\Local\Velka"
        )));
        assert!(!is_safe_target(&PathBuf::from(
            r"D:\Profils\moi\AppData\Local"
        )));
        assert!(!is_safe_target(&PathBuf::from(r"C:\")));
        assert!(!is_safe_target(&PathBuf::from("Velka")));
        assert!(!is_safe_target(&PathBuf::from(r"D:\Profils\moi\Documents")));
    }

    #[test]
    fn ligne_de_commande_donnees() {
        let l = cleanup_command_line(&PathBuf::from(r"D:\Profils\moi\AppData\Local\Velka"), None)
            .unwrap();
        assert!(l.starts_with("/C ping"));
        assert_eq!(l.matches("rmdir /s /q").count(), 2);
        assert!(cleanup_command_line(&PathBuf::from(r"C:\Windows"), None).is_none());
    }

    #[test]
    fn ligne_de_commande_installee() {
        let app = AppFiles {
            exe: PathBuf::from(r"D:\Profils\moi\AppData\Local\Programs\Velka\velka.exe"),
            unins: Some(PathBuf::from(
                r"D:\Profils\moi\AppData\Local\Programs\Velka\unins000.exe",
            )),
            desktop_lnk: Some(PathBuf::from(r"D:\Profils\moi\Desktop\Velka.lnk")),
        };
        let l = cleanup_command_line(
            &PathBuf::from(r"D:\Profils\moi\AppData\Local\Velka"),
            Some(&app),
        )
        .unwrap();
        assert_eq!(l.matches("unins000.exe\" /VERYSILENT").count(), 1);
        assert!(l.contains(r#"del /f /q "D:\Profils\moi\Desktop\Velka.lnk""#));
        assert_eq!(
            l.matches(r#"rmdir /s /q "D:\Profils\moi\AppData\Local\Programs\Velka""#)
                .count(),
            2
        );
        assert_eq!(
            l.matches(r#"rmdir /s /q "D:\Profils\moi\AppData\Local\Velka""#)
                .count(),
            2
        );
    }

    #[test]
    fn version_portable_ne_supprime_que_l_executable() {
        let app = AppFiles {
            exe: PathBuf::from(r"D:\Profils\moi\Desktop\velka.exe"),
            unins: None,
            desktop_lnk: None,
        };
        let l = cleanup_command_line(
            &PathBuf::from(r"D:\Profils\moi\AppData\Local\Velka"),
            Some(&app),
        )
        .unwrap();
        assert!(l.contains(
            r#"del /f /q "D:\Profils\moi\Desktop\velka.exe" "D:\Profils\moi\Desktop\velka.exe.old""#
        ));
        assert!(!l.contains(r#"rmdir /s /q "D:\Profils\moi\Desktop"#));
        assert!(!l.contains("unins000"));
    }
}
