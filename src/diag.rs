//! Diagnostic pour le support : version de l'application, Windows, carte graphique et versions des
//! composants (Streamlink, libmpv). Écrit dans `log.txt` à chaque démarrage et copié par « Copier le diagnostic »
//! (Réglages > Avancé et Système > Journaux et diagnostic). Aucune donnée personnelle ni chemin ni jeton : le texte
//! passe par `crash::clean`. Rien n'est envoyé automatiquement.
use crate::tools::{self, Tool};
use std::sync::OnceLock;

/// Carte graphique, lue une fois à la création de la fenêtre (le contexte OpenGL n'existe qu'à ce moment).
static GPU: OnceLock<String> = OnceLock::new();

/// Mémorise la carte graphique à partir des chaînes OpenGL du contexte actif.
pub fn set_gpu(gl: &eframe::glow::Context) {
    use eframe::glow::{self, HasContext};
    // SAFETY : le contexte OpenGL de la fenêtre est actif pendant la création de l'application ; ces trois lectures de
    // chaînes n'ont aucun effet de bord.
    let (renderer, vendor, version) = unsafe {
        (
            gl.get_parameter_string(glow::RENDERER),
            gl.get_parameter_string(glow::VENDOR),
            gl.get_parameter_string(glow::VERSION),
        )
    };
    let _ = GPU.set(format_gpu(&renderer, &vendor, &version));
    #[cfg(debug_assertions)]
    let version = if opengl_test() == Some("ancien") {
        "1.1.0".to_string()
    } else {
        version
    };
    if gl_below_3(&version) {
        GL_TOO_OLD.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// OpenGL annoncé avant 3.0 (pilote générique de Windows, bureau à distance, machine virtuelle, vieux pilote) : la
/// vidéo (rendu de libmpv) risque de rester noire.
static GL_TOO_OLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn gl_too_old() -> bool {
    GL_TOO_OLD.load(std::sync::atomic::Ordering::Relaxed)
}

/// Version d'OpenGL lue dans la chaîne du pilote (« 4.6.0 NVIDIA 560.94 », « OpenGL ES 3.0 … », « 1.1.0 ») : vrai si
/// elle est connue et antérieure à 3.0. Une chaîne illisible n'est pas jugée trop ancienne (pas de faux avertissement).
pub fn gl_below_3(version: &str) -> bool {
    let v = version.trim();
    let v = v.strip_prefix("OpenGL ES ").unwrap_or(v);
    let mut parts = v.split(|c: char| !c.is_ascii_digit());
    match parts.next().and_then(|m| m.parse::<u32>().ok()) {
        Some(major) => major < 3,
        None => false,
    }
}

/// Essai manuel en debug seulement : variable d'environnement `VELKA_ESSAI_OPENGL` = `absent` (fenêtre
/// impossible) ou `ancien` (OpenGL 1.1 annoncé). Sans effet dans l'exe publié.
#[cfg(debug_assertions)]
pub fn opengl_test() -> Option<&'static str> {
    match std::env::var("VELKA_ESSAI_OPENGL").ok()?.as_str() {
        "absent" => Some("absent"),
        "ancien" => Some("ancien"),
        _ => None,
    }
}

fn format_gpu(renderer: &str, vendor: &str, version: &str) -> String {
    let (renderer, vendor, version) = (renderer.trim(), vendor.trim(), version.trim());
    if renderer.is_empty() {
        return "inconnue".to_string();
    }
    let mut s = renderer.to_string();
    if !vendor.is_empty() {
        s = format!("{s} ({vendor})");
    }
    if !version.is_empty() {
        s = format!("{s}, OpenGL {version}");
    }
    s
}

/// Valeur texte de `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
#[cfg(windows)]
fn reg_string(value: windows::core::PCWSTR) -> Option<String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = [0u16; 128];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY : `buf` et `bytes` sont valides pour la durée de l'appel ; `bytes` donne la taille réelle du tampon.
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if r.0 != 0 {
        return None;
    }
    let n = (bytes as usize / 2).min(buf.len());
    let s = String::from_utf16_lossy(&buf[..n]);
    let s = s.trim_end_matches('\0').trim().to_string();
    (!s.is_empty()).then_some(s)
}

#[cfg(windows)]
fn windows_text() -> String {
    use windows::core::w;
    let product = reg_string(w!("ProductName")).unwrap_or_default();
    let display = reg_string(w!("DisplayVersion")).unwrap_or_default();
    let build = reg_string(w!("CurrentBuildNumber")).unwrap_or_default();
    format_windows(&product, &display, &build)
}

/// Ligne « système » du diagnostic : « Windows : Windows 11 Pro, 25H2, build 26300 ».
#[cfg(windows)]
fn os_line() -> String {
    format!("Windows : {}", windows_text())
}

/// Ligne « système » du diagnostic sur Mac : « macOS : 15.1 » (`sw_vers`, sans chemin ni nom).
#[cfg(target_os = "macos")]
fn os_line() -> String {
    let version = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "version inconnue".to_string());
    format!("macOS : {version}")
}

/// Ligne « système » du diagnostic sous Linux : « Linux : Ubuntu 24.04.5 LTS (wayland) » (nom de la distribution
/// dans `/etc/os-release`, type de session graphique ; sans chemin ni nom).
#[cfg(target_os = "linux")]
fn os_line() -> String {
    let release = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    format!("Linux : {}", format_linux(&release, &session))
}

/// « Ubuntu 24.04.5 LTS (wayland) » à partir du contenu de `os-release` (`PRETTY_NAME`, sinon `NAME`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn format_linux(os_release: &str, session: &str) -> String {
    let value = |key: &str| {
        os_release
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let name = value("PRETTY_NAME")
        .or_else(|| value("NAME"))
        .unwrap_or_else(|| "distribution inconnue".to_string());
    let session = session.trim();
    if session.is_empty()
        || session.len() > 16
        || !session.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        name
    } else {
        format!("{name} ({session})")
    }
}

/// Ni Windows, ni Mac, ni Linux.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn os_line() -> String {
    "Système : inconnu".to_string()
}

/// « Windows 11 Pro, 25H2, build 26300 ». Le registre dit encore « Windows 10 » sur Windows 11 : le numéro de build
/// (22000 et plus) tranche.
#[cfg_attr(not(windows), allow(dead_code))]
fn format_windows(product: &str, display: &str, build: &str) -> String {
    let (product, display, build) = (product.trim(), display.trim(), build.trim());
    let recent = build.parse::<u32>().is_ok_and(|b| b >= 22_000);
    let mut parts = vec![match (product.is_empty(), recent) {
        (true, _) => "Windows".to_string(),
        (false, true) => product.replacen("Windows 10", "Windows 11", 1),
        (false, false) => product.to_string(),
    }];
    if !display.is_empty() {
        parts.push(display.to_string());
    }
    if !build.is_empty() {
        parts.push(format!("build {build}"));
    }
    parts.join(", ")
}

/// Une ligne par composant : version, build libmpv (LGPL ou GPL), « chemin personnalisé » ou « absent ». Le chemin
/// lui-même n'est jamais écrit (il peut contenir un nom d'utilisateur).
fn format_component(name: &str, c: &tools::Component) -> String {
    if !c.found {
        return format!("{name} : absent");
    }
    let mut s = format!(
        "{name} : {}",
        c.version.as_deref().unwrap_or("version inconnue")
    );
    if let Some(build) = c.build {
        s = format!("{s}, build {build}");
    }
    if c.custom {
        s = format!("{s}, chemin personnalisé");
    }
    s
}

/// Texte du diagnostic. Lit des fichiers (et peut lancer `streamlink --version` pour un Streamlink venu d'ailleurs) :
/// à appeler dans un fil d'arrière-plan.
pub fn collect(streamlink_path: &str, libmpv_path: &str) -> String {
    let mut lines = vec![
        format!(
            "Velka {} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        ),
        os_line(),
        format!(
            "Carte graphique : {}",
            GPU.get().map_or("inconnue", String::as_str)
        ),
    ];
    for tool in Tool::ALL {
        let custom = match tool {
            Tool::Streamlink => streamlink_path,
            Tool::Libmpv => libmpv_path,
        };
        lines.push(format_component(
            tool.label(),
            &tools::component_info(tool, custom),
        ));
    }
    crate::crash::clean(&lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// versions d'OpenGL telles que les pilotes les écrivent.
    #[test]
    fn opengl_avant_3_reconnu() {
        assert!(
            gl_below_3("1.1.0"),
            "pilote générique de Windows (GDI Generic)"
        );
        assert!(
            gl_below_3("2.1.0 - Build 8.15.10.2302"),
            "vieux pilote Intel"
        );
        assert!(gl_below_3("OpenGL ES 2.0 (ANGLE 2.1.0)"));
        assert!(!gl_below_3("4.6.0 NVIDIA 560.94"));
        assert!(!gl_below_3("3.0 Mesa 23.1"));
        assert!(!gl_below_3("OpenGL ES 3.0 (ANGLE 2.1.0)"));
        assert!(
            !gl_below_3("10.0"),
            "pas de lecture caractère par caractère"
        );
        assert!(!gl_below_3(""), "inconnue : pas d'avertissement");
        assert!(!gl_below_3("version inconnue"));
    }

    #[test]
    fn windows_11_reconnu_par_le_numero_de_build() {
        assert_eq!(
            format_windows("Windows 10 Pro", "25H2", "26300"),
            "Windows 11 Pro, 25H2, build 26300"
        );
        assert_eq!(
            format_windows("Windows 10 Pro", "22H2", "19045"),
            "Windows 10 Pro, 22H2, build 19045"
        );
        assert_eq!(format_windows("", "", ""), "Windows");
        assert_eq!(
            format_windows("Windows 10 Home", "", "abc"),
            "Windows 10 Home, build abc",
            "build illisible : pas de Windows 11"
        );
    }

    #[test]
    fn carte_graphique_lisible_meme_incomplete() {
        assert_eq!(
            format_gpu(
                "NVIDIA GeForce RTX 4070/PCIe/SSE2",
                "NVIDIA Corporation",
                "3.3.0 NVIDIA 566.36"
            ),
            "NVIDIA GeForce RTX 4070/PCIe/SSE2 (NVIDIA Corporation), OpenGL 3.3.0 NVIDIA 566.36"
        );
        assert_eq!(format_gpu("  ", "x", "y"), "inconnue");
        assert_eq!(
            format_gpu("Intel(R) UHD Graphics", "", ""),
            "Intel(R) UHD Graphics"
        );
    }

    #[test]
    fn ligne_de_composant() {
        let c = |found, custom, version: Option<&str>, build| tools::Component {
            found,
            custom,
            version: version.map(str::to_string),
            build,
        };
        assert_eq!(
            format_component("libmpv", &c(false, false, None, None)),
            "libmpv : absent"
        );
        assert_eq!(
            format_component("Streamlink", &c(true, false, Some("8.6.3-1"), None)),
            "Streamlink : 8.6.3-1"
        );
        assert_eq!(
            format_component("Streamlink", &c(true, true, None, None)),
            "Streamlink : version inconnue, chemin personnalisé"
        );
        assert_eq!(
            format_component(
                "libmpv",
                &c(true, false, Some("2026-11-24-35efcf53d5"), Some("LGPL"))
            ),
            "libmpv : 2026-11-24-35efcf53d5, build LGPL"
        );
    }

    #[test]
    fn ligne_linux() {
        let r = "NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 24.04.5 LTS\"\nVERSION_ID=\"24.04\"\n";
        assert_eq!(format_linux(r, "wayland"), "Ubuntu 24.04.5 LTS (wayland)");
        assert_eq!(format_linux("NAME=Fedora Linux\n", ""), "Fedora Linux");
        assert_eq!(format_linux("", "x11"), "distribution inconnue (x11)");
        assert_eq!(format_linux(r, "/home/moi"), "Ubuntu 24.04.5 LTS");
    }

    /// Le diagnostic complet : version, Windows, carte graphique, deux composants, sans nom d'utilisateur ni chemin
    /// personnalisé (même si le chemin contient le nom du compte).
    #[test]
    fn diagnostic_complet_sans_donnee_personnelle() {
        let user = std::env::var("USERNAME").unwrap_or_default();
        let secret = format!("D:\\Profils\\{user}\\perso\\streamlink.exe");
        let text = collect(&secret, "C:\\introuvable\\libmpv-2.dll");
        assert!(
            text.starts_with(&format!("Velka {}", env!("CARGO_PKG_VERSION"))),
            "{text}"
        );
        let os = if cfg!(windows) {
            "\nWindows : Windows"
        } else if cfg!(target_os = "linux") {
            "\nLinux : "
        } else {
            "\nmacOS : "
        };
        assert!(
            text.contains(os) && text.contains("\nCarte graphique : "),
            "{text}"
        );
        assert!(
            text.contains("\nStreamlink : ") && text.contains("\nlibmpv : "),
            "{text}"
        );
        assert_eq!(text.lines().count(), 5, "{text}");
        assert!(
            !text.contains("perso") && !text.contains("introuvable"),
            "aucun chemin dans le diagnostic : {text}"
        );
        if user.len() >= 3 {
            assert!(
                !text.to_lowercase().contains(&user.to_lowercase()),
                "nom d'utilisateur : {text}"
            );
        }
    }
}
