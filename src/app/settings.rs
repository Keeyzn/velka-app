//! Réglages : lecture, compte Twitch, outils, journal.
use super::{Confirm, VelkaApp, View};
use crate::tools::Tool;
use crate::ui::first_run::ToolStatus;
use crate::ui::theme as t;
use eframe::egui::{self, RichText};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type ThemeSlot = Arc<Mutex<Option<Arc<egui::ColorImage>>>>;

/// Changement de thème en cours.
pub(super) enum ThemeShot {
    /// Photo de l'ancien thème demandée : le nouveau thème n'est appliqué qu'à son arrivée.
    Waiting { since: Instant, slot: ThemeSlot },
    /// Photo posée par-dessus le nouveau thème, de plus en plus transparente.
    Fading {
        tex: egui::TextureHandle,
        start: f64,
    },
}

/// Libellés par valeur enregistrée (0 à 4 ; 1080p est en dernier pour garder les anciens réglages valides).
pub const QUALITIES: [&str; 5] = ["Source", "720p", "480p", "Audio", "1080p"];
/// Ordre d'affichage, du plus haut au plus bas (valeurs enregistrées).
pub(super) const QUALITY_ORDER: [u8; 5] = [0, 4, 1, 2, 3];

/// Largeur approximative du sélecteur de qualité complet (pour décider de le replier dans la barre du lecteur).
pub fn quality_segmented_width(ui: &egui::Ui) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let texts: f32 = QUALITIES
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).to_string(), font.clone(), egui::Color32::WHITE)
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x
        })
        .sum();
    texts + 2.0 * (QUALITIES.len() as f32 - 1.0) + 8.0
}

/// Version repliée : un seul bouton (qualité active + flèche) qui déploie la liste au clic. Même hauteur que le
/// sélecteur complet, survol en fondu avec remplissage d'accent. Renvoie vrai si la valeur change.
pub fn quality_dropdown(ui: &mut egui::Ui, quality: &mut u8, height: f32, icon_only: bool) -> bool {
    quality_dropdown_sized(ui, quality, height, icon_only, 0.0)
}

/// Comme `quality_dropdown`, au moins `min_w` de large (page Multistream : même largeur que le bouton « Ajouter »).
pub fn quality_dropdown_min(ui: &mut egui::Ui, quality: &mut u8, height: f32, min_w: f32) -> bool {
    quality_dropdown_sized(ui, quality, height, false, min_w)
}

fn quality_dropdown_sized(
    ui: &mut egui::Ui,
    quality: &mut u8,
    height: f32,
    icon_only: bool,
    min_w: f32,
) -> bool {
    let font = egui::FontId::proportional(14.0);
    let widest = QUALITIES
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).to_string(), font.clone(), egui::Color32::WHITE)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let width = if icon_only {
        height
    } else {
        (widest + 12.0 + 8.0 + 12.0 + 10.0).max(min_w)
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let popup_id = egui::Popup::default_response_id(&resp);
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    let k = t::hover_k(ui, resp.id, resp.hovered() || open);
    let p = ui.painter();
    p.rect(
        rect,
        egui::CornerRadius::same(t::RADIUS_INNER),
        t::SURFACE_2().lerp_to_gamma(t::ACCENT(), k),
        egui::Stroke::new(1.0, t::BORDER().gamma_multiply(1.0 - k)),
        egui::StrokeKind::Inside,
    );
    let fg = t::TEXT().lerp_to_gamma(egui::Color32::WHITE, k);
    let current = QUALITIES
        .get(*quality as usize)
        .copied()
        .unwrap_or("Source");
    let st = egui::Stroke::new(1.6, fg);
    if icon_only {
        // Barre étroite : trois barres de signal (qualité) ; la qualité active est dite par l'infobulle.
        let c = rect.center();
        for (i, h) in [5.0_f32, 9.0, 13.0].into_iter().enumerate() {
            let x = c.x + (i as f32 - 1.0) * 5.0;
            p.line_segment([egui::pos2(x, c.y + 6.5), egui::pos2(x, c.y + 6.5 - h)], st);
        }
    } else {
        p.text(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            current,
            font,
            fg,
        );
        // Flèche vers le haut (la liste s'ouvre au-dessus de la barre).
        let c = egui::pos2(rect.right() - 14.0, rect.center().y);
        p.line_segment([c + egui::vec2(-4.0, 2.0), c + egui::vec2(0.0, -2.0)], st);
        p.line_segment([c + egui::vec2(0.0, -2.0), c + egui::vec2(4.0, 2.0)], st);
    }
    t::focus_ring(ui, &resp, rect, t::RADIUS_INNER);
    let name = crate::i18n::tr("Qualité");
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("{name} : {current}"),
        )
    });
    if icon_only && !open {
        let _ = t::show_tip(resp.clone(), format!("{name} : {current}"));
    }
    let mut changed = false;
    let menu = egui::Popup::from_toggle_button_response(&resp)
        .align(egui::RectAlign::TOP_END)
        .align_alternatives(&[egui::RectAlign::BOTTOM_END])
        .gap(6.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .frame(
            egui::Frame::new()
                .fill(t::SURFACE_2())
                .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(4.0),
        )
        .show(|ui| {
            let _dark = t::DarkScope::enter(ui);
            ui.set_width(rect.width().max(96.0));
            for &q in QUALITY_ORDER.iter() {
                let label = QUALITIES[q as usize];
                if t::menu_item(ui, label, q == *quality).clicked() && q != *quality {
                    *quality = q;
                    changed = true;
                }
            }
        });
    t::grow_popup(
        ui.ctx(),
        resp.id,
        open,
        menu.as_ref().map(|m| &m.response),
        rect.center(),
    );
    changed
}

/// Valeur de `sleep_timer` qui désigne la durée personnalisée (`sleep_custom_min`).
pub const SLEEP_CUSTOM: u8 = 5;

/// Libellés du minuteur de veille, dans l'ordre des valeurs de `sleep_timer` (page Réglages et menu du lecteur).
pub fn sleep_labels() -> [&'static str; 6] {
    let tr = crate::i18n::tr;
    [
        tr("Désactivé"),
        "15 min",
        "30 min",
        "1 h",
        "2 h",
        tr("Perso"),
    ]
}

/// Durée en minutes écrite en minutes sous une heure, puis en heures (« 45 min », « 1 h 30 », « 2 h »).
pub fn format_minutes(m: u32) -> String {
    if m < 60 {
        format!("{m} min")
    } else if m.is_multiple_of(60) {
        format!("{} h", m / 60)
    } else {
        format!("{} h {:02}", m / 60, m % 60)
    }
}

/// Réglage de la durée personnalisée du minuteur (minutes, 1 à 720) : uniquement « − » et « + », par pas de 5 minutes
/// (calés sur les multiples de 5), avec la durée affichée au centre (non modifiable à la souris). Renvoie vrai si la
/// valeur change. Même réglage dans les Réglages et dans le menu du lecteur.
pub fn sleep_custom_field(ui: &mut egui::Ui, minutes: &mut u32, s: f32) -> bool {
    let before = *minutes;
    // Bouton dessiné à la main : toute sa surface est cliquable (le bouton egui standard ne l'était qu'aux coins).
    // `s` : échelle (1 = Réglages, `PLAYER_MENU_SCALE` = menu du lecteur), appliquée à toutes les dimensions.
    fn step(ui: &mut egui::Ui, label: &str, s: f32) -> bool {
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(46.0, t::BTN_H) * s, egui::Sense::click());
        let k = t::hover_k(ui, resp.id, resp.hovered());
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(t::RADIUS_INNER),
            t::SURFACE_3().lerp_to_gamma(t::ACCENT(), k),
            egui::Stroke::new(1.0, t::BORDER().gamma_multiply(1.0 - k)),
            egui::StrokeKind::Inside,
        );
        let ink = egui::Stroke::new(2.0, t::TEXT().lerp_to_gamma(egui::Color32::WHITE, k));
        let c = rect.center();
        ui.painter().line_segment(
            [c - egui::vec2(7.0 * s, 0.0), c + egui::vec2(7.0 * s, 0.0)],
            ink,
        );
        if label == "+" {
            ui.painter().line_segment(
                [c - egui::vec2(0.0, 7.0 * s), c + egui::vec2(0.0, 7.0 * s)],
                ink,
            );
        }
        t::focus_ring(ui, &resp, rect, t::RADIUS_INNER);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        resp.clicked()
    }
    // Posés de droite à gauche dans les Réglages : l'ordre visuel est toujours « − [valeur] + ».
    let rtl = ui.layout().prefer_right_to_left();
    let (minus, plus) = ("−", "+");
    // Zone de valeur de largeur fixe : les boutons « − » et « + » ne bougent pas quand le texte change de longueur.
    let field = |ui: &mut egui::Ui, minutes: &mut u32| {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(112.0, t::BTN_H) * s, egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(t::RADIUS_INNER),
            t::SURFACE_3(),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format_minutes(*minutes),
            egui::FontId::proportional(14.5 * s),
            t::TEXT(),
        );
    };
    let (dec, inc);
    if rtl {
        inc = step(ui, plus, s);
        field(ui, minutes);
        dec = step(ui, minus, s);
    } else {
        dec = step(ui, minus, s);
        field(ui, minutes);
        inc = step(ui, plus, s);
    }
    if dec {
        *minutes = if *minutes > 5 {
            (*minutes - 1) / 5 * 5
        } else {
            1
        };
    }
    if inc {
        *minutes = ((*minutes / 5 + 1) * 5).min(720);
    }
    *minutes != before
}

/// Échelle du menu « Options du lecteur » par rapport à la taille des Réglages (1,0) : textes, boutons, interrupteur,
/// marges et largeur sont tous multipliés par cette valeur, donc les proportions restent les mêmes.
const PLAYER_MENU_SCALE: f32 = 0.85;

/// Menu « Options du lecteur » (engrenage de la barre du lecteur) : « Faible latence » et « Minuteur de veille », les
/// mêmes réglages que la page Réglages (aucune copie). S'ouvre au-dessus de la barre. Renvoie (faible latence changée,
/// minuteur changé). `sleep_left` : temps restant avant l'arrêt si le minuteur tourne.
pub fn player_options_menu(
    ui: &mut egui::Ui,
    resp: &egui::Response,
    live: egui::Rect,
    low_latency: &mut bool,
    sleep_timer: &mut u8,
    sleep_custom_min: &mut u32,
    sleep_left: Option<std::time::Duration>,
) -> (bool, bool) {
    let tr = crate::i18n::tr;
    let s = PLAYER_MENU_SCALE;
    let anchor_x = live.center().x;
    // Identifiant propre au menu : l'infobulle de l'engrenage utilise déjà celui par défaut de la réponse (même
    // identifiant sur deux calques = arrêt de egui).
    let menu_id = resp.id.with("player_options_menu");
    let open = egui::Popup::is_id_open(ui.ctx(), menu_id);
    let (mut latency, mut sleep) = (false, false);
    let menu = egui::Popup::from_toggle_button_response(resp)
        .id(menu_id)
        // Centré horizontalement sur la fenêtre du live : ancre = point de l'axe vertical du live, à la hauteur de
        // l'engrenage ; le menu s'ouvre au-dessus (ou dessous s'il n'y a pas de place), centré sur cette ancre.
        .anchor(egui::Rect::from_x_y_ranges(
            anchor_x..=anchor_x,
            resp.rect.y_range(),
        ))
        .align(egui::RectAlign::TOP)
        .align_alternatives(&[egui::RectAlign::BOTTOM])
        .gap(6.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(
            egui::Frame::new()
                .fill(t::SURFACE_2())
                .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(
                    (22.0 * s).round() as i8,
                    (18.0 * s).round() as i8,
                )),
        )
        .show(|ui| {
            let _dark = t::DarkScope::enter(ui);
            // Largeur fixe : le menu ne change pas de taille d'une image à l'autre (egui lit les zones cliquables sur
            // l'image précédente : un menu qui bouge décale les clics).
            ui.set_width(680.0 * s);
            ui.spacing_mut().item_spacing = egui::vec2(14.0, 12.0) * s;
            // Le sélecteur du minuteur (boutons egui) suit la même échelle : texte, marges et hauteur.
            let style = ui.style_mut();
            for font in style.text_styles.values_mut() {
                font.size *= s;
            }
            style.spacing.button_padding *= s;
            style.spacing.interact_size *= s;
            ui.label(
                egui::RichText::new(tr("Options du lecteur"))
                    .strong()
                    .size(15.0 * s)
                    .color(t::TEXT()),
            );
            // Faible latence et minuteur sur la même ligne.
            ui.horizontal(|ui| {
                latency =
                    t::inline_switch_scaled(ui, tr("Faible latence"), low_latency, s).changed();
                ui.add_space(t::SP_L * s);
                ui.label(
                    egui::RichText::new(tr("Minuteur de veille"))
                        .size(14.0 * s)
                        .color(t::TEXT()),
                );
                sleep = t::segmented(ui, sleep_timer, &sleep_labels());
            });
            if *sleep_timer == SLEEP_CUSTOM {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr("Durée personnalisée"))
                            .size(14.0 * s)
                            .color(t::TEXT()),
                    );
                    sleep |= sleep_custom_field(ui, sleep_custom_min, s);
                });
            }
            if let Some(left) = sleep_left {
                let secs = left.as_secs();
                let text = if secs >= 60 {
                    format!(
                        "{} {}",
                        tr("Arrêt du live dans"),
                        format_minutes(u32::try_from(secs.div_ceil(60)).unwrap_or(u32::MAX))
                    )
                } else {
                    format!("{} {} s", tr("Arrêt du live dans"), secs + 1)
                };
                ui.label(egui::RichText::new(text).size(14.0 * s).color(t::TEXT()));
                // Le décompte se met à jour pendant que le menu est ouvert.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(500));
            }
        });
    // Animation propre au menu (`menu_id`) : avec l'identifiant de l'engrenage, l'infobulle de l'engrenage (même
    // animation, jamais ouverte quand le menu l'est) la remettait à zéro à chaque image. Le menu restait alors réduit
    // à l'écran pendant que ses zones cliquables gardaient leur taille réelle : clics décalés sur « − » et « + ».
    t::grow_popup(
        ui.ctx(),
        menu_id,
        open,
        menu.as_ref().map(|m| &m.response),
        egui::pos2(anchor_x, resp.rect.center().y),
    );
    (latency, sleep)
}

/// Liste déroulante du périphérique de sortie audio : « Périphérique par défaut du système » (valeur vide) puis les
/// périphériques détectés. Un choix enregistré mais absent de la liste reste affiché (par son nom) tant qu'il est choisi.
/// Renvoie vrai si le choix change.
pub(super) fn output_device_dropdown(
    ui: &mut egui::Ui,
    current: &mut String,
    devices: &[(String, String)],
) -> bool {
    let default_label = crate::i18n::tr("Périphérique par défaut du système");
    let label = if current.is_empty() {
        default_label
    } else {
        devices
            .iter()
            .find(|(n, _)| n == current)
            .map_or(current.as_str(), |(_, d)| d.as_str())
    }
    .to_string();
    let font = egui::FontId::proportional(14.0);
    let width = ui.available_width().clamp(150.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, t::BTN_H), egui::Sense::click());
    let popup_id = egui::Popup::default_response_id(&resp);
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    let k = t::hover_k(ui, resp.id, resp.hovered() || open);
    let p = ui.painter();
    p.rect(
        rect,
        egui::CornerRadius::same(t::RADIUS_INNER),
        t::SURFACE_2().lerp_to_gamma(t::ACCENT(), k),
        egui::Stroke::new(1.0, t::BORDER().gamma_multiply(1.0 - k)),
        egui::StrokeKind::Inside,
    );
    let fg = t::TEXT().lerp_to_gamma(egui::Color32::WHITE, k);
    t::paint_one_line(
        p,
        egui::pos2(rect.left() + 12.0, rect.center().y - 9.0),
        &label,
        font.size,
        fg,
        width - 12.0 - 30.0,
    );
    // Flèche vers le bas (la liste s'ouvre sous le bouton).
    let st = egui::Stroke::new(1.6, fg);
    let c = egui::pos2(rect.right() - 14.0, rect.center().y);
    p.line_segment([c + egui::vec2(-4.0, -2.0), c + egui::vec2(0.0, 2.0)], st);
    p.line_segment([c + egui::vec2(0.0, 2.0), c + egui::vec2(4.0, -2.0)], st);
    t::focus_ring(ui, &resp, rect, t::RADIUS_INNER);
    let name = crate::i18n::tr("Périphérique de sortie");
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("{name} : {label}"),
        )
    });
    let mut changed = false;
    let menu = egui::Popup::from_toggle_button_response(&resp)
        .align(egui::RectAlign::BOTTOM_END)
        .align_alternatives(&[egui::RectAlign::TOP_END])
        .gap(6.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .frame(
            egui::Frame::new()
                .fill(t::SURFACE_2())
                .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(4.0),
        )
        .show(|ui| {
            let _dark = t::DarkScope::enter(ui);
            ui.set_width(rect.width().max(260.0));
            if t::menu_item(ui, default_label, current.is_empty()).clicked() && !current.is_empty()
            {
                current.clear();
                changed = true;
            }
            for (device, desc) in devices {
                if t::menu_item(ui, desc, device == current).clicked() && device != current {
                    current.clone_from(device);
                    changed = true;
                }
            }
        });
    t::grow_popup(
        ui.ctx(),
        resp.id,
        open,
        menu.as_ref().map(|m| &m.response),
        rect.center(),
    );
    changed
}

/// Sélecteur de qualité : affiche Source, 1080p, 720p, 480p, Audio ; renvoie vrai si la valeur change.
pub fn quality_segmented(ui: &mut egui::Ui, quality: &mut u8, with_tips: bool) -> bool {
    let labels: Vec<&str> = QUALITY_ORDER
        .iter()
        .map(|&q| QUALITIES[q as usize])
        .collect();
    let mut pos = QUALITY_ORDER
        .iter()
        .position(|&q| q == *quality)
        .unwrap_or(0) as u8;
    // Infobulle sur « Source » (réglages seulement : la barre du lecteur n'en a pas).
    let source_tip = crate::i18n::tr("Qualité maximale proposée par la chaîne, sans réduction.");
    let tips: Vec<&str> = if with_tips {
        vec![source_tip]
    } else {
        Vec::new()
    };
    let changed = t::segmented_tips(ui, &mut pos, &labels, &tips);
    if changed {
        *quality = QUALITY_ORDER.get(pos as usize).copied().unwrap_or(0);
    }
    changed
}

/// Icône de chaque section (même ordre que `SETTINGS_TABS`).
const TAB_ICONS: [t::NavIcon; 10] = [
    t::NavIcon::Login,
    t::NavIcon::General,
    t::NavIcon::Appearance,
    t::NavIcon::Play,
    t::NavIcon::Chat,
    t::NavIcon::Bell,
    t::NavIcon::Keyboard,
    t::NavIcon::Access,
    t::NavIcon::Settings,
    t::NavIcon::Info,
];

/// Largeur de la fenêtre des Réglages (points) en dessous de laquelle le menu des sections devient une barre d'icônes.
const SETTINGS_NARROW_W: f32 = 760.0;
const SETTINGS_NAV_W: f32 = 236.0;
const SETTINGS_RAIL_W: f32 = 64.0;

/// Sections des Réglages (libellés français, traduits à l'affichage), dans l'ordre du menu : le numéro d'une section est
/// sa place dans cette liste et dans `TAB_ICONS`. « Se déconnecter » est épinglé sous le menu. Dix sections, deux
/// niveaux au plus (une section, puis un groupe replié : la liste des langues dans « Général », les chemins
/// personnalisés dans « Avancé et Système »).
const SETTINGS_TABS: [&str; 10] = [
    "Compte",
    "Général",
    "Apparence",
    "Lecture vidéo",
    "Chat",
    "Notifications",
    "Raccourcis clavier",
    "Accessibilité",
    "Avancé et Système",
    "À propos",
];

/// Section « Compte » (compte Twitch).
const TAB_ACCOUNT: u8 = 0;

/// Section « Général » (langue, démarrage et fenêtre, accueil et barre latérale, recommandations).
const TAB_GENERAL: u8 = 1;

/// Section « Apparence » (thème, couleur d'accent, mise à l'échelle).
const TAB_APPEARANCE: u8 = 2;

/// Section « Lecture vidéo » (qualité et flux, audio, lecteur et comportement).
pub(super) const TAB_PLAYBACK: u8 = 3;

/// Section « Chat » (affichage, filtrage et alertes, émotes et badges).
const TAB_CHAT: u8 = 4;

/// Section « Notifications » (notifications Windows et notification de test).
const TAB_NOTIFICATIONS: u8 = 5;

/// Section « Raccourcis clavier » (liste modifiable, « Rétablir les raccourcis », encart des raccourcis fixes).
const TAB_SHORTCUTS: u8 = 6;

/// Section « Accessibilité » (mouvement et lisibilité, navigation et souris).
const TAB_ACCESSIBILITY: u8 = 7;

/// Section « Avancé et Système » (stockage et configuration, composants externes, journaux, zone de réinitialisation).
const TAB_SYSTEM: u8 = 8;

/// Section « À propos » (version, mises à jour, aide), dernière du menu.
const TAB_ABOUT: u8 = 9;

/// Index de la recherche des Réglages : libellé français (tel qu'affiché par la ligne) et onglet.
const SEARCH_INDEX: &[(&str, u8)] = &[
    ("Qualité par défaut", TAB_PLAYBACK),
    ("Faible latence", TAB_PLAYBACK),
    ("Netteté de l'image", TAB_PLAYBACK),
    ("Agrandissement de haute qualité", TAB_PLAYBACK),
    ("Décodage vidéo matériel (GPU)", TAB_PLAYBACK),
    ("Volume stable", TAB_PLAYBACK),
    ("Dossier d'enregistrement", TAB_PLAYBACK),
    ("Périphérique de sortie", TAB_PLAYBACK),
    ("Démarrer en muet", TAB_PLAYBACK),
    ("Cumuler le son des lives en Multistream", TAB_PLAYBACK),
    ("Volume fixe au lancement", TAB_PLAYBACK),
    ("Volume maximum", TAB_PLAYBACK),
    ("Toujours au premier plan", TAB_PLAYBACK),
    ("Fenêtre PiP au format de la vidéo", TAB_PLAYBACK),
    ("Lecteur au format de la vidéo", TAB_PLAYBACK),
    ("Masquage des contrôles", TAB_PLAYBACK),
    ("Molette de la souris sur la vidéo = volume", TAB_PLAYBACK),
    ("Clic sur la vidéo = pause", TAB_PLAYBACK),
    ("Minuteur de veille", TAB_PLAYBACK),
    ("Durée personnalisée", TAB_PLAYBACK),
    ("Mini-lecteur", TAB_PLAYBACK),
    ("Taille du texte", TAB_CHAT),
    ("Masquer les bots", TAB_CHAT),
    ("Horodatage des messages", TAB_CHAT),
    ("Chat à gauche de la vidéo", TAB_CHAT),
    ("Chat en surimpression en plein écran", TAB_CHAT),
    ("Mots surlignés", TAB_CHAT),
    ("Émotes 7TV, BTTV et FFZ", TAB_CHAT),
    ("Émotes animées", TAB_CHAT),
    ("Badges du chat", TAB_CHAT),
    ("Actualisation", TAB_GENERAL),
    ("Aperçus des lives", TAB_GENERAL),
    ("Chaînes suivies sur Twitch", TAB_GENERAL),
    ("Chaînes dans la barre latérale", TAB_GENERAL),
    ("Chaînes sur l'accueil", TAB_GENERAL),
    ("Se souvenir du jeu choisi", TAB_GENERAL),
    ("Clic sur une carte", TAB_GENERAL),
    ("Historique des chaînes", TAB_GENERAL),
    ("Effacer l'historique", TAB_GENERAL),
    ("Recommandations de chaînes", TAB_GENERAL),
    ("Nombre de recommandations", TAB_GENERAL),
    ("Thème", TAB_APPEARANCE),
    ("Taille de l'interface", TAB_APPEARANCE),
    ("Contraste élevé", TAB_ACCESSIBILITY),
    ("Focus clavier renforcé", TAB_ACCESSIBILITY),
    ("Palette de couleurs", TAB_APPEARANCE),
    ("Couleur d'accent personnalisée", TAB_APPEARANCE),
    ("Rétablir les couleurs par défaut", TAB_APPEARANCE),
    ("Langue", TAB_GENERAL),
    (
        "Boutons Précédent / Suivant de la souris",
        TAB_ACCESSIBILITY,
    ),
    ("Main sur les éléments cliquables", TAB_ACCESSIBILITY),
    ("Animations", TAB_ACCESSIBILITY),
    ("Vitesse de défilement", TAB_ACCESSIBILITY),
    ("Raccourcis clavier", TAB_SHORTCUTS),
    ("Pause / lecture", TAB_SHORTCUTS),
    ("Couper le son", TAB_SHORTCUTS),
    ("Volume +", TAB_SHORTCUTS),
    ("Volume -", TAB_SHORTCUTS),
    ("Plein écran", TAB_SHORTCUTS),
    ("Afficher / masquer le chat", TAB_SHORTCUTS),
    ("Image dans l'image", TAB_SHORTCUTS),
    ("Capture d'écran", TAB_SHORTCUTS),
    ("Infos techniques", TAB_SHORTCUTS),
    ("Quitter le plein écran, le PiP ou le live", TAB_SHORTCUTS),
    ("Rechercher une chaîne", TAB_SHORTCUTS),
    ("Zoom de l'interface", TAB_SHORTCUTS),
    ("Page précédente / suivante", TAB_SHORTCUTS),
    ("Effacer un raccourci", TAB_SHORTCUTS),
    ("Compte Twitch", TAB_ACCOUNT),
    (crate::i18n::os::STARTUP, TAB_GENERAL),
    (
        "Réduire dans la zone de notification à la fermeture",
        TAB_GENERAL,
    ),
    ("Chaîne ouverte au démarrage", TAB_GENERAL),
    ("Version de l'application", TAB_ABOUT),
    ("Vérification des mises à jour", TAB_ABOUT),
    ("Rechercher des mises à jour", TAB_ABOUT),
    (crate::i18n::os::NOTIFY, TAB_NOTIFICATIONS),
    ("Cache des images", TAB_SYSTEM),
    ("Redémarrer l'application", TAB_SYSTEM),
    ("Composants externes", TAB_SYSTEM),
    ("Chemins personnalisés (facultatif)", TAB_SYSTEM),
    ("Journaux et diagnostic", TAB_SYSTEM),
    ("Ouvrir le dossier du journal", TAB_SYSTEM),
    ("Ouvrir le dossier de configuration", TAB_SYSTEM),
    ("Copier le diagnostic", TAB_SYSTEM),
    ("Copier le rapport de plantage", TAB_SYSTEM),
    ("Zone de réinitialisation", TAB_SYSTEM),
    ("Réinitialiser", TAB_SYSTEM),
    ("Désinstaller", TAB_SYSTEM),
    ("Revoir le didacticiel", TAB_ABOUT),
    ("Licences des composants", TAB_ABOUT),
    ("Soutenir le projet", TAB_ABOUT),
];

/// Minuscules sans accents, pour une recherche tolérante (« nettete » trouve « Netteté »).
fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Mots d'un texte déjà replié (lettres et chiffres seulement).
fn words_of(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Pertinence d'un mot recherché pour un réglage (0 = absent). Le libellé compte bien plus que l'explication :
/// libellé identique > libellé qui commence par le mot > un mot du libellé qui commence ainsi > libellé qui le
/// contient > nom de l'onglet > explication.
fn word_score(w: &str, label: &str, tab: &str, help: &str) -> u32 {
    // Un mot très court (« son ») ne compte pas au milieu d'un autre mot (« person-nalisée ») : trop de faux résultats.
    let mid_word_ok = w.chars().count() >= 4;
    if label == w {
        100
    } else if label.starts_with(w) {
        80
    } else if words_of(label).iter().any(|x| x.starts_with(w)) {
        60
    } else if mid_word_ok && label.contains(w) {
        40
    } else if words_of(tab).iter().any(|x| x.starts_with(w)) {
        30
    } else if words_of(help).iter().any(|x| x.starts_with(w)) {
        15
    } else if mid_word_ok && help.contains(w) {
        5
    } else {
        0
    }
}

/// Réglages qui correspondent à la recherche (dans la langue affichée), du plus pertinent au moins pertinent.
/// Réglage absent de ce système (pistes Mac et Linux) : ni affiché ni proposé par la recherche.
fn hidden_on_this_os(label: &str) -> bool {
    !cfg!(windows) && label == "Réduire dans la zone de notification à la fermeture"
}

/// Chaque mot recherché doit être trouvé ; à égalité, l'ordre des onglets est gardé.
fn search(query: &str) -> Vec<(&'static str, u8)> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(u32, &'static str, u8)> = SEARCH_INDEX
        .iter()
        .filter(|&&(label, _)| !hidden_on_this_os(label))
        .filter_map(|&(label, tab)| {
            let shown = crate::i18n::tr(label);
            let l = fold(shown);
            let t = fold(crate::i18n::tr(SETTINGS_TABS[usize::from(tab)]));
            let mut h = crate::help::lookup(shown).map(fold).unwrap_or_default();
            if label == "Thème" {
                // Les noms des thèmes (dans la langue affichée) mènent aussi au réglage : « sombre » et « dark ».
                h.push(' ');
                h.push_str(&fold(&crate::ui::theme::theme_names().join(" ")));
            }
            let mut total = 0;
            for w in &words {
                let sc = word_score(w, &l, &t, &h);
                if sc == 0 {
                    return None;
                }
                total += sc;
            }
            Some((total, label, tab))
        })
        .collect();
    scored.sort_by_key(|x| std::cmp::Reverse(x.0)); // tri stable : l'ordre des onglets départage
    scored.into_iter().map(|(_, l, t)| (l, t)).collect()
}

/// Repère « les Réglages viennent de s'ouvrir » (effacé à la fermeture, voir `VelkaApp::ui`).
pub(super) fn overlay_open_id() -> egui::Id {
    egui::Id::unique("settings_overlay_open")
}

fn fade_id() -> egui::Id {
    egui::Id::unique("settings_fade")
}

/// Opacité du voile et de la fenêtre (0 à 1) : monte à l'ouverture des Réglages.
fn overlay_fade_in(ctx: &egui::Context) -> f32 {
    t::motion::eased(ctx, fade_id(), true, t::motion::MODAL)
}

/// Fermeture : le voile disparaît instantanément (pas de fondu), prêt pour la prochaine ouverture.
/// Appelée à chaque image où les Réglages sont fermés (la valeur reste à 0, donc aucun coût).
pub(super) fn overlay_fade_out(ctx: &egui::Context) {
    ctx.animate_bool_with_time(fade_id(), false, 0.0);
}

impl VelkaApp {
    /// Changement de thème : appelé quand l'utilisateur choisit un thème. On photographie l'affichage actuel (ancien
    /// thème) ; dès que la photo arrive, le nouveau thème est appliqué et la photo, posée par-dessus, devient
    /// transparente (voir `theme_fade_ui`). Faire passer les vraies couleurs de l'interface image par image obligeait
    /// egui à recalculer tout le texte à chaque image (environ 47 ms par image au lieu de 3 ms, soit 20 images par
    /// seconde) : ici, chaque image du fondu ne dessine qu'un rectangle.
    pub(super) fn begin_theme_change(&mut self, ctx: &egui::Context) {
        if t::anim_secs(ctx, 1.0) <= 0.0 {
            t::apply(ctx, self.settings.theme);
            self.theme_fade = None;
            return;
        }
        let slot: ThemeSlot = Arc::new(Mutex::new(None));
        let (writer, repaint) = (slot.clone(), ctx.clone());
        ctx.request_screenshot(move |image| {
            if let Ok(mut g) = writer.lock() {
                *g = Some(image);
            }
            repaint.request_repaint();
        });
        self.theme_fade = Some(ThemeShot::Waiting {
            since: Instant::now(),
            slot,
        });
        ctx.request_repaint();
    }

    /// À appeler à la fin de chaque image : applique le nouveau thème quand la photo de l'ancien est prête, puis
    /// dessine la photo par-dessus (opacité de 1 à 0, courbe douce en entrée et en sortie).
    pub(super) fn theme_fade_ui(&mut self, ctx: &egui::Context) {
        const SECS: f32 = 0.8;
        if let Some(ThemeShot::Waiting { since, slot }) = &self.theme_fade {
            let image = slot.lock().ok().and_then(|mut g| g.take());
            if let Some(image) = image {
                let tex = ctx.load_texture(
                    "theme_shot",
                    egui::ImageData::Color(image),
                    egui::TextureOptions::LINEAR,
                );
                t::apply(ctx, self.settings.theme);
                self.theme_fade = Some(ThemeShot::Fading {
                    tex,
                    start: ctx.input(|i| i.time),
                });
            } else if since.elapsed() > Duration::from_millis(600) {
                // Pas de photo (capture indisponible) : changement direct, sans fondu.
                t::apply(ctx, self.settings.theme);
                self.theme_fade = None;
            } else {
                ctx.request_repaint();
            }
        }
        if let Some(ThemeShot::Fading { tex, start }) = &self.theme_fade {
            let el = ((ctx.input(|i| i.time) - start) / f64::from(SECS)).clamp(0.0, 1.0) as f32;
            if el >= 1.0 {
                self.theme_fade = None;
                return;
            }
            let k = el * el * (3.0 - 2.0 * el);
            let alpha = ((1.0 - k) * 255.0).round() as u8;
            let layer = egui::LayerId::new(egui::Order::Debug, egui::Id::unique("theme_shot"));
            ctx.layer_painter(layer).image(
                tex.id(),
                ctx.content_rect(),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::from_white_alpha(alpha),
            );
            ctx.request_repaint();
        }
    }

    /// Réglages en overlay au-dessus de la page courante : voile sombre (un clic dessus ou Échap ferme), fenêtre
    /// centrée avec le menu des sections à gauche et leur contenu à droite. La barre de titre reste utilisable.
    pub(super) fn settings_overlay(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let bar_h = super::fenetre::TITLE_BAR_H;
        let mut close = false;
        let backdrop_id = egui::Id::unique("settings_backdrop");
        // Fenêtre modale : le clavier (Tab, flèches) et la souris n'atteignent plus la page du dessous. Le voile est la
        // couche la plus basse autorisée ; la barre de titre et la fenêtre des Réglages sont au-dessus.
        ctx.memory_mut(|m| {
            m.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, backdrop_id))
        });
        // À l'ouverture, le focus quitte la page du dessous et se place sur le champ de recherche des Réglages : Tab
        // démarre dans les Réglages (menu, puis contenu). Demandé en fin d'image (voir `VelkaApp::ui`), après
        // `focus_follows_click` qui sinon redonnerait le focus au bouton « Réglages » cliqué.
        let opened = overlay_open_id();
        if ctx.data_mut(|d| d.get_temp::<bool>(opened)).is_none() {
            self.settings_focus_frames = 2;
            // Réouverture : même section, même recherche (déjà gardées) et même défilement.
            self.settings_restore_scroll = 4;
            ctx.data_mut(|d| d.insert_temp(opened, true));
        }
        let fade = overlay_fade_in(ctx);
        egui::Area::new(backdrop_id)
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                // `CLICK` seul (pas `click()`) : le voile se clique mais n'est pas un arrêt de Tab.
                let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::CLICK);
                ui.painter().rect_filled(
                    r,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_black_alpha((130.0 * fade) as u8),
                );
                close |= resp.clicked();
            });
        // Barre de titre redessinée au-dessus du voile : déplacer, réduire, agrandir et fermer restent possibles.
        let bar = egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), bar_h));
        let bar_id = egui::Id::unique("settings_titlebar");
        egui::Area::new(bar_id)
            .order(egui::Order::Foreground)
            .fixed_pos(bar.min)
            .show(ctx, |ui| {
                ui.set_min_size(bar.size());
                ui.set_max_size(bar.size());
                self.title_bar(ui, ctx, "overlay");
            });
        let avail_h = screen.height() - bar_h;
        let size = egui::vec2(
            (screen.width() - 96.0)
                .clamp(520.0, 1180.0)
                .min(screen.width() - 16.0)
                .max(0.0),
            (avail_h - 64.0)
                .clamp(360.0, 860.0)
                .min(avail_h - 8.0)
                .max(0.0),
        );
        let rect = egui::Rect::from_center_size(
            egui::pos2(screen.center().x, screen.top() + bar_h + avail_h / 2.0),
            size,
        );
        let modal_id = egui::Id::unique("settings_modal");
        // Le fondu d'entrée est celui du voile (`fade`), pas celui, plus court, d'egui : voile et fenêtre arrivent ensemble.
        egui::Area::new(modal_id)
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .fade_in(false)
            .show(ctx, |ui| {
                // Fond opaque dès la première image : posé avant `set_opacity`, il cache la page du dessous pendant que le
                // contenu (texte, menu, cartes) apparaît en fondu. Même rectangle que le cadre (contenu + 1 pt de trait de
                // chaque côté), même rayon : le fond du cadre, qui se fond ensuite, retombe exactement dessus.
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, size + egui::vec2(2.0, 2.0)),
                    egui::CornerRadius::same(t::RADIUS_CARD),
                    t::BG(),
                );
                ui.set_opacity(fade);
                egui::Frame::new()
                    .fill(t::BG())
                    .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                    .corner_radius(egui::CornerRadius::same(t::RADIUS_CARD))
                    .show(ui, |ui| {
                        ui.set_min_size(size);
                        ui.set_max_size(size);
                        let inner = egui::Rect::from_min_size(ui.min_rect().min, size);
                        // Fenêtre étroite (minimale, zoom élevé) : le menu des sections devient une barre d'icônes (comme la barre
                        // latérale réduite de l'accueil) ; le burger la déplie par-dessus le contenu.
                        let narrow = size.x < SETTINGS_NARROW_W;
                        if !narrow {
                            self.settings_nav_open = false;
                        }
                        let open = narrow && self.settings_nav_open;
                        let nav_target = if narrow && !open {
                            SETTINGS_RAIL_W
                        } else {
                            SETTINGS_NAV_W
                        };
                        let nav_w = ctx.animate_value_with_time(
                            egui::Id::unique("settings_nav_w"),
                            nav_target,
                            t::move_secs(ctx, 0.18),
                        );
                        let nav = egui::Rect::from_min_size(inner.min, egui::vec2(nav_w, size.y));
                        // Place réservée au menu : barre d'icônes seulement (le menu déplié recouvre le contenu).
                        let reserved = if narrow {
                            SETTINGS_RAIL_W
                        } else {
                            SETTINGS_NAV_W
                        };
                        let content = egui::Rect::from_min_max(
                            egui::pos2(inner.left() + reserved, inner.top()),
                            inner.max,
                        );
                        let draw_nav = |this: &mut Self, ui: &mut egui::Ui| {
                            ui.painter().rect_filled(
                                nav,
                                egui::CornerRadius {
                                    nw: t::RADIUS_CARD,
                                    sw: t::RADIUS_CARD,
                                    ne: 0,
                                    se: 0,
                                },
                                t::SURFACE_1(),
                            );
                            ui.painter().vline(
                                nav.right(),
                                nav.y_range(),
                                egui::Stroke::new(1.0, t::BORDER()),
                            );
                            let pad = if nav_w < 150.0 {
                                egui::vec2(8.0, 18.0)
                            } else {
                                egui::vec2(14.0, 18.0)
                            };
                            ui.scope_builder(
                                egui::UiBuilder::new().max_rect(nav.shrink2(pad)),
                                |ui| {
                                    if nav_w < 150.0 {
                                        this.settings_rail(ui);
                                    } else {
                                        this.settings_menu(ui, ctx, narrow);
                                    }
                                },
                            );
                        };
                        let draw_content = |this: &mut Self, ui: &mut egui::Ui| {
                            ui.scope_builder(
                                // Fenêtre étroite (zoom élevé) : marges réduites pour laisser la place aux cartes.
                                egui::UiBuilder::new().max_rect(content.shrink2(egui::vec2(
                                    if size.x < 700.0 { 18.0 } else { 28.0 },
                                    18.0,
                                ))),
                                |ui| this.settings_content(ui, ctx),
                            );
                        };
                        if open {
                            // Menu déplié : dessiné après le contenu pour être au-dessus.
                            draw_content(self, ui);
                            draw_nav(self, ui);
                        } else {
                            draw_nav(self, ui);
                            draw_content(self, ui);
                        }
                    });
            });
        // Empilement fixe : voile, puis barre de titre, puis fenêtre des Réglages.
        for id in [backdrop_id, bar_id, modal_id] {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        }
        if self.capture_shortcut.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if close && self.view == View::Settings {
            self.go_back(ctx);
        }
    }

    /// Menu des sections réduit en barre d'icônes (fenêtre étroite) : burger, recherche, sections, compte. Le burger
    /// (ou la recherche, ou l'icône du compte) déplie le menu complet.
    fn settings_rail(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::tr;
        let btn = egui::vec2(ui.available_width(), t::ROW_H);
        let mut open =
            t::nav_icon_button(ui, t::NavIcon::Panel, btn, false, tr("Afficher le menu")).clicked();
        if t::nav_icon_button(
            ui,
            t::NavIcon::Search,
            btn,
            false,
            tr("Rechercher un réglage"),
        )
        .clicked()
        {
            open = true;
            self.settings_focus_frames = 2; // le champ de recherche prend le focus dès qu'il apparaît
        }
        ui.add_space(t::SP_S);
        let searching = !self.settings_query.trim().is_empty();
        let room = (ui.max_rect().bottom() - ui.cursor().top() - 46.0).max(80.0);
        t::auto_hide_scroll(
            ui,
            "settings_rail_scroll",
            false,
            egui::ScrollArea::vertical()
                .max_height(room)
                .auto_shrink([false, true]),
            |ui| {
                t::follow_keyboard_focus(ui);
                for i in 0..SETTINGS_TABS.len() as u8 {
                    let active = !searching && self.settings_tab == i;
                    let icon = TAB_ICONS
                        .get(usize::from(i))
                        .copied()
                        .unwrap_or(t::NavIcon::Settings);
                    if t::nav_icon_button(ui, icon, btn, active, tr(SETTINGS_TABS[usize::from(i)]))
                        .clicked()
                    {
                        self.settings_tab = i;
                        self.settings_query.clear();
                        self.capture_shortcut = None;
                    }
                }
            },
        );
        // Compte en bas : l'icône déplie le menu, où se trouve le bouton (avec sa confirmation en deux temps).
        let r = ui.max_rect();
        let bottom = egui::Rect::from_min_max(
            egui::pos2(r.left(), r.bottom() - t::ROW_H),
            egui::pos2(r.right(), r.bottom()),
        );
        let logged =
            !self.settings.oauth_token.is_empty() && !self.settings.account_login.is_empty();
        let (icon, tip) = if logged {
            (t::NavIcon::Logout, tr("Se déconnecter"))
        } else {
            (t::NavIcon::Login, tr("Se connecter"))
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            open |= t::nav_icon_button(ui, icon, btn, false, tip).clicked();
        });
        if open {
            self.settings_nav_open = true;
        }
    }

    /// Message sur la connexion Twitch, sous le menu des Réglages : `(texte, est une erreur)`. Un échec reste affiché
    /// jusqu'au prochain essai ; une annulation voulue par l'utilisateur ne l'est que quelques secondes, en ton neutre.
    fn login_notice(&self, ctx: &egui::Context) -> Option<(String, bool)> {
        let tr = crate::i18n::tr;
        if self.auth_cancel.is_some() || self.connected() {
            return None;
        }
        if let Some(e) = &self.auth_error {
            return Some((
                format!("{} {}", tr("Échec de la connexion."), tr(e_static(e))),
                true,
            ));
        }
        const SHOWN: std::time::Duration = std::time::Duration::from_secs(8);
        let at = self.auth_cancelled_at?;
        if at.elapsed() < SHOWN {
            ctx.request_repaint_after(SHOWN - at.elapsed());
            return Some((tr("Connexion annulée").to_string(), false));
        }
        None
    }

    /// Liste défilante du menu des Réglages. La molette agit sur toute la surface du menu (`nav`), y compris les espaces
    /// hors de la liste (titre, marges, fond) : elle y est transmise à la liste à l'image suivante, et jamais laissée au
    /// contenu ni à la page d'en dessous.
    fn nav_scroll(
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        salt: &'static str,
        nav: egui::Rect,
        room: f32,
        add: impl FnOnce(&mut egui::Ui),
    ) {
        let key = egui::Id::unique(("nav_scroll_fwd", salt));
        let (last, pending): (f32, f32) = ctx.data(|d| d.get_temp(key)).unwrap_or((0.0, 0.0));
        let mut area = egui::ScrollArea::vertical()
            .max_height(room)
            .auto_shrink([false, true]);
        if pending != 0.0 {
            area = area.vertical_scroll_offset((last - pending).max(0.0));
        }
        let out = t::auto_hide_scroll(ui, salt, false, area, |ui| {
            t::follow_keyboard_focus(ui);
            add(ui);
        });
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let over_nav = pointer.is_some_and(|p| nav.contains(p));
        let over_list = pointer.is_some_and(|p| out.inner_rect.contains(p));
        let dy = if over_nav && !over_list {
            ctx.input(|i| i.smooth_scroll_delta.y)
        } else {
            0.0
        };
        if over_nav {
            ctx.input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }
        ctx.data_mut(|d| d.insert_temp(key, (out.state.clamped_offset().y, dy)));
        if dy != 0.0 {
            ctx.request_repaint();
        }
    }

    /// Menu de gauche : recherche, puis les sections. `narrow` : menu déplié par le burger en fenêtre étroite
    /// (bouton pour le replier ; il se replie aussi quand on choisit une section).
    fn settings_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, narrow: bool) {
        let tr = crate::i18n::tr;
        // Message sous la liste, au-dessus du bouton « Se connecter » (échec ou annulation de la connexion).
        let notice = self.login_notice(ctx);
        let notice_h = if notice.is_some() { 40.0 } else { 0.0 };
        if narrow {
            if t::nav_icon_button(
                ui,
                t::NavIcon::Panel,
                egui::vec2(30.0, 30.0),
                false,
                tr("Masquer le menu"),
            )
            .clicked()
            {
                self.settings_nav_open = false;
            }
            ui.add_space(t::SP_S);
        }
        // Toute la surface du menu (marges comprises) reçoit la molette : voir `nav_scroll`.
        let nav_rect = ui.max_rect().expand2(egui::vec2(14.0, 18.0));
        let (edit, _) = t::search_input(
            ui,
            &mut self.settings_query,
            tr("Rechercher un réglage"),
            egui::Id::unique("settings_search"),
            "",
        );
        let searching = !self.settings_query.trim().is_empty();
        // Fenêtre étroite : les résultats s'affichent juste sous la barre de recherche (le libellé de chaque réglage
        // seulement), à la place de la liste des sections ; un clic referme le menu et mène au réglage.
        if narrow && searching {
            ui.add_space(t::SP_S);
            let results = search(&self.settings_query);
            let mut picked: Option<(&'static str, u8)> = None;
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                picked = results.first().copied();
            }
            let room = (ui.max_rect().bottom() - ui.cursor().top() - 44.0 - notice_h).max(80.0);
            Self::nav_scroll(ui, ctx, "settings_search_scroll", nav_rect, room, |ui| {
                if results.is_empty() {
                    ui.label(
                        RichText::new(tr("Aucun réglage trouvé."))
                            .size(13.0)
                            .color(t::MUTED()),
                    );
                }
                for (label, tab) in &results {
                    if t::menu_item(ui, tr(label), false).clicked() {
                        picked = Some((label, *tab));
                    }
                }
            });
            if let Some((label, tab)) = picked {
                self.pick_setting(ctx, label, tab);
            }
        } else {
            ui.add_space(t::SP_L);
            ui.label(
                RichText::new(tr("Réglages").to_uppercase())
                    .size(11.5)
                    .color(t::MUTED()),
            );
            ui.add_space(t::SP_XS);
            // Fenêtre basse (zoom élevé) : la liste des sections défile au-dessus du bouton « Se déconnecter » (épinglé en
            // bas) au lieu de cacher les dernières sections.
            let room = (ui.max_rect().bottom() - ui.cursor().top() - 44.0 - notice_h).max(80.0);
            Self::nav_scroll(ui, ctx, "settings_menu_scroll", nav_rect, room, |ui| {
                // Largeur réelle d'une entrée (la barre de défilement en retire un peu) : le bouton du bas la reprend.
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::unique("settings_tab_w"), ui.available_width())
                });
                for i in 0..SETTINGS_TABS.len() as u8 {
                    let active = !searching && self.settings_tab == i;
                    if t::menu_item_icon(
                        ui,
                        TAB_ICONS.get(usize::from(i)).copied(),
                        tr(SETTINGS_TABS[usize::from(i)]),
                        active,
                    )
                    .clicked()
                    {
                        self.settings_tab = i;
                        self.settings_query.clear();
                        self.capture_shortcut = None;
                        if narrow {
                            self.settings_nav_open = false;
                        }
                    }
                }
            });
        }
        // « Se déconnecter » tout en bas du menu (comme dans les applications grand public), seulement si un compte
        // Twitch est connecté.
        if !self.settings.oauth_token.is_empty() && !self.settings.account_login.is_empty() {
            let r = ui.max_rect();
            // Distance au bord bas de la fenêtre = distance au bord gauche (14 px) : le menu s'arrête 18 px au-dessus du bord.
            let bottom = egui::Rect::from_min_max(
                egui::pos2(r.left(), r.bottom() - 30.0),
                egui::pos2(r.right(), r.bottom() + 4.0),
            );
            // Déconnexion en deux temps : le premier clic arme le bouton (5 s), le second déconnecte.
            let armed = self
                .logout_armed
                .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(5));
            if !armed {
                self.logout_armed = None;
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
                let (kind, label) = if armed {
                    (t::BtnKind::Danger, tr("Confirmer la déconnexion"))
                } else {
                    (t::BtnKind::SecondaryRedText, tr("Se déconnecter"))
                };
                // Même largeur que les entrées du menu (Système, Compte…), armé ou non : le bouton ne change pas de taille.
                let tab_w = ctx
                    .data(|d| d.get_temp::<f32>(egui::Id::unique("settings_tab_w")))
                    .unwrap_or_else(|| ui.available_width());
                if t::icon_text_button_sized(ui, kind, t::NavIcon::Logout, label, tab_w, 34.0, true)
                    .clicked()
                {
                    if armed {
                        self.logout_armed = None;
                        self.logout();
                    } else {
                        self.logout_armed = Some(std::time::Instant::now());
                        ctx.request_repaint_after(std::time::Duration::from_millis(5100));
                    }
                }
            });
        } else {
            // Déconnecté : « Se connecter » au même endroit.
            let r = ui.max_rect();
            // Distance au bord bas de la fenêtre = distance au bord gauche (14 px) : le menu s'arrête 18 px au-dessus du bord.
            let bottom = egui::Rect::from_min_max(
                egui::pos2(r.left(), r.bottom() - 28.0),
                egui::pos2(r.right(), r.bottom() + 4.0),
            );
            if let Some((text, error)) = notice {
                let area = egui::Rect::from_min_max(
                    egui::pos2(r.left(), bottom.top() - notice_h),
                    egui::pos2(r.right(), bottom.top() - 4.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(area)
                        .layout(egui::Layout::bottom_up(egui::Align::Min)),
                    |ui| {
                        let color = if error { t::NEGATIVE() } else { t::MUTED() };
                        ui.add(
                            egui::Label::new(RichText::new(text).size(12.0).color(color)).wrap(),
                        );
                    },
                );
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
                // Connexion en cours (page Twitch ouverte, attente de l'accord) : barre de chargement dans le bouton ; au
                // survol il devient rouge « Annuler » et le clic annule la connexion.
                let loading = self.auth_cancel.is_some();
                let clicked = t::twitch_login_button(
                    ui,
                    tr("Se connecter"),
                    tr("Se déconnecter"),
                    tr("Annuler"),
                    loading,
                    crate::auth::available(),
                )
                .clicked();
                if clicked && loading {
                    self.cancel_login();
                } else if clicked {
                    self.start_login(ctx);
                }
            });
        }
    }

    /// Carte « Langue » (section Général, tout en haut) : la langue courante, et la liste complète (un deuxième niveau) seulement
    /// quand on l'ouvre. Les polices chinoise et coréenne ne sont chargées que liste ouverte.
    fn card_language(&mut self, cols: &mut [egui::Ui], pick: &mut dyn FnMut(f32) -> usize) {
        let tr = crate::i18n::tr;
        let c = pick(if self.lang_open { 1300.0 } else { 300.0 });
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Langue"));
            ui.add_space(t::SP_S);
            // Quatre langues visibles (la langue choisie en fait toujours partie), les autres dans un menu dépliable.
            let current = usize::from(self.settings.language).min(crate::i18n::LANGUAGES.len() - 1);
            let mut shown: Vec<usize> = vec![0, 1, 2, 3];
            if !shown.contains(&current) {
                shown[3] = current;
            }
            self.language_rows(ui, &shown);
            ui.add_space(t::SP_S);
            // Accordéon : l'en-tête se déplie sur place et montre les autres langues.
            let rest: Vec<usize> = (0..crate::i18n::LANGUAGES.len())
                .filter(|i| !shown.contains(i))
                .collect();
            // En-tête dessiné à la main : la zone passe par `animated_section` (0,2 s, comme la section des couleurs).
            // Les polices chinoise et coréenne (nom de ces langues) sont chargées dès que la carte est visible, pas au
            // clic : leur installation, lourde, figerait l'animation en route. Le dépliage attend qu'elles soient prêtes.
            self.lang_list_shown = true;
            let open = self.lang_open && crate::ui::fonts::carousel_ready();
            if t::disclosure_header(ui, "lang_accordion", tr("Autres langues"), open).clicked() {
                self.lang_open = !self.lang_open;
            }
            t::animated_section(ui, "lang_accordion", open, |ui| {
                ui.add_space(t::SP_S);
                self.language_rows(ui, &rest);
            });
        });
        cols[c].add_space(t::SP_M);
    }

    /// Lignes de langues : nom dans la langue elle-même à gauche, nom traduit à droite.
    fn language_rows(&mut self, ui: &mut egui::Ui, which: &[usize]) {
        ui.spacing_mut().item_spacing.y = 6.0;
        for &i in which {
            let l = &crate::i18n::LANGUAGES[i];
            let selected = usize::from(self.settings.language) == i;
            if t::choice_row(ui, l.native, crate::i18n::tr(l.name), selected).clicked() && !selected
            {
                self.settings.language = i as u8;
                crate::i18n::set(self.settings.language);
            }
        }
    }

    /// Réglage choisi dans les résultats de la recherche : ouvre sa section, la recherche est vidée, le menu déplié (fenêtre
    /// étroite) se referme et la ligne est mise en évidence (le contenu défile jusqu'à elle).
    fn pick_setting(&mut self, ctx: &egui::Context, label: &'static str, tab: u8) {
        self.settings_tab = tab;
        self.settings_query.clear();
        self.capture_shortcut = None;
        self.settings_nav_open = false;
        // Les blocs repliés (chemins personnalisés, liste des langues) s'ouvrent pour montrer le réglage trouvé.
        self.paths_open |= label == "Chemins personnalisés (facultatif)";
        self.lang_open |= label == "Langue";
        t::highlight_setting(ctx, crate::i18n::tr(label));
    }

    fn settings_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (mut update_all, mut recheck, mut check_wanted, mut restart_update) =
            (false, false, false, false);
        let mut redetect = false;
        let mut open_log = false;
        let mut copy_crash = false;
        let mut copy_diag = false;
        let has_crash = self.crash_report.is_some();
        let crash_copied = self.crash_copied;
        let diag_copied = self.diag_copied;
        let mut audio_changed = false;
        let mut video_changed = false;
        let (mut do_login, mut do_cancel_login, mut do_logout) = (false, false, false);
        let mut quality_changed = false;
        let mut hw_changed = false;
        let mut ask: Option<Confirm> = None;
        let mut confirmed: Option<Confirm> = None;
        let mut restart_app = false;
        let mut replay_onboarding = false;
        let mut cancel = false;
        let mut go_back = false;
        self.settings_prepare(ctx);
        // Les blocs dépliables sont repliés à chaque ouverture des Réglages : si la page n'a pas été dessinée au passage
        // précédent, on arrive d'ailleurs (la recherche, elle, les déplie ensuite dans `pick_setting`).
        let pass = ctx.cumulative_pass_nr();
        let seen_id = egui::Id::unique("settings_last_pass");
        if ctx
            .data(|d| d.get_temp::<u64>(seen_id))
            .is_none_or(|last| pass.saturating_sub(last) > 1)
        {
            self.paths_open = false;
        }
        ctx.data_mut(|d| d.insert_temp(seen_id, pass));

        // En-tête : titre de la section et bouton de fermeture (Échap) ; seules les cartes défilent en dessous.
        let tr = crate::i18n::tr;
        let title = if self.settings_query.trim().is_empty() {
            tr(SETTINGS_TABS[usize::from(self.settings_tab).min(SETTINGS_TABS.len() - 1)])
        } else {
            tr("Résultats de la recherche")
        };
        // Fenêtre basse : la phrase « Les changements s'appliquent tout de suite. » est retirée, les cartes gagnent de la place.
        let low = ui.available_height() < 600.0;
        // Fenêtre basse : en-tête plus court (40 px au lieu de 58), sans le mot « Échap » sous la croix.
        let hdr_h = if low { 40.0 } else { 58.0 };
        let (hdr, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), hdr_h),
            egui::Sense::hover(),
        );
        let btn = egui::Rect::from_min_size(
            egui::pos2(hdr.right() - 44.0, hdr.top()),
            egui::vec2(44.0, hdr_h),
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                hdr.min,
                egui::pos2(btn.left() - 12.0, hdr.bottom()),
            )),
            |ui| {
                ui.add_space(if low { 8.0 } else { 4.0 });
                t::heading(ui, title);
                if !low {
                    ui.label(
                        RichText::new(tr("Les changements s'appliquent tout de suite."))
                            .size(12.0)
                            .color(t::FAINT()),
                    );
                }
            },
        );
        if ui
            .scope_builder(egui::UiBuilder::new().max_rect(btn), |ui| {
                t::close_button(ui, tr("Fermer"), if low { "" } else { tr("Échap") })
            })
            .inner
            .clicked()
        {
            go_back = true;
        }
        ui.add_space(if low { t::SP_S } else { t::SP_L });
        let tab = self.settings_tab;
        if tab != TAB_SYSTEM {
            self.advanced_visible_prev = false;
        }
        let results = search(&self.settings_query);
        let searching = !self.settings_query.trim().is_empty();
        let mut picked: Option<(&'static str, u8)> = None;

        // La barre de défilement est placée dans la marge de droite et ne prend aucune place : le contenu garde la même
        // largeur dans toutes les sections, qu'elles défilent ou non (avant, tout se décalait de 10 px d'une section à
        // l'autre).
        // Marge de droite du contenu : 28 px. La barre (3 px, à 2 px du bord de la zone) a son centre à 3,5 px de ce bord :
        // avec 17,5 px elle tombe au milieu de la marge, entre les cartes et le bord de la fenêtre.
        const GUTTER: f32 = 17.5;
        let area = ui.available_rect_before_wrap();
        let wide =
            egui::Rect::from_min_max(area.min, egui::pos2(area.right() + GUTTER, area.bottom()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(wide), |ui| {
        ui.spacing_mut().scroll.floating_allocated_width = 0.0;
        // Descente douce jusqu'en bas, redemandée quelques images : la zone de couleur personnalisée est mesurée à l'image suivante.
        let scroll_bottom = self.advanced_scroll > 0;
        if scroll_bottom {
            self.advanced_scroll -= 1;
            ui.ctx().request_repaint();
        }
        // Menu déplié par-dessus le contenu (fenêtre étroite) : la molette ne fait défiler que le menu, pas ce qu'il recouvre.
        let mut scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .scroll_source(egui::containers::scroll_area::ScrollSource {
                mouse_wheel: !self.settings_nav_open,
                ..egui::containers::scroll_area::ScrollSource::default()
            });
        let restoring = self.settings_restore_scroll > 0;
        if restoring {
            scroll_area = scroll_area.vertical_scroll_offset(self.settings_scroll_mem);
            self.settings_restore_scroll -= 1;
            ui.ctx().request_repaint();
        }
        let scroll_out = t::auto_hide_scroll(ui, ("settings_scroll", tab), false, scroll_area, |ui| {
            t::follow_keyboard_focus(ui); // Tab : la zone défile pour suivre le focus
            ui.set_max_width(ui.available_width() - GUTTER);
            // Cartes en colonnes : au plus 2 ; écart de 20 px entre les cartes, horizontalement comme verticalement.
            let n = (((ui.available_width() + 20.0) / (500.0 + 20.0)).floor() as usize).clamp(1, 2);
            ui.spacing_mut().item_spacing.x = 20.0;
            ui.columns(n, |cols| {
                let mut heights = vec![0.0_f32; n];
                // Chaque carte va dans la colonne la moins haute (hauteurs estimées).
                let mut pick = |h: f32| -> usize {
                    let i = heights.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i);
                    heights[i] += h + 20.0;
                    i
                };
                if searching {
                    // Résultats de recherche : un clic ouvre l'onglet et met la ligne en évidence.
                    t::card(&mut cols[0], |ui| {
                        ui.set_width(ui.available_width());
                        t::card_title(ui, crate::i18n::tr("Résultats de la recherche"));
                        ui.add_space(t::SP_S);
                        if results.is_empty() {
                            t::card_body(ui, |ui| {
                                ui.label(RichText::new(crate::i18n::tr("Aucun réglage trouvé.")).color(t::MUTED()));
                            });
                        }
                        for (label, tab) in &results {
                            if t::search_result_row(ui, crate::i18n::tr(label), crate::i18n::tr(SETTINGS_TABS[usize::from(*tab)])).clicked() {
                                picked = Some((label, *tab));
                            }
                        }
                    });
                    return;
                }
                match tab {
                    // Lecture : vidéo, son, lecteur.
                    TAB_PLAYBACK => {
                        let c = pick(230.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            let tr = crate::i18n::tr;
                            t::card_title(ui, tr("Qualité et flux"));
                            ui.add_space(t::SP_S);
                            t::control_row(ui, tr("Qualité par défaut"), |ui| {
                                quality_changed = quality_segmented(ui, &mut self.settings.quality, true);
                            });
                            t::switch_row(ui, tr("Faible latence"), Some(tr("Désactiver pour un flux plus stable")), &mut self.settings.low_latency);
                            t::control_row(ui, tr("Netteté de l'image"), |ui| {
                                video_changed |= t::segmented(
                                    ui,
                                    &mut self.settings.video_sharpen,
                                    &[tr("Désactivée"), tr("Légère"), tr("Moyenne"), tr("Forte")],
                                );
                            });
                            video_changed |= t::switch_row(
                                ui,
                                tr("Agrandissement de haute qualité"),
                                Some(tr("Image plus fine en 480p et 720p")),
                                &mut self.settings.video_upscale,
                            )
                                .changed();
                            t::control_row(ui, tr("Décodage vidéo matériel (GPU)"), |ui| {
                                hw_changed = t::segmented(ui, &mut self.settings.hw_decode,&[tr("Automatique"), tr("Activé"), tr("Désactivé")]);
                            });
                        });
                        cols[c].add_space(t::SP_M);
                        audio_changed = self.card_sound(cols, &mut pick);
                        self.card_player(cols, &mut pick);
                        self.card_screenshots(cols, &mut pick);
                    }
                    // Chat.
                    TAB_CHAT => {
                        self.card_chat(cols, &mut pick, ctx);
                        self.card_chat_filter(cols, &mut pick);
                        self.card_emotes(cols, &mut pick);
                    }
                    // Général : langue, démarrage et fenêtre, accueil et barre latérale, puis recommandations.
                    TAB_GENERAL => {
                        self.card_language(cols, &mut pick);
                        self.card_startup(cols, &mut pick, ctx);
                        self.card_home_sidebar(cols, &mut pick, ctx);
                        self.card_reco(cols, &mut pick, ctx);
                    }
                    // Apparence : thème, couleur d'accent, puis mise à l'échelle.
                    TAB_APPEARANCE => {
                        let c = pick(110.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            let tr = crate::i18n::tr;
                            t::card_title(ui, tr("Thème"));
                            ui.add_space(t::SP_S);
                            t::control_row(ui, tr("Thème"), |ui| {
                                if t::theme_segmented(ui, &mut self.settings.theme, &t::theme_names()) {
                                    // L'ancien thème s'estompe doucement sur le nouveau (voir `begin_theme_change`).
                                    self.begin_theme_change(ctx);
                                }
                            });
                        });
                        cols[c].add_space(t::SP_M);
                        self.card_colors(cols, &mut pick, ctx);
                        self.card_scale(cols, &mut pick, ctx);
                    }
                    // Accessibilité : mouvement et lisibilité (animations, contraste), puis navigation et souris.
                    TAB_ACCESSIBILITY => {
                        self.card_motion(cols, &mut pick, ctx);
                        self.card_navigation(cols, &mut pick, ctx);
                    }
                    // Compte Twitch (« Se déconnecter » est aussi ici, comme sous le menu).
                    TAB_ACCOUNT => {
                        let c = pick(280.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            t::card_title(ui, crate::i18n::tr("Compte Twitch"));
                            ui.add_space(t::SP_S);
                            t::card_body(ui, |ui| {
                                let connected = !self.settings.oauth_token.is_empty() && !self.settings.account_login.is_empty();
                                if connected {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(crate::i18n::tr("Connecté en tant que")).color(t::MUTED()));
                                        ui.label(RichText::new(&self.settings.account_login).strong().color(t::TEXT()));
                                    });
                                    ui.label(
                                        RichText::new(crate::i18n::tr("Vous pouvez écrire dans le chat."))
                                            .size(12.0)
                                            .color(t::FAINT()),
                                    );
                                    let (follows_text, follows_color) = match self.follows_state {
                                        super::FollowsState::Ready => (
                                            format!("{} {}", self.followed.len(), crate::i18n::tr("chaînes suivies sur Twitch chargées.")),
                                            t::FAINT(),
                                        ),
                                        super::FollowsState::NeedsReconnect => (
                                            crate::i18n::tr("Pour charger vos chaînes suivies, déconnectez-vous puis reconnectez-vous.").to_string(),
                                            t::NEGATIVE(),
                                        ),
                                        super::FollowsState::Failed => {
                                            (crate::i18n::tr("Impossible de charger vos chaînes suivies pour le moment.").to_string(), t::FAINT())
                                        }
                                        _ => (crate::i18n::tr("Chargement de vos chaînes suivies…").to_string(), t::FAINT()),
                                    };
                                    ui.label(RichText::new(follows_text).size(12.0).color(follows_color));
                                    // Même déconnexion en deux temps que le bouton du menu (le premier clic arme 5 s, le second déconnecte).
                                    ui.add_space(t::SP_S);
                                    let armed = self.logout_armed_account.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(5));
                                    if !armed {
                                        self.logout_armed_account = None;
                                    }
                                    let (kind, label) = if armed {
                                        (t::BtnKind::Danger, crate::i18n::tr("Confirmer la déconnexion"))
                                    } else {
                                        (t::BtnKind::SecondaryRedText, crate::i18n::tr("Se déconnecter"))
                                    };
                                    if t::icon_text_button(ui, kind, t::NavIcon::Logout, label).clicked() {
                                        if armed {
                                            self.logout_armed_account = None;
                                            do_logout = true;
                                        } else {
                                            self.logout_armed_account = Some(std::time::Instant::now());
                                            ctx.request_repaint_after(std::time::Duration::from_millis(5100));
                                        }
                                    }
                                } else if let Some(code) = &self.auth_code {
                                    ui.label(RichText::new(crate::i18n::tr("Une page Twitch s'est ouverte. Vérifiez ce code, puis cliquez sur « Autoriser » :")).color(t::MUTED()));
                                    ui.label(RichText::new(&code.user_code).size(26.0).strong().color(t::TEXT()));
                                    ui.label(RichText::new(crate::i18n::tr("En attente de votre accord sur Twitch…")).size(12.0).color(t::FAINT()));
                                    ui.horizontal(|ui| {
                                        if t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Refresh, crate::i18n::tr("Rouvrir la page Twitch")).clicked() {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(code.verification_uri.clone()));
                                        }
                                        if t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Close, crate::i18n::tr("Annuler")).clicked() {
                                            do_cancel_login = true;
                                        }
                                    });
                                } else if self.auth_cancel.is_some() {
                                    ui.label(RichText::new(crate::i18n::tr("Connexion à Twitch…")).color(t::MUTED()));
                                } else {
                                    ui.label(
                                        RichText::new(crate::i18n::tr("Connectez-vous pour écrire dans le chat."))
                                            .color(t::MUTED()),
                                    );
                                    ui.add_enabled_ui(crate::auth::available(), |ui| {
                                        if t::icon_text_button(ui, t::BtnKind::Primary, t::NavIcon::Login, crate::i18n::tr("Se connecter avec Twitch")).clicked() {
                                            do_login = true;
                                        }
                                    });
                                    if !crate::auth::available() {
                                        ui.label(
                                            RichText::new(crate::i18n::tr("La connexion Twitch n'est pas disponible dans cette version."))
                                                .size(12.0)
                                                .color(t::FAINT()),
                                        );
                                    }
                                    if let Some((text, error)) = self.login_notice(ctx) {
                                        ui.label(RichText::new(text).color(if error { t::NEGATIVE() } else { t::MUTED() }));
                                    }
                                }
                                if crate::config::secrets_unsaved() && !self.settings.oauth_token.is_empty() {
                                    ui.add_space(t::SP_S);
                                    ui.label(
                                        RichText::new(crate::i18n::tr("La connexion Twitch n'a pas pu être enregistrée de façon sécurisée (trousseau du système indisponible) : elle sera perdue à la fermeture de Velka."))
                                            .size(12.0)
                                            .color(t::NEGATIVE()),
                                    );
                                }
                            });
                        });
                        cols[c].add_space(t::SP_M);

                    }
                    // Notifications : notifications Windows et notification de test.
                    TAB_NOTIFICATIONS => {
                        self.card_notifications(cols, &mut pick);
                    }
                    // Raccourcis clavier : liste modifiable, puis raccourcis fixes.
                    TAB_SHORTCUTS => {
                        self.card_shortcuts(cols, &mut pick, ctx);
                        self.card_shortcuts_fixed(cols, &mut pick);
                    }
                    // À propos : version de l'application, puis vérification des mises à jour.
                    TAB_ABOUT => {
                        self.card_about(cols, &mut pick);
                        self.card_updates(cols, &mut pick, ctx);
                        self.card_help(cols, &mut pick, &mut replay_onboarding);
                        self.card_licenses(cols, &mut pick);
                    }
                    // Avancé et Système : stockage et configuration ; composants externes ; journaux et diagnostic ; zone de réinitialisation.
                    TAB_SYSTEM => {
                        restart_app |= self.card_system(cols, &mut pick, ctx);
                        // À chaque ouverture de la section (et quand l'état a été invalidé) : on contrôle les composants.
                        if !std::mem::replace(&mut self.advanced_visible_prev, true) || self.tools_check == crate::tools::ToolsCheck::Idle {
                            check_wanted = true;
                        }
                        let c = pick(460.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            let title = crate::i18n::tr("Composants externes");
                            let title_resp = t::card_title(ui, title);
                            t::paint_highlight(ui, title_resp.rect, title);
                            ui.add_space(t::SP_S);
                            t::card_body(ui, |ui| {
                                for row in &self.rows {
                                    egui::Frame::new()
                                        .fill(t::SURFACE_2())
                                        .corner_radius(egui::CornerRadius::same(t::RADIUS_INNER))
                                        .inner_margin(12.0)
                                        .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(RichText::new(row.tool.label()).strong());
                                        let tr = crate::i18n::tr;
                                        let rep = match &self.tools_check {
                                            crate::tools::ToolsCheck::Done(v) => v.iter().find(|r| r.tool == row.tool),
                                            _ => None,
                                        };
                                        match &row.status {
                                            ToolStatus::Missing => {
                                                ui.label(RichText::new(tr("Non installé")).color(t::MUTED()));
                                            }
                                            ToolStatus::Installing(f, m) => {
                                                t::progress_bar(ui, *f, m);
                                            }
                                            ToolStatus::Ready(p) => {
                                                use crate::tools::UpdateState;
                                                // L'état est toujours écrit en toutes lettres (jamais la couleur seule).
                                                let (text, color) = match rep.map(|r| r.state) {
                                                    Some(UpdateState::UpToDate) => (tr("À jour"), t::POSITIVE()),
                                                    Some(UpdateState::Outdated) => (tr("Mise à jour disponible"), t::TEXT()),
                                                    Some(UpdateState::Custom) => (tr("Chemin personnalisé"), t::MUTED()),
                                                    _ => (tr("Prêt"), t::POSITIVE()),
                                                };
                                                ui.label(RichText::new(text).color(color));
                                                if let Some(r) = rep.filter(|r| r.state != UpdateState::Custom) {
                                                    let installed = r.installed.as_deref().unwrap_or(tr("Version inconnue"));
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} {installed} · {} {}",
                                                            tr("Installée :"),
                                                            tr("Disponible :"),
                                                            r.latest
                                                        ))
                                                        .size(12.0)
                                                        .color(t::MUTED()),
                                                    );
                                                }
                                                ui.label(RichText::new(p.as_str()).size(11.5).color(t::FAINT()));
                                            }
                                            ToolStatus::Failed(e) => {
                                                ui.label(RichText::new(e.as_str()).color(t::NEGATIVE()));
                                            }
                                        }
                                        // Échec de la dernière mise à jour : l'ancienne version est conservée.
                                        for (tool, e) in &self.tool_errors {
                                            if *tool == row.tool && !matches!(row.status, ToolStatus::Failed(_)) {
                                                let text = if matches!(row.status, ToolStatus::Ready(_)) {
                                                    format!("{} {e}", tr("Échec de la mise à jour, version précédente conservée :"))
                                                } else {
                                                    e.clone()
                                                };
                                                ui.add(egui::Label::new(RichText::new(text).size(12.0).color(t::NEGATIVE())).wrap());
                                            }
                                        }
                                        if row.tool == Tool::Libmpv && self.player.is_some() && rep.is_some_and(|r| r.state.needs_update()) {
                                            ui.label(
                                                RichText::new(tr("Utilisée en ce moment : redémarrez l'application pour la mettre à jour."))
                                                    .size(11.5)
                                                    .color(t::FAINT()),
                                            );
                                        }
                                    });
                                    ui.add_space(t::SP_S);
                                }
                                // Un seul bouton pour tous les composants : actif seulement s'il y a quelque chose à
                                // mettre à jour ; désactivé pour de vrai (et annoncé comme tel) pendant une opération
                                // ou quand tout est à jour ; jamais désactivé sur une erreur (on peut réessayer).
                                {
                                    use crate::tools::{ButtonState, ToolsCheck, UpdateState};
                                    let tr = crate::i18n::tr;
                                    let state = crate::tools::button_state(&self.tools_check, self.busy, self.player.is_some());
                                    if let ToolsCheck::Failed(e) = &self.tools_check {
                                        ui.add(egui::Label::new(RichText::new(e.as_str()).size(12.5).color(t::NEGATIVE())).wrap());
                                        ui.add_space(t::SP_XS);
                                    }
                                    let only_missing = matches!(&self.tools_check, ToolsCheck::Done(v)
                                        if v.iter().filter(|r| r.state.needs_update()).all(|r| r.state == UpdateState::Missing));
                                    let (label, kind, icon) = match state {
                                        ButtonState::Checking => (tr("Vérification…"), t::BtnKind::Status, t::NavIcon::Refresh),
                                        ButtonState::Updating => (tr("Mise à jour en cours…"), t::BtnKind::Status, t::NavIcon::Download),
                                        ButtonState::Update => (
                                            if only_missing { tr("Installer") } else { tr("Mettre à jour") },
                                            t::BtnKind::Primary,
                                            t::NavIcon::Download,
                                        ),
                                        // Coche (état), pas flèche de téléchargement (action) : le bouton n'a rien à télécharger.
                                        ButtonState::UpToDate => (tr("À jour"), t::BtnKind::Status, t::NavIcon::Check),
                                        ButtonState::Retry => (tr("Réessayer"), t::BtnKind::Secondary, t::NavIcon::Refresh),
                                        ButtonState::RestartRequired => (tr("Redémarrage nécessaire"), t::BtnKind::Primary, t::NavIcon::Refresh),
                                    };
                                    // Les états « Status » (À jour, Vérification…, Mise à jour en cours…) sont désactivés pour de
                                    // vrai : le bouton ne réagit à aucun clic. « Redémarrage nécessaire » est cliquable : il
                                    // redémarre l'application, qui met alors libmpv à jour au démarrage.
                                    if t::icon_text_button(ui, kind, icon, label).clicked() {
                                        match state {
                                            ButtonState::Update => update_all = true,
                                            ButtonState::Retry => recheck = true,
                                            ButtonState::RestartRequired => restart_update = true,
                                            _ => {}
                                        }
                                    }
                                    ui.add_space(t::SP_S);
                                }
                                // Bloc replié par défaut : un clic sur l'en-tête le déplie ou le replie, la recherche le déplie
                                // (`pick_setting`) et le surligne comme une ligne de réglage.
                                let paths_label = crate::i18n::tr("Chemins personnalisés (facultatif)");
                                let paths_block = egui::CollapsingHeader::new(paths_label).open(Some(self.paths_open)).show(ui, |ui| {
                                    let sl = ui.add(
                                        egui::TextEdit::singleline(&mut self.settings.streamlink_path)
                                            .hint_text("streamlink.exe")
                                            .desired_width(340.0),
                                    );
                                    let mp = ui.add(
                                        egui::TextEdit::singleline(&mut self.settings.libmpv_path)
                                            .hint_text("libmpv-2.dll")
                                            .desired_width(340.0),
                                    );
                                    redetect = sl.changed() || mp.changed();
                                });
                                if paths_block.header_response.clicked() {
                                    self.paths_open = !self.paths_open;
                                }
                                t::paint_highlight(ui, paths_block.header_response.rect, paths_label);
                            });
                        });
                        cols[c].add_space(t::SP_M);

                        let c = pick(110.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            let title = crate::i18n::tr("Journaux et diagnostic");
                            let title_resp = t::card_title(ui, title);
                            t::paint_highlight(ui, title_resp.rect, title);
                            ui.add_space(t::SP_S);
                            t::card_body(ui, |ui| {
                                let open_label = crate::i18n::tr("Ouvrir le dossier du journal");
                                let open_resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Folder, open_label);
                                open_log = open_resp.clicked();
                                t::paint_highlight(ui, open_resp.rect, open_label);
                                // Diagnostic pour le support : version, Windows, carte graphique, composants.
                                ui.add_space(t::SP_S);
                                let diag_label = crate::i18n::tr("Copier le diagnostic");
                                let diag_resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Download, diag_label);
                                copy_diag = diag_resp.clicked();
                                t::paint_highlight(ui, diag_resp.rect, diag_label);
                                if diag_copied {
                                    ui.label(RichText::new(crate::i18n::tr("Diagnostic copié.")).color(t::MUTED()));
                                }
                                // Rapport de plantage local (rien n'est envoyé) : à copier pour le support.
                                if has_crash {
                                    ui.add_space(t::SP_S);
                                    ui.label(
                                        RichText::new(crate::i18n::tr("Rapport de la dernière fermeture inattendue, sans donnée personnelle. Rien n'est envoyé automatiquement."))
                                            .color(t::MUTED()),
                                    );
                                    ui.add_space(t::SP_S);
                                    let copy_label = crate::i18n::tr("Copier le rapport de plantage");
                                    let copy_resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Download, copy_label);
                                    copy_crash = copy_resp.clicked();
                                    t::paint_highlight(ui, copy_resp.rect, copy_label);
                                    if crash_copied {
                                        ui.label(RichText::new(crate::i18n::tr("Rapport copié.")).color(t::MUTED()));
                                    }
                                }
                            });
                        });
                        cols[c].add_space(t::SP_M);

                        // Zone de réinitialisation, tout en bas : actions destructives, chacune avec une confirmation.
                        let c = pick(190.0);
                        t::card(&mut cols[c], |ui| {
                            ui.set_width(ui.available_width());
                            let title = crate::i18n::tr("Zone de réinitialisation");
                            let title_resp = t::card_title(ui, title);
                            t::paint_highlight(ui, title_resp.rect, title);
                            ui.add_space(t::SP_S);
                            t::card_body(ui, |ui| {
                                match self.confirm {
                                    None => {
                                        ui.label(
                                            RichText::new(crate::i18n::tr(
                                                "Réinitialiser remet les réglages, les favoris et l'historique à zéro. Les outils restent installés.",
                                            ))
                                                .color(t::MUTED()),
                                        );
                                        ui.horizontal_wrapped(|ui| {
                                            let reset_label = crate::i18n::tr("Réinitialiser");
                                            let reset_resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Refresh, reset_label);
                                            t::paint_highlight(ui, reset_resp.rect, reset_label);
                                            if reset_resp.clicked() {
                                                ask = Some(Confirm::Reset);
                                            }
                                            let uninstall_label = crate::i18n::tr("Désinstaller");
                                            let uninstall_resp = t::icon_text_button(ui, t::BtnKind::Danger, t::NavIcon::Trash, uninstall_label);
                                            t::paint_highlight(ui, uninstall_resp.rect, uninstall_label);
                                            if uninstall_resp.clicked() {
                                                ask = Some(Confirm::Uninstall);
                                            }
                                        });
                                        ui.add_space(t::SP_XS);
                                        ui.label(
                                            RichText::new(crate::i18n::tr(
                                                "Désinstaller supprime les outils, les réglages, l'historique et le cache, puis ferme l'application.",
                                            ))
                                                .size(12.0)
                                                .color(t::FAINT()),
                                        );
                                    }
                                    Some(c) => {
                                        let (text, label) = match c {
                                            Confirm::Reset => (
                                                "Tout remettre à zéro ? Réglages, favoris et historique seront perdus.",
                                                "Oui, réinitialiser",
                                            ),
                                            Confirm::Uninstall => (
                                                "Tout supprimer ? Outils, réglages, historique et cache seront effacés définitivement.",
                                                "Oui, désinstaller",
                                            ),
                                        };
                                        ui.label(RichText::new(crate::i18n::tr(text)).color(t::NEGATIVE()));
                                        ui.horizontal(|ui| {
                                            if t::icon_text_button(ui, t::BtnKind::Danger, t::NavIcon::Trash, crate::i18n::tr(label)).clicked() {
                                                confirmed = Some(c);
                                            }
                                            if t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Close, crate::i18n::tr("Annuler")).clicked() {
                                                ask = None;
                                                cancel = true;
                                            }
                                        });
                                    }
                                }
                            });
                        });
                        cols[c].add_space(t::SP_M);
                    }
                    _ => {}
                }
            });
            if scroll_bottom {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
            }
        });
        if !restoring {
            self.settings_scroll_mem = scroll_out.state.clamped_offset().y;
        }
        });

        if let Some((label, tab)) = picked {
            self.pick_setting(ctx, label, tab);
        }

        // Un live est en cours : la nouvelle qualité (ou le nouveau décodage matériel) s'applique tout de suite, flux relancé.
        if quality_changed || hw_changed {
            let running = self.watch.as_ref().is_some_and(|w| {
                matches!(
                    w.state,
                    super::WatchState::Playing
                        | super::WatchState::Starting
                        | super::WatchState::Switching
                        | super::WatchState::Refreshing
                )
            });
            if running {
                self.restart_stream(ctx);
            }
        }
        if do_login {
            self.start_login(ctx);
        }
        if do_cancel_login {
            self.cancel_login();
        }
        if do_logout {
            self.logout();
        }
        if audio_changed {
            self.apply_audio_filter();
        }
        if video_changed {
            self.apply_video_options();
        }
        if recheck {
            self.start_tools_check(ctx, true);
        }
        if update_all {
            self.start_tools_update(ctx);
        }
        if restart_update {
            self.restart_to_update_tools(ctx);
        }
        if check_wanted {
            self.start_tools_check(ctx, false);
        }
        if redetect {
            // Les chemins personnalisés ont changé : l'état des composants est à refaire.
            self.tools_check = crate::tools::ToolsCheck::Idle;
            self.redetect_tools();
        }
        if let Some(c) = ask {
            self.confirm = Some(c);
        }
        if cancel {
            self.confirm = None;
        }
        match confirmed {
            Some(Confirm::Reset) => {
                self.confirm = None;
                self.reset_all(ctx);
            }
            Some(Confirm::Uninstall) => {
                self.confirm = None;
                self.uninstall(ctx);
            }
            None => {}
        }
        if restart_app {
            self.restart_app(ctx);
        }
        if replay_onboarding {
            self.restart_onboarding(ctx);
            self.go_back(ctx);
        }
        if go_back {
            self.go_back(ctx);
        }
        if copy_crash {
            if let Some(r) = &self.crash_report {
                ctx.copy_text(r.clone());
                self.crash_copied = true;
            }
        }
        if copy_diag {
            // Composé en arrière-plan (lit des fichiers, peut lancer `streamlink --version`) ; copié à l'arrivée du message.
            let (streamlink, libmpv) = (
                self.settings.streamlink_path.clone(),
                self.settings.libmpv_path.clone(),
            );
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(super::Msg::Diagnostic(crate::diag::collect(
                    &streamlink,
                    &libmpv,
                )));
                ctx.request_repaint();
            });
        }
        if open_log {
            crate::procs::open_folder(&crate::logging::data_dir());
        }
    }
}

/// Les messages d'erreur de connexion viennent d'une liste fixe : on les traduit si on les reconnaît.
fn e_static(e: &str) -> &'static str {
    const KNOWN: [&str; 5] = [
        "Connexion refusée sur Twitch.",
        "Le code a expiré, recommencez.",
        "Twitch a refusé la connexion.",
        "Impossible de joindre Twitch. Vérifiez votre connexion.",
        "Twitch a refusé la demande de connexion.",
    ];
    KNOWN
        .iter()
        .copied()
        .find(|k| *k == e)
        .unwrap_or("Twitch a refusé la connexion.")
}

// ---------- Options supplémentaires ----------

fn dir_size(p: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

fn cache_bytes() -> u64 {
    crate::cache::dirs().iter().map(|d| dir_size(d)).sum()
}

fn clear_cache() {
    for d in crate::cache::dirs() {
        let _ = std::fs::remove_dir_all(&d);
    }
}

fn format_bytes(n: u64) -> String {
    if n >= 1_048_576 {
        format!(
            "{} {}",
            crate::i18n::decimal(n as f64 / 1_048_576.0, 1),
            crate::i18n::tr("Mo")
        )
    } else {
        format!("{} {}", n.div_ceil(1024), crate::i18n::tr("Ko"))
    }
}

/// Nom d'une touche pour l'affichage.
fn key_label(k: egui::Key) -> String {
    use egui::Key;
    let tr = crate::i18n::tr;
    match k {
        Key::Space => tr("Espace").to_string(),
        Key::ArrowUp => tr("Flèche haut").to_string(),
        Key::ArrowDown => tr("Flèche bas").to_string(),
        Key::ArrowLeft => tr("Flèche gauche").to_string(),
        Key::ArrowRight => tr("Flèche droite").to_string(),
        Key::Enter => tr("Entrée").to_string(),
        other => other.name().to_string(),
    }
}

mod cartes;

#[cfg(test)]
mod tests {
    /// Durée personnalisée écrite en minutes sous une heure, puis en heures.
    #[test]
    fn duree_en_minutes_puis_en_heures() {
        let f = super::format_minutes;
        assert_eq!(
            (f(1).as_str(), f(45).as_str(), f(59).as_str()),
            ("1 min", "45 min", "59 min")
        );
        assert_eq!(
            (
                f(60).as_str(),
                f(90).as_str(),
                f(125).as_str(),
                f(720).as_str()
            ),
            ("1 h", "1 h 30", "2 h 05", "12 h")
        );
    }

    #[test]
    fn onglets_et_raccourcis_traduits() {
        for name in super::SETTINGS_TABS {
            assert!(
                crate::i18n::english(name).is_some(),
                "onglet sans traduction : {name}"
            );
        }
        for (_, label, _) in super::super::SHORTCUTS {
            assert!(
                crate::i18n::english(label).is_some(),
                "raccourci sans traduction : {label}"
            );
        }
        for (label, tab) in super::SEARCH_INDEX {
            assert!(
                crate::i18n::english(label).is_some(),
                "réglage sans traduction : {label}"
            );
            assert!(usize::from(*tab) < super::SETTINGS_TABS.len());
        }
    }

    /// Navigation au clavier : la zone défilante des Réglages suit le focus.
    #[test]
    fn defilement_automatique_au_clavier() {
        let src = include_str!("settings.rs");
        let a = src.find("let scroll_out = scroll_area.show(ui").unwrap();
        assert!(
            src[a..a + 400].contains("follow_keyboard_focus"),
            "le contenu des Réglages suit le focus clavier"
        );
        assert!(include_str!("../ui/theme.rs").contains("pub fn follow_keyboard_focus"));
        assert!(include_str!("mod.rs").contains("track_focus_change"));
    }

    /// Les Réglages sont une fenêtre modale pour le clavier : Tab ne doit jamais atteindre la page du dessous.
    #[test]
    fn overlay_modal_pour_le_clavier() {
        let src = include_str!("settings.rs");
        let a = src.find("pub(super) fn settings_overlay").unwrap();
        let corps = &src[a..a + src[a..].find("fn settings_menu").unwrap()];
        assert!(corps.contains("set_modal_layer"), "couche modale");
        assert!(
            corps.contains("settings_focus_frames = 2"),
            "focus placé dans les Réglages à l'ouverture"
        );
        assert!(
            include_str!("mod.rs").contains("request_focus(egui::Id::unique(\"settings_search\"))")
        );
        assert!(
            corps.contains("egui::Sense::CLICK"),
            "le voile n'est pas un arrêt de Tab"
        );
        assert!(!corps.contains("allocate_exact_size(screen.size(), egui::Sense::click())"));
    }

    fn tab_of(label: &str) -> Vec<u8> {
        super::SEARCH_INDEX
            .iter()
            .filter(|(l, _)| *l == label)
            .map(|(_, t)| *t)
            .collect()
    }

    /// Dix sections, dans l'ordre exact du menu (le numéro d'une section est sa place) ; plus d'« Interface », d'« Accueil »
    /// ni de « Système » vides ; « Langue » n'est pas une section ; chaque constante pointe sa section.
    #[test]
    fn sections_et_ordre() {
        assert_eq!(
            super::SETTINGS_TABS,
            [
                "Compte",
                "Général",
                "Apparence",
                "Lecture vidéo",
                "Chat",
                "Notifications",
                "Raccourcis clavier",
                "Accessibilité",
                "Avancé et Système",
                "À propos",
            ]
        );
        assert_eq!(super::TAB_ICONS.len(), super::SETTINGS_TABS.len());
        for (tab, nom) in [
            (super::TAB_ACCOUNT, "Compte"),
            (super::TAB_GENERAL, "Général"),
            (super::TAB_APPEARANCE, "Apparence"),
            (super::TAB_PLAYBACK, "Lecture vidéo"),
            (super::TAB_CHAT, "Chat"),
            (super::TAB_NOTIFICATIONS, "Notifications"),
            (super::TAB_SHORTCUTS, "Raccourcis clavier"),
            (super::TAB_ACCESSIBILITY, "Accessibilité"),
            (super::TAB_SYSTEM, "Avancé et Système"),
            (super::TAB_ABOUT, "À propos"),
        ] {
            assert_eq!(super::SETTINGS_TABS[usize::from(tab)], nom);
        }
        for ancien in ["Interface", "Accueil", "Système", "Langue"] {
            assert!(
                !super::SETTINGS_TABS.contains(&ancien),
                "{ancien} n'est plus une section"
            );
        }
    }

    /// Chaque section du menu a au moins un réglage dans la recherche, et la recherche ne mène à aucune section inconnue.
    #[test]
    fn chaque_section_a_des_reglages_indexes() {
        for tab in 0..super::SETTINGS_TABS.len() as u8 {
            assert!(
                super::SEARCH_INDEX.iter().any(|(_, t)| *t == tab),
                "section sans réglage indexé : {}",
                super::SETTINGS_TABS[usize::from(tab)]
            );
        }
    }

    /// Chaque raccourci modifiable est trouvable par la recherche, dans la section « Raccourcis clavier ».
    #[test]
    fn chaque_raccourci_est_indexe() {
        for (_, label, _) in super::super::SHORTCUTS {
            assert!(
                super::SEARCH_INDEX
                    .iter()
                    .any(|(l, tab)| *l == label && *tab == super::TAB_SHORTCUTS),
                "raccourci non indexé : {label}"
            );
        }
    }

    /// Chaque réglage déplacé est indexé une seule fois, dans sa nouvelle section (aucun doublon).
    #[test]
    fn reglages_deplaces_une_seule_fois() {
        for (label, section) in [
            ("Thème", "Apparence"),
            ("Langue", "Général"),
            ("Animations", "Accessibilité"),
            ("Raccourcis clavier", "Raccourcis clavier"),
            ("Taille de l'interface", "Apparence"),
            ("Contraste élevé", "Accessibilité"),
            ("Focus clavier renforcé", "Accessibilité"),
            ("Palette de couleurs", "Apparence"),
            ("Couleur d'accent personnalisée", "Apparence"),
            ("Rétablir les couleurs par défaut", "Apparence"),
            ("Compte Twitch", "Compte"),
            (crate::i18n::os::STARTUP, "Général"),
            ("Chaîne ouverte au démarrage", "Général"),
            (crate::i18n::os::NOTIFY, "Notifications"),
            ("Cache des images", "Avancé et Système"),
            ("Composants externes", "Avancé et Système"),
            ("Chemins personnalisés (facultatif)", "Avancé et Système"),
            ("Journaux et diagnostic", "Avancé et Système"),
            ("Ouvrir le dossier du journal", "Avancé et Système"),
            ("Ouvrir le dossier de configuration", "Avancé et Système"),
            ("Copier le diagnostic", "Avancé et Système"),
            ("Copier le rapport de plantage", "Avancé et Système"),
            ("Zone de réinitialisation", "Avancé et Système"),
            ("Réinitialiser", "Avancé et Système"),
            ("Désinstaller", "Avancé et Système"),
            ("Vérification des mises à jour", "À propos"),
            ("Rechercher des mises à jour", "À propos"),
            ("Version de l'application", "À propos"),
            ("Effacer l'historique", "Général"),
            ("Revoir le didacticiel", "À propos"),
            ("Licences des composants", "À propos"),
        ] {
            let tabs = tab_of(label);
            assert_eq!(tabs.len(), 1, "{label} : indexé {} fois", tabs.len());
            assert_eq!(
                super::SETTINGS_TABS[usize::from(tabs[0])],
                section,
                "{label}"
            );
        }
        let mut labels: Vec<&str> = super::SEARCH_INDEX.iter().map(|(l, _)| *l).collect();
        labels.sort_unstable();
        let n = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), n, "libellé indexé deux fois");
    }

    /// Dans le code, chaque carte n'est affichée que dans sa section : pas de copie.
    #[test]
    fn cartes_une_seule_fois_dans_le_code() {
        let src = include_str!("settings.rs");
        let region = |from: &str, to: &str| {
            let a = src.find(from).unwrap();
            &src[a..a + src[a..].find(to).unwrap()]
        };
        let general = region("// Général : langue", "// Apparence : thème");
        let apparence = region("// Apparence : thème", "// Accessibilité : mouvement");
        let access = region("// Accessibilité : mouvement", "// Compte Twitch (");
        let compte = region("// Compte Twitch (", "// Notifications : notifications");
        let notifs = region(
            "// Notifications : notifications",
            "// Raccourcis clavier : liste",
        );
        let raccourcis = region("// Raccourcis clavier : liste", "// À propos : version");
        let apropos = region("// À propos : version", "// Avancé et Système : stockage");
        let systeme = region(
            "// Avancé et Système : stockage",
            "
                    _ => {}",
        );
        // Le thème est dans « Apparence » seulement.
        assert!(apparence.contains("theme_segmented") && !general.contains("theme_segmented"));
        assert!(
            apparence.contains("card_title(ui, tr(\"Thème\"))"),
            "groupe « Thème »"
        );
        for (nom, code) in [
            ("Général", general),
            ("Apparence", apparence),
            ("Compte", compte),
            ("Système", systeme),
        ] {
            assert!(
                nom == "Général" || !code.contains("card_language"),
                "{nom} : langue"
            );
            assert!(
                nom == "Général" || !code.contains("card_startup"),
                "{nom} : démarrage et fenêtre"
            );
            assert!(
                nom == "Général" || !code.contains("card_home_sidebar"),
                "{nom} : accueil et barre latérale"
            );
            assert!(
                nom == "Général" || !code.contains("card_reco"),
                "{nom} : recommandations"
            );
            assert!(
                !code.contains("card_notifications"),
                "{nom} : notifications"
            );
            assert!(
                !code.contains("card_navigation"),
                "{nom} : navigation et souris"
            );
            assert!(
                nom == "Apparence" || !code.contains("card_colors"),
                "{nom} : couleur d'accent"
            );
            assert!(
                nom == "Apparence" || !code.contains("card_scale"),
                "{nom} : mise à l'échelle"
            );
        }
        assert!(
            access.contains("card_motion")
                && access.contains("card_navigation")
                && !access.contains("card_colors")
                && !access.contains("card_scale")
        );
        // Apparence : « Thème », puis « Couleur d'accent », puis « Mise à l'échelle ».
        let (p_theme, p_accent, p_scale) = (
            apparence.find("card_title(ui, tr(\"Thème\"))").unwrap(),
            apparence.find("card_colors").unwrap(),
            apparence.find("card_scale").unwrap(),
        );
        assert!(
            p_theme < p_accent && p_accent < p_scale,
            "ordre Thème, Couleur d'accent, Mise à l'échelle"
        );
        assert!(
            notifs.contains("card_notifications") && !systeme.contains("notify_live"),
            "notifications dans leur section"
        );
        assert!(
            general.contains("card_language"),
            "la langue est dans Général"
        );
        assert!(
            general.find("card_language").unwrap() < general.find("card_startup").unwrap(),
            "ordre Langue, Démarrage et fenêtre"
        );
        assert!(
            general.find("card_startup").unwrap() < general.find("card_home_sidebar").unwrap(),
            "ordre Démarrage et fenêtre, Accueil et barre latérale"
        );
        assert!(
            general.find("card_home_sidebar").unwrap() < general.find("card_reco").unwrap(),
            "ordre Accueil et barre latérale, Recommandations"
        );
        assert!(
            !general.contains("card_mouse")
                && !general.contains("card_shortcuts")
                && !apparence.contains("card_shortcuts")
        );
        assert!(
            raccourcis.contains("card_shortcuts(") && raccourcis.contains("card_shortcuts_fixed")
        );
        assert!(
            raccourcis.find("card_shortcuts(").unwrap()
                < raccourcis.find("card_shortcuts_fixed").unwrap(),
            "liste avant l'encart"
        );
        assert!(
            compte.contains("Se déconnecter")
                && compte.contains("Confirmer la déconnexion")
                && compte.contains("do_logout = true"),
            "Compte a aussi « Se déconnecter » en deux temps"
        );
        assert!(systeme.contains("card_system") && systeme.contains("paths_open"));
        assert!(!systeme.contains("check_updates"));
        // Plus d'interrupteur « Avancé » : « Composants externes » puis « Journaux et diagnostic » sont toujours affichés,
        // puis, tout en bas, « Zone de réinitialisation » (Réinitialiser, Désinstaller).
        assert!(
            !systeme.contains("tr(\"Avancé\")") && !systeme.contains("settings_advanced"),
            "interrupteur « Avancé » supprimé"
        );
        assert!(
            !systeme.contains("tr(\"Données\")"),
            "« Données » est devenu « Zone de réinitialisation »"
        );
        let (p_comp, p_jour, p_zone) = (
            systeme.find("tr(\"Composants externes\")").unwrap(),
            systeme.find("tr(\"Journaux et diagnostic\")").unwrap(),
            systeme.find("tr(\"Zone de réinitialisation\")").unwrap(),
        );
        assert!(
            p_comp < p_jour && p_jour < p_zone,
            "ordre Composants externes, Journaux et diagnostic, Zone de réinitialisation"
        );
        assert!(
            systeme.find("tr(\"Réinitialiser\")").unwrap() > p_zone
                && systeme.find("tr(\"Désinstaller\")").unwrap() > p_zone,
            "Réinitialiser et Désinstaller sont dans la zone de réinitialisation"
        );
        assert!(
            systeme
                .find("tr(\"Chemins personnalisés (facultatif)\")")
                .unwrap()
                > p_comp
                && systeme
                    .find("tr(\"Chemins personnalisés (facultatif)\")")
                    .unwrap()
                    < p_jour
                && systeme.contains("CollapsingHeader"),
            "chemins personnalisés : bloc dépliable dans « Composants externes »"
        );
        assert!(
            systeme.find("card_system").unwrap() < p_comp,
            "ordre Système, Composants externes"
        );
        // « À propos » : la version puis les mises à jour ; plus rien de cela dans « Avancé et Système ».
        assert!(
            apropos.find("card_about").unwrap() < apropos.find("card_updates").unwrap(),
            "ordre Version, Mises à jour"
        );
        assert!(
            apropos.find("card_updates").unwrap() < apropos.find("card_help").unwrap(),
            "ordre Mises à jour, Aide"
        );
        assert!(
            apropos.find("card_help").unwrap() < apropos.find("card_licenses").unwrap(),
            "ordre Aide, Licences des composants"
        );
        assert!(
            apropos.contains("card_help") && !general.contains("card_help"),
            "le didacticiel est dans À propos"
        );
        assert!(
            !systeme.contains("card_updates") && !systeme.contains("card_about"),
            "version et mises à jour ne sont plus dans Système"
        );
        // « Se déconnecter » est épinglé sous le menu.
        let menu = region("fn settings_menu", "fn card_language");
        assert!(menu.contains("Se déconnecter") && menu.contains("self.logout()"));
    }

    /// La recherche trouve les réglages déplacés et les nouveaux réglages de couleurs, et mène à « Accessibilité ».
    #[test]
    fn recherche_reglages_accessibilite() {
        for (requete, libelle) in [
            ("contraste", "Contraste élevé"),
            ("focus", "Focus clavier renforcé"),
        ] {
            let r = super::search(requete);
            assert!(
                r.iter()
                    .any(|(l, t)| *l == libelle && *t == super::TAB_ACCESSIBILITY),
                "« {requete} » devrait mener à {libelle} dans Accessibilité : {r:?}"
            );
        }
    }

    /// Les réglages de couleur (groupe « Couleur d'accent ») et la taille (groupe « Mise à l'échelle ») se trouvent dans
    /// « Apparence », plus dans « Accessibilité ».
    #[test]
    fn recherche_couleur_d_accent() {
        for (requete, libelle) in [
            ("palette", "Palette de couleurs"),
            ("accent", "Couleur d'accent personnalisée"),
            ("retablir couleurs", "Rétablir les couleurs par défaut"),
            ("taille interface", "Taille de l'interface"),
        ] {
            let r = super::search(requete);
            assert!(
                r.iter()
                    .any(|(l, t)| *l == libelle && *t == super::TAB_APPEARANCE),
                "« {requete} » devrait mener à {libelle} dans Apparence : {r:?}"
            );
            assert!(
                !r.iter()
                    .any(|(l, t)| *l == libelle && *t == super::TAB_ACCESSIBILITY),
                "{libelle} encore dans Accessibilité"
            );
        }
    }

    /// La recherche trouve chaque réglage déplacé et mène à sa nouvelle section.
    #[test]
    fn recherche_reglages_deplaces() {
        for (requete, libelle, section) in [
            ("langue", "Langue", "Général"),
            ("theme", "Thème", "Apparence"),
            ("sombre", "Thème", "Apparence"),
            ("dark", "Thème", "Apparence"),
            ("mises a jour", "Vérification des mises à jour", "À propos"),
            ("version", "Version de l'application", "À propos"),
            ("journaux", "Journaux et diagnostic", "Avancé et Système"),
            (
                "journal",
                "Ouvrir le dossier du journal",
                "Avancé et Système",
            ),
            ("diagnostic", "Journaux et diagnostic", "Avancé et Système"),
            ("diagnostic", "Copier le diagnostic", "Avancé et Système"),
            (
                "plantage",
                "Copier le rapport de plantage",
                "Avancé et Système",
            ),
            ("composants", "Composants externes", "Avancé et Système"),
            ("streamlink", "Composants externes", "Avancé et Système"),
            ("libmpv", "Composants externes", "Avancé et Système"),
            (
                "chemins",
                "Chemins personnalisés (facultatif)",
                "Avancé et Système",
            ),
            ("zone", "Zone de réinitialisation", "Avancé et Système"),
            ("reinitialiser", "Réinitialiser", "Avancé et Système"),
            ("desinstaller", "Désinstaller", "Avancé et Système"),
            ("avance", "Composants externes", "Avancé et Système"),
            ("cache", "Cache des images", "Avancé et Système"),
            ("compte twitch", "Compte Twitch", "Compte"),
        ] {
            let r = super::search(requete);
            assert!(
                r.iter().any(|(l, t)| *l == libelle && super::SETTINGS_TABS[usize::from(*t)] == section),
                "« {requete} » devrait mener à {libelle} dans {section} : {r:?}"
            );
        }
    }

    #[test]
    fn recherche_tolerante() {
        // Les tests partagent la langue courante (français par défaut) : on ne cherche que des libellés français.
        let r = super::search("nettete");
        assert!(r
            .iter()
            .any(|(l, t)| *l == "Netteté de l'image" && *t == super::TAB_PLAYBACK));
        assert!(super::search("   ").is_empty());
        assert!(super::search("chat gauche")
            .iter()
            .any(|(l, _)| *l == "Chat à gauche de la vidéo"));
        assert!(super::search("zzzz introuvable").is_empty());
    }

    #[test]
    fn recherche_classee() {
        // « comp » : le réglage dont le libellé commence par « comp » d'abord, puis ce qui est dans l'onglet Compte (désormais seulement lui),
        // et seulement ensuite les réglages dont l'explication contient « compte ».
        let r = super::search("comp");
        assert_eq!(r.first().map(|x| x.0), Some("Compte Twitch"));
        let first = |q: &str| super::search(q).first().map(|x| x.0);
        assert_eq!(first("compte"), Some("Compte Twitch"));
        // L'ancien nom (« normalisation du son ») reste un synonyme : il est dans l'explication du réglage.
        assert_eq!(first("normalisation"), Some("Volume stable"));
        assert_eq!(first("volume stable"), Some("Volume stable"));
        // Mot court : jamais trouvé au milieu d'un autre mot (« personnalisée »).
        assert!(!super::search("son")
            .iter()
            .any(|(l, _)| l.contains("personnalisée")));
        assert_eq!(first("theme"), Some("Thème"));
        assert_eq!(first("volume max"), Some("Volume maximum"));
        assert_eq!(first("raccourci"), Some("Raccourcis clavier"));
        assert_eq!(first("veille"), Some("Minuteur de veille"));
        assert_eq!(first("emote"), Some("Émotes 7TV, BTTV et FFZ"));
        assert_eq!(first("notif"), Some(crate::i18n::os::NOTIFY));
    }
}
