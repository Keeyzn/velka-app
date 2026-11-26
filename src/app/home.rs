//! Accueil : statistiques, résultat de recherche, chaînes suivies en direct, récents.
use super::VelkaApp;
use crate::history::toggle_favorite;
use crate::twitch::LiveStatus;
use crate::ui::theme as t;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Vec2};

#[derive(Clone)]
struct LiveInfo {
    login: String,
    viewers: u64,
    game: String,
    title: String,
    avatar: Option<egui::TextureId>,
    thumb: Option<egui::TextureId>,
}

fn status_text(st: &LiveStatus) -> (String, Color32) {
    match st {
        LiveStatus::Live {
            viewers,
            game,
            title,
            ..
        } => {
            let mut s = format!(
                "{} {}",
                crate::login::format_viewers(*viewers),
                crate::i18n::tr("spectateurs")
            );
            if !game.is_empty() {
                s.push_str(&format!(" · {game}"));
            }
            if !title.is_empty() {
                s.push_str(&format!(" — {title}"));
            }
            (s, t::MUTED())
        }
        LiveStatus::Offline => (crate::i18n::tr("Hors ligne").into(), t::MUTED()),
        LiveStatus::Missing => (
            crate::i18n::tr("Cette chaîne n'existe pas.").into(),
            t::NEGATIVE(),
        ),
        LiveStatus::Unknown => (
            crate::i18n::tr("Statut indisponible, vous pouvez quand même essayer.").into(),
            t::MUTED(),
        ),
    }
}

/// Puce de l'historique : pastille verte si en direct, grise sinon (dessinée, sans glyphe).
pub(super) fn chip(ui: &mut egui::Ui, login: &str, live: bool) -> egui::Response {
    let b = egui::Button::new(
        RichText::new(format!("    {login}"))
            .font(t::name_font(14.0))
            .color(t::TEXT()),
    )
    .corner_radius(egui::CornerRadius::same(t::RADIUS_PILL));
    let r = ui.add(b);
    let c = Pos2::new(r.rect.left() + 15.0, r.rect.center().y);
    ui.painter()
        .circle_filled(c, 4.0, if live { t::POSITIVE() } else { t::FAINT() });
    r
}

/// Hauteur disponible (sous le titre) à partir de laquelle les jeux et les récents restent fixes.
const STICKY_MIN_H: f32 = 650.0;
/// Largeur à partir de laquelle « Accueil » et les compteurs tiennent sur une seule ligne (points). Le tri, lui, est
/// toujours sur la ligne d'« Accueil ».
const HOME_WIDE_MIN_W: f32 = 1060.0;
/// Échelle du bouton de tri (et de « Actualiser ») : tout est réduit dans la même proportion, rapport largeur / hauteur
/// inchangé.
const SORT_BTN_SCALE: f32 = 0.85;
/// Le bouton de tri n'affiche son infobulle qu'après ce délai de survol (secondes) : elle ne gêne pas l'usage courant.
const SORT_TIP_DELAY: f32 = 5.0;
/// Distance de défilement (points) au clic sur la flèche vers le bas de la page (une rangée de cartes environ).
const HOME_SCROLL_STEP: f32 = 300.0;

/// Pastilles de l'historique : clic = ouvrir, clic droit = retirer de l'historique.
/// Rangée à défilement horizontal (jeux en direct, récents). Quand elle déborde, la molette de la souris la fait défiler
/// de côté au survol ; si tout tient, la molette continue de faire défiler la page. Le débordement est mesuré à l'image
/// précédente (mémorisé par rangée).
fn wheel_row(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    salt: &'static str,
    content: impl FnOnce(&mut egui::Ui),
) {
    let overflow_id = egui::Id::unique(format!("{salt}_overflow"));
    let area = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 38.0));
    let overflowed = ctx
        .data(|d| d.get_temp::<bool>(overflow_id))
        .unwrap_or(false);
    if overflowed && ctx.pointer_hover_pos().is_some_and(|p| area.contains(p)) {
        ctx.input_mut(|i| {
            i.smooth_scroll_delta.x += i.smooth_scroll_delta.y;
            i.smooth_scroll_delta.y = 0.0;
        });
    }
    let out = t::auto_hide_scroll(
        ui,
        salt,
        true,
        egui::ScrollArea::horizontal().auto_shrink([false, true]),
        content,
    );
    let now = out.content_size.x > out.inner_rect.width() + 1.0;
    if now != overflowed {
        ctx.data_mut(|d| d.insert_temp(overflow_id, now));
    }
}

fn recent_chips(
    ui: &mut egui::Ui,
    items: &[String],
    is_live: &dyn Fn(&str) -> bool,
    to_watch: &mut Option<String>,
    remove_hist: &mut Option<String>,
) {
    for l in items {
        let r = chip(ui, l, is_live(l));
        if r.clicked() {
            *to_watch = Some(l.clone());
        }
        r.context_menu(|ui| {
            if ui
                .button(crate::i18n::tr("Retirer de l'historique"))
                .clicked()
            {
                *remove_hist = Some(l.clone());
                ui.close();
            }
        });
    }
}

/// Compteurs sur une ligne : valeur en couleur d'accentuation, puis libellé.
fn stats_inline(ui: &mut egui::Ui, stats: &[(&'static str, String); 3]) {
    ui.spacing_mut().item_spacing.x = 5.0;
    for (i, (label, value)) in stats.iter().enumerate() {
        if i > 0 {
            ui.add_space(t::SP_M);
        }
        // Un seul bloc de texte par stat : chiffre et libellé partagent la même ligne de base malgré leurs tailles.
        let base = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        let big = egui::text::TextFormat {
            font_id: egui::FontId::new(22.0, base.family.clone()),
            color: t::ACCENT_GFX(),
            ..Default::default()
        };
        let small = egui::text::TextFormat {
            font_id: egui::FontId::new(17.0, base.family),
            color: t::MUTED(),
            ..Default::default()
        };
        job.append(value, 0.0, big);
        job.append(label, 5.0, small);
        ui.label(job);
    }
}

/// Hauteur sous la miniature : nom, titre sur deux lignes au plus, jeu.
const CARD_INFO_H: f32 = 80.0;

/// Card de stream : miniature 16:9, pills LIVE + spectateurs, avatar, titre, chaîne, jeu.
fn stream_card(ui: &mut egui::Ui, width: f32, info: &LiveInfo, index: usize) -> egui::Response {
    // Apparition en fondu, en cascade de haut en bas (chaque carte attend un peu plus que la précédente).
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let key = egui::Id::unique(("card_first_seen", &info.login));
    let first = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with::<f64>(key, || now));
    ui.set_opacity(t::motion::stagger_in(&ctx, first, index));
    let viewers = ui
        .ctx()
        .animate_value_with_time(
            egui::Id::unique(("card_viewers", &info.login)),
            info.viewers as f32,
            t::anim_secs(ui.ctx(), 0.7),
        )
        .round() as u64;
    let pad = 8.0;
    let thumb_h = (width - 2.0 * pad) * 9.0 / 16.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, thumb_h + 2.0 * pad + CARD_INFO_H),
        Sense::click(),
    );
    let radius = egui::CornerRadius::same(t::RADIUS_CARD);
    let border = if resp.hovered() {
        t::BORDER_HOVER()
    } else {
        t::BORDER()
    };
    let p = ui.painter();
    p.rect_filled(rect, radius, t::SURFACE_1());
    p.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    t::focus_ring(ui, &resp, rect, t::RADIUS_CARD);
    let thumb = Rect::from_min_size(
        rect.min + Vec2::splat(pad),
        Vec2::new(width - 2.0 * pad, thumb_h),
    );
    p.rect_filled(thumb, egui::CornerRadius::same(12), t::SURFACE_2());
    match info.thumb {
        Some(id) => {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            p.add(
                egui::epaint::RectShape::filled(
                    thumb,
                    egui::CornerRadius::same(12),
                    Color32::WHITE,
                )
                .with_texture(id, uv),
            );
        }
        None => {
            let initial = info
                .login
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            p.text(
                thumb.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                egui::FontId::proportional(40.0),
                t::FAINT(),
            );
        }
    }

    let below = thumb.bottom() + 10.0;
    // Avatar cerclé de vert (chaîne en direct), puis le nom de la chaîne en premier et bien lisible.
    let avatar_c = Pos2::new(rect.left() + pad + 15.0, below + 18.0);
    t::avatar(ui, avatar_c, 14.0, &info.login, info.avatar);
    ui.painter()
        .circle_stroke(avatar_c, 16.5, egui::Stroke::new(1.8, t::POSITIVE()));
    let tx = rect.left() + pad + 40.0;
    let tw = width - pad - 40.0 - pad;
    t::paint_name_line(
        ui.painter(),
        Pos2::new(tx, below),
        &info.login,
        15.0,
        t::TEXT(),
        tw,
    );
    // Titre sur deux lignes au plus (coupé par « … »), puis le jeu en gris discret juste dessous.
    let mut game_y = below + 20.0;
    if !info.title.is_empty() {
        let mut job = egui::text::LayoutJob::simple(
            info.title.clone(),
            egui::FontId::proportional(12.5),
            t::MUTED(),
            tw,
        );
        job.wrap.max_rows = 2;
        job.wrap.overflow_character = Some('…');
        let galley = ui.painter().layout_job(job);
        game_y += galley.size().y + 3.0;
        ui.painter()
            .galley(Pos2::new(tx, below + 20.0), galley, t::MUTED());
    }
    if !info.game.is_empty() {
        t::paint_one_line(
            ui.painter(),
            Pos2::new(tx, game_y),
            &info.game,
            12.0,
            t::FAINT(),
            tw,
        );
    }

    t::paint_pressed(ui, &resp, rect, t::RADIUS_CARD);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
        thumb.min + Vec2::splat(8.0),
        Vec2::new(thumb.width() - 16.0, 24.0),
    )));
    child.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        t::live_pill(ui);
        let word = if viewers == 1 {
            crate::i18n::tr("spectateur")
        } else {
            crate::i18n::tr("spectateurs")
        };
        t::pill(
            ui,
            None,
            &format!("{} {word}", crate::login::format_viewers(viewers)),
            Color32::WHITE,
            t::ACCENT(),
        );
    });
    ui.set_opacity(1.0);
    resp
}

/// Carte « squelette » : mêmes dimensions qu'une vraie carte, avec un reflet qui défile pendant le chargement.
fn skeleton_card(ui: &mut egui::Ui, width: f32) {
    let pad = 8.0;
    let thumb_h = (width - 2.0 * pad) * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, thumb_h + 2.0 * pad + CARD_INFO_H),
        Sense::hover(),
    );
    let radius = egui::CornerRadius::same(t::RADIUS_CARD);
    let p = ui.painter();
    p.rect_filled(rect, radius, t::SURFACE_1());
    p.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, t::BORDER()),
        egui::StrokeKind::Inside,
    );
    let thumb = Rect::from_min_size(
        rect.min + Vec2::splat(pad),
        Vec2::new(width - 2.0 * pad, thumb_h),
    );
    let below = thumb.bottom() + 10.0;
    let tx = rect.left() + pad + 40.0;
    let tw = width - pad - 40.0 - pad;
    let bars = [
        (thumb, 12u8),
        (
            Rect::from_min_size(Pos2::new(tx, below + 2.0), Vec2::new(tw * 0.55, 13.0)),
            6,
        ),
        (
            Rect::from_min_size(Pos2::new(tx, below + 23.0), Vec2::new(tw * 0.9, 10.0)),
            5,
        ),
        (
            Rect::from_min_size(Pos2::new(tx, below + 38.0), Vec2::new(tw * 0.7, 10.0)),
            5,
        ),
        (
            Rect::from_min_size(Pos2::new(tx, below + 58.0), Vec2::new(tw * 0.4, 10.0)),
            5,
        ),
    ];
    for (r, cr) in bars {
        p.rect_filled(r, egui::CornerRadius::same(cr), t::SURFACE_2());
    }
    p.circle_filled(
        Pos2::new(rect.left() + pad + 15.0, below + 18.0),
        14.0,
        t::SURFACE_2(),
    );
    // Reflet qui traverse la miniature de gauche à droite.
    let time = ui.ctx().input(|i| i.time);
    if t::moves(ui.ctx()) {
        let phase = ((time * 0.8) % 1.0) as f32;
        let band = thumb.width() * 0.35;
        let x = thumb.left() - band + phase * (thumb.width() + band);
        let shine = Rect::from_min_max(
            Pos2::new(x.max(thumb.left()), thumb.top()),
            Pos2::new((x + band).min(thumb.right()), thumb.bottom()),
        );
        if shine.width() > 0.0 {
            p.rect_filled(
                shine,
                egui::CornerRadius::same(12),
                Color32::from_white_alpha(10),
            );
        }
        ui.ctx().request_repaint();
    }
}

/// Grille de squelettes affichée tant que les lives ne sont pas arrivés.
/// Elle remplit tout l'espace visible sous le curseur (au moins une rangée).
fn skeleton_grid(ui: &mut egui::Ui) {
    let gap = 16.0;
    let avail = ui.available_width();
    let cols = (((avail + gap) / (260.0 + gap)).floor() as usize).max(1);
    let w = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor() - 1.0;
    let card_h = (w - 16.0) * 9.0 / 16.0 + 16.0 + CARD_INFO_H;
    let visible = (ui.clip_rect().bottom() - ui.cursor().top()).max(0.0);
    let rows = ((visible / (card_h + gap)).ceil() as usize).clamp(1, 8);
    ui.spacing_mut().item_spacing = Vec2::splat(gap);
    ui.horizontal_wrapped(|ui| {
        for _ in 0..rows * cols {
            skeleton_card(ui, w);
        }
    });
    ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
}

/// Espace entre deux cartes de la grille (points).
const CARD_GAP: f32 = 16.0;

/// Nombre de colonnes et largeur d'une carte pour la largeur disponible `avail`.
fn grid_layout(avail: f32) -> (usize, f32) {
    let cols = (((avail + CARD_GAP) / (260.0 + CARD_GAP)).floor() as usize).max(1);
    let w = ((avail - CARD_GAP * (cols as f32 - 1.0)) / cols as f32).floor() - 1.0;
    (cols, w)
}

/// Largeur réellement occupée par la grille de cartes (un peu moins que la largeur disponible : arrondis des cartes).
/// Sert à aligner le bord droit du bouton de tri sur celui des cartes.
fn grid_width(avail: f32) -> f32 {
    let (cols, w) = grid_layout(avail);
    cols as f32 * w + (cols as f32 - 1.0) * CARD_GAP
}

/// Grille de cards ; renvoie la chaîne cliquée.
fn card_grid(ui: &mut egui::Ui, infos: &[LiveInfo], slot: u8) -> Option<String> {
    let mut clicked = None;
    let gap = CARD_GAP;
    let avail = ui.available_width();
    let (cols, w) = grid_layout(avail);
    // Placement à la main : chaque carte glisse vers sa place quand la liste change (un live qui s'arrête
    // fait remonter les suivantes au lieu de les faire sauter).
    let card_h = (w - 16.0) * 9.0 / 16.0 + 16.0 + CARD_INFO_H;
    let rows = infos.len().div_ceil(cols);
    let total_h = if rows == 0 {
        0.0
    } else {
        rows as f32 * card_h + (rows as f32 - 1.0) * gap
    };
    let (outer, _) = ui.allocate_exact_size(Vec2::new(avail, total_h), Sense::hover());
    let ctx = ui.ctx().clone();
    // Largeur de la grille changée (barre latérale qui se déplie, fenêtre redimensionnée) : les cartes suivent la mise en
    // page sans glisser, sinon elles traînent derrière. Le glissement ne sert que quand la liste elle-même change.
    let avail_id = egui::Id::unique("card_grid_avail");
    let last_avail = ctx.data(|d| d.get_temp::<f32>(avail_id));
    ctx.data_mut(|d| d.insert_temp(avail_id, avail));
    let snap = !t::moves(&ctx) || last_avail.is_none_or(|l| (l - avail).abs() > 0.5);
    let step = t::motion::glide_factor(ctx.input(|i| i.stable_dt).clamp(0.001, 0.05));
    let mut moving = false;
    for (i, info) in infos.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let target = Vec2::new(col as f32 * (w + gap), row as f32 * (card_h + gap));
        // Le défilement de la page déplace `outer` : on garde la position relative à la grille, pas l'absolue.
        let key = egui::Id::unique(("card_pos", &info.login));
        let rel = match ctx.data(|d| d.get_temp::<Vec2>(key)) {
            Some(cur) if !snap => {
                let next = cur + (target - cur) * step;
                if (target - next).length() < 0.5 {
                    target
                } else {
                    next
                }
            }
            _ => target,
        };
        moving |= rel != target;
        ctx.data_mut(|d| d.insert_temp(key, rel));
        let pos = outer.min + rel;
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("card_slot", &info.login))
                .max_rect(Rect::from_min_size(pos, Vec2::new(w, card_h))),
        );
        if stream_card(&mut child, w, info, i).clicked() {
            clicked = Some(info.login.clone());
        }
    }
    if moving {
        ctx.request_repaint();
    }
    forget_gone_cards(&ctx, infos, slot);
    clicked
}

/// Oublie la place et l'instant d'apparition des chaînes qui ne sont plus dans la grille `slot` (et dans aucune autre) :
/// la mémoire ne grossit pas au fil des jours, et une chaîne qui revient réapparaît en fondu comme une nouvelle.
fn forget_gone_cards(ctx: &egui::Context, infos: &[LiveInfo], slot: u8) {
    use std::collections::HashMap;
    let id = egui::Id::unique("card_grid_logins");
    let now: Vec<String> = infos.iter().map(|i| i.login.clone()).collect();
    let mut all: HashMap<u8, Vec<String>> = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if all.get(&slot) == Some(&now) {
        return;
    }
    let before = all.insert(slot, now.clone()).unwrap_or_default();
    for login in before
        .iter()
        .filter(|l| !now.contains(l) && !all.values().any(|v| v.contains(l)))
    {
        ctx.data_mut(|d| {
            d.remove_temp::<f64>(egui::Id::unique(("card_first_seen", login)));
            d.remove_temp::<Vec2>(egui::Id::unique(("card_pos", login)));
        });
    }
    ctx.data_mut(|d| d.insert_temp(id, all));
}

/// Trie les cards : le jeu choisi d'abord (s'il y en a un), puis spectateurs décroissants (0) ou croissants (1).
fn sort_infos(v: &mut [LiveInfo], mode: u8, game: Option<&str>) {
    let chosen = |l: &LiveInfo| game.is_some_and(|g| l.game.eq_ignore_ascii_case(g));
    v.sort_by(|a, b| {
        (!chosen(a)).cmp(&!chosen(b)).then_with(|| {
            if mode == 1 {
                a.viewers.cmp(&b.viewers)
            } else {
                b.viewers.cmp(&a.viewers)
            }
        })
    });
}

/// Jeux en direct dans les listes, avec le nombre de chaînes : les plus représentés d'abord.
fn games_of(lists: &[&[LiveInfo]]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for l in lists.iter().flat_map(|l| l.iter()) {
        if l.game.is_empty() {
            continue;
        }
        match counts
            .iter_mut()
            .find(|(g, _)| g.eq_ignore_ascii_case(&l.game))
        {
            Some((_, n)) => *n += 1,
            None => counts.push((l.game.clone(), 1)),
        }
    }
    counts.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    counts
}

/// Pastille de choix d'un jeu (couleur d'accent quand elle est sélectionnée).
fn game_chip(ui: &mut egui::Ui, name: &str, count: usize, selected: bool) -> egui::Response {
    // Nom en texte normal, nombre de chaînes à la couleur d'accent (blanc sur la pastille choisie).
    let mut text = egui::text::LayoutJob::default();
    let font = egui::FontId::proportional(13.0);
    let (name_col, count_col) = if selected {
        (Color32::WHITE, Color32::WHITE)
    } else {
        (t::TEXT(), t::ACCENT_GFX())
    };
    text.append(
        &format!("{name}  "),
        0.0,
        egui::TextFormat::simple(font.clone(), name_col),
    );
    text.append(
        &count.to_string(),
        0.0,
        egui::TextFormat::simple(font, count_col),
    );
    let b = egui::Button::new(text)
        .fill(if selected {
            t::ACCENT()
        } else {
            t::SURFACE_2()
        })
        .stroke(if selected {
            egui::Stroke::NONE
        } else {
            egui::Stroke::new(1.0, t::BORDER_HOVER())
        })
        .corner_radius(egui::CornerRadius::same(t::RADIUS_PILL))
        .min_size(Vec2::new(0.0, 30.0));
    ui.add(b)
}

impl VelkaApp {
    fn live_followed_infos(&mut self) -> Vec<LiveInfo> {
        // Une chaîne suivie sur Twitch y reste même si elle est aussi en favori (comme dans la barre latérale).
        let logins = self.live_followed_with(self.settings.home_limit(), true);
        let mut v = self.live_infos(logins);
        sort_infos(&mut v, self.settings.home_sort, self.home_game.as_deref());
        v
    }

    fn live_favorites(&mut self) -> Vec<LiveInfo> {
        let twitch = self.live_followed_with(self.settings.home_limit(), true);
        let logins: Vec<String> = self
            .settings
            .favorites
            .iter()
            .filter(|f| !twitch.contains(f))
            .cloned()
            .collect();
        let mut v = self.live_infos(logins);
        sort_infos(&mut v, self.settings.home_sort, self.home_game.as_deref());
        v
    }

    fn live_infos(&mut self, logins: Vec<String>) -> Vec<LiveInfo> {
        let show_thumbs = self.settings.show_thumbnails;
        let (avatars, thumbs) = (&mut self.avatars, &mut self.thumbs);
        logins
            .iter()
            .filter_map(|l| match self.status_cache.get(l).map(|(_, s)| s) {
                Some(LiveStatus::Live {
                    viewers,
                    game,
                    title,
                    ..
                }) => Some(LiveInfo {
                    login: l.clone(),
                    viewers: *viewers,
                    game: game.clone(),
                    title: title.clone(),
                    avatar: avatars.get(l),
                    thumb: if show_thumbs { thumbs.get(l) } else { None },
                }),
                _ => None,
            })
            .collect()
    }

    /// Bouton de tri par spectateurs (et « Actualiser » en mode manuel), à poser dans une disposition de droite à
    /// gauche : le tri est tout à droite, « Actualiser » à sa gauche. Un seul bouton de tri : chaque clic alterne entre
    /// décroissant (flèche vers le bas) et croissant (flèche vers le haut) ; l'infobulle décrit l'ordre actuel.
    fn home_sort_buttons(&mut self, ui: &mut egui::Ui, refresh_now: &mut bool) {
        let descending = self.settings.home_sort == 0;
        let tip = if descending {
            crate::i18n::tr("Affiche d'abord les lives qui ont le plus de spectateurs.")
        } else {
            crate::i18n::tr("Affiche d'abord les lives qui ont le moins de spectateurs.")
        };
        // L'icône se retourne en douceur (flèche vers le bas = décroissant, vers le haut = croissant) et le bouton passe
        // du neutre (décroissant, par défaut) à l'accent (croissant) ; sans animation (niveau « Réduit » ou
        // « Désactivé », voir `move_secs`), tout bascule d'un coup.
        let ctx = ui.ctx().clone();
        let turn = ctx.animate_bool_with_time_and_easing(
            egui::Id::unique("home_sort_arrow"),
            !descending,
            t::move_secs(&ctx, t::motion::SORT_TURN),
            egui::emath::easing::cubic_out,
        );
        let angle = turn * std::f32::consts::PI;
        let sort =
            t::icon_text_button_sort(ui, crate::i18n::tr("Spectateurs"), SORT_BTN_SCALE, angle);
        if t::show_tip_after(sort, tip, SORT_TIP_DELAY).clicked() {
            self.settings.home_sort = if descending { 1 } else { 0 };
        }
        if self.settings.refresh_mode == 2
            && t::icon_text_button_scaled(
                ui,
                t::BtnKind::Secondary,
                t::NavIcon::Refresh,
                crate::i18n::tr("Actualiser"),
                SORT_BTN_SCALE,
            )
            .clicked()
        {
            *refresh_now = true;
        }
    }

    /// Ligne du titre : « Accueil » et compteurs. Deux largeurs pour les compteurs : assez large, ils tiennent sur la
    /// même ligne ; sinon ils passent sur une seconde ligne. Le tri par spectateurs est sur le titre de la première
    /// section de cartes (`section_heading`).
    fn home_title_row(&mut self, ui: &mut egui::Ui, stats: &[(&'static str, String); 3]) {
        let wide = ui.available_width() >= HOME_WIDE_MIN_W;
        ui.horizontal(|ui| {
            t::heading(ui, crate::i18n::tr("Accueil"));
            if wide {
                ui.add_space(t::SP_L);
                stats_inline(ui, stats);
            }
        });
        if !wide {
            ui.add_space(t::SP_S);
            ui.horizontal_wrapped(|ui| stats_inline(ui, stats));
        }
        ui.add_space(t::SP_M);
    }

    /// Titre d'une section de cartes. `with_sort` : le bouton de tri (et « Actualiser ») est sur la même ligne, à droite,
    /// avec le même bord droit que les cartes du dessous.
    fn section_heading(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        with_sort: bool,
        refresh_now: &mut bool,
    ) {
        let label = RichText::new(title).size(15.0).strong().color(t::TEXT());
        if !with_sort {
            ui.label(label);
            return;
        }
        let grid_w = grid_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_max_width(grid_w);
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.home_sort_buttons(ui, refresh_now)
            });
        });
    }

    /// Barre des jeux en direct (on choisit un jeu, ses lives passent en tête) puis ligne « Récents » (reprendre une chaîne).
    fn home_chip_rows(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        games: &[(String, usize)],
        pick_game: &mut Option<Option<String>>,
        to_watch: &mut Option<String>,
        remove_hist: &mut Option<String>,
    ) {
        if !games.is_empty() || self.home_game.is_some() {
            ui.label(
                RichText::new(crate::i18n::tr("Jeux en direct"))
                    .size(13.0)
                    .color(t::MUTED()),
            );
            ui.add_space(t::SP_S);
            wheel_row(ui, ctx, "games_bar", |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let chosen = self.home_game.clone();
                    let absent = chosen
                        .as_ref()
                        .filter(|c| !games.iter().any(|(g, _)| g.eq_ignore_ascii_case(c)));
                    if let Some(c) = absent {
                        if game_chip(ui, c, 0, true).clicked() {
                            *pick_game = Some(None);
                        }
                    }
                    for (g, n) in games {
                        let selected = chosen.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(g));
                        if game_chip(ui, g, *n, selected).clicked() {
                            *pick_game = Some(if selected { None } else { Some(g.clone()) });
                        }
                    }
                });
            });
            let none_for_game = self
                .home_game
                .as_ref()
                .is_some_and(|c| !games.iter().any(|(g, _)| g.eq_ignore_ascii_case(c)));
            if none_for_game {
                ui.label(
                    RichText::new(crate::i18n::tr("Aucun live pour ce jeu en ce moment."))
                        .size(12.0)
                        .color(t::FAINT()),
                );
            }
            ui.add_space(t::SP_M);
        }
        // Reprendre : toutes les chaînes de l'historique (15 au plus), sur une ligne à défilement horizontal. C'est
        // le seul endroit de l'accueil qui les liste (la carte « Récents » du bas a été supprimée).
        if !self.history.items.is_empty() {
            let recent = &self.history.items[..];
            let cache = &self.status_cache;
            let is_live =
                |l: &str| matches!(cache.get(l).map(|(_, s)| s), Some(LiveStatus::Live { .. }));
            let label = RichText::new(crate::i18n::tr("Récents"))
                .size(13.0)
                .color(t::MUTED());
            // Le libellé est toujours au-dessus de la rangée, quelle que soit la taille de la fenêtre (comme « Jeux en direct »).
            ui.label(label);
            ui.add_space(t::SP_S);
            wheel_row(ui, ctx, "recent_bar", |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    recent_chips(ui, recent, &is_live, to_watch, remove_hist);
                });
            });
            ui.add_space(t::SP_M);
        }
    }

    pub(super) fn home_main(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_watch: Option<String> = None;
        let mut toggle_fav: Option<String> = None;
        let mut remove_hist: Option<String> = None;
        let live = self.live_favorites();
        let live_follow = self.live_followed_infos();
        let games = games_of(&[&live, &live_follow]);
        let mut pick_game: Option<Option<String>> = None;
        let mut refresh_now = false;
        let mut card_pick: Option<String> = None;
        // Compteurs (favoris + chaînes suivies sur Twitch) qui défilent doucement vers leur nouvelle valeur.
        let all_follow = self.live_followed_with(usize::MAX, true);
        let follow_viewers: u64 = all_follow
            .iter()
            .filter_map(|l| match self.status_cache.get(l).map(|(_, s)| s) {
                Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
                _ => None,
            })
            .sum();
        let total_target: u64 = live.iter().map(|l| l.viewers).sum::<u64>() + follow_viewers;
        let total_viewers = ctx
            .animate_value_with_time(
                egui::Id::unique("stat_viewers"),
                total_target as f32,
                t::anim_secs(ctx, 1.6),
            )
            .round() as u64;
        let live_count = ctx
            .animate_value_with_time(
                egui::Id::unique("stat_live"),
                (live.len() + all_follow.len()) as f32,
                t::anim_secs(ctx, 1.2),
            )
            .round() as usize;
        let typed = crate::login::normalize(&self.input);
        // Chargement en cours : on montre des squelettes au lieu d'une page vide.
        let follows_loading = self.settings.show_follows
            && matches!(self.follows_state, super::FollowsState::Loading);
        let favs_loading = !self.settings.favorites.is_empty()
            && self
                .settings
                .favorites
                .iter()
                .all(|l| !self.status_cache.contains_key(l));

        let stats = [
            (crate::i18n::tr("En direct"), live_count.to_string()),
            (
                crate::i18n::tr("Favoris"),
                self.settings.favorites.len().to_string(),
            ),
            (
                crate::i18n::tr("Spectateurs"),
                crate::login::format_viewers(total_viewers),
            ),
        ];
        self.home_title_row(ui, &stats);
        // Assez de hauteur : jeux et récents restent fixes sous le titre. Sinon ils défilent avec les listes, pour ne pas
        // réduire la zone des cartes à une ou deux lignes.
        let sticky = ui.available_height() >= STICKY_MIN_H;
        if sticky {
            self.home_chip_rows(
                ui,
                ctx,
                &games,
                &mut pick_game,
                &mut to_watch,
                &mut remove_hist,
            );
        }

        // Les listes défilent ; le titre reste fixe, et les jeux et récents aussi quand la fenêtre est assez haute.
        let scroll_down = std::mem::take(&mut self.home_scroll_down);
        let scroll_out = t::auto_hide_scroll(
            ui,
            "home_scroll",
            false,
            egui::ScrollArea::vertical().auto_shrink([false, false]),
            |ui| {
                t::follow_keyboard_focus(ui);
                if scroll_down {
                    ui.scroll_with_delta(Vec2::new(0.0, -HOME_SCROLL_STEP));
                }
                if !sticky {
                    self.home_chip_rows(
                        ui,
                        ctx,
                        &games,
                        &mut pick_game,
                        &mut to_watch,
                        &mut remove_hist,
                    );
                }
                // Résultat de la recherche.
                if !self.input.trim().is_empty() {
                    t::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        t::card_title(ui, crate::i18n::tr("Résultat"));
                        ui.add_space(t::SP_S);
                        match &typed {
                            Some(l) => {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(l).font(t::name_font(18.0)).color(t::TEXT()),
                                    );
                                    if matches!(
                                        self.status_cache.get(l).map(|(_, s)| s),
                                        Some(LiveStatus::Live { .. })
                                    ) {
                                        t::live_pill(ui);
                                    }
                                });
                                match self.status_cache.get(l).map(|(_, s)| s) {
                                    Some(st) => {
                                        let (text, color) = status_text(st);
                                        ui.label(RichText::new(text).color(color));
                                    }
                                    None => {
                                        ui.label(
                                            RichText::new(crate::i18n::tr("Vérification…"))
                                                .color(t::MUTED()),
                                        );
                                    }
                                }
                                ui.add_space(t::SP_M);
                                ui.horizontal(|ui| {
                                    if t::icon_text_button(
                                        ui,
                                        t::BtnKind::Primary,
                                        t::NavIcon::Play,
                                        crate::i18n::tr("Regarder"),
                                    )
                                    .clicked()
                                    {
                                        to_watch = Some(l.clone());
                                    }
                                    let fav = self.settings.favorites.contains(l);
                                    if t::icon_text_button(
                                        ui,
                                        t::BtnKind::Secondary,
                                        if fav {
                                            t::NavIcon::StarFilled
                                        } else {
                                            t::NavIcon::StarPlus
                                        },
                                        if fav {
                                            crate::i18n::tr("Favori")
                                        } else {
                                            crate::i18n::tr("Ajouter aux favoris")
                                        },
                                    )
                                    .clicked()
                                    {
                                        toggle_fav = Some(l.clone());
                                    }
                                });
                            }
                            None => {
                                ui.label(
                                    RichText::new(crate::i18n::tr("Nom de chaîne invalide."))
                                        .color(t::NEGATIVE()),
                                );
                            }
                        }
                    });
                    ui.add_space(t::SP_L);
                }

                // Chaînes suivies sur le compte Twitch connecté (hors favoris) : en premier.
                if !live_follow.is_empty() || follows_loading {
                    self.section_heading(
                        ui,
                        crate::i18n::tr("En direct parmi vos chaînes suivies sur Twitch"),
                        true,
                        &mut refresh_now,
                    );
                    ui.add_space(t::SP_S);
                    if live_follow.is_empty() {
                        skeleton_grid(ui);
                    } else if let Some(l) = card_grid(ui, &live_follow, 0) {
                        card_pick = Some(l);
                    }
                    ui.add_space(t::SP_L);
                }

                // Favoris en direct.
                // Le tri est sur le titre de la première section visible : celui-ci s'il n'y a pas de chaînes suivies.
                let sort_here = live_follow.is_empty() && !follows_loading;
                self.section_heading(
                    ui,
                    crate::i18n::tr("En direct parmi vos favoris"),
                    sort_here,
                    &mut refresh_now,
                );
                ui.add_space(t::SP_S);
                if live.is_empty() && favs_loading {
                    skeleton_grid(ui);
                } else if live.is_empty() {
                    // État vide : une ligne discrète avec une étoile, plutôt qu'une grande carte vide.
                    egui::Frame::new()
                        .fill(t::SURFACE_1())
                        .stroke(t::hairline())
                        .corner_radius(egui::CornerRadius::same(t::RADIUS_INNER))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                                t::paint_nav_icon(
                                    ui.painter(),
                                    r.center(),
                                    t::NavIcon::Star,
                                    t::FAINT(),
                                );
                                ui.label(
                                    RichText::new(crate::i18n::tr(
                                        "Aucun favori en direct pour le moment.",
                                    ))
                                    .color(t::MUTED()),
                                );
                            });
                        });
                } else if let Some(l) = card_grid(ui, &live, 1) {
                    card_pick = Some(l);
                }
                ui.add_space(t::SP_L);

                // Le mini-lecteur flotte au-dessus du contenu : une marge en bas permet de faire défiler les dernières
                // cartes et pastilles au-dessus de lui au lieu de les laisser cachées.
                if self.mini_visible() {
                    if let Some(r) = self.mini_last {
                        ui.add_space(r.height() + 20.0);
                    }
                }
            },
        );
        // Cartes cachées sous le bord bas : même indicateur que la liste de la barre latérale (`theme::scroll_down_hint`).
        if t::scroll_down_hint(ui, &scroll_out) {
            self.home_scroll_down = true;
        }
        // Liste défilée : un fondu sous la barre des jeux évite que les cartes y soient coupées net.
        if scroll_out.state.clamped_offset().y > 1.0 {
            let r = scroll_out.inner_rect;
            let strips = 8;
            for i in 0..strips {
                let k = 1.0 - i as f32 / strips as f32;
                let top = r.top() + i as f32 * 2.0;
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(r.left(), top),
                        egui::vec2(r.width(), 2.0),
                    ),
                    egui::CornerRadius::ZERO,
                    t::BG().gamma_multiply(k * k),
                );
            }
        }

        if let Some(g) = pick_game {
            if self.settings.remember_game {
                self.settings.home_game = g.clone().unwrap_or_default();
            }
            self.home_game = g;
        }
        if refresh_now {
            // Actualisation manuelle : le prochain passage redemande les statuts et les chaînes suivies.
            self.last_status_req = None;
            self.follows_at = None;
        }
        if let Some(l) = card_pick {
            self.start_watch(ctx, l);
            if self.settings.card_click == 1 {
                self.pending_pip = true;
            }
        }
        if let Some(l) = toggle_fav {
            toggle_favorite(&mut self.settings.favorites, &l);
            if self.settings.favorites.contains(&l) {
                self.refresh_after_follow(ctx, &l);
            }
        }
        if let Some(l) = remove_hist {
            self.history.remove(&l);
            if let Err(e) = self.history.save() {
                crate::logging::log(&format!("Historique non enregistré : {e}"));
            }
        }
        if let Some(l) = to_watch {
            self.start_watch(ctx, l);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(login: &str, viewers: u64, game: &str) -> LiveInfo {
        LiveInfo {
            login: login.into(),
            viewers,
            game: game.into(),
            title: String::new(),
            avatar: None,
            thumb: None,
        }
    }

    #[test]
    fn tri_des_lives() {
        let mut v = vec![
            info("a", 10, "Valorant"),
            info("b", 500, "GTA"),
            info("c", 50, "GTA"),
            info("d", 5, "Apex"),
        ];
        let names = |v: &[LiveInfo]| {
            v.iter()
                .map(|l| l.login.clone())
                .collect::<Vec<_>>()
                .join("")
        };
        sort_infos(&mut v, 0, None);
        assert_eq!(names(&v), "bcad");
        sort_infos(&mut v, 1, None);
        assert_eq!(names(&v), "dacb");
        // Le jeu choisi passe devant, puis l'ordre des spectateurs s'applique dans chaque groupe.
        sort_infos(&mut v, 0, Some("valorant"));
        assert_eq!(names(&v), "abcd");
        sort_infos(&mut v, 1, Some("GTA"));
        assert_eq!(names(&v), "cbda");
    }

    #[test]
    fn jeux_en_direct() {
        let a = vec![
            info("a", 1, "GTA"),
            info("b", 2, "Valorant"),
            info("x", 3, ""),
        ];
        let b = vec![info("c", 1, "gta"), info("d", 1, "Apex")];
        let g = games_of(&[&a, &b]);
        assert_eq!(
            g,
            vec![
                ("GTA".to_string(), 2),
                ("Apex".to_string(), 1),
                ("Valorant".to_string(), 1)
            ]
        );
    }
}
