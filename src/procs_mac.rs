//! Équivalents Mac des fonctions système de `procs.rs`. Aucune dépendance en plus de `libc` :
//! les boîtes de dialogue et notifications passent par `osascript`, le presse-papiers par `pbpaste`, le démarrage à
//! l'ouverture de session par un LaunchAgent. Les textes sont passés à `osascript` en arguments (`argv`), jamais
//! collés dans le script : aucun échappement à faire, aucune injection possible.
use std::path::{Path, PathBuf};
use std::process::Command;

const OSASCRIPT: &str = "/usr/bin/osascript";

/// Lance un petit script AppleScript `on run argv … end run` avec `args`. Renvoie la sortie si le script réussit.
fn osascript(lines: &[&str], args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(OSASCRIPT);
    cmd.args(["-e", "on run argv"]);
    for l in lines {
        cmd.args(["-e", l]);
    }
    cmd.args(["-e", "end run"]).args(args);
    let out = cmd.output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .trim_end_matches('\n')
            .to_string()
    })
}

// Instance unique, attente de fin de processus et heure locale : `procs_unix.rs` (communs au Mac et à Linux).

/// Première langue préférée de macOS (« fr-FR », « zh-Hant-TW »…), lue dans `AppleLanguages`.
pub fn user_locale() -> Option<String> {
    let out = Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    first_language(&String::from_utf8_lossy(&out.stdout))
}

/// Premier élément de la liste écrite par `defaults` : `(\n    "fr-FR",\n    en\n)`.
pub(crate) fn first_language(plist: &str) -> Option<String> {
    plist
        .lines()
        .map(|l| {
            l.trim()
                .trim_matches(|c| c == '(' || c == ')' || c == ',' || c == '"')
                .trim()
        })
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// Boîte d'alerte bloquante (fenêtre impossible à créer, avertissement OpenGL).
pub fn message_box(text: &str, warning: bool) {
    let style = if warning { "warning" } else { "critical" };
    let line = format!("display alert \"Velka\" message (item 1 of argv) as {style}");
    let _ = osascript(&[&line], &[text]);
}

/// Notification du Centre de notifications (live qui commence, notification de test). Le clic n'ouvre pas encore la
/// chaîne sur Mac : `login` est ignoré.
pub fn notify(title: &str, text: &str, _login: &str) {
    let (title, text) = (title.to_string(), text.to_string());
    // `osascript` prend environ 100 ms : jamais sur le fil d'interface.
    std::thread::spawn(move || {
        let _ = osascript(
            &["display notification (item 2 of argv) with title (item 1 of argv)"],
            &[&title, &text],
        );
    });
}

/// Fenêtre de choix d'un dossier (bloquante, à appeler depuis un fil dédié). `None` si l'utilisateur annule.
pub fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    let start = start
        .filter(|d| d.is_dir())
        .map(|d| d.to_string_lossy().into_owned());
    let out = match &start {
        Some(dir) => osascript(
            &["POSIX path of (choose folder default location (POSIX file (item 1 of argv)))"],
            &[dir],
        ),
        None => osascript(&["POSIX path of (choose folder)"], &[]),
    }?;
    let path = out.trim();
    (!path.is_empty()).then(|| PathBuf::from(path.trim_end_matches('/')))
}

/// Ouvre un dossier dans le Finder.
pub fn open_folder(dir: &Path) {
    let _ = Command::new("/usr/bin/open").arg(dir).spawn();
}

/// LaunchAgent qui lance Velka à l'ouverture de session (équivalent de la clé « Run » de Windows).
fn launch_agent() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| {
        d.home_dir()
            .join("Library/LaunchAgents/app.velka.login.plist")
    })
}

pub fn startup_enabled() -> bool {
    launch_agent().is_some_and(|p| p.exists())
}

/// Active ou retire le démarrage à l'ouverture de session. Pris en compte à la prochaine ouverture de session.
pub fn set_startup(on: bool) -> bool {
    let Some(plist) = launch_agent() else {
        return false;
    };
    if !on {
        return match std::fs::remove_file(&plist) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        };
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(parent) = plist.parent() else {
        return false;
    };
    std::fs::create_dir_all(parent).is_ok()
        && std::fs::write(&plist, launch_agent_plist(&exe)).is_ok()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn launch_agent_plist(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
\t<key>Label</key>\n\t<string>app.velka.login</string>\n\
\t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t</array>\n\
\t<key>RunAtLoad</key>\n\t<true/>\n\
</dict>\n</plist>\n",
        xml_escape(&exe.to_string_lossy())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premiere_langue_de_macos() {
        assert_eq!(
            first_language("(\n    \"fr-FR\",\n    en\n)\n").as_deref(),
            Some("fr-FR")
        );
        assert_eq!(
            first_language("(\n    en,\n    \"zh-Hant-TW\"\n)").as_deref(),
            Some("en")
        );
        assert_eq!(first_language("(\n)"), None);
        assert_eq!(
            crate::i18n::index_for_locale("zh-Hant-TW"),
            crate::i18n::index_for_locale("zh-TW")
        );
    }

    #[test]
    fn launch_agent_lisible() {
        let p = launch_agent_plist(Path::new(
            "/Applications/Velka & co.app/Contents/MacOS/velka",
        ));
        assert!(
            p.contains("<string>/Applications/Velka &amp; co.app/Contents/MacOS/velka</string>"),
            "{p}"
        );
        assert!(p.contains("<key>RunAtLoad</key>"));
    }
}
