//! Identité visuelle (DESIGN.md) : tokens, thème egui et petits composants.
//! Dark premium : niveaux de gris très proches, bordures hairline, une seule couleur d'accent (orange par défaut).
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Stroke,
    Vec2,
};

use std::sync::atomic::{AtomicU8, Ordering};

/// Jeu de couleurs d'un thème.
#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    s1: Color32,
    s2: Color32,
    s3: Color32,
    border: Color32,
    border_hover: Color32,
    text: Color32,
    muted: Color32,
    faint: Color32,
    positive: Color32,
    negative: Color32,
}

/// Thème noir (valeur 0, nom interne SOMBRE, affiché « Dark » depuis le 24/11/2026) : presque noir, surfaces très
/// proches (DESIGN.md).
static SOMBRE: Palette = Palette {
    bg: Color32::from_rgb(9, 9, 11),
    s1: Color32::from_rgb(16, 16, 18),
    s2: Color32::from_rgb(22, 22, 24),
    s3: Color32::from_rgb(29, 29, 32),
    border: Color32::from_rgba_premultiplied(15, 15, 15, 15),
    border_hover: Color32::from_rgba_premultiplied(31, 31, 31, 31),
    text: Color32::from_rgb(250, 250, 250),
    muted: Color32::from_rgb(139, 139, 147),
    // 4,5:1 minimum sur bg, s1 et s2 (WCAG 1.4.3) ; avant (85, 85, 92) = 2,4 à 2,7:1.
    faint: Color32::from_rgb(127, 127, 134),
    positive: Color32::from_rgb(52, 211, 153),
    negative: Color32::from_rgb(240, 82, 91),
};

/// Thème bleu nuit (valeur 1, nom interne DARK, affiché « Sombre » depuis le 24/11/2026) : ardoise, un peu plus
/// lumineux que le noir.
static DARK: Palette = Palette {
    bg: Color32::from_rgb(17, 20, 27),
    s1: Color32::from_rgb(23, 27, 36),
    s2: Color32::from_rgb(30, 35, 46),
    s3: Color32::from_rgb(39, 45, 59),
    border: Color32::from_rgba_premultiplied(18, 18, 18, 18),
    border_hover: Color32::from_rgba_premultiplied(36, 36, 36, 36),
    text: Color32::from_rgb(236, 240, 247),
    muted: Color32::from_rgb(146, 154, 171),
    // Avant (96, 104, 121) = 2,8 à 3,3:1.
    faint: Color32::from_rgb(130, 138, 155),
    positive: Color32::from_rgb(52, 211, 153),
    negative: Color32::from_rgb(240, 82, 91),
};

/// Clair : fond gris clair, cartes blanches, texte presque noir, bordures nettes.
static CLAIR: Palette = Palette {
    bg: Color32::from_rgb(236, 238, 243),
    s1: Color32::from_rgb(255, 255, 255),
    s2: Color32::from_rgb(228, 231, 238),
    s3: Color32::from_rgb(210, 214, 225),
    border: Color32::from_rgba_premultiplied(0, 0, 0, 46),
    border_hover: Color32::from_rgba_premultiplied(0, 0, 0, 84),
    text: Color32::from_rgb(8, 9, 13),
    muted: Color32::from_rgb(72, 77, 92),
    // Avant (112, 117, 132) = 3,7:1 sur s2 ; (21, 128, 61) = 4,05:1 sur s2.
    faint: Color32::from_rgb(98, 103, 118),
    positive: Color32::from_rgb(13, 120, 53),
    negative: Color32::from_rgb(200, 30, 40),
};

/// Contraste élevé (thème noir) : texte au moins à 7:1 (WCAG AAA), bordures bien visibles.
static SOMBRE_HC: Palette = Palette {
    bg: Color32::from_rgb(9, 9, 11),
    s1: Color32::from_rgb(16, 16, 18),
    s2: Color32::from_rgb(22, 22, 24),
    s3: Color32::from_rgb(29, 29, 32),
    border: Color32::from_rgba_premultiplied(110, 110, 110, 110),
    border_hover: Color32::from_rgba_premultiplied(170, 170, 170, 170),
    text: Color32::from_rgb(255, 255, 255),
    muted: Color32::from_rgb(200, 200, 208),
    faint: Color32::from_rgb(170, 170, 178),
    positive: Color32::from_rgb(110, 231, 183),
    negative: Color32::from_rgb(255, 150, 156),
};

/// Contraste élevé (thème bleu nuit).
static DARK_HC: Palette = Palette {
    bg: Color32::from_rgb(17, 20, 27),
    s1: Color32::from_rgb(23, 27, 36),
    s2: Color32::from_rgb(30, 35, 46),
    s3: Color32::from_rgb(39, 45, 59),
    border: Color32::from_rgba_premultiplied(110, 110, 110, 110),
    border_hover: Color32::from_rgba_premultiplied(170, 170, 170, 170),
    text: Color32::from_rgb(255, 255, 255),
    muted: Color32::from_rgb(206, 212, 226),
    faint: Color32::from_rgb(176, 184, 202),
    positive: Color32::from_rgb(110, 231, 183),
    negative: Color32::from_rgb(255, 150, 156),
};

/// Contraste élevé (Clair).
static CLAIR_HC: Palette = Palette {
    bg: Color32::from_rgb(236, 238, 243),
    s1: Color32::from_rgb(255, 255, 255),
    s2: Color32::from_rgb(228, 231, 238),
    s3: Color32::from_rgb(210, 214, 225),
    border: Color32::from_rgba_premultiplied(0, 0, 0, 140),
    border_hover: Color32::from_rgba_premultiplied(0, 0, 0, 200),
    text: Color32::from_rgb(0, 0, 0),
    muted: Color32::from_rgb(45, 48, 60),
    faint: Color32::from_rgb(58, 62, 76),
    positive: Color32::from_rgb(0, 80, 30),
    negative: Color32::from_rgb(140, 0, 12),
};

/// Contraste élevé actif (réglage, ou thème de contraste de Windows en mode automatique).
static HIGH_CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Focus clavier renforcé : anneau épais, à double contour, toujours visible sur l'élément ayant le focus.
static FOCUS_STRONG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// À appeler avant `apply` : choisit les palettes à contraste élevé.
pub fn set_high_contrast(on: bool) {
    HIGH_CONTRAST.store(on, Ordering::Relaxed);
}

pub fn set_focus_strong(on: bool) {
    FOCUS_STRONG.store(on, Ordering::Relaxed);
}

/// Animations « réduites » (réglage de l'application, jamais celui de Windows) : fondus et couleurs gardés, déplacements coupés.
static REDUCED_MOTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_reduced_motion(on: bool) {
    REDUCED_MOTION.store(on, Ordering::Relaxed);
}

/// Épaisseur de l'anneau de focus.
pub fn focus_width() -> f32 {
    if FOCUS_STRONG.load(Ordering::Relaxed) {
        3.5
    } else {
        2.0
    }
}

use std::sync::atomic::{AtomicBool, AtomicU32};

/// Couleur d'accent par défaut (#CA4F24, orange).
pub const ACCENT_NEW: [u8; 3] = [202, 79, 36];

/// Palettes prédéfinies : (nom français, couleur d'accent). Chaque couleur donne au moins 4,5:1 avec le texte blanc
/// des boutons (test `palettes_conformes`) ; le choix « Personnalisée » vient après ces cinq-là (voir
/// `Settings::palette`). L'indice est enregistré dans `settings.json` : ne pas réordonner.
pub const PALETTES: [(&str, [u8; 3]); 5] = [
    ("Turquoise", [15, 118, 110]),
    ("Bleu", [37, 99, 235]),
    ("Vert", [21, 128, 61]),
    ("Orange foncé", [194, 65, 12]),
    ("Rose", [200, 40, 120]),
];

fn pack(c: Color32) -> u32 {
    (u32::from(c.r()) << 16) | (u32::from(c.g()) << 8) | u32::from(c.b())
}

fn unpack(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Couleur d'accent choisie : 0 = aucune (orange par défaut), sinon `1 << 24 | rgb`.
static ACC_CUSTOM: AtomicU32 = AtomicU32::new(0);
/// Couleurs dérivées de l'accent (voir `compute_accent`) : orange par défaut, recalculées par `set_accent`.
static ACC_FILL: AtomicU32 = AtomicU32::new(0xCA4F24);
static ACC_HOVER: AtomicU32 = AtomicU32::new(0xCA4F24);
static ACC_TEXT_DARK: AtomicU32 = AtomicU32::new(0xCA4F24);
static ACC_TEXT_LIGHT: AtomicU32 = AtomicU32::new(0xCA4F24);
/// La couleur choisie est-elle corrigée pour que le texte blanc y soit lisible ? (faux = couleur appliquée telle quelle)
static ACC_BOOST: AtomicBool = AtomicBool::new(true);

/// Couleurs dérivées d'un accent : fond des boutons (texte blanc dessus), survol, et variantes pour les traits, anneaux
/// et textes sur fond sombre / clair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccentSet {
    pub fill: Color32,
    pub hover: Color32,
    pub text_dark: Color32,
    pub text_light: Color32,
}

/// Contraste minimal imposé : 4,5:1 (AA), 7:1 en contraste élevé (AAA).
pub fn required_ratio(high_contrast: bool) -> f32 {
    if high_contrast {
        7.0
    } else {
        4.5
    }
}

/// Le texte blanc d'un bouton est-il assez lisible sur ce fond d'accent ?
pub fn fill_ok(c: Color32, ratio: f32) -> bool {
    contrast(Color32::WHITE, c) >= ratio
}

fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Couleur conforme la plus proche : `c` si elle l'est déjà, sinon `c` assombrie par pas de 1 % (même teinte) jusqu'à
/// ce que le texte blanc y soit lisible.
pub fn nearest_compliant_fill(c: Color32, ratio: f32) -> Color32 {
    if fill_ok(c, ratio) {
        return c;
    }
    (1..=100)
        .map(|i| mix(c, Color32::BLACK, i as f32 / 100.0))
        .find(|m| fill_ok(*m, ratio))
        .unwrap_or(Color32::BLACK)
}

/// Variante de l'accent lisible comme texte, trait ou anneau sur les fonds du thème sombre (`dark`) ou clair :
/// éclaircie (ou assombrie) jusqu'à `ratio` sur bg, surface-1 et surface-2.
fn accent_text_on(fill: Color32, dark: bool, high_contrast: bool, ratio: f32) -> Color32 {
    let p = palette_for(if dark { 0 } else { 2 }, high_contrast);
    let target = if dark { Color32::WHITE } else { Color32::BLACK };
    (0..=50)
        .map(|i| mix(fill, target, i as f32 / 50.0))
        .find(|m| [p.bg, p.s1, p.s2].iter().all(|b| contrast(*m, *b) >= ratio))
        .unwrap_or(target)
}

/// Couleurs dérivées pour un accent choisi (`None` = orange par défaut, `ACCENT_NEW`).
#[cfg(test)]
pub fn compute_accent(custom: Option<[u8; 3]>, high_contrast: bool) -> AccentSet {
    compute_accent_with(custom, high_contrast, true)
}

/// Comme `compute_accent` ; `boost` faux : la couleur choisie est gardée telle quelle, même peu contrastée.
pub fn compute_accent_with(custom: Option<[u8; 3]>, high_contrast: bool, boost: bool) -> AccentSet {
    let [r, g, b] = custom.unwrap_or(ACCENT_NEW);
    let ratio = required_ratio(high_contrast);
    let chosen = Color32::from_rgb(r, g, b);
    let fill = if boost {
        nearest_compliant_fill(chosen, ratio)
    } else {
        chosen
    };
    // Survol : un peu plus clair tant que le texte blanc reste lisible, sinon la même couleur.
    let hover = [0.20_f32, 0.16, 0.12, 0.08, 0.04]
        .iter()
        .map(|k| mix(fill, Color32::WHITE, *k))
        .find(|m| fill_ok(*m, ratio))
        .unwrap_or(fill);
    AccentSet {
        fill,
        hover,
        text_dark: accent_text_on(fill, true, high_contrast, ratio),
        text_light: accent_text_on(fill, false, high_contrast, ratio),
    }
}

fn recompute_accent() {
    let c = ACC_CUSTOM.load(Ordering::Relaxed);
    let custom = (c != 0).then_some([(c >> 16) as u8, (c >> 8) as u8, c as u8]);
    let set = compute_accent_with(
        custom,
        HIGH_CONTRAST.load(Ordering::Relaxed),
        ACC_BOOST.load(Ordering::Relaxed),
    );
    ACC_FILL.store(pack(set.fill), Ordering::Relaxed);
    ACC_HOVER.store(pack(set.hover), Ordering::Relaxed);
    ACC_TEXT_DARK.store(pack(set.text_dark), Ordering::Relaxed);
    ACC_TEXT_LIGHT.store(pack(set.text_light), Ordering::Relaxed);
}

/// Choisit la couleur d'accent (`None` = orange par défaut). À appeler après `set_high_contrast` et avant `apply`.
pub fn set_accent(custom: Option<[u8; 3]>, boost: bool) {
    ACC_BOOST.store(boost, Ordering::Relaxed);
    let v = custom.map_or(0, |[r, g, b]| (1 << 24) | pack(Color32::from_rgb(r, g, b)));
    ACC_CUSTOM.store(v, Ordering::Relaxed);
    recompute_accent();
}

/// Contraste élevé actif ? (pour afficher le bon minimum dans les Réglages)
pub fn high_contrast_active() -> bool {
    HIGH_CONTRAST.load(Ordering::Relaxed)
}

/// `#RRGGBB`.
pub fn hex_of(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Lit `#RRGGBB` ou `RRGGBB`.
#[cfg(test)]
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Fond des boutons principaux et des pastilles « sélectionnées » (le texte posé dessus est blanc).
#[allow(non_snake_case)]
pub fn ACCENT() -> Color32 {
    unpack(ACC_FILL.load(Ordering::Relaxed))
}

/// Fond d'un bouton principal survolé.
#[allow(non_snake_case)]
pub fn ACCENT_HOVER() -> Color32 {
    unpack(ACC_HOVER.load(Ordering::Relaxed))
}

/// Accent lisible comme texte, anneau ou lien sur les fonds du thème courant.
#[allow(non_snake_case)]
pub fn ACCENT_TEXT() -> Color32 {
    accent_text_for(CURRENT.load(Ordering::Relaxed) == 2)
}

pub fn accent_text_for(light: bool) -> Color32 {
    unpack(if light {
        ACC_TEXT_LIGHT.load(Ordering::Relaxed)
    } else {
        ACC_TEXT_DARK.load(Ordering::Relaxed)
    })
}

/// Traits (contours, sélection) : variante lisible de l'accent.
#[allow(non_snake_case)]
pub fn ACCENT_LINE() -> Color32 {
    ACCENT_TEXT()
}

/// Petits dessins (curseurs, pastilles radio) : variante lisible de l'accent.
#[allow(non_snake_case)]
pub fn ACCENT_GFX() -> Color32 {
    ACCENT_TEXT()
}

/// Teinte légère de l'accent (14 %), pour les pastilles discrètes.
#[allow(non_snake_case)]
pub fn ACCENT_SOFT() -> Color32 {
    let f = ACCENT();
    let k = |v: u8| (f32::from(v) * 0.14).round() as u8;
    Color32::from_rgba_premultiplied(k(f.r()), k(f.g()), k(f.b()), 36)
}

/// Thème courant : 0 = noir (affiché « Dark »), 1 = bleu nuit (affiché « Sombre »), 2 = Clair.
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Noms des thèmes dans la langue courante, dans l'ordre des valeurs enregistrées (0 = noir, 1 = bleu nuit). Depuis le
/// 24/11/2026 le noir s'affiche « Dark » et le bleu nuit « Sombre » (les valeurs de `settings.json` n'ont pas changé).
pub fn theme_names() -> [&'static str; 4] {
    [
        crate::i18n::tr("Dark"),
        crate::i18n::tr("Sombre"),
        crate::i18n::tr("Clair"),
        crate::i18n::tr("Système"),
    ]
}

/// Valeur du réglage « thème » qui suit Windows (clair → Clair, sombre → thème noir, affiché « Dark »).
pub const THEME_SYSTEM: u8 = 3;

/// Thème réellement affiché pour un réglage (résout « Système » avec le thème de Windows).
fn resolve(ctx: &egui::Context, theme: u8) -> u8 {
    if theme == THEME_SYSTEM {
        if ctx.system_theme() == Some(egui::Theme::Light) {
            2
        } else {
            0
        }
    } else {
        theme.min(2)
    }
}

/// À appeler à chaque image : en mode « Système », suit un changement de thème de Windows sans redémarrer.
pub fn sync_system_theme(ctx: &egui::Context, theme: u8) {
    if theme == THEME_SYSTEM && resolve(ctx, theme) != CURRENT.load(Ordering::Relaxed) {
        apply(ctx, theme);
    }
}

fn palette_for(theme: u8, high_contrast: bool) -> &'static Palette {
    match (theme, high_contrast) {
        (1, false) => &DARK,
        (2, false) => &CLAIR,
        (1, true) => &DARK_HC,
        (2, true) => &CLAIR_HC,
        (_, true) => &SOMBRE_HC,
        _ => &SOMBRE,
    }
}

fn palette_of(theme: u8) -> Palette {
    *palette_for(theme, HIGH_CONTRAST.load(Ordering::Relaxed))
}

fn pal() -> Palette {
    palette_of(CURRENT.load(Ordering::Relaxed))
}

#[allow(non_snake_case)]
pub fn BG() -> Color32 {
    pal().bg
}
#[allow(non_snake_case)]
pub fn SURFACE_1() -> Color32 {
    pal().s1
}
#[allow(non_snake_case)]
pub fn SURFACE_2() -> Color32 {
    pal().s2
}
#[allow(non_snake_case)]
pub fn SURFACE_3() -> Color32 {
    pal().s3
}
/// Bordure hairline (6 % d'opacité).
#[allow(non_snake_case)]
pub fn BORDER() -> Color32 {
    pal().border
}
/// Bordure au survol (12 %).
#[allow(non_snake_case)]
pub fn BORDER_HOVER() -> Color32 {
    pal().border_hover
}
#[allow(non_snake_case)]
pub fn TEXT() -> Color32 {
    pal().text
}
#[allow(non_snake_case)]
pub fn MUTED() -> Color32 {
    pal().muted
}
#[allow(non_snake_case)]
pub fn FAINT() -> Color32 {
    pal().faint
}
#[allow(non_snake_case)]
pub fn POSITIVE() -> Color32 {
    pal().positive
}
#[allow(non_snake_case)]
pub fn NEGATIVE() -> Color32 {
    pal().negative
}
/// Rouge du remplissage d'un bouton à texte blanc (« danger », « Annuler » du chargement). Le rouge du thème, sauf quand
/// il est trop clair pour du texte blanc (contraste élevé sur fond sombre : un rose pâle qui donnait un voile blanc sur
/// le bouton) : il est alors assombri jusqu'à 7:1 avec le blanc (`negative_fill_of`).
#[allow(non_snake_case)]
pub fn NEGATIVE_FILL() -> Color32 {
    negative_fill_of(NEGATIVE())
}
fn negative_fill_of(n: Color32) -> Color32 {
    if contrast(Color32::WHITE, n) >= 3.0 {
        return n;
    }
    (1..=20)
        .map(|i| n.lerp_to_gamma(Color32::BLACK, i as f32 * 0.05))
        .find(|c| contrast(Color32::WHITE, *c) >= 7.0)
        .unwrap_or(Color32::BLACK)
}
/// Rouge d'un bouton « danger » survolé : `NEGATIVE_FILL` éclairci de 20 % (texte blanc toujours lisible, au moins 4,5:1
/// dans les six palettes).
#[allow(non_snake_case)]
pub fn NEGATIVE_HOVER() -> Color32 {
    negative_hover_of(NEGATIVE_FILL())
}
fn negative_hover_of(fill: Color32) -> Color32 {
    fill.lerp_to_gamma(Color32::WHITE, 0.2)
}

/// Gabarits de hauteur (DESIGN.md) : ligne de réglage ou d'entrée de menu (38, 50 avec une précision) et bouton, icône
/// ou ligne de liste (32). Les écrans qui doivent s'aligner sur eux les prennent d'ici, pas en dur.
pub const ROW_H: f32 = 38.0;
pub const ROW_H_HINT: f32 = 50.0;
pub const BTN_H: f32 = 32.0;
/// Anneau de focus clavier : variante lisible de l'accent, au moins 3:1 sur toutes les surfaces.
#[allow(non_snake_case)]
pub fn FOCUS() -> Color32 {
    ACCENT_TEXT()
}

// ---------- Contraste (WCAG 2.x) ----------

fn channel(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color32) -> f32 {
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// Rapport de contraste entre deux couleurs opaques (1 à 21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Rapproche `c` du blanc (fond sombre) ou du noir (fond clair) jusqu'à 4,5:1 sur `bg` (pseudos du chat).
pub fn readable_on(c: Color32, bg: Color32) -> Color32 {
    if contrast(c, bg) >= 4.5 {
        return c;
    }
    let target = if luminance(bg) < 0.18 {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let mix = |k: f32| {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
        Color32::from_rgb(
            m(c.r(), target.r()),
            m(c.g(), target.g()),
            m(c.b(), target.b()),
        )
    };
    (1..=20)
        .map(|i| mix(i as f32 * 0.05))
        .find(|m| contrast(*m, bg) >= 4.5)
        .unwrap_or(target)
}

thread_local! {
    /// Libellé de la ligne de réglage dont on crée les contrôles (voir `control_row`) : sert à nommer pour le lecteur
    /// d'écran les segments et curseurs, qui sinon seraient lus sans contexte (« 20 s » au lieu d'« Actualisation : 20 s »).
    static ROW_LABEL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn row_label() -> Option<String> {
    ROW_LABEL.with(|l| l.borrow().clone())
}

/// Donne un nom lu par le lecteur d'écran (Narrateur, NVDA) à un élément qui n'a pas de texte visible.
pub fn a11y_label(resp: &Response, label: &str) {
    resp.ctx
        .accesskit_node_builder(resp.id, |n| n.set_label(label));
}

/// Vrai quand la dernière action était au clavier : l'anneau de focus ne s'affiche qu'alors (comme `:focus-visible`).
static FOCUS_VISIBLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn focus_visible() -> bool {
    FOCUS_VISIBLE.load(Ordering::Relaxed) || FOCUS_STRONG.load(Ordering::Relaxed)
}

/// À appeler au début de chaque image : une touche de navigation montre le focus, un clic le cache.
/// Le liseré d'accent des boutons egui « actifs » suit la même règle, pour qu'un bouton cliqué ne reste pas surligné.
pub fn track_focus_modality(ctx: &egui::Context) {
    use egui::Key;
    let (keyboard, pointer) = ctx.input(|i| {
        let keyboard = i.events.iter().any(|e| {
            matches!(e, egui::Event::Key { key, pressed: true, .. } if matches!(key,
                Key::Tab | Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
                    | Key::Enter | Key::Space | Key::Home | Key::End))
        });
        (keyboard, i.pointer.any_pressed())
    });
    let now = if pointer {
        false
    } else if keyboard {
        true
    } else {
        return;
    };
    if FOCUS_VISIBLE.swap(now, Ordering::Relaxed) != now {
        let hover = BORDER_HOVER();
        ctx.global_style_mut(|s| {
            s.visuals.widgets.active.bg_stroke =
                Stroke::new(1.0, if now { ACCENT_LINE() } else { hover });
        });
    }
}

/// Le focus a changé à cette image (touche de navigation, clic, demande de focus).
static FOCUS_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// À appeler au début de chaque image (après `track_focus_modality`) : repère un changement de focus.
pub fn track_focus_change(ctx: &egui::Context) {
    let id = ctx.memory(|m| m.focused());
    let key = egui::Id::unique("last_focused_widget");
    let prev = ctx.data(|d| d.get_temp::<Option<egui::Id>>(key)).flatten();
    FOCUS_CHANGED.store(id.is_some() && id != prev, Ordering::Relaxed);
    ctx.data_mut(|d| d.insert_temp(key, id));
}

/// Zone défilante dont les barres sont invisibles au repos. Elles apparaissent en fondu pendant un défilement (molette,
/// glisser, pavé tactile, clavier) et tant que le curseur est sur leur piste, puis s'effacent après `SCROLLBAR_IDLE_SECS`
/// d'inactivité. La gouttière réservée reste en place et la piste de détection garde la même géométrie, visible ou non :
/// seule l'opacité varie. `horizontal` : la zone défile de côté (piste en bas) au lieu de verticalement (piste à droite).
pub fn auto_hide_scroll<R>(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug + Copy,
    horizontal: bool,
    area: egui::ScrollArea,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::containers::scroll_area::ScrollAreaOutput<R> {
    const SCROLLBAR_IDLE_SECS: f64 = 0.9;
    const SCROLLBAR_FADE_SECS: f32 = 0.25;
    const SCROLLBAR_SHOWN_OPACITY: f32 = 0.65;
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let seen_id = egui::Id::unique(("autohide_scroll_seen", salt));
    let (last_offset, last_move, held_before) = ctx
        .data(|d| d.get_temp::<(Vec2, f64, bool)>(seen_id))
        .unwrap_or((Vec2::ZERO, f64::NEG_INFINITY, false));
    let visible = held_before || now - last_move < SCROLLBAR_IDLE_SECS;
    // Un seul niveau d'opacité animé (0 au repos, 0,65 au défilement, 1 sur la piste) : apparition, disparition et survol
    // partagent le même fondu.
    let target = if held_before {
        1.0
    } else if visible {
        SCROLLBAR_SHOWN_OPACITY
    } else {
        0.0
    };
    let level = ctx.animate_value_with_time(
        egui::Id::unique(("autohide_scroll_level", salt)),
        target,
        SCROLLBAR_FADE_SECS,
    );

    let saved = ui.spacing().scroll;
    {
        // Constante : une zone imbriquée hériterait sinon de l'opacité déjà réduite de la zone parente.
        let s = &mut ui.spacing_mut().scroll;
        s.dormant_handle_opacity = level;
        s.active_handle_opacity = level;
        s.interact_handle_opacity = level;
    }
    let out = area.id_salt(salt).show(ui, add);
    let scroll = ui.spacing().scroll;
    ui.spacing_mut().scroll = saved;

    // Piste complète (toute la longueur, bande de la largeur de la barre au bord de la zone), même sans barre visible.
    let inner = out.inner_rect;
    let overflow = if horizontal {
        out.content_size.x > inner.width() + 1.0
    } else {
        out.content_size.y > inner.height() + 1.0
    };
    let gutter = scroll.allocated_width();
    let band = scroll.bar_width + 2.0 * scroll.bar_outer_margin;
    let track = if !overflow {
        Rect::NOTHING
    } else if horizontal {
        let bottom = inner.bottom() + gutter;
        Rect::from_min_max(
            Pos2::new(inner.left(), bottom - band),
            Pos2::new(inner.right(), bottom),
        )
    } else {
        let right = inner.right() + gutter;
        Rect::from_min_max(
            Pos2::new(right - band, inner.top()),
            Pos2::new(right, inner.bottom()),
        )
    };
    let (pos, down, pressed) = ctx.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.primary_down(),
            i.pointer.primary_pressed(),
        )
    });
    let on_track = pos.is_some_and(|p| track.contains(p));
    let drag_id = egui::Id::unique(("autohide_scroll_drag", salt));
    let was_drag = ctx.data(|d| d.get_temp::<bool>(drag_id)).unwrap_or(false);
    let dragging = down && (was_drag || (pressed && on_track));
    ctx.data_mut(|d| d.insert_temp(drag_id, dragging));
    let held = on_track || dragging;

    let offset = out.state.clamped_offset();
    let moved = (offset - last_offset).abs().max_elem() > 0.01;
    let last_move = if moved || held { now } else { last_move };
    ctx.data_mut(|d| d.insert_temp(seen_id, (offset, last_move, held)));
    if moved || visible || held {
        let remaining = (SCROLLBAR_IDLE_SECS - (now - last_move)).max(0.0);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining + 0.02));
    }
    out
}

/// Indicateur « il y a de la suite en bas » d'une zone défilante verticale : fondu du fond sur le bord bas et petit bouton
/// rond avec une flèche vers le bas, affichés seulement quand du contenu est caché sous le bord. À appeler juste après
/// `auto_hide_scroll`. Renvoie `true` au clic : l'appelant fait alors défiler sa zone (`scroll_with_delta` dans son contenu).
pub fn scroll_down_hint<R>(
    ui: &mut egui::Ui,
    out: &egui::containers::scroll_area::ScrollAreaOutput<R>,
) -> bool {
    let hidden = out.content_size.y - out.inner_rect.height() - out.state.clamped_offset().y;
    if hidden <= 2.0 {
        return false;
    }
    // Fondu du fond sous le bouton : la ligne coupée s'efface proprement au lieu de passer derrière lui.
    let inner = out.inner_rect;
    for i in 0..9 {
        let k = (i as f32 + 1.0) / 9.0;
        let y = inner.bottom() - 36.0 + i as f32 * 4.0;
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 4.0)),
            CornerRadius::ZERO,
            BG().gamma_multiply(k * k),
        );
    }
    let c = Pos2::new(inner.center().x, inner.bottom() - 16.0);
    let r = Rect::from_center_size(c, Vec2::splat(26.0));
    let resp = ui.interact(r, out.id.with("scroll_down_hint"), Sense::click());
    let fill = if resp.hovered() {
        SURFACE_3()
    } else {
        SURFACE_2()
    };
    let p = ui.painter();
    p.circle_filled(c, 13.0, fill);
    p.circle_stroke(c, 13.0, Stroke::new(1.0, BORDER_HOVER()));
    let st = Stroke::new(1.8, ACCENT_GFX());
    p.line_segment([c + Vec2::new(-4.5, -2.0), c + Vec2::new(0.0, 2.5)], st);
    p.line_segment([c + Vec2::new(0.0, 2.5), c + Vec2::new(4.5, -2.0)], st);
    if resp.has_focus() && focus_visible() {
        p.circle_stroke(c, 16.0, Stroke::new(focus_width(), FOCUS()));
    }
    resp.clicked()
}

/// Navigation au clavier : à appeler au début du contenu d'une zone défilante. Quand le focus vient de changer après
/// une touche de navigation (Tab, flèches…) et que l'élément focalisé est dans cette zone mais hors de la vue, la zone
/// défile pour le montrer (egui ne le fait pas seul). Sans effet après un clic : la souris défile comme elle veut.
pub fn follow_keyboard_focus(ui: &egui::Ui) {
    if !FOCUS_VISIBLE.load(Ordering::Relaxed) || !FOCUS_CHANGED.load(Ordering::Relaxed) {
        return;
    }
    let ctx = ui.ctx();
    let Some(id) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(r) = ctx.read_response(id) else {
        return;
    };
    let area = ui.max_rect();
    if r.layer_id != ui.layer_id()
        || !area.x_range().contains(r.rect.center().x)
        || r.rect.center().y < area.top()
    {
        return; // l'élément n'est pas dans cette zone défilante
    }
    ui.scroll_to_rect(r.rect, None);
}

/// À appeler à la fin de chaque image : un clic place aussi le focus sur l'élément cliqué (comme dans Windows),
/// pour que Tab reparte de là et non du début de la page. `enabled = false` sur la page du lecteur, où Espace et
/// les flèches sont des raccourcis et ne doivent pas activer le dernier bouton cliqué.
pub fn focus_follows_click(ctx: &egui::Context, enabled: bool) {
    if !enabled {
        return;
    }
    if let Some(id) = ctx.interaction_snapshot(|s| s.clicked) {
        ctx.memory_mut(|m| {
            if !m.has_focus(id) {
                m.request_focus(id);
            }
        });
    }
}

/// Anneau de focus clavier autour d'un composant dessiné à la main (les boutons egui ont déjà le leur).
/// Affiché seulement après une touche de navigation (pas après un clic de souris).
pub fn focus_ring(ui: &egui::Ui, resp: &Response, rect: Rect, radius: u8) {
    if resp.has_focus() && focus_visible() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            CornerRadius::same(radius.saturating_add(2)),
            Stroke::new(focus_width(), FOCUS()),
            egui::StrokeKind::Outside,
        );
        if FOCUS_STRONG.load(Ordering::Relaxed) {
            // Double contour : un filet de la couleur du fond entre l'anneau et le composant, lisible sur tout fond.
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(radius),
                Stroke::new(1.5, BG()),
                egui::StrokeKind::Outside,
            );
        }
    }
}

/// Pendant sa vie, les couleurs sont celles du thème Sombre : pour les surcouches posées sur la
/// vidéo (toujours noire), qui doivent rester lisibles quel que soit le thème.
pub struct DarkScope(u8);

impl DarkScope {
    pub fn enter(ui: &mut egui::Ui) -> Self {
        let g = Self(CURRENT.swap(0, Ordering::Relaxed));
        *ui.visuals_mut() = visuals_of(&palette_of(0), false);
        g
    }
}

impl Drop for DarkScope {
    fn drop(&mut self) {
        CURRENT.store(self.0, Ordering::Relaxed);
    }
}

/// Progression douce (0 à 1) de l'effet de survol d'un élément : sert à fondre remplissage, icône et texte.
pub fn hover_k(ui: &egui::Ui, id: egui::Id, hovered: bool) -> f32 {
    motion::eased(ui.ctx(), id.with("hover_k"), hovered, motion::HOVER)
}

/// Voile sombre d'un composant pressé : retour dès l'appui, retiré si le pointeur sort avant le relâchement.
pub fn paint_pressed(ui: &egui::Ui, resp: &Response, rect: Rect, radius: u8) {
    if resp.is_pointer_button_down_on() && resp.contains_pointer() && ui.is_enabled() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(radius),
            Color32::from_black_alpha(46),
        );
    }
}

/// Contour d'un bouton secondaire (le survol remplit l'intérieur d'accent, sans contour).
fn hover_stroke(hov: bool) -> Stroke {
    if hov {
        Stroke::new(1.0, BORDER_HOVER())
    } else {
        Stroke::new(1.0, BORDER())
    }
}

/// Échelle d'espacement unique (en points) : 4 dans un groupe serré, 8 entre éléments liés, 12 entre blocs,
/// 16 entre sections, 24 entre grandes zones. Pas d'autres valeurs isolées dans les écrans.
pub const SP_XS: f32 = 4.0;
pub const SP_S: f32 = 8.0;
pub const SP_M: f32 = 12.0;
pub const SP_L: f32 = 16.0;

pub const RADIUS_CARD: u8 = 16;
pub const RADIUS_INNER: u8 = 10;
pub const RADIUS_PILL: u8 = 255;

pub fn hairline() -> Stroke {
    Stroke::new(1.0, BORDER())
}

/// Famille de police semi-grasse (noms de chaîne).
fn name_family() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

/// Police d'un nom de chaîne : un cran plus gras que le texte courant.
pub fn name_font(size: f32) -> FontId {
    FontId::new(size, name_family())
}

/// Applique un thème (0 = noir « Dark », 1 = bleu nuit « Sombre », 2 = Clair) à toute l'application.
/// Police Inter (licence OFL, voir `assets/fonts`) en tête de la famille proportionnelle ;
/// les polices par défaut restent en repli pour les caractères qu'elle ne couvre pas.
pub fn install_fonts(ctx: &egui::Context) {
    install_fonts_with(ctx, Vec::new());
}

/// Comme `install_fonts`, avec des polices de repli en plus après Inter (nom et données, réglages compris) :
/// sert au chinois, au japonais et au coréen, absents d'Inter.
pub fn install_fonts_with(ctx: &egui::Context, extra: Vec<(String, egui::FontData)>) {
    let mut fonts = egui::FontDefinitions::default();
    for (i, (name, data)) in extra.into_iter().enumerate() {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(data));
        // Texte courant : juste après Inter (inséré ensuite en tête) ; police à chasse fixe : en dernier recours.
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.insert(i.min(list.len()), name.clone());
        }
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            list.push(name);
        }
    }
    fonts.font_data.insert(
        "Inter".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Regular.otf"
        ))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "Inter".to_owned());
    }
    // Graisse « semi-grasse » (Inter SemiBold) pour les noms de chaîne ; mêmes polices de repli que le texte courant.
    fonts.font_data.insert(
        "InterSemiBold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-SemiBold.otf"
        ))),
    );
    let mut semi = vec!["InterSemiBold".to_owned()];
    if let Some(family) = fonts.families.get(&egui::FontFamily::Proportional) {
        semi.extend(family.iter().cloned());
    }
    fonts.families.insert(name_family(), semi);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, theme: u8) {
    let theme = resolve(ctx, theme);
    CURRENT.store(theme, Ordering::Relaxed);
    let light = theme == 2;
    let v = visuals_of(&palette_of(theme), light);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if light {
        egui::ThemePreference::Light
    } else {
        egui::ThemePreference::Dark
    });
    let pressed = if focus_visible() {
        ACCENT_LINE()
    } else {
        palette_of(theme).border_hover
    };
    ctx.global_style_mut(|s| {
        s.visuals.widgets.active.bg_stroke = Stroke::new(1.0, pressed);
        // Comportement d'application native : aucun texte statique n'est sélectionnable (ni curseur « texte » au survol).
        // Exceptions réactivées localement : champs de saisie (toujours sélectionnables) et messages du chat.
        s.interaction.selectable_labels = false;
        // Défilements automatiques (vers « Avancé », « Personnaliser »…) : même durée (0,35 s) que l'ouverture et la fermeture des zones dépliables.
        s.scroll_animation = egui::style::ScrollAnimation::duration(0.35);
        s.spacing.item_spacing = Vec2::new(10.0, 8.0);
        s.spacing.button_padding = Vec2::new(14.0, 7.0);
        s.spacing.interact_size.y = 30.0;
        // Barre de défilement fine et discrète : sans fond, poignée grise claire de 3 px (6 px au survol), posée dans une
        // gouttière réservée à droite pour ne jamais recouvrir le contenu.
        s.spacing.scroll = egui::style::ScrollStyle {
            floating: true,
            bar_width: 6.0,
            floating_width: 3.0,
            floating_allocated_width: 10.0,
            handle_min_length: 36.0,
            bar_inner_margin: 2.0,
            bar_outer_margin: 2.0,
            foreground_color: true,
            dormant_background_opacity: 0.0,
            active_background_opacity: 0.0,
            interact_background_opacity: 0.0,
            dormant_handle_opacity: 0.4,
            active_handle_opacity: 0.65,
            interact_handle_opacity: 1.0,
            ..egui::style::ScrollStyle::floating()
        };
    });
}

fn visuals_of(p: &Palette, light: bool) -> egui::Visuals {
    let mut v = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    let hair = Stroke::new(1.0, p.border);
    v.panel_fill = p.bg;
    v.window_fill = p.s1;
    v.extreme_bg_color = p.s2;
    v.faint_bg_color = p.s1;
    v.code_bg_color = p.s2;
    v.window_stroke = hair;
    v.window_corner_radius = CornerRadius::same(RADIUS_CARD);
    v.menu_corner_radius = CornerRadius::same(RADIUS_INNER);
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.hyperlink_color = accent_text_for(light);
    // Texte sélectionné : fond d'accentuation, texte blanc ou noir selon le meilleur contraste.
    let sel_bg = ACCENT();
    let sel_text = if contrast(Color32::WHITE, sel_bg) >= contrast(Color32::BLACK, sel_bg) {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    v.selection.bg_fill = sel_bg;
    v.selection.stroke = Stroke::new(
        if FOCUS_STRONG.load(Ordering::Relaxed) {
            focus_width()
        } else {
            1.0
        },
        sel_text,
    );
    let r = CornerRadius::same(RADIUS_INNER);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.s1;
    w.noninteractive.weak_bg_fill = p.s1;
    w.noninteractive.bg_stroke = hair;
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.s2;
    w.inactive.weak_bg_fill = p.s2;
    w.inactive.bg_stroke = hair;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.s3;
    w.hovered.weak_bg_fill = p.s3;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border_hover);
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = r;
    w.active.bg_fill = p.s3;
    w.active.weak_bg_fill = p.s3;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT_LINE());
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = r;
    w.open.bg_fill = p.s3;
    w.open.weak_bg_fill = p.s3;
    w.open.bg_stroke = hair;
    w.open.corner_radius = r;
    v
}

// ---------- Conteneurs ----------

/// Card : surface-1, bordure hairline, rayon 16, marge 20.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(SURFACE_1())
        .stroke(hairline())
        .corner_radius(CornerRadius::same(RADIUS_CARD))
        .inner_margin(20.0)
        .show(ui, add)
}

/// Titre de card : 13 px, grisé.
pub fn card_title(ui: &mut egui::Ui, text: &str) -> Response {
    ui.label(RichText::new(text).size(13.0).color(MUTED()))
}

pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(22.0).strong().color(TEXT()));
}

// ---------- Boutons ----------

/// Bouton à bascule (par exemple « Chat ») : plus grand que les boutons ordinaires ; actif = fond
/// surface-3 et liseré d'accent, inactif = surface-2 ; le texte reste toujours bien lisible.
pub fn toggle_button(ui: &mut egui::Ui, label: &str, on: bool) -> Response {
    let b = egui::Button::new(RichText::new(label).size(14.5).color(if on {
        Color32::WHITE
    } else {
        TEXT()
    }))
    .min_size(Vec2::new(72.0, 34.0))
    .fill(if on { ACCENT() } else { SURFACE_2() })
    .stroke(if on {
        Stroke::NONE
    } else {
        Stroke::new(1.0, BORDER_HOVER())
    })
    .corner_radius(CornerRadius::same(RADIUS_INNER));
    let resp = ui.add(b);
    focus_ring(ui, &resp, resp.rect, RADIUS_INNER);
    resp
}

/// Contrôle segmenté : conteneur surface-2, segment actif surface-3.
/// Dans un parent de droite à gauche (lignes de réglage), il est d'abord mesuré dans un passage invisible, puis dessiné
/// de gauche à droite dans exactement cette place : l'ordre de tabulation suit ainsi l'ordre visuel des segments
/// (avant, les segments étaient créés à l'envers et Tab les parcourait de droite à gauche).
pub fn segmented(ui: &mut egui::Ui, current: &mut u8, labels: &[&str]) -> bool {
    segmented_tips(ui, current, labels, &[])
}

/// Comme `segmented`, avec une infobulle par choix (`tips[i]`, vide ou absente = aucune).
pub fn segmented_tips(ui: &mut egui::Ui, current: &mut u8, labels: &[&str], tips: &[&str]) -> bool {
    let ltr = egui::Layout::left_to_right(egui::Align::Center);
    if !ui.layout().prefer_right_to_left() {
        return segmented_ltr(ui, current, labels, tips);
    }
    let mut probe = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("segmented_probe")
            .sizing_pass()
            .invisible()
            .layout(ltr),
    );
    let mut copy = *current;
    segmented_ltr(&mut probe, &mut copy, labels, tips);
    let size = probe.min_rect().size();
    ui.allocate_ui_with_layout(size, ltr, |ui| segmented_ltr(ui, current, labels, tips))
        .inner
}

fn segmented_ltr(ui: &mut egui::Ui, current: &mut u8, labels: &[&str], tips: &[&str]) -> bool {
    let mut changed = false;
    egui::Frame::new()
        .fill(SURFACE_2())
        .stroke(hairline())
        .corner_radius(CornerRadius::same(RADIUS_INNER))
        .inner_margin(3.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, l) in labels.iter().enumerate() {
                    let active = *current as usize == i;
                    let filled = active;
                    let b = egui::Button::new(RichText::new(*l).color(if filled {
                        Color32::WHITE
                    } else if active {
                        TEXT()
                    } else {
                        MUTED()
                    }))
                    .fill(if filled {
                        ACCENT()
                    } else if active {
                        SURFACE_3()
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(if active && !filled {
                        Stroke::new(1.2, ACCENT_LINE())
                    } else {
                        Stroke::NONE
                    })
                    .corner_radius(CornerRadius::same(8));
                    let mut resp = ui.add(b);
                    let k = hover_k(ui, resp.id, resp.hovered() && !active);
                    if k > 0.0 {
                        // Survol : intérieur rempli (en fondu) de la couleur d'accent, libellé vers le blanc.
                        ui.painter().rect_filled(
                            resp.rect,
                            CornerRadius::same(8),
                            ACCENT().gamma_multiply(k),
                        );
                        let font = egui::TextStyle::Button.resolve(ui.style());
                        ui.painter().text(
                            resp.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            *l,
                            font,
                            MUTED().lerp_to_gamma(Color32::WHITE, k),
                        );
                    }
                    if let Some(tip) = tips.get(i).filter(|t| !t.is_empty()) {
                        resp = show_tip(resp, tip);
                    }
                    focus_ring(ui, &resp, resp.rect, 8);
                    let name = match row_label() {
                        Some(row) => format!("{row} : {l}"),
                        None => l.to_string(),
                    };
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::RadioButton,
                            ui.is_enabled(),
                            active,
                            &name,
                        )
                    });
                    if resp.clicked() && !active {
                        *current = i as u8;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Zone qui s'ouvre et se ferme en douceur (hauteur et opacité animées sur `motion::SECTION`). Le contenu n'est dessiné
/// que tant que la zone est (en partie) ouverte ; sa hauteur est mémorisée pour animer l'ouverture suivante.
/// Renvoie l'avancement (0 fermée, 1 ouverte).
pub fn animated_section(
    ui: &mut egui::Ui,
    id: &str,
    open: bool,
    add: impl FnOnce(&mut egui::Ui),
) -> f32 {
    let ctx = ui.ctx().clone();
    let key = egui::Id::unique(("animated_section", id));
    // Dépliage = déplacement : en animations réduites, la zone apparaît ou disparaît d'un coup.
    let t = motion::eased(&ctx, key, open, move_secs(&ctx, motion::SECTION));
    if t <= 0.0 {
        return t;
    }
    let height_key = key.with("height");
    if t >= 1.0 {
        let top = ui.cursor().top();
        add(ui);
        let h = ui.cursor().top() - top;
        ctx.data_mut(|d| d.insert_temp(height_key, h));
        return t;
    }
    let full = ctx.data(|d| d.get_temp::<f32>(height_key)).unwrap_or(260.0);
    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), full * t));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), 4000.0),
    )));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.set_opacity(t);
    add(&mut child);
    let measured = child.min_rect().height();
    ctx.data_mut(|d| d.insert_temp(height_key, measured));
    ui.allocate_rect(rect, Sense::hover());
    t
}

/// En-tête d'une zone dépliable, dessiné à la main : flèche qui pivote (sur `motion::SECTION`, comme la zone dépliée par
/// `animated_section`), libellé qui vire à la couleur d'accent au survol, anneau de focus au clavier. Pas d'animation
/// propre à egui, donc ouverture et flèche restent synchrones avec la zone. Le clic est à traiter par l'appelant.
pub fn disclosure_header(ui: &mut egui::Ui, id: &str, label: &str, open: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), BTN_H), Sense::click());
    let key = egui::Id::unique(("disclosure_header", id));
    // Flèche : 0 = vers la droite (fermé), 1 = vers le bas (ouvert). Un pivot est un déplacement : coupé en mode réduit.
    let turn = motion::eased(
        ui.ctx(),
        key.with("turn"),
        open,
        move_secs(ui.ctx(), motion::SECTION),
    );
    let k = hover_k(ui, key, resp.hovered());
    let color = TEXT().lerp_to_gamma(ACCENT_TEXT(), k);
    let p = ui.painter();
    let c = Pos2::new(rect.left() + 12.0, rect.center().y);
    let angle = turn * std::f32::consts::FRAC_PI_2;
    let (sin, cos) = angle.sin_cos();
    let pts: Vec<Pos2> = [
        Vec2::new(-2.5, -4.5),
        Vec2::new(3.5, 0.0),
        Vec2::new(-2.5, 4.5),
    ]
    .iter()
    .map(|v| c + Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos))
    .collect();
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    p.text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        color,
    );
    paint_pressed(ui, &resp, rect, RADIUS_INNER);
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    resp
}

/// Icône d'un thème : 0 = lune (noir, « Dark »), 1 = lune et étoile (bleu nuit, « Sombre »), 2 = soleil (Clair),
/// 3 = écran (Système).
fn paint_theme_icon(p: &egui::Painter, c: Pos2, kind: usize, fg: Color32, bg: Color32) {
    let st = Stroke::new(1.5, fg);
    match kind {
        0 | 1 => {
            p.circle_filled(c, 6.0, fg);
            p.circle_filled(c + Vec2::new(3.2, -2.8), 5.2, bg);
            if kind == 1 {
                p.circle_filled(c + Vec2::new(4.6, -3.4), 1.3, fg);
            }
        }
        2 => {
            p.circle_stroke(c, 3.4, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 5.6, c + d * 7.4], st);
            }
        }
        _ => {
            let r = Rect::from_center_size(c - Vec2::new(0.0, 1.0), Vec2::new(14.0, 9.5));
            p.rect_stroke(r, CornerRadius::same(2), st, egui::StrokeKind::Inside);
            p.line_segment([c + Vec2::new(-3.0, 6.0), c + Vec2::new(3.0, 6.0)], st);
        }
    }
}

/// Sélecteur du thème : comme `segmented`, avec une icône devant chaque option.
pub fn theme_segmented(ui: &mut egui::Ui, current: &mut u8, labels: &[&str]) -> bool {
    let mut changed = false;
    egui::Frame::new()
        .fill(SURFACE_2())
        .stroke(hairline())
        .corner_radius(CornerRadius::same(RADIUS_INNER))
        .inner_margin(3.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, l) in labels.iter().enumerate() {
                    let active = *current as usize == i;
                    let font = FontId::proportional(14.0);
                    let galley = ui.painter().layout_no_wrap(l.to_string(), font, MUTED());
                    let size = Vec2::new(12.0 + 16.0 + 8.0 + galley.size().x + 12.0, 30.0);
                    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                    let k = if active {
                        1.0
                    } else {
                        hover_k(ui, resp.id, resp.hovered())
                    };
                    let fg = MUTED().lerp_to_gamma(Color32::WHITE, k);
                    let galley =
                        ui.painter()
                            .layout_no_wrap(l.to_string(), FontId::proportional(14.0), fg);
                    let p = ui.painter();
                    p.rect_filled(rect, CornerRadius::same(8), ACCENT().gamma_multiply(k));
                    let bg = SURFACE_2().lerp_to_gamma(ACCENT(), k);
                    paint_theme_icon(p, Pos2::new(rect.left() + 20.0, rect.center().y), i, fg, bg);
                    p.galley(
                        Pos2::new(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0),
                        galley,
                        fg,
                    );
                    focus_ring(ui, &resp, rect, 8);
                    let name = match row_label() {
                        Some(row) => format!("{row} : {l}"),
                        None => l.to_string(),
                    };
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::RadioButton,
                            ui.is_enabled(),
                            active,
                            &name,
                        )
                    });
                    if resp.clicked() && !active {
                        *current = i as u8;
                        changed = true;
                    }
                }
            });
        });
    changed
}

// ---------- Badges ----------

/// Pill : texte 11–12 px, point facultatif.
pub fn pill(
    ui: &mut egui::Ui,
    dot: Option<Color32>,
    text: &str,
    fg: Color32,
    bg: Color32,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.5), fg);
    let dot_w = if dot.is_some() { 12.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 16.0 + dot_w, galley.size().y + 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    paint_pill_in(ui, rect, resp.id, dot, galley, bg);
    resp
}

/// Pastille de valeur numérique d'une largeur fixe (celle de `widest`, ex. « 255 »), texte centré : les curseurs qui
/// la suivent restent alignés d'une ligne à l'autre, quel que soit le nombre de chiffres.
pub fn value_pill(
    ui: &mut egui::Ui,
    text: &str,
    widest: &str,
    fg: Color32,
    bg: Color32,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.5), fg);
    let max_w = ui
        .painter()
        .layout_no_wrap(widest.to_string(), FontId::proportional(11.5), fg)
        .size()
        .x
        .max(galley.size().x);
    let size = Vec2::new(max_w + 16.0, galley.size().y + 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_PILL), bg);
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        Color32::WHITE,
    );
    resp
}

fn paint_pill_in(
    ui: &egui::Ui,
    rect: Rect,
    id: egui::Id,
    dot: Option<Color32>,
    galley: std::sync::Arc<egui::Galley>,
    bg: Color32,
) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(RADIUS_PILL), bg);
    let mut x = rect.left() + 8.0;
    if let Some(c) = dot {
        // Le point pulse doucement à son apparition (quelques fois seulement, sauf si les animations sont réduites), puis
        // reste fixe : une pulsation sans fin réveillerait l'application en permanence.
        let mut c = c;
        if moves(ui.ctx()) {
            let now = ui.input(|i| i.time);
            let start = ui.ctx().data_mut(|d| {
                *d.get_temp_mut_or_insert_with::<f64>(id.with("pulse_start"), || now)
            });
            let age = now - start;
            if age < motion::PULSE_SECS * motion::PULSE_CYCLES {
                let phase = (age / motion::PULSE_SECS) as f32 * std::f32::consts::TAU;
                c = c.gamma_multiply(1.0 - 0.35 * (0.5 - 0.5 * phase.cos()));
                ui.ctx().request_repaint();
            }
        }
        p.circle_filled(Pos2::new(x + 3.5, rect.center().y), 3.5, c);
        x += 12.0;
    }
    p.galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        Color32::WHITE,
    );
}

pub fn live_pill(ui: &mut egui::Ui) -> Response {
    pill(
        ui,
        Some(POSITIVE()),
        "LIVE",
        Color32::WHITE,
        Color32::from_black_alpha(215),
    )
}

// ---------- Éléments de liste ----------

/// Avatar rond : la photo de la chaîne si elle est chargée, sinon l'initiale du nom.
pub fn avatar(
    ui: &egui::Ui,
    center: Pos2,
    radius: f32,
    name: &str,
    photo: Option<egui::TextureId>,
) {
    if let Some(id) = photo {
        const SEGMENTS: u32 = 48;
        let mut mesh = egui::Mesh::with_texture(id);
        let uv_at = |p: Pos2| {
            Pos2::new(
                0.5 + (p.x - center.x) / (2.0 * radius),
                0.5 + (p.y - center.y) / (2.0 * radius),
            )
        };
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center,
            uv: uv_at(center),
            color: Color32::WHITE,
        });
        for i in 0..SEGMENTS {
            let a = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let p = center + Vec2::new(a.cos(), a.sin()) * radius;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv: uv_at(p),
                color: Color32::WHITE,
            });
        }
        for i in 0..SEGMENTS {
            mesh.indices
                .extend_from_slice(&[0, 1 + i, 1 + (i + 1) % SEGMENTS]);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
        ui.painter().circle_stroke(center, radius, hairline());
        return;
    }
    ui.painter().circle_filled(center, radius, SURFACE_3());
    ui.painter().circle_stroke(center, radius, hairline());
    let initial = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        initial,
        FontId::proportional(radius * 0.95),
        MUTED(),
    );
}

/// Icônes dessinées (traits fins, 16 px) des entrées de navigation et des boutons.
#[derive(Clone, Copy, PartialEq)]
pub enum NavIcon {
    Home,
    Settings,
    Back,
    Stop,
    Play,
    /// Utilisé par l'outil d'administration des clés (qui partage ce fichier).
    #[allow(dead_code)]
    Plus,
    Check,
    Close,
    Login,
    Logout,
    Refresh,
    Trash,
    Folder,
    Download,
    Pip,
    /// Panneau latéral (bouton réduire / agrandir la barre).
    Panel,
    Search,
    /// Plume (légèreté).
    Feather,
    /// Bouton marche / arrêt (redémarrer).
    Power,
    /// Étoile (favori) : contour, ou pleine quand la chaîne est en favori.
    Star,
    StarFilled,
    /// Étoile avec un petit plus : ajouter aux favoris.
    StarPlus,
    /// Bulle de discussion (chat).
    Chat,
    /// Accessibilité : personne dans un cercle.
    Access,
    /// Apparence : disque dont la moitié droite est pleine (thème).
    Appearance,
    /// Général : globe (méridien et équateur).
    General,
    /// Notifications : cloche.
    Bell,
    /// Raccourcis clavier : clavier.
    Keyboard,
    /// À propos : « i » dans un cercle.
    Info,
    /// Multistream : quatre carrés arrondis (grille 2 x 2).
    Grid,
    /// Haut-parleur avec deux ondes (son actif) ou barré d'une croix (son coupé).
    Sound,
    SoundOff,
    /// Punaise : fenêtre gardée au premier plan (barrée d'un trait : plus au premier plan).
    Pin,
    PinOff,
}

/// Icône dessinée à l'échelle `scale` autour de son centre `c` (traits compris). À 1 : `paint_nav_icon`.
pub fn paint_nav_icon_scaled(
    p: &egui::Painter,
    c: Pos2,
    icon: NavIcon,
    color: Color32,
    scale: f32,
) {
    if (scale - 1.0).abs() < f32::EPSILON {
        paint_nav_icon(p, c, icon, color);
        return;
    }
    let layer = p.layer_id();
    let start = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    paint_nav_icon(p, c, icon, color);
    let end = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let tr = egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale), scale);
    p.ctx()
        .graphics_mut(|g| g.entry(layer).transform_range(start, end, tr));
}

pub fn paint_nav_icon(p: &egui::Painter, c: Pos2, icon: NavIcon, color: Color32) {
    let st = Stroke::new(1.5, color);
    let v = |x: f32, y: f32| Pos2::new(c.x + x, c.y + y);
    let line = |pts: Vec<Pos2>| {
        p.add(egui::Shape::line(pts, st));
    };
    match icon {
        NavIcon::Home => {
            // Toit + corps de la maison.
            line(vec![v(-7.5, -0.5), v(0.0, -7.0), v(7.5, -0.5)]);
            line(vec![v(-5.0, -2.5), v(-5.0, 6.5), v(5.0, 6.5), v(5.0, -2.5)]);
            p.rect_stroke(
                Rect::from_center_size(v(0.0, 3.5), Vec2::new(3.4, 6.0)),
                CornerRadius::same(1),
                st,
                egui::StrokeKind::Middle,
            );
        }
        NavIcon::Settings => {
            // Roue dentée : anneau, huit dents, moyeu.
            p.circle_stroke(c, 4.6, st);
            p.circle_stroke(c, 1.7, st);
            let teeth = Stroke::new(2.6, color);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 5.6, c + d * 7.6], teeth);
            }
        }
        NavIcon::Back => {
            line(vec![v(7.0, 0.0), v(-7.0, 0.0)]);
            line(vec![v(-2.0, -5.0), v(-7.0, 0.0), v(-2.0, 5.0)]);
        }
        NavIcon::Stop => {
            p.rect_filled(
                Rect::from_center_size(c, Vec2::splat(9.0)),
                CornerRadius::same(2),
                color,
            );
        }
        NavIcon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![v(-3.5, -6.0), v(-3.5, 6.0), v(6.5, 0.0)],
                color,
                Stroke::NONE,
            ));
        }
        NavIcon::Plus => {
            line(vec![v(-6.0, 0.0), v(6.0, 0.0)]);
            line(vec![v(0.0, -6.0), v(0.0, 6.0)]);
        }
        NavIcon::Check => line(vec![v(-5.5, 0.5), v(-1.8, 4.5), v(5.5, -4.5)]),
        NavIcon::Close => {
            line(vec![v(-5.0, -5.0), v(5.0, 5.0)]);
            line(vec![v(-5.0, 5.0), v(5.0, -5.0)]);
        }
        NavIcon::Login => {
            // Silhouette : tête + épaules.
            p.circle_stroke(v(0.0, -3.5), 3.2, st);
            let shoulders: Vec<Pos2> = (0..=8)
                .map(|i| {
                    let t = i as f32 / 8.0 * std::f32::consts::PI;
                    v(6.5 * t.cos(), 7.0 - 5.0 * t.sin())
                })
                .collect();
            line(shoulders);
        }
        NavIcon::Logout => {
            line(vec![
                v(-1.0, -6.5),
                v(-6.5, -6.5),
                v(-6.5, 6.5),
                v(-1.0, 6.5),
            ]);
            line(vec![v(-2.0, 0.0), v(7.0, 0.0)]);
            line(vec![v(3.5, -3.5), v(7.0, 0.0), v(3.5, 3.5)]);
        }
        NavIcon::Refresh => {
            // Arc de 270 degrés (ouvert en haut à droite) terminé par une pointe de flèche pleine.
            let radius = 5.8;
            let a0 = 60.0_f32.to_radians();
            let a1 = a0 + 270.0_f32.to_radians();
            let at = |a: f32| v(radius * a.cos(), -radius * a.sin());
            let arc: Vec<Pos2> = (0..=24)
                .map(|i| at(a0 + (a1 - a0) * i as f32 / 24.0))
                .collect();
            line(arc);
            let end = at(a1);
            // Sens de parcours à l'arrivée (écran : y vers le bas) et perpendiculaire.
            let dir = Vec2::new(-a1.sin(), -a1.cos());
            let perp = Vec2::new(-dir.y, dir.x);
            p.add(egui::Shape::convex_polygon(
                vec![
                    end + dir * 3.2,
                    end - dir * 1.2 + perp * 3.6,
                    end - dir * 1.2 - perp * 3.6,
                ],
                color,
                Stroke::NONE,
            ));
        }
        NavIcon::Trash => {
            line(vec![v(-6.5, -4.0), v(6.5, -4.0)]);
            line(vec![
                v(-2.5, -4.0),
                v(-2.5, -6.5),
                v(2.5, -6.5),
                v(2.5, -4.0),
            ]);
            line(vec![v(-5.0, -4.0), v(-4.0, 6.5), v(4.0, 6.5), v(5.0, -4.0)]);
        }
        NavIcon::Folder => {
            line(vec![
                v(-7.0, -5.0),
                v(-2.0, -5.0),
                v(0.0, -3.0),
                v(7.0, -3.0),
                v(7.0, 5.5),
                v(-7.0, 5.5),
                v(-7.0, -5.0),
            ]);
        }
        NavIcon::Download => {
            line(vec![v(0.0, -6.5), v(0.0, 3.0)]);
            line(vec![v(-4.0, -1.0), v(0.0, 3.0), v(4.0, -1.0)]);
            line(vec![v(-6.5, 6.5), v(6.5, 6.5)]);
        }
        NavIcon::Panel => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::new(15.0, 12.0)),
                CornerRadius::same(2),
                st,
                egui::StrokeKind::Middle,
            );
            line(vec![v(-2.5, -6.0), v(-2.5, 6.0)]);
        }
        NavIcon::Search => {
            p.circle_stroke(v(-1.5, -1.5), 5.0, st);
            line(vec![v(2.2, 2.2), v(7.0, 7.0)]);
        }
        NavIcon::Feather => {
            // Plume : contour en fuseau le long de la diagonale, tige qui dépasse en bas à gauche, nervure centrale.
            let (base, tip) = (Vec2::new(-4.0, 4.0), Vec2::new(6.5, -6.5));
            let axis = tip - base;
            let norm = Vec2::new(axis.y, -axis.x).normalized();
            let at = |t: f32, side: f32| {
                let w = 4.2 * (std::f32::consts::PI * t).sin() * (1.0 - 0.25 * t);
                let q = base + axis * t + norm * (w * side);
                v(q.x, q.y)
            };
            let mut outline: Vec<Pos2> = (0..=16).map(|i| at(i as f32 / 16.0, 1.0)).collect();
            outline.extend((0..=16).rev().map(|i| at(i as f32 / 16.0, -1.0)));
            line(outline);
            line(vec![
                v(-7.0, 7.0),
                v(base.x + axis.x * 0.8, base.y + axis.y * 0.8),
            ]);
        }
        NavIcon::Star | NavIcon::StarFilled => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 7.6 } else { 3.3 };
                    v(r * a.cos(), 0.6 + r * a.sin())
                })
                .collect();
            if icon == NavIcon::StarFilled {
                let mut mesh = egui::epaint::Mesh::default();
                mesh.colored_vertex(c + Vec2::new(0.0, 0.6), color);
                for q in &pts {
                    mesh.colored_vertex(*q, color);
                }
                for i in 0..10u32 {
                    mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
                }
                p.add(egui::Shape::mesh(mesh));
            }
            let mut closed = pts;
            closed.push(closed[0]);
            p.add(egui::Shape::line(closed, Stroke { width: 1.4, color }));
        }
        NavIcon::StarPlus => {
            // Étoile un peu plus petite, décalée vers le bas à gauche, et un plus en haut à droite.
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 6.6 } else { 2.9 };
                    v(-1.6 + r * a.cos(), 1.4 + r * a.sin())
                })
                .collect();
            let mut closed = pts;
            closed.push(closed[0]);
            p.add(egui::Shape::line(closed, Stroke { width: 1.4, color }));
            line(vec![v(4.2, -6.4), v(8.2, -6.4)]);
            line(vec![v(6.2, -8.4), v(6.2, -4.4)]);
        }
        NavIcon::Chat => {
            p.rect_stroke(
                Rect::from_center_size(v(0.0, -1.0), Vec2::new(15.0, 10.5)),
                CornerRadius::same(3),
                st,
                egui::StrokeKind::Middle,
            );
            line(vec![v(-3.5, 4.2), v(-5.0, 7.5), v(0.0, 4.2)]);
        }
        NavIcon::Access => {
            p.circle_stroke(c, 7.5, st);
            p.circle_filled(v(0.0, -3.2), 1.4, color);
            line(vec![v(-4.0, -1.0), v(4.0, -1.0)]);
            line(vec![v(0.0, -1.0), v(0.0, 2.0)]);
            line(vec![v(0.0, 2.0), v(-2.2, 5.2)]);
            line(vec![v(0.0, 2.0), v(2.2, 5.2)]);
        }
        NavIcon::Appearance => {
            p.circle_stroke(c, 7.5, st);
            let half: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                    v(7.5 * a.cos(), 7.5 * a.sin())
                })
                .collect();
            p.add(egui::Shape::convex_polygon(half, color, Stroke::NONE));
        }
        NavIcon::General => {
            // Globe : cercle, équateur et méridien (ellipse étroite).
            p.circle_stroke(c, 7.5, st);
            line(vec![v(-7.5, 0.0), v(7.5, 0.0)]);
            let meridian: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / 24.0;
                    v(3.4 * a.cos(), 7.5 * a.sin())
                })
                .collect();
            line(meridian);
        }
        NavIcon::Bell => {
            // Cloche : dôme, flancs, rebord et battant.
            let dome: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * i as f32 / 16.0;
                    v(5.2 * a.cos(), -1.5 - 5.2 * a.sin())
                })
                .collect();
            line(dome);
            line(vec![v(-5.2, -1.5), v(-5.2, 3.0), v(-7.2, 5.0)]);
            line(vec![v(5.2, -1.5), v(5.2, 3.0), v(7.2, 5.0)]);
            line(vec![v(-7.2, 5.0), v(7.2, 5.0)]);
            p.circle_filled(v(0.0, 7.4), 1.5, color);
        }
        NavIcon::Keyboard => {
            // Clavier : boîtier, deux rangées de touches et barre d'espace.
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::new(16.0, 10.0)),
                CornerRadius::same(2),
                st,
                egui::StrokeKind::Middle,
            );
            for x in [-4.5_f32, -1.5, 1.5, 4.5] {
                p.circle_filled(v(x, -2.0), 0.9, color);
            }
            for x in [-3.0_f32, 0.0, 3.0] {
                p.circle_filled(v(x, 0.7), 0.9, color);
            }
            line(vec![v(-3.5, 3.2), v(3.5, 3.2)]);
        }
        NavIcon::Info => {
            // Cercle, point et trait du « i ».
            p.circle_stroke(c, 7.5, st);
            p.circle_filled(v(0.0, -3.4), 1.0, color);
            line(vec![v(0.0, -0.6), v(0.0, 4.2)]);
        }
        NavIcon::Power => {
            // Arc ouvert en haut, avec le trait vertical du symbole « marche / arrêt ».
            let radius = 6.0;
            let (a0, a1) = (125.0_f32.to_radians(), 415.0_f32.to_radians());
            let arc: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = a0 + (a1 - a0) * i as f32 / 24.0;
                    v(radius * a.cos(), 0.8 - radius * a.sin())
                })
                .collect();
            line(arc);
            line(vec![v(0.0, -7.0), v(0.0, -1.0)]);
        }
        NavIcon::Pip => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::new(15.0, 11.0)),
                CornerRadius::same(2),
                st,
                egui::StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_size(v(0.5, 0.5), Vec2::new(5.5, 3.5)),
                CornerRadius::same(1),
                color,
            );
        }
        NavIcon::Grid => {
            // Quatre écrans de 6 px, séparés de 2 px.
            for (x, y) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)] {
                p.rect_stroke(
                    Rect::from_center_size(v(x, y), Vec2::splat(6.0)),
                    CornerRadius::same(1),
                    st,
                    egui::StrokeKind::Middle,
                );
            }
        }
        NavIcon::Sound | NavIcon::SoundOff => {
            // Même dessin que le haut-parleur du lecteur (`Icon::Speaker`), à la taille des icônes de navigation.
            let pts = vec![
                v(-7.0, -2.2),
                v(-4.1, -2.2),
                v(0.7, -6.4),
                v(0.7, 6.4),
                v(-4.1, 2.2),
                v(-7.0, 2.2),
            ];
            p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
            if icon == NavIcon::Sound {
                for r in [3.8_f32, 7.0] {
                    let arc: Vec<Pos2> = (-3..=3)
                        .map(|i| {
                            let a = i as f32 * 0.22;
                            v(1.3 + r * a.cos(), r * a.sin())
                        })
                        .collect();
                    line(arc);
                }
            } else {
                line(vec![v(3.2, -2.6), v(8.4, 2.6)]);
                line(vec![v(3.2, 2.6), v(8.4, -2.6)]);
            }
        }
        NavIcon::Pin | NavIcon::PinOff => {
            // Punaise vue de face : tête, collerette, pointe ; barrée d'un trait quand elle est désactivée.
            p.rect_stroke(
                Rect::from_center_size(v(0.0, -4.0), Vec2::new(5.0, 6.0)),
                CornerRadius::same(1),
                st,
                egui::StrokeKind::Middle,
            );
            line(vec![v(-5.0, -0.6), v(5.0, -0.6)]);
            line(vec![v(0.0, -0.6), v(0.0, 7.0)]);
            if icon == NavIcon::PinOff {
                line(vec![v(-6.5, -7.0), v(6.5, 7.0)]);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum BtnKind {
    Secondary,
    /// Secondaire qui devient rouge au survol (arrêter, quitter).
    SecondaryDanger,
    /// Secondaire au texte et à l'icône rouges, qui se remplit de rouge (texte blanc) au survol (« Se déconnecter »).
    SecondaryRedText,
    Primary,
    Danger,
    /// Bouton d'état désactivé (« À jour », « Vérification… ») : reste lisible (texte MUTED, au moins 4,5:1) au lieu
    /// d'être estompé comme les autres boutons désactivés.
    Status,
}

/// Bouton avec icône dessinée à gauche du texte.
pub fn icon_text_button(ui: &mut egui::Ui, kind: BtnKind, icon: NavIcon, text: &str) -> Response {
    icon_text_button_min(ui, kind, icon, text, 0.0)
}

/// Largeur naturelle d'un bouton `icon_text_button` portant `text`.
pub fn icon_text_button_width(ui: &egui::Ui, text: &str) -> f32 {
    let g =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(14.0), Color32::WHITE);
    12.0 + 16.0 + 8.0 + g.size().x + 14.0
}

/// Comme `icon_text_button`, avec une largeur minimale (pour que deux boutons côte à côte aient la même taille).
pub fn icon_text_button_min(
    ui: &mut egui::Ui,
    kind: BtnKind,
    icon: NavIcon,
    text: &str,
    min_w: f32,
) -> Response {
    icon_text_button_sized(ui, kind, icon, text, min_w, BTN_H, false)
}

/// Comme `icon_text_button_min`, avec une hauteur choisie (34 px = hauteur d'une entrée du menu des Réglages). `exact` :
/// la largeur `min_w` est imposée (le texte est réduit s'il ne tient pas) au lieu d'être un minimum.
pub fn icon_text_button_sized(
    ui: &mut egui::Ui,
    kind: BtnKind,
    icon: NavIcon,
    text: &str,
    min_w: f32,
    h: f32,
    exact: bool,
) -> Response {
    icon_text_button_core(
        ui,
        kind,
        BtnIcon::Nav(icon),
        text,
        min_w,
        h,
        exact,
        false,
        1.0,
    )
}

/// Bouton `icon_text_button` réduit homothétiquement : hauteur, largeur, texte, icône et marges sont multipliés par
/// `scale` (< 1), donc le rapport largeur / hauteur ne change pas.
pub fn icon_text_button_scaled(
    ui: &mut egui::Ui,
    kind: BtnKind,
    icon: NavIcon,
    text: &str,
    scale: f32,
) -> Response {
    icon_text_button_core(
        ui,
        kind,
        BtnIcon::Nav(icon),
        text,
        0.0,
        BTN_H * scale,
        false,
        false,
        scale,
    )
}

/// Bouton de tri par spectateurs (réduit comme `icon_text_button_scaled`). `angle` va de 0 (décroissant) à `PI`
/// (croissant), animé par l'appelant : l'icône se retourne verticalement et le bouton passe en douceur de neutre
/// (décroissant, par défaut) à la couleur d'accent (croissant).
pub fn icon_text_button_sort(ui: &mut egui::Ui, text: &str, scale: f32, angle: f32) -> Response {
    icon_text_button_core(
        ui,
        BtnKind::Secondary,
        BtnIcon::Sort(angle),
        text,
        0.0,
        BTN_H * scale,
        false,
        false,
        scale,
    )
}
/// Bouton de la même hauteur et du même style que `icon_text_button`, réduit à son icône (barre étroite). `tip` : nom
/// du bouton, en infobulle et pour les lecteurs d'écran.
pub fn icon_only_button(ui: &mut egui::Ui, kind: BtnKind, icon: NavIcon, tip: &str) -> Response {
    let resp = icon_text_button_core(
        ui,
        kind,
        BtnIcon::Nav(icon),
        tip,
        BTN_H,
        BTN_H,
        true,
        true,
        1.0,
    );
    show_tip(resp, tip)
}

/// `scale` : 1 = taille normale ; en dessous, tout le bouton est réduit dans la même proportion (voir
/// `icon_text_button_scaled`). Les chemins `exact` et `icon_only` ne s'utilisent qu'à 1.
#[allow(clippy::too_many_arguments)]
fn icon_text_button_core(
    ui: &mut egui::Ui,
    kind: BtnKind,
    icon: BtnIcon,
    text: &str,
    min_w: f32,
    h: f32,
    exact: bool,
    icon_only: bool,
    scale: f32,
) -> Response {
    let font = FontId::proportional(14.0 * scale);
    let natural_w = icon_text_button_width(ui, text) * scale;
    let size = Vec2::new(if exact { min_w } else { natural_w.max(min_w) }, h);
    // « Status » = vraiment désactivé (aucun clic, aucun focus, annoncé comme désactivé) mais lisible : on n'utilise pas
    // `add_enabled_ui`, qui estompe tout (texte à 2,3:1 seulement).
    let sense = if kind == BtnKind::Status {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hov = resp.hovered() && kind != BtnKind::Status;
    // Fondu doux du survol (remplissage d'accent ou rouge, texte et icône vers le blanc).
    let fade_to = match kind {
        BtnKind::Secondary if icon != BtnIcon::Nav(NavIcon::Check) => Some(ACCENT()),
        BtnKind::SecondaryDanger | BtnKind::SecondaryRedText => Some(NEGATIVE_FILL()),
        _ => None,
    };
    let fades = fade_to.is_some() || matches!(kind, BtnKind::Primary | BtnKind::Danger);
    let k = if fades {
        hover_k(ui, resp.id, hov)
    } else {
        0.0
    };
    // Bouton de tri : la couleur d'accent monte avec le retournement de l'icône (croissant = accent).
    let k = match icon {
        BtnIcon::Sort(angle) => k.max((angle / std::f32::consts::PI).clamp(0.0, 1.0)),
        BtnIcon::Nav(_) => k,
    };
    let (mut fill, stroke, mut fg) = match kind {
        BtnKind::Secondary | BtnKind::SecondaryDanger | BtnKind::SecondaryRedText
            if let Some(to) = fade_to =>
        {
            (
                SURFACE_2().lerp_to_gamma(to, k),
                Stroke::new(1.0, BORDER().gamma_multiply(1.0 - k)),
                if kind == BtnKind::SecondaryRedText {
                    NEGATIVE()
                } else {
                    TEXT()
                }
                .lerp_to_gamma(Color32::WHITE, k),
            )
        }
        BtnKind::Secondary => (
            if hov { SURFACE_3() } else { SURFACE_2() },
            hover_stroke(hov),
            TEXT(),
        ),
        BtnKind::SecondaryDanger | BtnKind::SecondaryRedText => {
            (SURFACE_2(), Stroke::new(1.0, BORDER()), TEXT())
        }
        BtnKind::Primary => (
            ACCENT().lerp_to_gamma(ACCENT_HOVER(), k),
            Stroke::NONE,
            Color32::WHITE,
        ),
        BtnKind::Danger => (
            NEGATIVE_FILL().lerp_to_gamma(NEGATIVE_HOVER(), k),
            Stroke::NONE,
            Color32::WHITE,
        ),
        BtnKind::Status => (SURFACE_2(), Stroke::new(1.0, BORDER()), MUTED()),
    };
    if !ui.is_enabled() && kind != BtnKind::Status {
        fill = fill.gamma_multiply(0.6);
        fg = fg.gamma_multiply(0.5);
    }
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(RADIUS_INNER),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    paint_pressed(ui, &resp, rect, RADIUS_INNER);
    // Contenu centré : deux boutons de même largeur ont les mêmes marges internes.
    let natural = icon_text_button_width_of(p, text) * scale;
    if icon_only {
        paint_btn_icon(p, rect.center(), icon, fg, 1.0);
    } else if exact && natural > rect.width() {
        // Largeur imposée trop juste pour le texte : l'icône à gauche, le texte réduit pour tenir sans déborder.
        let text_w = natural - 50.0;
        let avail = (rect.width() - 30.0 - 8.0).max(20.0);
        let font = FontId::proportional((14.0 * avail / text_w).clamp(10.0, 14.0));
        paint_btn_icon(
            p,
            Pos2::new(rect.left() + 16.0, rect.center().y),
            icon,
            fg,
            1.0,
        );
        p.text(
            Pos2::new(rect.left() + 30.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            fg,
        );
    } else {
        let dx = (rect.width() - natural) / 2.0;
        paint_btn_icon(
            p,
            Pos2::new(rect.left() + dx + 20.0 * scale, rect.center().y),
            icon,
            fg,
            scale,
        );
        p.text(
            Pos2::new(rect.left() + dx + 36.0 * scale, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            fg,
        );
    }
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled() && kind != BtnKind::Status,
            text,
        )
    });
    resp
}

/// Icône d'un bouton : une icône de la barre, ou l'icône de tri par spectateurs retournée de `angle` radians
/// (0 = décroissant, `PI` = croissant).
#[derive(Clone, Copy, PartialEq)]
enum BtnIcon {
    Nav(NavIcon),
    Sort(f32),
}
fn paint_btn_icon(p: &egui::Painter, c: Pos2, icon: BtnIcon, color: Color32, scale: f32) {
    match icon {
        BtnIcon::Nav(i) => paint_nav_icon_scaled(p, c, i, color, scale),
        BtnIcon::Sort(angle) => {
            // Trois traits à gauche de longueurs décroissantes (le plus long en haut) et une flèche verticale vers le bas
            // à droite, dont la tige part de la hauteur du trait du milieu. Un retournement vertical (`flip` de 1 à -1,
            // 0 à mi-course) donne l'icône croissante : traits croissants, flèche vers le haut. Mêmes traits (1,5 px) que
            // les autres icônes ; bouts arrondis par un petit disque à chaque extrémité.
            let flip = angle.cos();
            let pt = |x: f32, y: f32| Pos2::new(c.x + x * scale, c.y + y * flip * scale);
            let st = Stroke::new(1.5 * scale, color);
            let cap = 0.75 * scale;
            let stroke = |pts: &[Pos2]| {
                p.add(egui::Shape::line(pts.to_vec(), st));
                for (i, q) in pts.iter().enumerate() {
                    if i == 0 || i + 1 == pts.len() {
                        p.circle_filled(*q, cap, color);
                    }
                }
            };
            stroke(&[pt(-8.0, -5.0), pt(0.0, -5.0)]);
            stroke(&[pt(-8.0, 0.0), pt(-2.5, 0.0)]);
            stroke(&[pt(-8.0, 5.0), pt(-5.0, 5.0)]);
            stroke(&[pt(5.5, 0.0), pt(5.5, 6.5)]);
            stroke(&[pt(2.5, 3.5), pt(5.5, 6.5), pt(8.5, 3.5)]);
        }
    }
}
fn icon_text_button_width_of(p: &egui::Painter, text: &str) -> f32 {
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(14.0), Color32::WHITE);
    12.0 + 16.0 + 8.0 + g.size().x + 14.0
}

/// Roue chromatique : la teinte suit l'angle, la saturation le rayon (centre = blanc). Renvoie `true` si la
/// couleur a changé. La luminosité (`hsv.v`) n'est pas modifiée ici.
pub fn color_wheel(ui: &mut egui::Ui, hsv: &mut egui::ecolor::Hsva, label: &str) -> bool {
    use egui::ecolor::Hsva;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(168.0), Sense::click_and_drag());
    let c = rect.center();
    let radius = rect.width() / 2.0 - 3.0;
    let n = 120usize;
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(c, Color32::from(Hsva::new(0.0, 0.0, 1.0, 1.0)));
    for i in 0..=n {
        let h = i as f32 / n as f32;
        let a = h * std::f32::consts::TAU;
        mesh.colored_vertex(
            c + Vec2::new(a.cos(), a.sin()) * radius,
            Color32::from(Hsva::new(h, 1.0, 1.0, 1.0)),
        );
    }
    for i in 0..n as u32 {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    let p = ui.painter();
    p.add(egui::Shape::mesh(mesh));
    p.circle_stroke(c, radius, Stroke::new(1.0, BORDER_HOVER()));
    let mut changed = false;
    if resp.clicked() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let d = pos - c;
            let h = (d.y.atan2(d.x) / std::f32::consts::TAU).rem_euclid(1.0);
            let s = (d.length() / radius).min(1.0);
            if (h - hsv.h).abs() > f32::EPSILON || (s - hsv.s).abs() > f32::EPSILON {
                hsv.h = h;
                hsv.s = s;
                changed = true;
            }
        }
    }
    // Repère : rond blanc cerclé de noir à la position de la couleur choisie.
    let a = hsv.h * std::f32::consts::TAU;
    let hp = c + Vec2::new(a.cos(), a.sin()) * (hsv.s * radius);
    let p = ui.painter();
    p.circle_stroke(hp, 7.0, Stroke::new(3.0, Color32::BLACK));
    p.circle_stroke(hp, 7.0, Stroke::new(1.6, Color32::WHITE));
    focus_ring_circle(ui, &resp, c, radius);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    changed
}

fn focus_ring_circle(ui: &egui::Ui, resp: &Response, c: Pos2, r: f32) {
    if resp.has_focus() && focus_visible() {
        ui.painter()
            .circle_stroke(c, r + 3.0, Stroke::new(focus_width(), FOCUS()));
    }
}

/// Logo Twitch (bulle et deux barres), dessiné dans un carré de côté `size` centré sur `c`.
fn paint_twitch_logo(p: &egui::Painter, c: Pos2, size: f32, fg: Color32, bar: Color32) {
    let s = size / 24.0;
    let o = c - Vec2::splat(size / 2.0);
    let pt = |x: f32, y: f32| o + Vec2::new(x * s, y * s);
    let body = vec![
        pt(0.54, 4.1),
        pt(2.15, 0.0),
        pt(22.15, 0.0),
        pt(22.15, 15.25),
        pt(16.4, 21.0),
        pt(0.54, 21.0),
    ];
    p.add(egui::Shape::convex_polygon(body, fg, Stroke::NONE));
    let tail = vec![
        pt(6.27, 20.5),
        pt(12.5, 20.5),
        pt(9.5, 24.0),
        pt(6.27, 24.0),
    ];
    p.add(egui::Shape::convex_polygon(tail, fg, Stroke::NONE));
    for x in [11.5_f32, 16.5] {
        p.rect_filled(
            Rect::from_min_size(pt(x, 5.2), Vec2::new(1.8 * s, 5.4 * s)),
            CornerRadius::ZERO,
            bar,
        );
    }
}

/// Bouton « Se connecter » avec le logo Twitch. `loading` : connexion en cours, une barre de chargement
/// glisse dans le bouton ; au survol (ou au clavier) il devient rouge et affiche `cancel_text` : un clic annule la
/// connexion (le même bouton, pas un autre). `enabled` faux : bouton grisé.
pub fn twitch_login_button(
    ui: &mut egui::Ui,
    text: &str,
    same_size_as: &str,
    cancel_text: &str,
    loading: bool,
    enabled: bool,
) -> Response {
    let font = FontId::proportional(14.0);
    // Mêmes dimensions que `icon_text_button` avec le texte `same_size_as` (le bouton « Se déconnecter »).
    let ref_w = ui
        .painter()
        .layout_no_wrap(same_size_as.to_string(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let sense = if enabled || loading {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(12.0 + 16.0 + 8.0 + ref_w + 14.0, BTN_H), sense);
    let hov = resp.hovered() && enabled;
    // Chargement : le survol fond vers le rouge (k), l'animation s'efface et « Annuler » apparaît.
    let k = if loading {
        hover_k(ui, resp.id, resp.hovered() || resp.has_focus())
    } else {
        0.0
    };
    let mut fill = if loading {
        ACCENT().lerp_to_gamma(NEGATIVE_FILL(), k)
    } else if hov {
        ACCENT_HOVER()
    } else {
        ACCENT()
    };
    let mut fg = Color32::WHITE;
    if !enabled && !loading {
        fill = fill.gamma_multiply(0.6);
        fg = fg.gamma_multiply(0.5);
    }
    let r = CornerRadius::same(RADIUS_INNER);
    let p = ui.painter();
    p.rect_filled(rect, r, fill);
    if !loading {
        paint_twitch_logo(
            p,
            Pos2::new(rect.left() + 21.0, rect.center().y - 0.5),
            15.0,
            fg,
            fill,
        );
        p.text(
            Pos2::new(rect.left() + 40.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            fg,
        );
    } else {
        // Connexion en cours : ni texte ni logo, seulement une barre indéterminée (un segment clair qui
        // traverse le bouton en boucle).
        let h = 6.0;
        let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 24.0, h));
        let a = 1.0 - k;
        p.rect_filled(
            track,
            CornerRadius::same(2),
            Color32::from_black_alpha(70).gamma_multiply(a),
        );
        let phase = ((ui.ctx().input(|i| i.time) * 0.9) % 1.0) as f32;
        let seg = track.width() * 0.35;
        let x0 = track.left() - seg + phase * (track.width() + seg);
        let part = Rect::from_min_max(
            Pos2::new(x0.max(track.left()), track.top()),
            Pos2::new((x0 + seg).min(track.right()), track.bottom()),
        );
        if part.width() > 0.0 {
            p.rect_filled(
                part,
                CornerRadius::same(2),
                Color32::WHITE.gamma_multiply(a),
            );
        }
        if k > 0.0 {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                cancel_text,
                font,
                fg.gamma_multiply(k),
            );
        }
        ui.ctx().request_repaint();
    }
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    let name = if loading { cancel_text } else { text };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled || loading, name));
    resp
}

/// Noyau des curseurs : fine piste, remplissage blanc (accent au-delà du repère `mark`), poignée
/// ronde qui grossit au survol. Renvoie la nouvelle fraction (0..1) si l'utilisateur l'a changée.
/// Au clavier (après Tab) : flèches gauche/droite = un pas `step`, Origine/Fin = minimum/maximum.
/// `fill` : couleur de la partie remplie (`None` = encre du thème).
fn slider_core(
    ui: &mut egui::Ui,
    width: f32,
    frac: f32,
    mark: Option<f32>,
    step: f32,
    fill: Option<Color32>,
    scale: f32,
) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click_and_drag());
    let (x0, x1) = (rect.left() + 8.0 * scale, rect.right() - 8.0 * scale);
    let mut new = None;
    if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            new = Some(((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0));
        }
    }
    if resp.has_focus() {
        // Les flèches horizontales agissent sur le curseur au lieu de déplacer le focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            )
        });
        let (left, right, home, end) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        let delta = if right {
            step
        } else if left {
            -step
        } else {
            0.0
        };
        if delta != 0.0 {
            new = Some((frac + delta).clamp(0.0, 1.0));
        } else if home {
            new = Some(0.0);
        } else if end {
            new = Some(1.0);
        }
    }
    focus_ring(ui, &resp, rect.shrink2(Vec2::new(2.0, 8.0)), 8);
    let name = row_label().unwrap_or_default();
    resp.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), frac as f64, &name));
    let shown = new.unwrap_or(frac).clamp(0.0, 1.0);
    let active = resp.hovered() || resp.dragged();
    // Survol et glissé en fondu : la piste s'éclaircit et la poignée grossit en douceur (au lieu d'un saut).
    let hot = motion::eased(ui.ctx(), resp.id.with("slider_hot"), active, motion::HOVER);
    let grab = motion::eased(
        ui.ctx(),
        resp.id.with("slider_grab"),
        resp.dragged(),
        motion::HOVER,
    );
    let y = rect.center().y;
    let x_of = |f: f32| x0 + (x1 - x0) * f;
    let p = ui.painter();
    let band = |a: f32, b: f32, color: Color32| {
        p.rect_filled(
            Rect::from_min_max(Pos2::new(a, y - 2.0 * scale), Pos2::new(b, y + 2.0 * scale)),
            CornerRadius::same((2.0 * scale).round() as u8),
            color,
        );
    };
    // Couleurs du thème : blanc sur fond sombre (inchangé, lecteur compris), encre sombre en thème Clair
    // (avant, tout était blanc et le curseur disparaissait sur les cartes blanches du thème Clair).
    let light = CURRENT.load(Ordering::Relaxed) == 2;
    let ink = if light { TEXT() } else { Color32::WHITE };
    let track = if light {
        Color32::from_black_alpha((44.0 + 16.0 * hot).round() as u8)
    } else {
        Color32::from_white_alpha((34.0 + 12.0 * hot).round() as u8)
    };
    band(x0, x1, track);
    let xv = x_of(shown);
    match mark {
        Some(m) => {
            let xm = x_of(m);
            band(x0, xv.min(xm), fill.unwrap_or(ink));
            if xv > xm {
                // Au-delà de 100 % : la couleur du thème, pour bien distinguer l'amplification.
                band(xm, xv, if fill.is_some() { ink } else { ACCENT_GFX() });
            }
        }
        None => band(x0, xv, fill.unwrap_or(ink)),
    }
    let r = scale * (5.5 + 1.5 * hot + 0.5 * grab);
    p.circle_filled(Pos2::new(xv, y), r + 1.5, Color32::from_black_alpha(90));
    p.circle_filled(Pos2::new(xv, y), r, ink);
    new
}

/// Curseur de volume (0 à `max`, repère à 100 %). Renvoie `true` si la valeur change.
pub fn volume_slider(ui: &mut egui::Ui, value: &mut u8, max: u8, width: f32) -> bool {
    volume_slider_scaled(ui, value, max, width, 1.0)
}

/// Comme `volume_slider`, avec la piste et la poignée agrandies de `scale` (PiP) ; la hauteur de la zone ne change pas.
pub fn volume_slider_scaled(
    ui: &mut egui::Ui,
    value: &mut u8,
    max: u8,
    width: f32,
    scale: f32,
) -> bool {
    let mark = (max > 100).then(|| 100.0 / max as f32);
    match slider_core(
        ui,
        width,
        *value as f32 / max as f32,
        mark,
        5.0 / max.max(1) as f32,
        Some(ACCENT_GFX()),
        scale,
    ) {
        Some(f) => {
            let nv = (f * max as f32).round() as u8;
            let changed = nv != *value;
            *value = nv;
            changed
        }
        None => false,
    }
}

/// Curseur de volume vertical (barre du lecteur trop étroite pour le curseur horizontal) : 0 en bas, `max` en haut,
/// repère à 100 %. Mêmes couleurs que le curseur horizontal. Flèches haut/bas au clavier. `true` si la valeur change.
pub fn volume_slider_vertical(ui: &mut egui::Ui, value: &mut u8, max: u8, height: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, height), Sense::click_and_drag());
    let (y_top, y_bot) = (rect.top() + 10.0, rect.bottom() - 10.0);
    let frac = *value as f32 / max.max(1) as f32;
    let mut new: Option<f32> = None;
    if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            new = Some(((y_bot - pos.y) / (y_bot - y_top)).clamp(0.0, 1.0));
        }
    }
    if resp.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let (up, down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        let step = 5.0 / max.max(1) as f32;
        if up {
            new = Some((frac + step).clamp(0.0, 1.0));
        } else if down {
            new = Some((frac - step).clamp(0.0, 1.0));
        }
    }
    focus_ring(ui, &resp, rect.shrink2(Vec2::new(2.0, 2.0)), 8);
    resp.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), frac as f64, crate::i18n::tr("Volume"))
    });
    let shown = new.unwrap_or(frac).clamp(0.0, 1.0);
    let active = resp.hovered() || resp.dragged();
    let x = rect.center().x;
    let y_of = |f: f32| y_bot - (y_bot - y_top) * f;
    let p = ui.painter();
    let band = |a: f32, b: f32, color: Color32| {
        p.rect_filled(
            Rect::from_min_max(Pos2::new(x - 2.0, a.min(b)), Pos2::new(x + 2.0, a.max(b))),
            CornerRadius::same(2),
            color,
        );
    };
    let track = Color32::from_white_alpha(if active { 46 } else { 34 });
    band(y_top, y_bot, track);
    let yv = y_of(shown);
    if max > 100 {
        let ym = y_of(100.0 / max as f32);
        band(y_bot, yv.max(ym), ACCENT_GFX());
        if yv < ym {
            band(ym, yv, Color32::WHITE);
        }
    } else {
        band(y_bot, yv, ACCENT_GFX());
    }
    let r = if resp.dragged() {
        7.5
    } else if active {
        7.0
    } else {
        5.5
    };
    p.circle_filled(Pos2::new(x, yv), r + 1.5, Color32::from_black_alpha(90));
    p.circle_filled(Pos2::new(x, yv), r, Color32::WHITE);
    match new {
        Some(f) => {
            let nv = (f * max as f32).round() as u8;
            let changed = nv != *value;
            *value = nv;
            changed
        }
        None => false,
    }
}

/// Valeur du curseur pour la fraction `f` : `min` + un nombre entier de pas (avant, le minimum était oublié :
/// le curseur de taille du chat restait à 0,8 et celui de vitesse de défilement s'arrêtait à 2,6).
fn snap(f: f32, min: f32, max: f32, step: f32) -> f32 {
    (min + (f * (max - min) / step).round() * step).clamp(min, max)
}

/// Curseur à valeur décimale entre `min` et `max`, par pas de `step`. Renvoie `true` si elle change.
pub fn value_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
    width: f32,
) -> bool {
    match slider_core(
        ui,
        width,
        (*value - min) / (max - min),
        None,
        step / (max - min),
        Some(ACCENT_GFX()),
        1.0,
    ) {
        Some(f) => {
            let nv = snap(f, min, max, step);
            let changed = (nv - *value).abs() > f32::EPSILON;
            *value = nv.clamp(min, max);
            changed
        }
        None => false,
    }
}

/// Bouton carré qui ne porte qu'une icône (barre latérale réduite).
pub fn nav_icon_button(
    ui: &mut egui::Ui,
    icon: NavIcon,
    size: Vec2,
    active: bool,
    tip: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let k = hover_k(ui, resp.id, resp.hovered());
    let base = if active {
        SURFACE_3()
    } else {
        Color32::TRANSPARENT
    };
    let fill = base.lerp_to_gamma(ACCENT(), k);
    let color = TEXT().lerp_to_gamma(Color32::WHITE, k);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_INNER), fill);
    paint_pressed(ui, &resp, rect, RADIUS_INNER);
    paint_nav_icon(ui.painter(), rect.center(), icon, color);
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), active, tip)
    });
    show_tip(resp, tip)
}

/// Pastille « ? » : l'explication du réglage s'affiche au survol.
fn paint_help(ui: &egui::Ui, c: Pos2, resp: &Response, label: &str) {
    let col = if resp.hovered() || (resp.has_focus() && focus_visible()) {
        TEXT()
    } else {
        MUTED()
    };
    ui.painter().circle_stroke(c, 8.0, Stroke::new(1.2, col));
    ui.painter().text(
        c,
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(11.5),
        col,
    );
    if resp.has_focus() && focus_visible() {
        ui.painter()
            .circle_stroke(c, 11.0, Stroke::new(focus_width(), FOCUS()));
    }
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            true,
            format!("{} : {label}", crate::i18n::tr("Aide")),
        )
    });
}

/// Fait grandir une infobulle ou un menu depuis son point d'ancrage (le point du cadre le plus proche de `anchor`) à
/// l'ouverture : de `motion::POPUP_FROM` à 1 en `motion::POPUP`, ease-out, sans toucher à la zone cliquable. À appeler
/// à chaque image, ouverte ou non (`open`), avec la réponse du popup quand il est affiché. Fermé, l'état revient à 0 sur
/// le champ. Aucun effet si les déplacements sont réduits ou coupés (niveau d'animation).
pub fn grow_popup(
    ctx: &egui::Context,
    id: egui::Id,
    open: bool,
    popup: Option<&Response>,
    anchor: Pos2,
) {
    let secs = if open {
        move_secs(ctx, motion::POPUP)
    } else {
        0.0
    };
    let k = ctx.animate_bool_with_time_and_easing(
        id.with("popup_grow"),
        open,
        secs,
        egui::emath::easing::cubic_out,
    );
    let Some(popup) = popup else { return };
    if k >= 1.0 {
        return;
    }
    let s = motion::POPUP_FROM + (1.0 - motion::POPUP_FROM) * k;
    let origin = popup.rect.clamp(anchor);
    let tr = egui::emath::TSTransform::new(origin.to_vec2() * (1.0 - s), s);
    ctx.transform_layer_shapes_after_rounding(popup.layer_id, tr);
}

/// Infobulle centrée horizontalement sur le composant (au-dessus, ou dessous s'il n'y a pas de place), au lieu de
/// suivre le pointeur : plus propre pour un bouton isolé.
pub fn show_tip_centered(resp: Response, text: impl AsRef<str>) -> Response {
    let key = resp.id.with("tip_centered");
    let focus_open = resp.has_focus() && focus_visible();
    let mut shown: bool = resp.ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    if resp.hovered() {
        if !shown && egui::Tooltip::should_show_tooltip(&resp, false) {
            shown = true;
            resp.ctx.data_mut(|d| d.insert_temp(key, true));
        }
    } else if shown {
        shown = false;
        resp.ctx.data_mut(|d| d.remove::<bool>(key));
    }
    let frame = egui::Frame::new()
        .fill(SURFACE_3())
        .stroke(Stroke::new(1.0, BORDER_HOVER()))
        .corner_radius(CornerRadius::same(RADIUS_INNER))
        .inner_margin(egui::Margin::symmetric(12, 9));
    let open = shown || focus_open;
    let shown_popup = egui::Popup::from_response(&resp)
        .kind(egui::PopupKind::Tooltip)
        .align(egui::RectAlign::TOP)
        .align_alternatives(&[egui::RectAlign::BOTTOM])
        .gap(10.0)
        .width(360.0)
        .frame(frame)
        .sense(Sense::hover())
        .open(open)
        .show(|ui| {
            ui.style_mut().interaction.selectable_labels = false;
            ui.label(RichText::new(text.as_ref()).size(14.5).color(TEXT()));
        });
    grow_popup(
        &resp.ctx,
        resp.id,
        open,
        shown_popup.as_ref().map(|p| &p.response),
        resp.rect.center(),
    );
    resp
}

/// Comme `show_tip`, mais le survol à la souris doit durer `secs` secondes avant que l'infobulle ne s'ouvre (le délai
/// repart de zéro quand le pointeur quitte le composant). Au clavier (focus visible), elle s'ouvre tout de suite.
pub fn show_tip_after(resp: Response, text: impl AsRef<str>, secs: f32) -> Response {
    let key = resp.id.with("tip_since");
    if resp.hovered() {
        let now = resp.ctx.input(|i| i.time);
        let since = resp
            .ctx
            .data_mut(|d| *d.get_temp_mut_or_insert_with::<f64>(key, || now));
        let remaining = secs - (now - since) as f32;
        if remaining > 0.0 && !(resp.has_focus() && focus_visible()) {
            resp.ctx
                .request_repaint_after(std::time::Duration::from_secs_f32(remaining));
            return resp;
        }
    } else {
        resp.ctx.data_mut(|d| d.remove::<f64>(key));
    }
    show_tip(resp, text)
}

/// Infobulle au-dessus du curseur (en dessous seulement s'il n'y a pas de place), en 14,5 px, texte plein contraste
/// sur fond surface-3 avec bordure marquée. Remplace `on_hover_text` partout.
/// L'infobulle s'ouvre aussi quand le composant a le focus clavier (Tab), sinon elle reste inaccessible sans souris.
pub fn show_tip(resp: Response, text: impl AsRef<str>) -> Response {
    // Une fois ouverte, l'infobulle reste à sa place (là où le pointeur était à l'ouverture) tant que le pointeur
    // reste sur le composant : un léger mouvement ne la ferme plus et ne la rouvre plus ailleurs.
    let pos_key = resp.id.with("tip_pos");
    let focus_open = resp.has_focus() && focus_visible();
    let mut fixed: Option<Pos2> = resp.ctx.data(|d| d.get_temp::<Pos2>(pos_key));
    let open = if resp.hovered() {
        if fixed.is_none() && egui::Tooltip::should_show_tooltip(&resp, false) {
            fixed = resp.ctx.input(|i| i.pointer.hover_pos());
            if let Some(p) = fixed {
                resp.ctx.data_mut(|d| d.insert_temp(pos_key, p));
            }
        }
        fixed.is_some() || focus_open
    } else {
        if fixed.take().is_some() {
            resp.ctx.data_mut(|d| d.remove::<Pos2>(pos_key));
        }
        focus_open
    };
    let frame = egui::Frame::new()
        .fill(SURFACE_3())
        .stroke(Stroke::new(1.0, BORDER_HOVER()))
        .corner_radius(CornerRadius::same(RADIUS_INNER))
        .inner_margin(egui::Margin::symmetric(12, 9));
    let mut popup = egui::Popup::from_response(&resp).kind(egui::PopupKind::Tooltip);
    // Au clavier, l'infobulle se place au-dessus du composant (le pointeur peut être ailleurs).
    if let Some(p) = fixed {
        popup = popup.at_position(p);
    } else if resp.hovered() || !focus_open {
        popup = popup.at_pointer();
    }
    // Près du haut de la fenêtre (boutons Réduire, Agrandir, Fermer), pas de place au-dessus : on place d'emblée l'infobulle
    // dessous, sinon elle se recale sur le pointeur, lui prend le survol et clignote en boucle.
    let near_top = fixed
        .or_else(|| resp.ctx.input(|i| i.pointer.hover_pos()))
        .is_some_and(|p| p.y < 90.0);
    let (first, second) = if near_top {
        (egui::RectAlign::BOTTOM, egui::RectAlign::TOP)
    } else {
        (egui::RectAlign::TOP, egui::RectAlign::BOTTOM)
    };
    let shown_popup = popup
        .align(first)
        .align_alternatives(&[second])
        .gap(16.0)
        .width(360.0)
        .frame(frame)
        .sense(Sense::hover())
        .open(open)
        .show(|ui| {
            ui.style_mut().interaction.selectable_labels = false;
            ui.label(RichText::new(text.as_ref()).size(14.5).color(TEXT()));
        });
    let anchor = fixed.unwrap_or_else(|| resp.rect.center());
    grow_popup(
        &resp.ctx,
        resp.id,
        open,
        shown_popup.as_ref().map(|p| &p.response),
        anchor,
    );
    resp
}

/// Libellé suivi de sa pastille d'aide, s'il en a une (voir `help.rs`).
pub fn help_label(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.5).color(TEXT()));
        if let Some(help) = crate::help::lookup(label) {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            paint_help(ui, r.center(), &resp, label);
            show_tip(resp, help);
        }
    });
}

/// Durée d'animation : `secs`, ou 0 quand les animations sont coupées (réglage de l'application).
pub fn anim_secs(ctx: &egui::Context, secs: f32) -> f32 {
    if ctx.global_style().animation_time > 0.0 {
        secs
    } else {
        0.0
    }
}

/// Vrai si les déplacements sont animés : animations complètes (ni coupées, ni réduites).
pub fn moves(ctx: &egui::Context) -> bool {
    ctx.global_style().animation_time > 0.0 && !REDUCED_MOTION.load(Ordering::Relaxed)
}

/// Durée d'un déplacement (glissement, dépliage, cascade, reflet) : `secs`, ou 0 si les animations sont réduites ou coupées.
pub fn move_secs(ctx: &egui::Context, secs: f32) -> f32 {
    if moves(ctx) {
        secs
    } else {
        0.0
    }
}

/// Jetons de mouvement : durées (secondes) et courbe uniques pour les animations de l'interface.
pub mod motion {
    use eframe::egui;

    /// Fondu de survol d'un bouton, d'une ligne ou d'une icône.
    pub const HOVER: f32 = 0.12;
    /// Glissement de la poignée d'un interrupteur.
    pub const TOGGLE: f32 = 0.15;
    /// Apparition et disparition des contrôles du lecteur.
    pub const CONTROLS: f32 = 0.15;
    /// Ouverture de la fenêtre des Réglages (voile et fenêtre ensemble).
    pub const MODAL: f32 = 0.20;
    /// Dépliage et repli d'une section.
    pub const SECTION: f32 = 0.20;
    /// Didacticiel : apparition, puis fermeture (plus courte : on veut reprendre la main).
    pub const INTRO_IN: f32 = 0.45;
    pub const INTRO_OUT: f32 = 0.28;
    /// Apparition en cascade (cartes, listes) : fondu d'un élément, décalage entre deux éléments, et nombre d'éléments
    /// décalés (les suivants arrivent avec le dernier). Durée totale maximale : 0,30 s.
    pub const ENTER: f32 = 0.15;
    pub const STAGGER_STEP: f64 = 0.025;
    pub const STAGGER_ITEMS: usize = 6;
    /// Glissement d'un élément vers sa nouvelle place : part de la distance parcourue après `dt` secondes d'une image
    /// (départ vif, arrivée douce ; indépendant de la fréquence d'images et repris en route si la cible change).
    pub fn glide_factor(dt: f32) -> f32 {
        egui::emath::exponential_smooth_factor(0.99, 0.25, dt)
    }

    /// Infobulle ou menu qui s'ouvre : il grandit depuis son point d'ancrage, de `POPUP_FROM` à sa taille réelle.
    pub const POPUP: f32 = 0.15;
    /// Demi-tour de la flèche du bouton de tri de l'accueil (décroissant / croissant).
    pub const SORT_TURN: f32 = 0.25;
    pub const POPUP_FROM: f32 = 0.92;

    /// Message bref : apparition et disparition en fondu.
    pub const TOAST_IN: f32 = 0.12;
    pub const TOAST_OUT: f32 = 0.30;
    /// Point LIVE : une pulsation dure `PULSE_SECS`, répétée `PULSE_CYCLES` fois à l'apparition, puis le point reste fixe.
    pub const PULSE_SECS: f64 = 1.2;
    pub const PULSE_CYCLES: f64 = 3.0;

    /// Opacité (0 à 1) d'un message bref affiché depuis `age` secondes pour `total` secondes. Réveille l'interface
    /// seulement pendant les fondus, et une fois pour la fin ; 0 une fois `total` écoulé. `fade_in` faux : pas de fondu
    /// d'entrée (message dont le minuteur repart à chaque action, comme le volume à la molette : il clignoterait).
    pub fn toast_alpha(ctx: &egui::Context, age: f32, total: f32, fade_in: bool) -> f32 {
        if age >= total {
            return 0.0;
        }
        let remaining = total - age;
        if super::anim_secs(ctx, 1.0) <= 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(remaining));
            return 1.0;
        }
        let entering = fade_in && age < TOAST_IN;
        if entering || remaining < TOAST_OUT {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(remaining - TOAST_OUT));
        }
        let appear = if fade_in { age / TOAST_IN } else { 1.0 };
        egui::emath::easing::cubic_out(appear.min(remaining / TOAST_OUT).clamp(0.0, 1.0))
    }

    /// Opacité (0 à 1) d'un élément de rang `index` apparu à l'instant `started` (secondes d'egui) : fondu en ease-out,
    /// décalé selon le rang. Demande l'image suivante tant que le fondu dure ; 1 si les animations sont coupées.
    pub fn stagger_in(ctx: &egui::Context, started: f64, index: usize) -> f32 {
        let dur = f64::from(super::move_secs(ctx, ENTER));
        if dur <= 0.0 {
            return 1.0;
        }
        let delay = index.min(STAGGER_ITEMS) as f64 * STAGGER_STEP;
        let k = ((ctx.input(|i| i.time) - started - delay) / dur).clamp(0.0, 1.0) as f32;
        if k < 1.0 {
            ctx.request_repaint();
        }
        egui::emath::easing::cubic_out(k)
    }

    /// Progression 0 à 1 d'un état booléen : démarrage vif, arrivée douce, dans les deux sens (0 si les animations sont
    /// coupées). Reprend depuis la valeur en cours si l'état change en route.
    pub fn eased(ctx: &egui::Context, id: egui::Id, on: bool, secs: f32) -> f32 {
        ctx.animate_bool_with_time_and_easing(
            id,
            on,
            super::anim_secs(ctx, secs),
            egui::emath::easing::cubic_out,
        )
    }
}

/// Ligne de réglage « libellé à gauche, contrôle aligné à droite » (curseur, sélecteur…),
/// au même gabarit que les lignes à interrupteur.
pub fn control_row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    control_row_impl(ui, label, true, add);
}

/// Comme `control_row`, mais le contrôle reste toujours à droite du libellé (jamais dessous) : pour des lignes qui
/// doivent rester alignées entre elles (Rouge / Vert / Bleu).
pub fn control_row_inline(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    control_row_impl(ui, label, false, add);
}

fn control_row_impl(
    ui: &mut egui::Ui,
    label: &str,
    stackable: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    // Contrôles posés d'abord, à droite ; le libellé (et son « ? ») est dessiné ensuite dans la place restante,
    // centré verticalement et avec les mêmes écarts que `switch_row`.
    let width = ui.available_width();
    let start = ui.cursor().min;
    let help = crate::help::lookup(label);
    // Place trop juste pour le libellé à côté des contrôles (fenêtre étroite, zoom élevé) : le libellé prend sa propre
    // ligne et les contrôles passent dessous. La largeur des contrôles est mémorisée d'une image à l'autre.
    let need_id = ui.scope_id().with(("control_row_w", label));
    let controls_w: f32 = ui.ctx().data(|d| d.get_temp(need_id)).unwrap_or(0.0);
    let pad = if help.is_some() { 36.0 } else { 12.0 };
    let label_w = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(14.5), TEXT())
        .size()
        .x;
    let stacked = stackable
        && controls_w > 0.0
        && width - controls_w - 14.0 - 12.0 - pad < label_w.min(170.0) + 4.0;
    // Passage d'un état à l'autre en douceur : `k` (0 = à côté, 1 = dessous) fait glisser le contrôle vers le bas, le
    // libellé vers le haut et le retrait de droite disparaître, sans saut (durée 0 si les animations sont coupées).
    let k =
        ui.ctx()
            .animate_bool_with_time(need_id.with("stack"), stacked, move_secs(ui.ctx(), 0.18));
    let label_h = 26.0 * k;
    if label_h > 0.0 {
        ui.add_space(label_h);
    }
    // Le retrait de droite (aligné sur les autres lignes) ne s'efface que si le contrôle n'a pas assez de largeur.
    let tight = controls_w + 14.0 + 12.0 > width;
    let right_pad = if tight { 14.0 * (1.0 - k) } else { 14.0 };
    // Sous le libellé, le contrôle s'aligne à gauche, sur le début du texte (et non plus collé à droite) :
    // la ligne de contrôles se rétrécit jusqu'à sa propre largeur (posée de droite à gauche, dans un rectangle placé à
    // gauche : les champs qui prennent la place disponible gardent la même largeur).
    let row_w =
        (width + ((12.0 + controls_w + right_pad) - width) * k).clamp(controls_w.min(width), width);
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(row_rect.min, Vec2::new(row_w, ROW_H)))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    child.set_min_height(ROW_H);
    // Sur sa propre ligne, le contrôle a besoin de toute la largeur : le retrait à droite s'efface.
    child.add_space(right_pad);
    ROW_LABEL.with(|l| *l.borrow_mut() = Some(label.to_string()));
    add(&mut child);
    ROW_LABEL.with(|l| *l.borrow_mut() = None);
    let used = child.min_rect();
    if used.height() > ROW_H {
        ui.allocate_space(Vec2::new(0.0, used.height() - ROW_H));
    }
    let measured = used.width() - right_pad;
    if (measured - controls_w).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(need_id, measured));
        ui.ctx().request_repaint();
    }
    // Le rectangle des contrôles ne couvre qu'eux : la ligne entière part de la position de départ.
    let rect = Rect::from_min_size(start, Vec2::new(width, label_h + used.height().max(ROW_H)));
    let side_left = rect.right() - (measured + 14.0).min(width);
    let controls_left = side_left + (rect.right() - side_left) * k;
    let text_w = (controls_left - rect.left() - 12.0 - pad).max(60.0);
    let ty = start.y + 10.0 - 6.0 * k;
    paint_highlight(ui, rect, label);
    paint_one_line(
        ui.painter(),
        Pos2::new(rect.left() + 12.0, ty),
        label,
        14.5,
        TEXT(),
        text_w,
    );
    if let Some(help) = help {
        let w = ui
            .painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(14.5), TEXT())
            .size()
            .x
            .min(text_w);
        let c = Pos2::new(rect.left() + 12.0 + w + 16.0, ty + 9.0);
        let hr = ui.interact(
            Rect::from_center_size(c, Vec2::splat(20.0)),
            need_id.with(("help", label)),
            Sense::click(),
        );
        paint_help(ui, c, &hr, label);
        show_tip(hr, help);
    }
}

/// Ligne de réglage à mettre en évidence (choisie dans la recherche des Réglages) : libellé, début, défilement fait.
static HIGHLIGHT: std::sync::Mutex<Option<(String, f64, bool)>> = std::sync::Mutex::new(None);
const HIGHLIGHT_SECS: f64 = 2.5;

/// Met en évidence la ligne `label` quelques instants et la fait défiler jusqu'au centre.
pub fn highlight_setting(ctx: &egui::Context, label: &str) {
    if let Ok(mut h) = HIGHLIGHT.lock() {
        *h = Some((label.to_string(), ctx.input(|i| i.time), false));
    }
    ctx.request_repaint();
}

/// Fond d'accent qui s'estompe derrière la ligne recherchée (et défilement jusqu'à elle la première fois).
pub fn paint_highlight(ui: &egui::Ui, rect: Rect, label: &str) {
    let Ok(mut h) = HIGHLIGHT.lock() else {
        return;
    };
    let Some((target, start, scrolled)) = h.as_mut() else {
        return;
    };
    if target != label {
        return;
    }
    let age = ui.input(|i| i.time) - *start;
    if age > HIGHLIGHT_SECS {
        *h = None;
        return;
    }
    if !*scrolled {
        *scrolled = true;
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
    }
    let a = (1.0 - age / HIGHLIGHT_SECS).clamp(0.0, 1.0) as f32;
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(RADIUS_INNER),
        ACCENT().gamma_multiply(0.28 * a),
    );
    ui.ctx().request_repaint();
}

/// Ligne de résultat de la recherche des Réglages : libellé à gauche, onglet à droite, toute la ligne cliquable.
pub fn search_result_row(ui: &mut egui::Ui, label: &str, tab: &str) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_INNER), SURFACE_2());
    }
    let ty = rect.center().y - 9.0;
    paint_one_line(
        ui.painter(),
        Pos2::new(rect.left() + 12.0, ty),
        label,
        14.5,
        TEXT(),
        (rect.width() - 200.0).max(60.0),
    );
    ui.painter().text(
        Pos2::new(rect.right() - 14.0, rect.center().y),
        Align2::RIGHT_CENTER,
        tab,
        FontId::proportional(12.5),
        MUTED(),
    );
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("{label} ({tab})"),
        )
    });
    resp
}

/// Entrée du menu de gauche des Réglages (texte seul) : choisie ou survolée = fond d'accent.
pub fn menu_item(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    menu_item_icon(ui, None, label, active)
}

/// Comme `menu_item`, avec une petite icône devant le texte.
pub fn menu_item_icon(
    ui: &mut egui::Ui,
    icon: Option<NavIcon>,
    label: &str,
    active: bool,
) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    // Choisie ou survolée : intérieur rempli de la couleur d'accent, texte blanc (contraste garanti sur l'accent).
    let k = if active {
        1.0
    } else {
        hover_k(ui, resp.id, resp.hovered())
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), ACCENT().gamma_multiply(k));
    let color = MUTED().lerp_to_gamma(Color32::WHITE, k);
    let text_x = if let Some(icon) = icon {
        paint_nav_icon(
            ui.painter(),
            Pos2::new(rect.left() + 22.0, rect.center().y),
            icon,
            color,
        );
        rect.left() + 40.0
    } else {
        rect.left() + 12.0
    };
    paint_one_line(
        ui.painter(),
        Pos2::new(text_x, rect.center().y - 9.0),
        label,
        14.5,
        color,
        rect.right() - text_x - 12.0,
    );
    focus_ring(ui, &resp, rect, 8);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), active, label)
    });
    resp
}

/// Ligne de choix unique (liste des langues) : bouton radio à gauche, titre, texte secondaire aligné à droite.
/// Choisie = fond surface-3 et liseré d'accent, radio rempli (pas seulement une couleur).
pub fn choice_row(ui: &mut egui::Ui, title: &str, sub: &str, selected: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let fill = if selected || resp.hovered() {
        SURFACE_3()
    } else {
        SURFACE_2()
    };
    let stroke = if selected {
        Stroke::new(1.4, ACCENT_LINE())
    } else {
        Stroke::new(1.0, BORDER())
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(RADIUS_INNER),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let c = Pos2::new(rect.left() + 24.0, rect.center().y);
    if selected {
        p.circle_stroke(c, 8.0, Stroke::new(2.0, ACCENT_GFX()));
        p.circle_filled(c, 4.0, ACCENT_GFX());
    } else {
        p.circle_stroke(c, 8.0, Stroke::new(1.5, MUTED()));
    }
    let sub_w = p
        .layout_no_wrap(sub.to_string(), FontId::proportional(13.0), MUTED())
        .size()
        .x
        .min(rect.width() * 0.4);
    paint_one_line(
        p,
        Pos2::new(rect.left() + 44.0, rect.center().y - 9.5),
        title,
        15.0,
        TEXT(),
        rect.width() - 64.0 - sub_w,
    );
    p.text(
        Pos2::new(rect.right() - 16.0, rect.center().y),
        Align2::RIGHT_CENTER,
        sub,
        FontId::proportional(13.0),
        MUTED(),
    );
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    let name = format!("{title} ({sub})");
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::RadioButton, ui.is_enabled(), selected, &name)
    });
    resp
}

/// Bouton rond de fermeture d'un overlay, avec la touche (« Échap ») écrite dessous, comme dans les applications
/// de bureau courantes.
pub fn close_button(ui: &mut egui::Ui, tip: &str, key: &str) -> Response {
    // Sans indication de touche (`key` vide : fenêtre basse), le bouton est plus court et la croix centrée.
    let compact = key.is_empty();
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(44.0, if compact { 40.0 } else { 58.0 }),
        Sense::click(),
    );
    let c = Pos2::new(
        rect.center().x,
        rect.top() + if compact { 20.0 } else { 18.0 },
    );
    // Survol (en fondu) : plus de contour, fond doux et croix remplie de la couleur d'accent.
    let k = hover_k(ui, resp.id, resp.hovered());
    let p = ui.painter();
    p.circle_filled(c, 17.0, SURFACE_3().gamma_multiply(k));
    let st = Stroke::new(1.6 + 0.4 * k, TEXT().lerp_to_gamma(ACCENT_TEXT(), k));
    let d = 5.0;
    p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
    p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
    if !compact {
        p.text(
            Pos2::new(c.x, rect.bottom() - 7.0),
            Align2::CENTER_CENTER,
            key,
            FontId::proportional(11.5),
            MUTED(),
        );
    }
    if resp.has_focus() && focus_visible() {
        p.circle_stroke(c, 20.0, Stroke::new(focus_width(), FOCUS()));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    show_tip(resp, tip)
}

/// Contenu libre d'une carte de réglages (textes, boutons), aligné sur les lignes de réglage (retrait de 12 px).
pub fn card_body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Petit lien cliquable sous une ligne de réglage (renvoie vers un réglage voisin) : texte d'accent, souligné au
/// survol, flèche dessinée à droite (pas de glyphe dans le texte : la police ne l'a pas dans toutes les langues). Aligné
/// sur le texte des lignes de réglage (retrait de 12 px).
pub fn setting_link(ui: &mut egui::Ui, text: &str) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(13.5), ACCENT_TEXT());
    let (tw, th) = (galley.size().x, galley.size().y);
    let arrow_w = 12.0;
    let size = Vec2::new(12.0 + tw + 6.0 + arrow_w, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let k = hover_k(ui, resp.id, resp.hovered());
    let color = ACCENT_TEXT().lerp_to_gamma(TEXT(), 0.35 * k);
    let x0 = rect.left() + 12.0;
    let y = rect.center().y;
    let p = ui.painter();
    p.galley_with_override_text_color(Pos2::new(x0, y - th / 2.0), galley, color);
    let ink = Stroke::new(1.6, color);
    let ax = x0 + tw + 6.0;
    p.line_segment([Pos2::new(ax, y), Pos2::new(ax + arrow_w - 1.0, y)], ink);
    p.line_segment(
        [
            Pos2::new(ax + arrow_w - 5.0, y - 4.0),
            Pos2::new(ax + arrow_w - 1.0, y),
        ],
        ink,
    );
    p.line_segment(
        [
            Pos2::new(ax + arrow_w - 5.0, y + 4.0),
            Pos2::new(ax + arrow_w - 1.0, y),
        ],
        ink,
    );
    if k > 0.0 {
        p.line_segment(
            [
                Pos2::new(x0, y + th / 2.0 + 1.0),
                Pos2::new(x0 + tw, y + th / 2.0 + 1.0),
            ],
            Stroke::new(1.0, color.gamma_multiply(k)),
        );
    }
    focus_ring(ui, &resp, rect.expand2(Vec2::new(-4.0, -2.0)), RADIUS_INNER);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, ui.is_enabled(), text));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Libellé suivi, juste à côté, de son interrupteur (piste d'accent quand activé, poignée blanche qui glisse).
/// Le libellé et l'interrupteur sont cliquables ensemble ; tient dans une rangée de boutons.
pub fn inline_switch(ui: &mut egui::Ui, label: &str, on: &mut bool) -> Response {
    inline_switch_scaled(ui, label, on, 1.0)
}

/// `inline_switch` dont toutes les dimensions (texte, piste, poignée, hauteur) sont multipliées par `s` (1 = taille
/// normale) : mêmes proportions, utilisé par le menu du lecteur.
pub fn inline_switch_scaled(ui: &mut egui::Ui, label: &str, on: &mut bool, s: f32) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(14.0 * s), TEXT());
    let (tw, th) = (galley.size().x, galley.size().y);
    let size = Vec2::new(tw + (10.0 + 40.0) * s, BTN_H * s);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = motion::eased(
        ui.ctx(),
        egui::Id::unique(("inline_switch", label)),
        *on,
        motion::TOGGLE,
    );
    let off_fill = SURFACE_3();
    let fill = mix(off_fill, ACCENT(), k);
    let p = ui.painter();
    p.galley(
        Pos2::new(rect.left(), rect.center().y - th / 2.0),
        galley,
        TEXT(),
    );
    let track = Rect::from_min_size(
        Pos2::new(rect.left() + tw + 10.0 * s, rect.center().y - 11.0 * s),
        Vec2::new(40.0, 22.0) * s,
    );
    p.rect(
        track,
        CornerRadius::same((11.0 * s).round() as u8),
        fill,
        Stroke::new(1.0, if *on { fill } else { BORDER_HOVER() }),
        egui::StrokeKind::Inside,
    );
    let x = track.left() + 11.0 * s + (track.width() - 22.0 * s) * k;
    p.circle_filled(Pos2::new(x, track.center().y), 8.0 * s, Color32::WHITE);
    focus_ring(ui, &resp, rect.expand2(Vec2::new(4.0, -2.0)), RADIUS_INNER);
    let on_now = *on;
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), on_now, label)
    });
    resp
}

/// Dessin d'un interrupteur de 44 × 24 : piste qui passe à la couleur d'accent, poignée blanche ; `k` = avancement (0 à 1) de l'animation.
fn paint_switch(ui: &egui::Ui, track: Rect, on: bool, k: f32) {
    let off_fill = SURFACE_3();
    let fill = Color32::from_rgb(
        (off_fill.r() as f32 + (ACCENT().r() as f32 - off_fill.r() as f32) * k) as u8,
        (off_fill.g() as f32 + (ACCENT().g() as f32 - off_fill.g() as f32) * k) as u8,
        (off_fill.b() as f32 + (ACCENT().b() as f32 - off_fill.b() as f32) * k) as u8,
    );
    let stroke = Stroke::new(1.0, if on { ACCENT_LINE() } else { BORDER_HOVER() });
    ui.painter().rect(
        track,
        CornerRadius::same(12),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let knob_x = track.left() + 12.0 + (track.width() - 24.0) * k;
    ui.painter()
        .circle_filled(Pos2::new(knob_x, track.center().y), 9.0, Color32::WHITE);
}

/// Ligne de réglage avec interrupteur à droite : le libellé (et sa précision) est cliquable.
pub fn switch_row(ui: &mut egui::Ui, label: &str, hint: Option<&str>, on: &mut bool) -> Response {
    let h = if hint.is_some() { ROW_H_HINT } else { ROW_H };
    let (rect, mut resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_INNER), SURFACE_2());
    }
    paint_highlight(ui, rect, label);
    let text_w = (rect.width() - 80.0).max(60.0);
    let (ty, hy) = if hint.is_some() {
        (rect.top() + 8.0, rect.top() + 28.0)
    } else {
        (rect.center().y - 9.0, 0.0)
    };
    paint_one_line(
        ui.painter(),
        Pos2::new(rect.left() + 12.0, ty),
        label,
        14.5,
        TEXT(),
        text_w,
    );
    if let Some(hint) = hint {
        paint_one_line(
            ui.painter(),
            Pos2::new(rect.left() + 12.0, hy),
            hint,
            12.0,
            MUTED(),
            text_w,
        );
    }
    if let Some(help) = crate::help::lookup(label) {
        let w = ui
            .painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(14.5), TEXT())
            .size()
            .x
            .min(text_w);
        let c = Pos2::new(rect.left() + 12.0 + w + 16.0, ty + 9.0);
        let hr = ui.interact(
            Rect::from_center_size(c, Vec2::splat(20.0)),
            resp.id.with("help"),
            Sense::click(),
        );
        paint_help(ui, c, &hr, label);
        show_tip(hr, help);
    }
    // Interrupteur : piste couleur d'accent quand activé, poignée blanche qui glisse.
    // Animation rattachée au libellé, pas à la position : en changeant d'onglet, l'interrupteur affiché au même
    // endroit est un autre réglage et ne doit pas « glisser » depuis la valeur du précédent.
    let k = motion::eased(
        ui.ctx(),
        egui::Id::unique(("switch_anim", label)),
        *on,
        motion::TOGGLE,
    );
    let track = Rect::from_center_size(
        Pos2::new(rect.right() - 36.0, rect.center().y),
        Vec2::new(44.0, 24.0),
    );
    paint_switch(ui, track, *on, k);
    // Focus clavier : anneau autour de toute la ligne (Espace ou Entrée bascule le réglage).
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *on, label)
    });
    resp
}

/// Interrupteur seul (même rendu que celui de `switch_row`), à poser dans une ligne de contrôles. `key` identifie
/// l'animation (le libellé du réglage).
pub fn switch_only(ui: &mut egui::Ui, key: &str, on: &mut bool) -> Response {
    let (track, mut resp) = ui.allocate_exact_size(Vec2::new(44.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = motion::eased(
        ui.ctx(),
        egui::Id::unique(("switch_anim_only", key)),
        *on,
        motion::TOGGLE,
    );
    paint_switch(ui, track, *on, k);
    focus_ring(ui, &resp, track, 12);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *on, key)
    });
    resp
}

/// Entrée de navigation : active = surface-3 ; texte et icône restent bien lisibles.
pub fn nav_item(ui: &mut egui::Ui, icon: NavIcon, label: &str, active: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let k = hover_k(ui, resp.id, resp.hovered());
    let base = if active {
        SURFACE_3()
    } else {
        Color32::TRANSPARENT
    };
    let fill = base.lerp_to_gamma(ACCENT(), k);
    let color = TEXT().lerp_to_gamma(Color32::WHITE, k);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_INNER), fill);
    paint_pressed(ui, &resp, rect, RADIUS_INNER);
    paint_nav_icon(
        ui.painter(),
        rect.left_center() + Vec2::new(20.0, 0.0),
        icon,
        color,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(38.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        color,
    );
    focus_ring(ui, &resp, rect, RADIUS_INNER);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), active, label)
    });
    resp
}

/// Champ de recherche : fond surface-2, loupe à gauche, badge de raccourci à droite.
/// Renvoie le champ de saisie et le cadre complet (pour aligner une liste de résultats dessous).
pub fn search_input(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    id: egui::Id,
    badge: &str,
) -> (Response, Rect) {
    let frame = egui::Frame::new()
        .fill(SURFACE_2())
        .stroke(hairline())
        .corner_radius(CornerRadius::same(RADIUS_INNER))
        .inner_margin(egui::Margin::symmetric(10, 6));
    let full = ui.available_width();
    let out = frame.show(ui, |ui| {
        ui.set_min_width((full - 22.0).max(60.0));
        ui.horizontal(|ui| {
            // Loupe dessinée (pas d'icône emoji).
            let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            let c = r.center() - Vec2::splat(1.5);
            let st = Stroke::new(1.4, MUTED());
            ui.painter().circle_stroke(c, 4.5, st);
            ui.painter()
                .line_segment([c + Vec2::splat(3.3), c + Vec2::splat(7.0)], st);
            // Sans badge de raccourci, le texte d'aide prend toute la largeur.
            let w = (ui.available_width() - if badge.is_empty() { 4.0 } else { 52.0 }).max(40.0);
            // Curseur de saisie à la couleur d'accent.
            ui.visuals_mut().text_cursor.stroke.color = ACCENT_GFX();
            let edit = ui.add(
                egui::TextEdit::singleline(text)
                    .id(id)
                    .hint_text(RichText::new(hint).color(FAINT()))
                    .frame(egui::Frame::NONE)
                    .align(egui::Align2::LEFT_CENTER)
                    .min_size(Vec2::new(w, 22.0))
                    .desired_width(w),
            );
            if !badge.is_empty() {
                ui.label(RichText::new(badge).size(10.5).color(FAINT()));
            }
            a11y_label(&edit, hint);
            edit
        })
        .inner
    });
    (out.inner, out.response.rect)
}

/// Texte tronqué sur une ligne (ellipse), peint à une position donnée.
pub fn paint_one_line(
    p: &egui::Painter,
    pos: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
) {
    paint_one_line_font(p, pos, text, FontId::proportional(size), color, width);
}

/// Comme `paint_one_line` pour un nom de chaîne (police semi-grasse).
pub fn paint_name_line(
    p: &egui::Painter,
    pos: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
) {
    paint_one_line_font(p, pos, text, name_font(size), color, width);
}

fn paint_one_line_font(
    p: &egui::Painter,
    pos: Pos2,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
) {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = p.layout_job(job);
    p.galley(pos, galley, color);
}

// ---------- Icônes dessinées (sans glyphes ni emojis) ----------

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Play,
    Pause,
    Speaker,
    Muted,
    Close,
    Fullscreen,
    ExitFullscreen,
    Pip,
    ChevronRight,
    ChevronLeft,
    /// Roue dentée : options du lecteur.
    Gear,
    /// Fenêtre avec une petite incrustation en bas à droite et une flèche : retour à la taille et à la place d'origine.
    Reset,
}

#[derive(Clone, Copy, PartialEq)]
pub enum IconStyle {
    /// Bouton secondaire (surface-2).
    Normal,
    /// Sur la vidéo : fond noir translucide.
    Overlay,
    /// Fond sombre opaque, icône blanche qui passe au rouge au survol (fermer).
    AccentDanger,
    /// Fond sombre opaque, icône blanche qui passe à la couleur d'accentuation au survol.
    AccentLight,
}

/// Bouton carré arrondi portant une icône dessinée.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    size: f32,
    style: IconStyle,
    tip: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let k = hover_k(ui, resp.id, resp.hovered());
    let (fill, stroke) = match style {
        IconStyle::Normal => (
            SURFACE_2().lerp_to_gamma(ACCENT(), k),
            Stroke::new(1.0, BORDER().gamma_multiply(1.0 - k)),
        ),
        IconStyle::Overlay => (
            Color32::from_black_alpha(150).lerp_to_gamma(Color32::from_black_alpha(210), k),
            Stroke::NONE,
        ),
        IconStyle::AccentDanger | IconStyle::AccentLight => (
            Color32::from_black_alpha(205).lerp_to_gamma(Color32::from_black_alpha(240), k),
            Stroke::NONE,
        ),
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(8),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    paint_pressed(ui, &resp, rect, 8);
    let c = rect.center();
    let s = size;
    let ink = match style {
        IconStyle::AccentDanger => Color32::WHITE.lerp_to_gamma(NEGATIVE(), k),
        IconStyle::AccentLight => Color32::WHITE.lerp_to_gamma(ACCENT(), k),
        _ => Color32::WHITE,
    };
    let line = Stroke::new((s * 0.07).max(1.6), ink);
    match icon {
        Icon::Reset => {
            // Grand cadre, petit lecteur plein en bas à droite.
            let frame = Rect::from_center_size(
                Pos2::new(c.x - s * 0.02, c.y - s * 0.02),
                Vec2::splat(s * 0.5),
            );
            p.rect_stroke(frame, CornerRadius::same(2), line, egui::StrokeKind::Middle);
            let small = Rect::from_min_size(
                Pos2::new(frame.right() - s * 0.26, frame.bottom() - s * 0.18),
                Vec2::new(s * 0.3, s * 0.2),
            );
            p.rect_filled(small, CornerRadius::same(1), ink);
        }
        Icon::Pause => {
            for dx in [-0.12, 0.12] {
                let r = Rect::from_center_size(
                    Pos2::new(c.x + s * dx, c.y),
                    Vec2::new(s * 0.12, s * 0.38),
                );
                p.rect_filled(r, CornerRadius::same(1), ink);
            }
        }
        Icon::Play => {
            let pts = vec![
                Pos2::new(c.x - s * 0.10, c.y - s * 0.20),
                Pos2::new(c.x - s * 0.10, c.y + s * 0.20),
                Pos2::new(c.x + s * 0.20, c.y),
            ];
            p.add(egui::Shape::convex_polygon(pts, ink, Stroke::NONE));
        }
        Icon::Pip => {
            // Fenêtre avec une petite incrustation en bas à droite.
            let outer = Rect::from_center_size(c, Vec2::new(s * 0.46, s * 0.34));
            p.rect_stroke(outer, CornerRadius::same(2), line, egui::StrokeKind::Middle);
            let inner = Rect::from_min_size(
                Pos2::new(outer.right() - s * 0.20, outer.bottom() - s * 0.14),
                Vec2::new(s * 0.15, s * 0.10),
            );
            p.rect_filled(inner, CornerRadius::same(1), ink);
        }
        Icon::Fullscreen | Icon::ExitFullscreen => {
            // Quatre angles : tournés vers l'extérieur pour agrandir, vers l'intérieur pour quitter.
            let (d, arm) = if icon == Icon::Fullscreen {
                (s * 0.20, s * 0.12)
            } else {
                (s * 0.07, s * 0.13)
            };
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = c + Vec2::new(sx * d, sy * d);
                let dir = if icon == Icon::Fullscreen { -1.0 } else { 1.0 };
                p.line_segment([corner, corner + Vec2::new(sx * arm * dir, 0.0)], line);
                p.line_segment([corner, corner + Vec2::new(0.0, sy * arm * dir)], line);
            }
        }
        Icon::ChevronRight | Icon::ChevronLeft => {
            let (d, h) = (s * 0.09, s * 0.17);
            let dx = if matches!(icon, Icon::ChevronRight) {
                d
            } else {
                -d
            };
            p.line_segment([c + Vec2::new(-dx, -h), c + Vec2::new(dx, 0.0)], line);
            p.line_segment([c + Vec2::new(dx, 0.0), c + Vec2::new(-dx, h)], line);
        }
        Icon::Gear => {
            // Anneau, huit dents, moyeu (même dessin que l'engrenage du bouton « Réglages »).
            p.circle_stroke(c, s * 0.17, line);
            p.circle_stroke(c, s * 0.06, line);
            let teeth = Stroke::new((s * 0.09).max(2.0), ink);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * (s * 0.215), c + d * (s * 0.30)], teeth);
            }
        }
        Icon::Close => {
            let d = s * 0.17;
            p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], line);
            p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], line);
        }
        Icon::Speaker | Icon::Muted => {
            // Haut-parleur épuré : boîtier + cône pleins (même position dans les deux états, pas de saut au
            // changement), puis deux ondes simples ou une croix nette, au même trait que les autres icônes.
            let bx = c.x - s * 0.22;
            let pts = vec![
                Pos2::new(bx, c.y - s * 0.07),
                Pos2::new(bx + s * 0.09, c.y - s * 0.07),
                Pos2::new(bx + s * 0.24, c.y - s * 0.20),
                Pos2::new(bx + s * 0.24, c.y + s * 0.20),
                Pos2::new(bx + s * 0.09, c.y + s * 0.07),
                Pos2::new(bx, c.y + s * 0.07),
            ];
            p.add(egui::Shape::convex_polygon(pts, ink, Stroke::NONE));
            if icon == Icon::Speaker {
                let centre = Pos2::new(bx + s * 0.26, c.y);
                for r in [0.12, 0.22] {
                    let arc: Vec<Pos2> = (-3..=3)
                        .map(|i| {
                            let a = i as f32 * 0.22;
                            Pos2::new(centre.x + s * r * a.cos(), centre.y + s * r * a.sin())
                        })
                        .collect();
                    p.add(egui::Shape::line(arc, line));
                }
            } else {
                let o = Pos2::new(bx + s * 0.40, c.y);
                let d = s * 0.08;
                p.line_segment([o + Vec2::new(-d, -d), o + Vec2::new(d, d)], line);
                p.line_segment([o + Vec2::new(-d, d), o + Vec2::new(d, -d)], line);
            }
        }
    }
    focus_ring(ui, &resp, rect, 8);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tip));
    show_tip(resp, tip)
}

/// Barre de progression lisible : fond surface-3, remplissage d'accent, texte centré. Le texte est dessiné deux fois,
/// chacun rogné à une des deux zones : sur la partie remplie il prend la couleur (blanc ou noir) la plus contrastée avec
/// l'accent, sur la partie vide celle du texte du thème, ramenée à au moins 4,5:1 sur la surface-3. Un mot à cheval sur
/// la limite change de couleur à l'endroit exact où le fond change.
pub fn progress_bar(ui: &mut egui::Ui, fraction: f32, text: &str) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let r = CornerRadius::same(8);
    let p = ui.painter();
    p.rect_filled(rect, r, SURFACE_3());
    let f = fraction.clamp(0.0, 1.0);
    let split = if f > 0.0 {
        (rect.width() * f).max(12.0).min(rect.width())
    } else {
        0.0
    };
    let fill = Rect::from_min_size(rect.min, Vec2::new(split, rect.height()));
    if f > 0.0 {
        p.rect_filled(fill, r, ACCENT());
    }
    let label = format!("{text}  {} %", (f * 100.0).round() as u32);
    // Blanc tant qu'il atteint 4,5:1 sur l'accent ; noir seulement pour un accent trop clair.
    let on_fill = if contrast(Color32::WHITE, ACCENT()) >= 4.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let on_empty = readable_on(TEXT(), SURFACE_3());
    let rest = Rect::from_min_max(Pos2::new(rect.left() + split, rect.top()), rect.max);
    for (zone, color) in [(fill, on_fill), (rest, on_empty)] {
        if zone.width() > 0.0 {
            p.with_clip_rect(zone.intersect(p.clip_rect())).text(
                rect.center(),
                Align2::CENTER_CENTER,
                &label,
                FontId::proportional(12.5),
                color,
            );
        }
    }
    resp
}

// ---- Fenêtre sans bordure Windows : barre de titre, bords, redimensionnement ----
// Partagé entre l'application (`app/fenetre.rs`) et l'outil d'admin (même cadre dans les deux fenêtres).

/// Hauteur de la barre de titre intégrée.
pub const TITLE_BAR_H: f32 = 36.0;

/// macOS : la fenêtre garde son cadre natif (bords, ombre, redimensionnement) et ses trois boutons ronds en haut à
/// gauche ; l'application ne dessine alors que la zone de déplacement. Windows : aucun cadre, tout est dessiné ici.
pub const NATIVE_FRAME: bool = cfg!(target_os = "macos");

/// Cadre de la fenêtre principale (appli et outil d'admin). Windows : sans bordure ni barre système. macOS : cadre
/// natif, barre de titre transparente et sans titre, contenu sur toute la hauteur (sous les boutons ronds).
/// Piège macOS : repasser `Decorations(true)` après `Decorations(false)` perd le contenu pleine hauteur (winit refait
/// le style de la fenêtre sans lui) ; ne jamais retirer le cadre sur Mac.
pub fn window_frame(vb: egui::ViewportBuilder) -> egui::ViewportBuilder {
    if NATIVE_FRAME {
        vb.with_decorations(true)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
    } else {
        vb.with_decorations(false)
    }
}

/// Zone de déplacement de la fenêtre : glisser = déplacer (le système gère l'ancrage aux bords et la sortie du mode
/// agrandi), double-clic = agrandir ou restaurer ; pas d'arrêt au clavier.
pub fn window_drag_zone(ui: &egui::Ui, ctx: &egui::Context, rect: Rect, salt: &str) {
    let drag = ui.interact(
        rect,
        egui::Id::unique(("title_drag", salt)),
        Sense::CLICK | Sense::DRAG,
    );
    if drag.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if drag.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

#[derive(Clone, Copy)]
enum WinButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Bouton de fenêtre au style de Windows 11 : fond gris au survol, rouge pour Fermer ; icône fine dessinée.
fn win_button(ui: &egui::Ui, resp: &Response, r: Rect, kind: WinButton, label: &str) -> Response {
    let hovered = resp.hovered() || resp.is_pointer_button_down_on();
    let close = matches!(kind, WinButton::Close);
    let p = ui.painter();
    if hovered {
        let fill = if close {
            Color32::from_rgb(196, 43, 28)
        } else {
            SURFACE_3()
        };
        p.rect_filled(r, CornerRadius::ZERO, fill);
    }
    let ink = if hovered && close {
        Color32::WHITE
    } else {
        TEXT()
    };
    let st = Stroke::new(1.0, ink);
    let c = r.center();
    let h = 5.0;
    match kind {
        WinButton::Minimize => {
            p.line_segment([c + Vec2::new(-h, 0.0), c + Vec2::new(h, 0.0)], st);
        }
        WinButton::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(2.0 * h)),
                CornerRadius::same(1),
                st,
                egui::StrokeKind::Middle,
            );
        }
        WinButton::Restore => {
            let back = Rect::from_center_size(c + Vec2::new(1.5, -1.5), Vec2::splat(2.0 * h - 2.0));
            p.line_segment(
                [back.left_top() + Vec2::new(0.0, 0.0), back.right_top()],
                st,
            );
            p.line_segment([back.right_top(), back.right_bottom()], st);
            p.rect_stroke(
                Rect::from_center_size(c + Vec2::new(-1.5, 1.5), Vec2::splat(2.0 * h - 2.0)),
                CornerRadius::same(1),
                st,
                egui::StrokeKind::Middle,
            );
        }
        WinButton::Close => {
            p.line_segment([c + Vec2::new(-h, -h), c + Vec2::new(h, h)], st);
            p.line_segment([c + Vec2::new(-h, h), c + Vec2::new(h, -h)], st);
        }
    }
    focus_ring(ui, resp, r.shrink(3.0), 4);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    show_tip(resp.clone(), label)
}

/// Barre de titre intégrée (la fenêtre n'a pas de bordure Windows) : zone de déplacement (double-clic = agrandir
/// ou restaurer) et boutons Réduire, Agrandir / Restaurer, Fermer en haut à droite, comme ceux de Windows.
/// `salt` distingue les identifiants quand la barre est dessinée plusieurs fois ; `labels` = Réduire, Agrandir,
/// Restaurer, Fermer (déjà traduits par l'appelant). macOS (`NATIVE_FRAME`) : zone de déplacement seule, les boutons
/// ronds du système sont en haut à gauche.
pub fn window_title_bar(ui: &mut egui::Ui, ctx: &egui::Context, salt: &str, labels: [&str; 4]) {
    const BTN: Vec2 = Vec2::new(46.0, TITLE_BAR_H);
    let rect = ui.max_rect();
    if NATIVE_FRAME {
        window_drag_zone(ui, ctx, rect, salt);
        return;
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let buttons_left = rect.right() - 3.0 * BTN.x;
    window_drag_zone(
        ui,
        ctx,
        Rect::from_min_max(rect.min, Pos2::new(buttons_left, rect.bottom())),
        salt,
    );
    let [minimize, maximize, restore, close] = labels;
    let buttons = [
        (WinButton::Minimize, minimize),
        (
            if maximized {
                WinButton::Restore
            } else {
                WinButton::Maximize
            },
            if maximized { restore } else { maximize },
        ),
        (WinButton::Close, close),
    ];
    for (i, (kind, label)) in buttons.into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(buttons_left + i as f32 * BTN.x, rect.top()), BTN);
        let resp = ui.interact(r, egui::Id::unique(("win_button", salt, i)), Sense::click());
        if win_button(ui, &resp, r, kind, label).clicked() {
            match kind {
                WinButton::Minimize => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true))
                }
                WinButton::Maximize | WinButton::Restore => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized))
                }
                WinButton::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }
}

/// Direction de redimensionnement selon la position du pointeur près des bords.
pub fn resize_zone(
    rect: Rect,
    p: Pos2,
    edge: f32,
) -> Option<(egui::ResizeDirection, egui::CursorIcon)> {
    use egui::{CursorIcon, ResizeDirection};
    let (l, r) = (p.x - rect.left() < edge, rect.right() - p.x < edge);
    let (tp, b) = (p.y - rect.top() < edge, rect.bottom() - p.y < edge);
    match (l, r, tp, b) {
        (true, _, true, _) => Some((ResizeDirection::NorthWest, CursorIcon::ResizeNwSe)),
        (_, true, true, _) => Some((ResizeDirection::NorthEast, CursorIcon::ResizeNeSw)),
        (true, _, _, true) => Some((ResizeDirection::SouthWest, CursorIcon::ResizeNeSw)),
        (_, true, _, true) => Some((ResizeDirection::SouthEast, CursorIcon::ResizeNwSe)),
        (true, ..) => Some((ResizeDirection::West, CursorIcon::ResizeHorizontal)),
        (_, true, ..) => Some((ResizeDirection::East, CursorIcon::ResizeHorizontal)),
        (_, _, true, _) => Some((ResizeDirection::North, CursorIcon::ResizeVertical)),
        (_, _, _, true) => Some((ResizeDirection::South, CursorIcon::ResizeVertical)),
        _ => None,
    }
}

/// Bords de la fenêtre sans bordure : redimensionnement à la souris (sauf agrandie), et liseré fin quand elle
/// flotte (Windows 11 dessine en plus ses coins arrondis). Rien sur macOS : le cadre natif fait tout.
pub fn window_edges(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized || NATIVE_FRAME {
        return;
    }
    let rect = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("window_border"),
    ))
    .rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, BORDER_HOVER()),
        egui::StrokeKind::Inside,
    );
    if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
        if let Some((dir, icon)) = resize_zone(rect, pos, 5.0) {
            ctx.set_cursor_icon(icon);
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    }
}

/// Fenêtre agrandie : Windows la fait déborder de l'écran de l'épaisseur de son cadre (environ 8 px de chaque côté).
/// Sans bordure Windows, ce débordement couperait le contenu : on le compense par une marge égale.
pub fn maximized_inset(ctx: &egui::Context) -> f32 {
    ctx.input(|i| {
        let vp = i.viewport();
        match (vp.maximized, vp.outer_rect, vp.monitor_size) {
            (Some(true), Some(outer), Some(monitor)) => {
                ((outer.width() - monitor.x) / 2.0).clamp(0.0, 16.0)
            }
            _ => 0.0,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noms affichés (français) : le noir (valeur 0) s'appelle « Dark », le bleu nuit (valeur 1) « Sombre ».
    #[test]
    fn noms_des_themes_noir_et_bleu_nuit() {
        let names = theme_names();
        assert_eq!((names[0], names[1], names[2]), ("Dark", "Sombre", "Clair"));
        // Valeur 0 : presque noir, quasi neutre ; valeur 1 : bleuté (B nettement au-dessus de R).
        let (noir, bleu) = (palette_of(0).bg, palette_of(1).bg);
        assert!(noir.r() < 16 && noir.b() <= noir.r() + 3, "valeur 0 = noir");
        assert!(bleu.b() > bleu.r() + 8, "valeur 1 = bleu nuit");
    }

    /// Texte courant (TEXT, MUTED, FAINT, POSITIVE, NEGATIVE) : au moins 4,5:1 sur les fonds où il est posé
    /// (bg, surface-1, surface-2), dans les trois thèmes (WCAG 1.4.3, adapté par WCAG2ICT).
    #[test]
    fn contrastes_du_texte_aa() {
        for (name, p) in [("Sombre", &SOMBRE), ("Dark", &DARK), ("Clair", &CLAIR)] {
            for (tok, c) in [
                ("text", p.text),
                ("muted", p.muted),
                ("faint", p.faint),
                ("positive", p.positive),
                ("negative", p.negative),
            ] {
                for (surf, bg) in [("bg", p.bg), ("s1", p.s1), ("s2", p.s2)] {
                    let r = contrast(c, bg);
                    assert!(r >= 4.5, "{name} : {tok} sur {surf} = {r:.2}:1");
                }
            }
        }
    }

    /// Contraste élevé : texte, valeurs positive et négative au moins à 7:1 (bg, surface-1, surface-2) et bordures au
    /// moins à 3:1 sur leur surface (WCAG 1.4.6 et 1.4.11), dans les trois thèmes.
    #[test]
    fn contraste_eleve_7_a_1() {
        // Couleur translucide (prémultipliée) posée sur un fond opaque.
        let over = |c: Color32, bg: Color32| {
            let k =
                |f: u8, b: u8| (f as f32 + b as f32 * (1.0 - c.a() as f32 / 255.0)).round() as u8;
            Color32::from_rgb(k(c.r(), bg.r()), k(c.g(), bg.g()), k(c.b(), bg.b()))
        };
        for theme in 0..3u8 {
            let p = palette_for(theme, true);
            for (tok, c) in [
                ("text", p.text),
                ("muted", p.muted),
                ("faint", p.faint),
                ("positive", p.positive),
                ("negative", p.negative),
            ] {
                for (surf, bg) in [("bg", p.bg), ("s1", p.s1), ("s2", p.s2)] {
                    let r = contrast(c, bg);
                    assert!(r >= 7.0, "thème {theme} : {tok} sur {surf} = {r:.2}:1");
                }
            }
            for (surf, bg) in [("bg", p.bg), ("s1", p.s1), ("s2", p.s2)] {
                let r = contrast(over(p.border, bg), bg);
                assert!(r >= 3.0, "thème {theme} : bordure sur {surf} = {r:.2}:1");
            }
        }
    }

    /// Le contraste élevé ne change que les couleurs de texte et de bordure, jamais les surfaces.
    #[test]
    fn contraste_eleve_garde_les_surfaces() {
        for theme in 0..3u8 {
            let (n, h) = (palette_for(theme, false), palette_for(theme, true));
            assert_eq!((n.bg, n.s1, n.s2, n.s3), (h.bg, h.s1, h.s2, h.s3));
        }
    }

    /// Palettes prédéfinies : le texte blanc des boutons y est lisible (4,5:1) ; en contraste élevé la couleur est
    /// assombrie jusqu'à 7:1 ; les variantes pour texte, traits et anneaux tiennent le minimum sur tous les fonds, dans
    /// les trois thèmes.
    #[test]
    fn palettes_conformes() {
        for (name, rgb) in PALETTES {
            let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            assert!(
                fill_ok(c, 4.5),
                "{name} : texte blanc sur l'accent = {:.2}:1",
                contrast(Color32::WHITE, c)
            );
            for hc in [false, true] {
                let ratio = required_ratio(hc);
                let set = compute_accent(Some(rgb), hc);
                assert!(
                    fill_ok(set.fill, ratio),
                    "{name} (contraste élevé : {hc}) : fond de bouton"
                );
                assert!(
                    fill_ok(set.hover, ratio),
                    "{name} (contraste élevé : {hc}) : survol"
                );
                for (light, text) in [(false, set.text_dark), (true, set.text_light)] {
                    let p = palette_for(if light { 2 } else { 0 }, hc);
                    for (surf, bg) in [("bg", p.bg), ("s1", p.s1), ("s2", p.s2)] {
                        let r = contrast(text, bg);
                        assert!(
                            r >= ratio,
                            "{name} : texte d'accent sur {surf} (clair : {light}) = {r:.2}:1"
                        );
                    }
                }
            }
        }
    }

    /// Toute couleur proposée par l'utilisateur ressort conforme, la couleur déjà conforme est gardée telle quelle,
    /// et l'alternative garde la teinte (jamais plus sombre que nécessaire).
    #[test]
    fn alternative_conforme_la_plus_proche() {
        let essais = [
            [255, 255, 0],
            [255, 255, 255],
            [0, 255, 255],
            [60, 110, 255],
            [200, 200, 200],
            [0, 0, 0],
            [128, 128, 128],
            [255, 0, 0],
        ];
        for rgb in essais {
            let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            for ratio in [4.5, 7.0] {
                let alt = nearest_compliant_fill(c, ratio);
                assert!(
                    fill_ok(alt, ratio),
                    "{rgb:?} -> {alt:?} reste insuffisant ({:.2}:1)",
                    contrast(Color32::WHITE, alt)
                );
                if fill_ok(c, ratio) {
                    assert_eq!(alt, c, "une couleur conforme n'est pas modifiée");
                } else {
                    // Un pas de moins serait encore insuffisant : c'est bien la plus proche.
                    let pas = (1..=100)
                        .find(|i| fill_ok(mix(c, Color32::BLACK, *i as f32 / 100.0), ratio))
                        .unwrap_or(100);
                    assert!(!fill_ok(
                        mix(c, Color32::BLACK, (pas - 1) as f32 / 100.0),
                        ratio
                    ));
                    assert!(
                        alt.r() <= c.r() && alt.g() <= c.g() && alt.b() <= c.b(),
                        "assombrie, jamais éclaircie"
                    );
                }
            }
        }
    }

    /// Sans couleur choisie, l'accent est l'orange par défaut.
    #[test]
    fn accent_par_defaut_orange() {
        assert_eq!(
            compute_accent(None, false),
            compute_accent(Some(ACCENT_NEW), false)
        );
        assert_eq!(
            compute_accent(None, true),
            compute_accent(Some(ACCENT_NEW), true)
        );
        assert_eq!(unpack(ACC_FILL.load(Ordering::Relaxed)).r(), ACCENT_NEW[0]);
    }

    #[test]
    fn codes_couleur() {
        assert_eq!(parse_hex("#CA4F24"), Some([202, 79, 36]));
        assert_eq!(parse_hex("ca4f24"), Some([202, 79, 36]));
        assert_eq!(parse_hex("#91"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
        assert_eq!(hex_of([202, 79, 36]), "#CA4F24");
        for (_, rgb) in PALETTES {
            assert_eq!(parse_hex(&hex_of(rgb)), Some(rgb));
        }
    }

    /// Interrupteurs d'accessibilité : défaut, activation, retour à l'état initial.
    #[test]
    fn interrupteurs_d_accessibilite() {
        assert!(!HIGH_CONTRAST.load(Ordering::Relaxed) && !FOCUS_STRONG.load(Ordering::Relaxed));
        assert!((focus_width() - 2.0).abs() < f32::EPSILON);
        set_high_contrast(true);
        set_focus_strong(true);
        assert_eq!(palette_of(0).text, Color32::WHITE);
        assert!(focus_visible(), "focus renforcé : l'anneau reste visible");
        assert!(focus_width() > 3.0);
        set_high_contrast(false);
        set_focus_strong(false);
        assert_eq!(palette_of(0).muted, SOMBRE.muted);
        assert!((focus_width() - 2.0).abs() < f32::EPSILON);
    }

    /// Texte de la barre de progression (M11) : au moins 4,5:1 sur la partie vide (surface-3, trois thèmes) et sur la
    /// partie remplie (blanc ou noir, le plus contrasté, sur chacun des accents proposés).
    #[test]
    fn contraste_de_la_barre_de_progression() {
        for (name, p) in [("Sombre", &SOMBRE), ("Dark", &DARK), ("Clair", &CLAIR)] {
            let r = contrast(readable_on(p.text, p.s3), p.s3);
            assert!(r >= 4.5, "{name} : texte sur surface-3 = {r:.2}:1");
        }
        for (name, rgb) in PALETTES {
            let accent = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let best = contrast(Color32::WHITE, accent).max(contrast(Color32::BLACK, accent));
            assert!(best >= 4.5, "accent {name} : meilleur texte = {best:.2}:1");
        }
    }

    /// Anneau de focus : au moins 3:1 sur toutes les surfaces (WCAG 1.4.11).
    #[test]
    fn contraste_du_focus() {
        for (theme, p) in [(0u8, &SOMBRE), (1, &DARK), (2, &CLAIR)] {
            CURRENT.store(theme, Ordering::Relaxed);
            for bg in [p.bg, p.s1, p.s2, p.s3] {
                assert!(
                    contrast(FOCUS(), bg) >= 3.0,
                    "thème {theme} : {:.2}:1",
                    contrast(FOCUS(), bg)
                );
            }
        }
        CURRENT.store(0, Ordering::Relaxed);
    }

    /// Boutons rouges à texte blanc : le rouge du thème reste tel quel là où il est assez foncé (Sombre, Dark, Clair et
    /// Clair en contraste élevé), et il est assombri à 7:1 avec le blanc en contraste élevé sur fond sombre (sinon rose
    /// pâle : voile blanc). Le survol éclaircit le remplissage et garde le texte blanc à 4,5:1 au moins ; il reste
    /// proche de l'ancien rouge fixe (244, 110, 118) en Sombre et Dark.
    #[test]
    fn rouge_des_boutons_suit_le_theme() {
        let close = |a: Color32, b: Color32| {
            a.r().abs_diff(b.r()) <= 8 && a.g().abs_diff(b.g()) <= 8 && a.b().abs_diff(b.b()) <= 8
        };
        for p in [&SOMBRE, &DARK, &CLAIR, &CLAIR_HC] {
            assert_eq!(negative_fill_of(p.negative), p.negative);
        }
        for p in [&SOMBRE_HC, &DARK_HC] {
            let fill = negative_fill_of(p.negative);
            assert!(
                contrast(Color32::WHITE, fill) >= 7.0,
                "base {:.2}:1",
                contrast(Color32::WHITE, fill)
            );
        }
        assert!(close(
            negative_hover_of(SOMBRE.negative),
            Color32::from_rgb(244, 110, 118)
        ));
        assert!(close(
            negative_hover_of(DARK.negative),
            Color32::from_rgb(244, 110, 118)
        ));
        for p in [&SOMBRE_HC, &DARK_HC, &CLAIR_HC] {
            let hover = negative_hover_of(negative_fill_of(p.negative));
            assert!(
                contrast(Color32::WHITE, hover) >= 4.5,
                "survol {:.2}:1",
                contrast(Color32::WHITE, hover)
            );
        }
    }

    #[test]
    fn pseudo_lisible() {
        let bleu = Color32::from_rgb(0, 0, 255);
        assert!(contrast(readable_on(bleu, SOMBRE.s1), SOMBRE.s1) >= 4.5);
        let jaune = Color32::from_rgb(255, 255, 0);
        assert!(contrast(readable_on(jaune, CLAIR.s1), CLAIR.s1) >= 4.5);
        let deja_lisible = Color32::from_rgb(50, 205, 50);
        assert_eq!(readable_on(deja_lisible, SOMBRE.s1), deja_lisible);
    }

    #[test]
    fn curseur_atteint_ses_bornes() {
        assert!((snap(0.0, 0.8, 1.5, 0.05) - 0.8).abs() < 1e-4);
        assert!((snap(1.0, 0.8, 1.5, 0.05) - 1.5).abs() < 1e-4);
        assert!((snap(0.5, 0.8, 1.5, 0.05) - 1.15).abs() < 1e-4);
        assert!((snap(1.0, 0.5, 3.0, 0.1) - 3.0).abs() < 1e-4);
        assert!((snap(0.0, 0.5, 3.0, 0.1) - 0.5).abs() < 1e-4);
    }
}
