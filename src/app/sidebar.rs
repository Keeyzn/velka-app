//! Barre latérale : logo, recherche, navigation, chaînes suivies.
use super::{VelkaApp, View};
use crate::twitch::LiveStatus;
use crate::ui::theme as t;
use eframe::egui::{self, Align2, FontId, Pos2, RichText, Sense, Vec2};

/// Lignes « squelette » de la liste des chaînes (photo, deux traits, reflet qui descend de ligne en ligne).
/// `compact` : barre réduite, seulement des pastilles rondes.
/// Affiche le titre « Plateforme » de la barre latérale (masqué pour l'instant).
const SHOW_PLATFORM_TITLE: bool = false;

/// Hauteur d'une ligne de la barre déployée (la barre réduite garde 46 px).
const ROW_H: f32 = 40.0;

/// Numéro de ligne des chaînes d'un groupe en train de se replier : elles ne se parcourent plus aux flèches.
const NO_NAV: usize = usize::MAX;

fn skeleton_rows(ui: &mut egui::Ui, ctx: &egui::Context, rows: usize, compact: bool) {
    let time = ctx.input(|i| i.time);
    for i in 0..rows {
        let row_h = if compact { 46.0 } else { ROW_H };
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
        let p = ui.painter();
        if compact {
            p.circle_filled(rect.center(), 15.0, t::SURFACE_2());
        } else {
            p.circle_filled(
                Pos2::new(rect.left() + 20.0, rect.center().y),
                14.0,
                t::SURFACE_2(),
            );
            let w = rect.width() - 54.0;
            let r = |y: f32, f: f32, h: f32| {
                egui::Rect::from_min_size(
                    Pos2::new(rect.left() + 42.0, rect.top() + y),
                    Vec2::new(w * f, h),
                )
            };
            p.rect_filled(
                r(7.0, 0.6, 11.0),
                egui::CornerRadius::same(5),
                t::SURFACE_2(),
            );
            p.rect_filled(
                r(23.0, 0.4, 9.0),
                egui::CornerRadius::same(4),
                t::SURFACE_2(),
            );
        }
        if t::moves(ctx) {
            let phase = ((time * 0.9 - i as f64 * 0.08) % 1.0) as f32;
            let a = (1.0 - (phase * 2.0 - 1.0).abs()).max(0.0) * 14.0;
            p.rect_filled(
                rect,
                egui::CornerRadius::same(t::RADIUS_INNER),
                egui::Color32::from_white_alpha(a as u8),
            );
        }
    }
    ctx.request_repaint();
}

impl VelkaApp {
    pub(super) fn sidebar_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_watch: Option<String> = None;
        let mut remove_fav: Option<String> = None;
        let mut goto: Option<View> = None;
        let mut remove_hist: Option<String> = None;
        let mut pick_input: Option<String> = None;

        // Logo + bouton pour réduire la barre.
        ui.horizontal(|ui| {
            crate::ui::logo::show(ui, 26.0, t::ACCENT_GFX(), Sense::hover());
            ui.label(RichText::new("Velka").size(15.0).strong().color(t::TEXT()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if t::nav_icon_button(
                    ui,
                    t::NavIcon::Panel,
                    Vec2::splat(30.0),
                    false,
                    crate::i18n::tr("Réduire la barre latérale"),
                )
                .clicked()
                {
                    self.settings.sidebar_collapsed = true;
                    self.side_force = false;
                }
            });
        });
        ui.add_space(t::SP_M);

        // Recherche (Ctrl+K pour la focaliser).
        let search_id = egui::Id::unique("channel_search");
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K))
            | std::mem::take(&mut self.focus_search)
        {
            ctx.memory_mut(|m| m.request_focus(search_id));
        }
        let (edit, field) = t::search_input(
            ui,
            &mut self.input,
            crate::i18n::tr("Rechercher"),
            search_id,
            "Ctrl K",
        );

        // Suggestions : favoris et récents qui correspondent, chaînes en direct d'abord.
        let focused = edit.has_focus();
        let typed_login = crate::login::normalize(&self.input);
        let needle = self
            .input
            .trim()
            .trim_start_matches(['@', '#'])
            .to_lowercase();
        let mut rows: Vec<String> = Vec::new();
        for l in self
            .settings
            .favorites
            .iter()
            .chain(self.history.items.iter())
        {
            // Une chaîne suivie masquée n'apparaît plus ici tant qu'on ne tape pas son nom.
            let hidden = needle.is_empty() && self.settings.search_hidden.contains(l);
            if !rows.contains(l) && !hidden && (needle.is_empty() || l.contains(&needle)) {
                rows.push(l.clone());
            }
        }
        let suggestions = &self.suggestions;
        let viewers_of = |l: &String| match self.status_cache.get(l).map(|(_, s)| s) {
            Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
            _ => suggestions
                .iter()
                .find(|s| s.login == *l)
                .and_then(|s| s.viewers),
        };
        rows.sort_by_key(|l| std::cmp::Reverse(viewers_of(l).unwrap_or(0)));
        rows.truncate(6);
        // Autocomplétion Twitch : les chaînes proposées (les plus connues d'abord) après les favoris.
        if self.suggest_for == self.suggest_want {
            for s in suggestions.iter().take(6) {
                if !rows.contains(&s.login) {
                    rows.push(s.login.clone());
                }
            }
        }
        if let Some(t) = &typed_login {
            if !rows.contains(t) {
                rows.push(t.clone());
            }
        }
        if focused && !rows.is_empty() {
            let (down, up) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                )
            });
            let last = rows.len() - 1;
            if down {
                self.palette_sel = Some(self.palette_sel.map_or(0, |s| (s + 1).min(last)));
            }
            if up {
                self.palette_sel = Some(self.palette_sel.map_or(last, |s| s.saturating_sub(1)));
            }
        }
        if focused && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            ctx.memory_mut(|m| m.surrender_focus(search_id));
        }
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            to_watch = match self.palette_sel.and_then(|i| rows.get(i)) {
                Some(l) => Some(l.clone()),
                None => typed_login.clone(),
            };
            pick_input = to_watch.clone();
        }
        if !focused && !self.palette_hover {
            self.palette_sel = None;
        }
        if (focused || self.palette_hover) && !rows.is_empty() {
            // Exactement sous le champ, même largeur des deux côtés (avant : décalée et plus large).
            let pos = Pos2::new(field.left(), field.bottom() + 6.0);
            let area = egui::Area::new(egui::Id::unique("search_palette"))
                .fixed_pos(pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(t::SURFACE_1())
                        .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            // Largeur intérieure = largeur du champ − marges (2 × 6) − bordure (2 × 1).
                            ui.set_width(field.width() - 14.0);
                            // Rien de tapé : la liste est l'historique des chaînes déjà regardées.
                            let heading = if needle.is_empty() {
                                crate::i18n::tr("Récents")
                            } else {
                                crate::i18n::tr("Résultats")
                            };
                            ui.label(RichText::new(heading).size(11.0).color(t::FAINT()));
                            for (i, l) in rows.iter().enumerate() {
                                let live = viewers_of(l);
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), t::BTN_H),
                                    Sense::click(),
                                );
                                if self.palette_sel == Some(i) || resp.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(8),
                                        t::SURFACE_3(),
                                    );
                                }
                                t::paint_pressed(ui, &resp, rect, 8);
                                t::focus_ring(ui, &resp, rect, 8);
                                resp.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::Role::Button, true, l.as_str())
                                });
                                // Photo seulement pour une chaîne qui existe (pas pour une saisie en cours).
                                let exists = suggestions.iter().any(|s| s.login == *l)
                                    || matches!(
                                        self.status_cache.get(l).map(|(_, s)| s),
                                        Some(LiveStatus::Live { .. } | LiveStatus::Offline)
                                    );
                                let photo = if exists { self.avatars.get(l) } else { None };
                                t::avatar(
                                    ui,
                                    Pos2::new(rect.left() + 16.0, rect.center().y),
                                    11.0,
                                    l,
                                    photo,
                                );
                                t::paint_name_line(
                                    ui.painter(),
                                    Pos2::new(rect.left() + 34.0, rect.top() + 8.0),
                                    l,
                                    13.0,
                                    t::TEXT(),
                                    rect.width() - 34.0 - 78.0,
                                );
                                let in_history = self.history.items.contains(l)
                                    || self.settings.favorites.contains(l);
                                let right = if in_history {
                                    rect.right() - 28.0
                                } else {
                                    rect.right() - 8.0
                                };
                                if let Some(v) = live {
                                    ui.painter().text(
                                        Pos2::new(right, rect.center().y),
                                        Align2::RIGHT_CENTER,
                                        crate::login::format_viewers(v),
                                        FontId::proportional(12.0),
                                        t::POSITIVE(),
                                    );
                                }
                                // Petite croix : retire cette chaîne de l'historique.
                                let mut on_cross = false;
                                if in_history {
                                    let cr = egui::Rect::from_center_size(
                                        Pos2::new(rect.right() - 14.0, rect.center().y),
                                        Vec2::splat(22.0),
                                    );
                                    let cresp = ui.interact(
                                        cr,
                                        egui::Id::unique(("palette_del", l)),
                                        Sense::click(),
                                    );
                                    on_cross = cresp.hovered() || cresp.has_focus();
                                    cresp.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::Button,
                                            true,
                                            crate::i18n::tr("Retirer de l'historique"),
                                        )
                                    });
                                    if cresp.hovered() {
                                        ui.painter().circle_filled(
                                            cr.center(),
                                            10.0,
                                            t::SURFACE_2(),
                                        );
                                    }
                                    let col = if cresp.hovered() {
                                        t::NEGATIVE()
                                    } else {
                                        t::FAINT()
                                    };
                                    let st = egui::Stroke::new(1.5, col);
                                    let c = cr.center();
                                    ui.painter().line_segment(
                                        [c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)],
                                        st,
                                    );
                                    ui.painter().line_segment(
                                        [c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)],
                                        st,
                                    );
                                    let cresp = t::show_tip(
                                        cresp,
                                        crate::i18n::tr("Retirer de l'historique"),
                                    );
                                    if cresp.clicked() {
                                        remove_hist = Some(l.clone());
                                    }
                                }
                                if resp.clicked() && !on_cross {
                                    to_watch = Some(l.clone());
                                    pick_input = Some(l.clone());
                                }
                            }
                        });
                });
            self.palette_hover = ctx
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| area.response.rect.contains(p));
        } else {
            self.palette_hover = false;
        }
        ui.add_space(t::SP_L);

        // Titre « Plateforme » masqué à l'écran, code conservé : passer `SHOW_PLATFORM_TITLE` à `true`.
        if SHOW_PLATFORM_TITLE {
            ui.label(
                RichText::new(crate::i18n::tr("Plateforme"))
                    .size(13.0)
                    .color(t::MUTED()),
            );
        }
        if t::nav_item(
            ui,
            t::NavIcon::Home,
            crate::i18n::tr("Accueil"),
            self.view == View::Home,
        )
        .clicked()
        {
            goto = Some(View::Home);
        }
        if t::nav_item(
            ui,
            t::NavIcon::Grid,
            crate::i18n::tr("Multistream"),
            self.view == View::Multi,
        )
        .clicked()
        {
            goto = Some(View::Multi);
        }
        // Réglages : épinglé en bas de la colonne.
        egui::Panel::bottom("sidebar_bottom")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 20,
                bottom: 0,
            }))
            .show_separator_line(true)
            .show(ui, |ui| {
                if t::nav_item(
                    ui,
                    t::NavIcon::Settings,
                    crate::i18n::tr("Réglages"),
                    self.view == View::Settings,
                )
                .clicked()
                {
                    goto = Some(View::Settings);
                }
            });
        ui.add_space(t::SP_L);

        if self.settings.show_follows
            && self.connected()
            && self.follows_state == super::FollowsState::NeedsReconnect
        {
            ui.label(
                RichText::new(crate::i18n::tr(
                    "Reconnectez-vous dans Réglages pour afficher vos chaînes suivies sur Twitch.",
                ))
                .size(12.0)
                .color(t::FAINT()),
            );
        }
        // D'abord les chaînes suivies sur Twitch en direct, puis les favoris : en direct (les plus regardés
        // d'abord), ensuite hors ligne.
        let viewers_of = |l: &String| match self.status_cache.get(l).map(|(_, s)| s) {
            Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
            _ => None,
        };
        let twitch_live = self.live_followed_with(self.settings.sidebar_limit(), true);
        // Une chaîne suivie sur Twitch (en direct) passe devant, même si elle est aussi en favori.
        let mut favs: Vec<String> = self
            .settings
            .favorites
            .iter()
            .filter(|f| !twitch_live.contains(f))
            .cloned()
            .collect();
        favs.sort_by_key(|l| std::cmp::Reverse(viewers_of(l).map(|v| v + 1).unwrap_or(0)));
        let items: Vec<(String, bool)> = twitch_live
            .into_iter()
            .map(|l| (l, false))
            .chain(favs.into_iter().map(|l| (l, true)))
            .collect();
        let mut add_fav: Option<String> = None;
        let scroll_down = std::mem::take(&mut self.sidebar_scroll_down);
        // Chargement en cours : lignes « squelette » à la place de la liste.
        let loading = (self.settings.show_follows
            && self.follows_state == super::FollowsState::Loading
            && items.is_empty())
            || (!self.settings.favorites.is_empty()
                && self
                    .settings
                    .favorites
                    .iter()
                    .all(|l| !self.status_cache.contains_key(l)));
        // La barre de défilement ne prend pas de place : sinon, dès que la liste dépasse (un groupe qui se déplie), la
        // gouttière de 10 px apparaît et décale vers la gauche la flèche des titres et les nombres de spectateurs.
        let saved_alloc = ui.spacing().scroll.floating_allocated_width;
        ui.spacing_mut().scroll.floating_allocated_width = 0.0;
        let scroll_out = t::auto_hide_scroll(
            ui,
            "sidebar_scroll",
            false,
            egui::ScrollArea::vertical().auto_shrink([false, false]),
            |ui| {
                t::follow_keyboard_focus(ui);
                if scroll_down {
                    ui.scroll_with_delta(Vec2::new(0.0, -160.0));
                }
                if loading {
                    // Autant de lignes que la zone visible peut en contenir.
                    let rows = (((ui.clip_rect().bottom() - ui.cursor().top()).max(0.0) / ROW_H)
                        .ceil() as usize)
                        .clamp(1, 30);
                    skeleton_rows(ui, ctx, rows, false);
                    return;
                }
                // Chaînes suivies sur Twitch pas encore arrivées : des lignes squelettes avant les favoris.
                let follows_pending = self.settings.show_follows
                    && self.connected()
                    && matches!(
                        self.follows_state,
                        super::FollowsState::Idle | super::FollowsState::Loading
                    )
                    && items.iter().all(|(_, f)| *f);
                let mut twitch_header = false;
                if follows_pending {
                    twitch_header = true;
                    ui.label(
                        RichText::new(crate::i18n::tr("Suivies sur Twitch"))
                            .size(13.0)
                            .color(t::MUTED()),
                    );
                    skeleton_rows(ui, ctx, 4, false);
                }
                // Colonne des spectateurs : largeur commune à toutes les lignes (point et nom du jeu alignés).
                let col_w = items
                    .iter()
                    .filter_map(|(l, _)| match self.status_cache.get(l).map(|(_, s)| s) {
                        Some(LiveStatus::Live { viewers, .. }) => Some(
                            ui.painter()
                                .layout_no_wrap(
                                    crate::login::format_viewers(*viewers),
                                    FontId::proportional(12.0),
                                    t::TEXT(),
                                )
                                .size()
                                .x,
                        ),
                        _ => None,
                    })
                    .fold(30.0_f32, f32::max);
                // Deux groupes repliables : chaînes suivies sur Twitch, puis favoris. Seules les lignes d'un groupe déplié
                // se parcourent aux flèches (`nav_len`) ; celles d'un groupe en train de se replier gardent `NO_NAV`.
                let n_twitch = items.iter().filter(|(_, f)| !*f).count();
                let nav_len = items
                    .iter()
                    .filter(|(_, f)| !self.side_collapsed[usize::from(*f)])
                    .count();
                let mut next_nav = 0usize;
                for (g, group) in [&items[..n_twitch], &items[n_twitch..]]
                    .into_iter()
                    .enumerate()
                {
                    if group.is_empty() {
                        continue;
                    }
                    if g == 1 && n_twitch > 0 {
                        // Trait fin entre les deux groupes.
                        ui.add_space(t::SP_S);
                        let (line, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 1.0),
                            Sense::hover(),
                        );
                        ui.painter()
                            .hline(line.x_range(), line.center().y, t::hairline());
                        ui.add_space(t::SP_S);
                    }
                    twitch_header |= g == 0;
                    let title = if g == 0 {
                        crate::i18n::tr("Suivies sur Twitch")
                    } else {
                        crate::i18n::tr("Favoris")
                    };
                    // Titre cliquable : replie ou déplie le groupe. La flèche et la liste partagent la même progression
                    // (`animated_section`), donc la même durée et la même courbe.
                    let (hrect, hresp) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
                    if hresp.clicked() {
                        self.side_collapsed[g] = !self.side_collapsed[g];
                    }
                    let open = !self.side_collapsed[g];
                    let hk = t::hover_k(ui, hresp.id, hresp.hovered());
                    let hcolor = t::MUTED().lerp_to_gamma(t::TEXT(), hk);
                    hresp
                        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
                    let section_t = t::animated_section(
                        ui,
                        if g == 0 { "side_twitch" } else { "side_favs" },
                        open,
                        |ui| {
                            let base_opacity = ui.opacity();
                            for (row_i, (login, is_fav)) in group.iter().enumerate() {
                                let idx = if open {
                                    next_nav += 1;
                                    next_nav - 1
                                } else {
                                    NO_NAV
                                };
                                // Apparition en fondu, en cascade de haut en bas.
                                let now = ctx.input(|i| i.time);
                                let key = egui::Id::unique(("side_first_seen", login));
                                let first = ctx.data_mut(|d| {
                                    *d.get_temp_mut_or_insert_with::<f64>(key, || now)
                                });
                                ui.set_opacity(
                                    base_opacity
                                        * t::motion::stagger_in(
                                            ctx,
                                            first,
                                            if g == 0 { row_i } else { n_twitch + row_i },
                                        ),
                                );
                                let live = match self.status_cache.get(login).map(|(_, s)| s) {
                                    Some(LiveStatus::Live { viewers, game, .. }) => {
                                        Some((*viewers, game.clone()))
                                    }
                                    _ => None,
                                };
                                // Une seule ligne de la liste est un arrêt de Tab (sinon il faut une vingtaine de Tab pour atteindre le
                                // contenu) ; les flèches haut/bas, Début et Fin passent d'une ligne à l'autre.
                                let rove = self.side_rove.min(nav_len.saturating_sub(1));
                                let sense = if idx == rove {
                                    Sense::click()
                                } else {
                                    Sense::CLICK
                                };
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), ROW_H),
                                    sense,
                                );
                                if self.side_focus_to == Some(idx) {
                                    self.side_focus_to = None;
                                    resp.request_focus();
                                    ui.scroll_to_rect(rect, None);
                                }
                                if resp.has_focus() && idx != NO_NAV {
                                    self.side_rove = idx;
                                    ui.memory_mut(|m| {
                                        m.set_focus_lock_filter(
                                            resp.id,
                                            egui::EventFilter {
                                                vertical_arrows: true,
                                                ..Default::default()
                                            },
                                        )
                                    });
                                    let (down, up, home, end) = ctx.input_mut(|i| {
                                        let none = egui::Modifiers::NONE;
                                        (
                                            i.consume_key(none, egui::Key::ArrowDown),
                                            i.consume_key(none, egui::Key::ArrowUp),
                                            i.consume_key(none, egui::Key::Home),
                                            i.consume_key(none, egui::Key::End),
                                        )
                                    });
                                    let last = nav_len.saturating_sub(1);
                                    let target = if down {
                                        Some((idx + 1).min(last))
                                    } else if up {
                                        Some(idx.saturating_sub(1))
                                    } else if home {
                                        Some(0)
                                    } else if end {
                                        Some(last)
                                    } else {
                                        None
                                    };
                                    if let Some(to) = target.filter(|&to| to != idx) {
                                        self.side_rove = to;
                                        self.side_focus_to = Some(to);
                                        ctx.request_repaint();
                                    }
                                }
                                if resp.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(t::RADIUS_INNER),
                                        t::SURFACE_2(),
                                    );
                                }
                                t::paint_pressed(ui, &resp, rect, t::RADIUS_INNER);
                                t::focus_ring(ui, &resp, rect, t::RADIUS_INNER);
                                resp.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::Button,
                                        true,
                                        login.as_str(),
                                    )
                                });
                                let photo = self.avatars.get(login);
                                t::avatar(
                                    ui,
                                    Pos2::new(rect.left() + 20.0, rect.center().y),
                                    14.0,
                                    login,
                                    photo,
                                );
                                let count = live.as_ref().map(|(v, _)| {
                                    ui.painter().layout_no_wrap(
                                        crate::login::format_viewers(*v),
                                        FontId::proportional(12.0),
                                        t::TEXT(),
                                    )
                                });
                                let text_w = rect.width()
                                    - 44.0
                                    - if live.is_some() { col_w + 30.0 } else { 0.0 };
                                t::paint_name_line(
                                    ui.painter(),
                                    Pos2::new(rect.left() + 42.0, rect.top() + 5.0),
                                    login,
                                    13.5,
                                    t::TEXT(),
                                    text_w,
                                );
                                let sub = match &live {
                                    Some((_, g)) if !g.is_empty() => g.clone(),
                                    Some(_) => crate::i18n::tr("En direct").to_string(),
                                    None => crate::i18n::tr("Hors ligne").to_string(),
                                };
                                // Le nom du jeu est en couleur d'accent ; « Hors ligne » et « En direct » (sans jeu) restent en gris.
                                let sub_color = if matches!(&live, Some((_, g)) if !g.is_empty()) {
                                    t::ACCENT_TEXT()
                                } else {
                                    t::MUTED()
                                };
                                t::paint_one_line(
                                    ui.painter(),
                                    Pos2::new(rect.left() + 42.0, rect.top() + 21.5),
                                    &sub,
                                    12.0,
                                    sub_color,
                                    text_w,
                                );
                                if let Some(galley) = count {
                                    let p = ui.painter();
                                    // Point vert sur un axe fixe (même position pour toutes les chaînes) ; le nombre part juste après.
                                    let x = rect.right() - 10.0 - col_w;
                                    p.circle_filled(
                                        Pos2::new(x - 9.0, rect.center().y),
                                        3.5,
                                        t::POSITIVE(),
                                    );
                                    p.galley(
                                        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
                                        galley,
                                        t::TEXT(),
                                    );
                                }
                                if resp.clicked() {
                                    to_watch = Some(login.clone());
                                }
                                resp.context_menu(|ui| {
                                    if *is_fav {
                                        if ui
                                            .button(crate::i18n::tr("Retirer des favoris"))
                                            .clicked()
                                        {
                                            remove_fav = Some(login.clone());
                                            ui.close();
                                        }
                                    } else if ui
                                        .button(crate::i18n::tr("Ajouter aux favoris"))
                                        .clicked()
                                    {
                                        add_fav = Some(login.clone());
                                        ui.close();
                                    }
                                });
                            }
                        },
                    );
                    // Titre et flèche dessinés après la liste : rien de ce que les lignes peignent (survol, anneau de focus)
                    // ne peut passer par-dessus.
                    ui.painter().text(
                        Pos2::new(hrect.left(), hrect.center().y),
                        Align2::LEFT_CENTER,
                        title,
                        FontId::proportional(13.0),
                        hcolor,
                    );
                    t::focus_ring(ui, &hresp, hrect, 8);
                    // Flèche du titre : à droite quand le groupe est replié, vers le bas quand il est déplié.
                    let c = Pos2::new(hrect.right() - 14.0, hrect.center().y);
                    let (sin, cos) = (section_t * std::f32::consts::FRAC_PI_2).sin_cos();
                    let pts: Vec<Pos2> = [
                        Vec2::new(-3.2, -5.6),
                        Vec2::new(4.5, 0.0),
                        Vec2::new(-3.2, 5.6),
                    ]
                    .iter()
                    .map(|v| c + Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos))
                    .collect();
                    ui.painter()
                        .add(egui::Shape::convex_polygon(pts, hcolor, egui::Stroke::NONE));
                }
                ui.set_opacity(1.0);
                if items.iter().all(|(_, is_fav)| !*is_fav) {
                    if twitch_header {
                        ui.add_space(t::SP_M);
                    }
                    ui.label(
                        RichText::new(crate::i18n::tr("Favoris"))
                            .size(13.0)
                            .color(t::MUTED()),
                    );
                    ui.label(RichText::new(crate::i18n::tr("Aucun favori pour le moment. Cherchez une chaîne puis choisissez « Ajouter aux favoris ».")).size(12.0).color(t::FAINT()));
                }
            },
        );

        ui.spacing_mut().scroll.floating_allocated_width = saved_alloc;

        // Chaînes cachées sous le bord : petit bouton en forme de flèche vers le bas.
        if t::scroll_down_hint(ui, &scroll_out) {
            self.sidebar_scroll_down = true;
        }

        if let Some(l) = pick_input {
            // Le champ montre le nom complet de la chaîne choisie, pas le début tapé.
            self.settings.search_hidden.retain(|h| *h != l);
            self.input = l;
        }
        if let Some(l) = remove_hist {
            if self.settings.favorites.contains(&l) && !self.settings.search_hidden.contains(&l) {
                self.settings.search_hidden.push(l.clone());
            }
            self.history.remove(&l);
            if let Err(e) = self.history.save() {
                crate::logging::log(&format!("Historique non enregistré : {e}"));
            }
        }
        if let Some(l) = add_fav {
            if !self.settings.favorites.contains(&l) {
                self.settings.favorites.push(l.clone());
                self.refresh_after_follow(ctx, &l);
            }
        }
        if let Some(l) = remove_fav {
            self.settings.favorites.retain(|f| *f != l);
        }
        if let Some(v) = goto {
            self.view = v;
            self.prev_view = View::Home;
        }
        if let Some(l) = to_watch {
            self.start_watch(ctx, l);
        }
    }

    /// Barre latérale réduite : seulement des icônes et les photos des chaînes.
    pub(super) fn sidebar_collapsed_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_watch: Option<String> = None;
        let mut goto: Option<View> = None;
        let w = ui.available_width();
        let btn = Vec2::new(w, t::ROW_H);

        // Logo (clic = agrandir la barre) et bouton pour l'agrandir.
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
        let tile = egui::Rect::from_center_size(r.center(), Vec2::splat(26.0));
        crate::ui::logo::paint(ui, tile, t::ACCENT_GFX());
        t::focus_ring(ui, &resp, tile, 8);
        let mut expand = resp.clicked();
        expand |= t::nav_icon_button(
            ui,
            t::NavIcon::Panel,
            btn,
            false,
            crate::i18n::tr("Agrandir la barre latérale"),
        )
        .clicked();

        // Recherche : ouvre la barre et place le curseur dans le champ (Ctrl+K fait pareil).
        let mut search = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K));
        search |= t::nav_icon_button(
            ui,
            t::NavIcon::Search,
            btn,
            false,
            crate::i18n::tr("Rechercher"),
        )
        .clicked();
        if expand || search {
            self.settings.sidebar_collapsed = false;
            // Fenêtre trop étroite (repli automatique) : l'ouvrir à la main la garde ouverte.
            self.side_force |= self.side_auto;
            // Le champ n'existe pas encore (il apparaît pendant l'agrandissement) : la demande est gardée jusque-là.
            self.focus_search |= search;
        }
        // Même écart (8 px) entre toutes les icônes : logo, menu, recherche, accueil, puis les photos des chaînes.
        if t::nav_icon_button(
            ui,
            t::NavIcon::Home,
            btn,
            self.view == View::Home,
            crate::i18n::tr("Accueil"),
        )
        .clicked()
        {
            goto = Some(View::Home);
        }
        if t::nav_icon_button(
            ui,
            t::NavIcon::Grid,
            btn,
            self.view == View::Multi,
            crate::i18n::tr("Multistream"),
        )
        .clicked()
        {
            goto = Some(View::Multi);
        }

        // Réglages en bas, comme dans la barre complète ; la barre au-dessus est celle d'origine (pleine largeur, neutre).
        egui::Panel::bottom("sidebar_bottom_collapsed")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 12,
                bottom: 0,
            }))
            .show_separator_line(true)
            .show(ui, |ui| {
                if t::nav_icon_button(
                    ui,
                    t::NavIcon::Settings,
                    btn,
                    self.view == View::Settings,
                    crate::i18n::tr("Réglages"),
                )
                .clicked()
                {
                    goto = Some(View::Settings);
                }
            });

        // Chaînes : photos seulement, point vert quand elles sont en direct.
        let viewers_of = |l: &String| match self.status_cache.get(l).map(|(_, s)| s) {
            Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
            _ => None,
        };
        let twitch_live = self.live_followed_with(self.settings.sidebar_limit(), true);
        // Une chaîne suivie sur Twitch (en direct) passe devant, même si elle est aussi en favori.
        let mut favs: Vec<String> = self
            .settings
            .favorites
            .iter()
            .filter(|f| !twitch_live.contains(f))
            .cloned()
            .collect();
        favs.sort_by_key(|l| std::cmp::Reverse(viewers_of(l).map(|v| v + 1).unwrap_or(0)));
        let items: Vec<(String, bool)> = twitch_live
            .into_iter()
            .map(|l| (l, false))
            .chain(favs.into_iter().map(|l| (l, true)))
            .collect();
        let loading = (self.settings.show_follows
            && self.follows_state == super::FollowsState::Loading
            && items.is_empty())
            || (!self.settings.favorites.is_empty()
                && self
                    .settings
                    .favorites
                    .iter()
                    .all(|l| !self.status_cache.contains_key(l)));
        // Barre séparatrice entre Accueil et la première chaîne (même largeur que celle au-dessus de Réglages).
        if loading || !items.is_empty() {
            ui.add_space(t::SP_XS);
            let y = ui.cursor().top();
            ui.painter().line_segment(
                [
                    Pos2::new(ui.max_rect().left(), y),
                    Pos2::new(ui.max_rect().right(), y),
                ],
                egui::Stroke::new(1.0, t::BORDER_HOVER()),
            );
            ui.add_space(t::SP_S);
        }
        // La barre de défilement ne prend pas de place : les photos sont centrées comme les icônes du dessus.
        ui.spacing_mut().scroll.floating_allocated_width = 0.0;
        t::auto_hide_scroll(
            ui,
            "sidebar_collapsed_scroll",
            false,
            egui::ScrollArea::vertical().auto_shrink([false, false]),
            |ui| {
                t::follow_keyboard_focus(ui);
                if loading {
                    let rows = (((ui.clip_rect().bottom() - ui.cursor().top()).max(0.0) / 46.0)
                        .ceil() as usize)
                        .clamp(1, 30);
                    skeleton_rows(ui, ctx, rows, true);
                    return;
                }
                let mut separated = false;
                for (login, is_fav) in &items {
                    if *is_fav && !separated {
                        separated = true;
                        if items.iter().any(|(_, f)| !*f) {
                            ui.add_space(t::SP_XS);
                            let y = ui.cursor().top();
                            ui.painter().line_segment(
                                [
                                    Pos2::new(ui.min_rect().left(), y),
                                    Pos2::new(ui.min_rect().right(), y),
                                ],
                                egui::Stroke::new(1.0, t::BORDER_HOVER()),
                            );
                            ui.add_space(t::SP_S);
                        }
                    }
                    let viewers = viewers_of(login);
                    // Rangée de 38 px (comme les boutons d'icône) : même écart que les icônes du dessus.
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), t::ROW_H),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(t::RADIUS_INNER),
                            t::SURFACE_2(),
                        );
                    }
                    t::paint_pressed(ui, &resp, rect, t::RADIUS_INNER);
                    let c = rect.center();
                    t::focus_ring(ui, &resp, rect, t::RADIUS_INNER);
                    let photo = self.avatars.get(login);
                    t::avatar(ui, c, 15.0, login, photo);
                    if viewers.is_some() {
                        ui.painter()
                            .circle_filled(c + Vec2::new(12.0, 12.0), 5.5, t::SURFACE_1());
                        ui.painter()
                            .circle_filled(c + Vec2::new(12.0, 12.0), 4.3, t::POSITIVE());
                    }
                    let tip = match viewers {
                        Some(v) => format!("{login} · {}", crate::login::format_viewers(v)),
                        None => login.clone(),
                    };
                    if t::show_tip(resp, tip).clicked() {
                        to_watch = Some(login.clone());
                    }
                }
            },
        );

        if let Some(v) = goto {
            self.view = v;
            self.prev_view = View::Home;
        }
        if let Some(l) = to_watch {
            self.start_watch(ctx, l);
        }
    }
}
