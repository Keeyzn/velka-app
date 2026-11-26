//! Essai de démarrage automatique (workflow de publication) : avec la variable d'environnement
//! `VELKA_SMOKE_TEST`, l'application utilise un dossier de données temporaire, ouvre sa fenêtre,
//! dessine quelques images puis se ferme seule en écrivant `essai.txt`.
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const VAR: &str = "VELKA_SMOKE_TEST";
/// Durée minimale d'affichage avant la fermeture.
const RUN_FOR: Duration = Duration::from_secs(3);
const MIN_FRAMES: u32 = 10;

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os(VAR).is_some())
}

/// Dossier de données de l'essai (jamais celui de l'utilisateur).
pub fn data_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("velka-essai").join("Velka")
}

/// Essai vidéo : avec `VELKA_SMOKE_LIBMPV` (chemin d'une libmpv) en plus de `VELKA_SMOKE_TEST`,
/// l'essai crée aussi un vrai lecteur (rendu OpenGL dans la fenêtre), lui fait jouer une mire générée par FFmpeg
/// (`lavfi`, aucun réseau) et attend des images vidéo avant de se fermer. Sans la variable : rien ne change.
fn video_lib() -> Option<std::path::PathBuf> {
    enabled()
        .then(|| std::env::var_os("VELKA_SMOKE_LIBMPV"))
        .flatten()
        .map(std::path::PathBuf::from)
}

/// Délai maximal d'attente des images vidéo avant de conclure à l'échec.
const VIDEO_WAIT: Duration = Duration::from_secs(30);

thread_local! {
    /// Lecteur de l'essai vidéo (fil d'interface seulement : il n'est pas transférable d'un fil à l'autre).
    static VIDEO: std::cell::RefCell<Option<crate::mpv::Player>> = const { std::cell::RefCell::new(None) };
}

/// Crée le lecteur de l'essai au premier appel, le dessine, et renvoie le nombre d'images vidéo affichées
/// (`None` : pas d'essai vidéo demandé ; texte d'erreur si le lecteur n'a pas pu démarrer).
fn video_step(
    ctx: &eframe::egui::Context,
    get_proc: Option<&crate::mpv::GetProc>,
) -> Option<Result<u64, String>> {
    use eframe::egui;
    let lib = video_lib()?;
    static FAILED: OnceLock<String> = OnceLock::new();
    if let Some(e) = FAILED.get() {
        return Some(Err(e.clone()));
    }
    VIDEO.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            let created = get_proc
                .ok_or_else(|| "pas de contexte OpenGL".to_string())
                .and_then(|gp| {
                    // Mêmes options que le lecteur de l'application (`mpv::MPV_OPTIONS`), plus la lecture en boucle de la
                    // vidéo d'essai.
                    let mut opts = crate::mpv::default_options();
                    opts.push(("loop-file".into(), "inf".into()));
                    // Journal de mpv de l'essai (dossier temporaire de l'essai) : utile si aucune image n'arrive.
                    let log = data_dir().join("mpv-essai.log");
                    let p = crate::mpv::Player::new(gp.clone(), ctx, &lib, Some(&log), &opts)?;
                    // Source : `VELKA_SMOKE_VIDEO` (fichier ou adresse, par exemple un MPEG-TS H.264/AAC comme un live
                    // Twitch), sinon la mire de FFmpeg (demande une libmpv avec `lavfi`).
                    let source = std::env::var("VELKA_SMOKE_VIDEO")
                        .unwrap_or_else(|_| "av://lavfi:testsrc=size=640x360:rate=30".into());
                    p.load(&source)?;
                    Ok(p)
                });
            match created {
                Ok(p) => *v = Some(p),
                Err(e) => {
                    crate::logging::log(&format!("Essai vidéo : lecteur impossible : {e}"));
                    let _ = FAILED.set(e.clone());
                    return Some(Err(e));
                }
            }
        }
        let p = v.as_ref()?;
        egui::Area::new(egui::Id::unique("essai_video"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                p.paint(
                    ui,
                    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 180.0)),
                );
            });
        let shown = p
            .get_property("estimated-frame-number")
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        Some(Ok(shown))
    })
}

/// Arrête le lecteur de l'essai pendant que le contexte OpenGL est encore courant.
fn video_stop() {
    VIDEO.with(|v| drop(v.borrow_mut().take()));
}

/// Appelé à chaque image : ferme la fenêtre une fois l'essai réussi.
pub fn tick(ctx: &eframe::egui::Context, get_proc: Option<&crate::mpv::GetProc>) {
    static START: OnceLock<Instant> = OnceLock::new();
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    if !enabled() {
        return;
    }
    let start = *START.get_or_init(Instant::now);
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    let video = video_step(ctx, get_proc);
    // Essai vidéo : on attend des images (au plus `VIDEO_WAIT`) ; le résultat est ajouté à `essai.txt`.
    let video_text = match &video {
        None => String::new(),
        Some(Ok(n)) if *n > 0 => format!(" video {n}"),
        Some(Ok(_)) if start.elapsed() < VIDEO_WAIT => {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        Some(Ok(_)) => " video 0".to_string(),
        Some(Err(e)) => format!(" video erreur : {e}"),
    };
    if start.elapsed() >= RUN_FOR && frames >= MIN_FRAMES {
        // La fermeture prend une ou deux images : le résultat n'est écrit qu'une fois.
        static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if DONE.swap(true, Ordering::Relaxed) {
            return;
        }
        video_stop();
        let _ = std::fs::write(
            data_dir().join("essai.txt"),
            format!("ok {frames}{video_text}\n"),
        );
        crate::logging::log(&format!("Essai de démarrage réussi ({frames} images)"));
        if video.is_some() {
            // Après l'essai vidéo, la sortie normale restait bloquée sous Windows (cause non trouvée) : le résultat est écrit, on termine le processus directement.
            crate::procs::terminate_now();
        }
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
    } else {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    /// L'essai vidéo reprend bien les options du lecteur (sinon une option absente d'une libmpv passerait inaperçue).
    #[test]
    fn options_du_lecteur() {
        let o = crate::mpv::default_options();
        assert!(o.iter().any(|(k, v)| k == "vo" && v == "libmpv"), "{o:?}");
        assert!(o.iter().any(|(k, _)| k == "osc"), "{o:?}");
    }
}
