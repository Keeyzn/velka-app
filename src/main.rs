#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Aucun unwrap()/expect() dans le code de l'application (seuls les tests en ont le droit) : une erreur réseau ou disque ne doit jamais fermer l'application.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod auth;
mod avatars;
mod cache;
mod chat;
mod cli;
mod config;
mod crash;
mod diag;
mod follows;
mod help;
mod history;
mod i18n;
mod logging;
mod login;
mod mpv;
mod net;
mod procs;
mod publication;
mod smoke;
mod stream;
#[cfg(test)]
mod testutil;
mod thumbs;
mod tools;
mod twitch;
mod ui;
mod uninstall;
mod update;

/// Prend l'instance unique ; après une relance, réessaie 30 fois à 100 ms d'intervalle avant d'abandonner.
fn take_single_instance(patient: bool) -> bool {
    let tries = if patient { 30 } else { 1 };
    for i in 0..tries {
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if procs::acquire_single_instance() {
            return true;
        }
    }
    false
}

fn main() -> eframe::Result<()> {
    logging::init();
    crash::install();
    crash::prune();
    if crash::read().is_some() {
        logging::log("Plantage précédent : rapport dans crash.txt (Réglages > Avancé et Système > Journaux et diagnostic)");
    }
    procs::set_app_id();

    // Une seule instance : une seconde ouverture transmet sa demande à la première.
    let cli = cli::current();
    for w in &cli.warnings {
        logging::log(&format!("Ligne de commande : {w}"));
    }
    if cli.diag {
        logging::log("Mode diagnostic (--diag) : journal détaillé du lecteur dans mpv.log");
    }
    // Relance par une mise à jour (`--wait-pid`) : on attend la fermeture de l'ancien processus (au plus 30 s),
    // puis on réessaie la prise de l'instance unique pendant 3 s, le temps que Windows libère son verrou.
    let relaunched = cli.wait_pid.is_some();
    if let Some(pid) = cli.wait_pid {
        procs::wait_for_exit(pid, std::time::Duration::from_secs(30));
    }
    // L'essai de démarrage a son propre dossier : il ne transmet rien à une instance déjà ouverte.
    if !smoke::enabled() && !take_single_instance(relaunched) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        cli::send_to_running(&args);
        procs::focus_existing_window();
        return Ok(());
    }

    // Nettoyage d'une mise à jour précédente ; si la nouvelle version n'avait jamais démarré, l'ancienne est remise.
    if update::startup() {
        return Ok(());
    }
    if !smoke::enabled() {
        // Nom et logo dans les notifications Windows (en arrière-plan : n'allonge pas le démarrage).
        std::thread::spawn(procs::ensure_start_menu_shortcut);
        // Cache d'images : fichiers de plus de 30 jours et dépassement de 100 Mo supprimés.
        std::thread::spawn(cache::prune);
    }
    let mut settings = config::Settings::load();

    // Place utile de l'écran principal (points) : la fenêtre agrandie de 20 % ne la dépasse pas (petit écran).
    let limit = procs::monitor_rects().first().map_or([f32::MAX; 2], |r| {
        let scale = procs::system_scale();
        [
            (r.2 - r.0) as f32 / scale - 40.0,
            (r.3 - r.1) as f32 / scale - 90.0,
        ]
    });
    // Anciennes installations : la taille mémorisée est agrandie de 20 % une seule fois.
    if settings.grow_window_once(limit) && !smoke::enabled() {
        let _ = settings.save();
    }

    // Taille et position mémorisées (la position n'est reprise que si elle est visible à l'écran).
    let size = settings
        .window_size
        .filter(|s| s[0] >= config::MIN_WINDOW[0] && s[1] >= config::MIN_WINDOW[1])
        .unwrap_or([0, 1].map(|i| {
            config::DEFAULT_WINDOW[i]
                .min(limit[i])
                .max(config::MIN_WINDOW[i])
        }));
    // Sans barre ni bordure Windows : l'application dessine sa barre de titre et ses boutons (app/fenetre.rs) ; sur
    // macOS, cadre natif et boutons ronds du système, barre transparente (`theme::window_frame`).
    let mut viewport = ui::theme::window_frame(eframe::egui::ViewportBuilder::default())
        .with_title("Velka")
        .with_inner_size(size)
        .with_min_inner_size(config::MIN_WINDOW)
        .with_maximized(settings.window_maximized);
    if let Some(icon) = ui::logo::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    // Linux : identifiant de la fenêtre (Wayland `app_id`, X11 `WM_CLASS`) ; le bureau s'en sert pour retrouver le
    // lanceur `velka.desktop` du paquet (icône dans la barre des tâches, regroupement).
    #[cfg(target_os = "linux")]
    {
        viewport = viewport.with_app_id("velka");
    }
    let saved_pos = settings
        .window_pos
        .filter(|p| procs::position_is_visible(*p, settings.window_ppp));
    // `--screen N` : ouvre la fenêtre sur l'écran demandé.
    let screen_pos = cli.screen.and_then(|n| {
        let rect = *procs::monitor_rects().get(n - 1)?;
        let scale = procs::system_scale();
        Some([
            (rect.0 as f32 + 60.0) / scale,
            (rect.1 as f32 + 60.0) / scale,
        ])
    });
    let start_pos = screen_pos.or(saved_pos);
    if let Some(pos) = start_pos {
        viewport = viewport.with_position(pos);
        if screen_pos.is_some() {
            viewport = viewport.with_maximized(false);
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        // Première ouverture (aucune position mémorisée ou visible) : fenêtre au centre de l'écran, pas dans le coin.
        centered: start_pos.is_none(),
        ..Default::default()
    };
    // Langue des messages d'erreur d'affichage ci-dessous (l'application la règle aussi à sa création).
    i18n::set(settings.language);
    #[cfg(debug_assertions)]
    if diag::opengl_test() == Some("absent") {
        display_failed("essai VELKA_ESSAI_OPENGL=absent");
        return Ok(());
    }
    let result = eframe::run_native(
        "Velka",
        options,
        Box::new(|cc| Ok(Box::new(app::VelkaApp::new(cc)))),
    );
    if let Err(e) = &result {
        #[cfg(target_os = "linux")]
        if retry_with_x11(&e.to_string()) {
            return Ok(());
        }
        display_failed(&e.to_string());
    }
    result
}

/// Variable posée sur la relance par X11 : un seul nouvel essai.
#[cfg(target_os = "linux")]
const X11_RETRY: &str = "VELKA_REPLI_X11";

/// Linux : fenêtre Wayland impossible (pilote OpenGL qui n'initialise pas EGL sous Wayland, vu dans WSLg) alors qu'un
/// affichage X11 existe (XWayland) : on relance l'application sans `WAYLAND_DISPLAY`, donc par X11, une seule fois.
/// La relance attend la fin de ce processus (`--wait-pid`) avant de prendre l'instance unique.
#[cfg(target_os = "linux")]
fn retry_with_x11(detail: &str) -> bool {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if !set("WAYLAND_DISPLAY") || !set("DISPLAY") || set(X11_RETRY) {
        return false;
    }
    let Ok(exe) = procs::relaunch_exe() else {
        return false;
    };
    logging::log(&format!(
        "Fenêtre impossible à créer sous Wayland ({detail}) : nouvel essai par X11"
    ));
    std::process::Command::new(exe)
        .args(args_without_wait_pid(std::env::args().skip(1)))
        .arg("--wait-pid")
        .arg(std::process::id().to_string())
        .env_remove("WAYLAND_DISPLAY")
        .env(X11_RETRY, "1")
        .spawn()
        .is_ok()
}

/// Arguments de la ligne de commande sans `--wait-pid` (remplacé par celui de la relance).
#[cfg(any(target_os = "linux", test))]
fn args_without_wait_pid(args: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if std::mem::take(&mut skip) {
            continue;
        }
        if a == "--wait-pid" {
            skip = true;
        } else if !a.starts_with("--wait-pid=") {
            out.push(a);
        }
    }
    out
}

/// La fenêtre n'a pas pu être créée (le plus souvent : pas d'OpenGL 3.0, pilote générique de Windows, bureau à
/// distance, machine virtuelle). Sans fenêtre ni console, l'utilisateur ne voyait rien : message clair.
fn display_failed(detail: &str) {
    logging::log(&format!("Fenêtre impossible à créer (OpenGL) : {detail}"));
    if !smoke::enabled() {
        procs::message_box(
            i18n::tr("Velka n'a pas pu ouvrir sa fenêtre : la carte graphique ou son pilote ne prend pas en charge OpenGL 3.0. Mettez à jour le pilote de la carte graphique, puis relancez Velka. Sur un bureau à distance ou une machine virtuelle, l'affichage peut ne pas être pris en charge."),
            false,
        );
    }
}

#[cfg(test)]
mod tests_relance {
    #[test]
    fn relance_sans_ancien_wait_pid() {
        let a = |v: &[&str]| super::args_without_wait_pid(v.iter().map(|s| s.to_string()));
        assert_eq!(a(&["xqc", "--wait-pid", "12", "--diag"]), ["xqc", "--diag"]);
        assert_eq!(a(&["--wait-pid=7", "xqc"]), ["xqc"]);
        assert!(a(&[]).is_empty());
    }
}
