//! Polices de Windows pour le chinois, le japonais et le coréen (absents d'Inter), chargées seulement quand il le
//! faut : rien n'est ajouté à l'exécutable et la mémoire n'est utilisée que pour ces langues.
use crate::i18n::Cjk;
use eframe::egui;
use std::sync::Mutex;

/// Polices installées avec Windows, par ordre de préférence : (fichier, n° de la police dans le fichier).
/// On prend les variantes « UI » (celles de l'interface de Windows) : kana proportionnels, interlignage serré.
fn candidates(c: Cjk) -> &'static [(&'static str, u32)] {
    match c {
        Cjk::ChineseSimplified => &[("msyh.ttc", 1), ("simsun.ttc", 0)],
        Cjk::ChineseTraditional => &[("msjh.ttc", 1), ("mingliu.ttc", 0)],
        Cjk::Japanese => &[
            ("YuGothM.ttc", 1),
            ("YuGothR.ttc", 1),
            ("meiryo.ttc", 0),
            ("msgothic.ttc", 0),
        ],
        Cjk::Korean => &[("malgun.ttf", 0), ("gulim.ttc", 0)],
    }
}

/// Écritures dont la police est actuellement chargée.
static INSTALLED: Mutex<Vec<Cjk>> = Mutex::new(Vec::new());

/// Chargement en arrière-plan : écritures demandées et polices lues, prêtes à être installées.
type Loaded = Vec<(String, egui::FontData)>;

struct Loading {
    loading: Option<Vec<Cjk>>,
    ready: Option<(Vec<Cjk>, Loaded)>,
}
static STATE: Mutex<Loading> = Mutex::new(Loading {
    loading: None,
    ready: None,
});

fn load(c: Cjk) -> Option<(String, egui::FontData)> {
    let windir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into());
    let dir = windir.join("Fonts");
    for (f, index) in candidates(c) {
        if let Ok(bytes) = std::fs::read(dir.join(f)) {
            let mut data = egui::FontData::from_owned(bytes);
            data.index = *index;
            return Some((format!("{c:?}"), data));
        }
    }
    crate::logging::log(&format!("Police introuvable pour {c:?}"));
    None
}

/// Vrai quand les polices du carrousel des langues (chinois et coréen) sont installées : on peut alors dérouler la
/// liste sans que l'installation, lourde, tombe en plein milieu de l'animation.
pub fn carousel_ready() -> bool {
    INSTALLED
        .lock()
        .is_ok_and(|v| v.contains(&Cjk::ChineseSimplified) && v.contains(&Cjk::Korean))
}

/// Charge la police de la langue courante si elle en a besoin. Avec `carousel` (carrousel des langues affiché),
/// charge aussi le chinois (qui couvre aussi le japonais) et le coréen pour écrire le nom de chaque langue.
/// Les polices inutiles sont libérées dès qu'elles ne servent plus.
pub fn ensure(ctx: &egui::Context, carousel: bool) {
    let mut want = Vec::new();
    if let Some(c) = crate::i18n::current().cjk {
        want.push(c);
    }
    if carousel {
        for c in [Cjk::ChineseSimplified, Cjk::Korean] {
            if !want.contains(&c) {
                want.push(c);
            }
        }
    }
    let Ok(mut installed) = INSTALLED.lock() else {
        return;
    };
    if *installed == want {
        return;
    }
    if want.is_empty() {
        crate::ui::theme::install_fonts_with(ctx, Vec::new());
        *installed = want;
        return;
    }
    // La lecture des fichiers de police se fait en arrière-plan : l'interface ne fige pas.
    let Ok(mut st) = STATE.lock() else {
        return;
    };
    if let Some((c, fonts)) = st.ready.take() {
        if c == want {
            crate::ui::theme::install_fonts_with(ctx, fonts);
            *installed = want;
            ctx.request_repaint(); // la liste des langues attend cette installation pour se dérouler
            return;
        }
    }
    if st.loading.as_ref() != Some(&want) {
        st.loading = Some(want.clone());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let fonts: Vec<_> = want.iter().filter_map(|c| load(*c)).collect();
            if let Ok(mut st) = STATE.lock() {
                st.loading = None;
                st.ready = Some((want, fonts));
            }
            ctx.request_repaint();
        });
    }
}
