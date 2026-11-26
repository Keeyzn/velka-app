//! Page lecteur : vidéo dans une card, chat à côté, infos du stream dessous.
//! Modes : normal, plein écran (F) et picture-in-picture (P).
use super::{VelkaApp, View, WatchState};
use crate::twitch::{format_duration, LiveStatus};
use crate::ui::theme as t;
use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, Margin, Pos2, Rect, RichText, Sense, Vec2,
    ViewportCommand,
};
use std::time::{Duration, Instant};

/// Les contrôles se masquent après ce délai sans mouvement de souris.
const INFO_HEIGHT: f32 = 88.0;
/// Largeur minimale du chat (points) : en dessous, les messages ne seraient plus lisibles.
const CHAT_MIN_W: f32 = 240.0;
/// Distance (points) à tirer encore au-delà de la largeur minimale pour que le chat se ferme.
const CHAT_CLOSE_PULL: f32 = 90.0;
const GAP: f32 = 12.0;
/// Épaisseur de la zone de redimensionnement du PiP.
const EDGE: f32 = 8.0;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Fullscreen,
    Pip,
}

/// Hauteur commune des contrôles de la barre du bas (boutons carrés et sélecteur de qualité).
const BAR_H: f32 = 36.0;

/// Dégradé vertical noir sur `rect` (opacité `from` en haut, `to` en bas, 0 à 255), sous les barres de contrôles.
fn scrim(ctx: &egui::Context, id: &'static str, rect: Rect, from: u8, to: u8) {
    if from == 0 && to == 0 {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::unique(id),
    ));
    let mut mesh = egui::epaint::Mesh::default();
    let (a, b) = (
        Color32::from_black_alpha(from),
        Color32::from_black_alpha(to),
    );
    mesh.colored_vertex(rect.left_top(), a);
    mesh.colored_vertex(rect.right_top(), a);
    mesh.colored_vertex(rect.right_bottom(), b);
    mesh.colored_vertex(rect.left_bottom(), b);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// Placeholder du lecteur : fond des cartes de l'accueil avec un reflet qui défile, logo qui tourne sur lui-même
/// et libellé dessous. `animated` faux (ou animations réduites) : cadre fixe, logo immobile.
pub(super) fn paint_placeholder(ui: &egui::Ui, rect: Rect, radius: u8, text: &str, animated: bool) {
    let ctx = ui.ctx();
    let cr = CornerRadius::same(radius);
    let p = ui.painter();
    p.rect_filled(rect, cr, t::SURFACE_1());
    let loading = animated;
    let animated = animated && t::moves(ctx);
    let time = ctx.input(|i| i.time);
    // Chargement : le logo et le libellé n'apparaissent (en fondu) qu'après `LOADER_DELAY`, pour qu'une connexion
    // rapide ne fasse pas clignoter un logo. Message fixe (audio seul) : affiché tout de suite.
    let (age, shown) = if loading {
        let age = loader_age(ctx, time);
        let fade = f64::from(t::anim_secs(ctx, LOADER_FADE));
        let k = if fade > 0.0 {
            ((age - LOADER_DELAY) / fade).clamp(0.0, 1.0)
        } else if age >= LOADER_DELAY {
            1.0
        } else {
            0.0
        };
        if k < 1.0 {
            ctx.request_repaint();
        }
        (age, egui::emath::easing::cubic_out(k as f32))
    } else {
        (0.0, 1.0)
    };
    if animated {
        let phase = ((time * 0.5) % 1.0) as f32;
        let band = rect.width() * 0.35;
        let x = rect.left() - band + phase * (rect.width() + band);
        let shine = Rect::from_min_max(
            Pos2::new(x.max(rect.left()), rect.top()),
            Pos2::new((x + band).min(rect.right()), rect.bottom()),
        );
        if shine.width() > 0.0 {
            p.rect_filled(shine, cr, Color32::from_white_alpha(10));
        }
    }
    let size = (rect.width().min(rect.height()) * 0.22).clamp(26.0, 64.0);
    let show_text = rect.height() >= 110.0 && rect.width() >= 140.0;
    let gap = 14.0;
    let font = egui::FontId::proportional(14.0);
    let galley = p.layout_no_wrap(text.to_string(), font, t::MUTED().gamma_multiply(shown));
    let group_h = size
        + if show_text {
            gap + galley.size().y
        } else {
            0.0
        };
    let top = rect.center().y - group_h / 2.0;
    let logo = Rect::from_center_size(
        Pos2::new(rect.center().x, top + size / 2.0),
        Vec2::splat(size),
    );
    if animated {
        // Rotation à vitesse constante (un tour par cycle), comme un indicateur d'attente : elle part du logo droit
        // à son apparition.
        let k = (((age - LOADER_DELAY).max(0.0) / LOADER_TURN_SECS) % 1.0) as f32;
        crate::ui::logo::paint_rotated(
            ui,
            logo,
            t::ACCENT_GFX().gamma_multiply(shown),
            k * std::f32::consts::TAU,
        );
        ctx.request_repaint();
    } else {
        crate::ui::logo::paint(ui, logo, t::MUTED().gamma_multiply(shown));
    }
    if show_text {
        let pos = Pos2::new(rect.center().x - galley.size().x / 2.0, logo.bottom() + gap);
        ui.painter().galley(pos, galley, t::MUTED());
    }
}

/// Délai (secondes) avant que le logo et le libellé d'un chargement apparaissent, durée de leur fondu, et durée d'un tour du logo.
const LOADER_DELAY: f64 = 0.25;
const LOADER_FADE: f32 = 0.15;
const LOADER_TURN_SECS: f64 = 1.6;

/// Secondes écoulées depuis le début de l'attente en cours. Une attente est continue tant que le placeholder est dessiné
/// à chaque image ; s'il disparaît plus de 0,25 s, la suivante repart de zéro.
fn loader_age(ctx: &egui::Context, now: f64) -> f64 {
    let id = egui::Id::unique("loader_since");
    let (first, last) =
        ctx.data_mut(|d| *d.get_temp_mut_or_insert_with::<(f64, f64)>(id, || (now, now)));
    let first = if now - last > 0.25 { now } else { first };
    ctx.data_mut(|d| d.insert_temp(id, (first, now)));
    now - first
}

impl VelkaApp {
    fn mode(&self) -> Mode {
        if self.pip {
            Mode::Pip
        } else if self.fullscreen {
            Mode::Fullscreen
        } else {
            Mode::Normal
        }
    }

    pub(super) fn watch_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let chat_opts = crate::ui::chat::ChatOpts {
            scale: self.settings.chat_font_scale,
            timestamps: self.settings.chat_timestamps,
            badges: self
                .chat_extras
                .clone()
                .filter(|_| self.settings.chat_badges),
            followers_locked: self.followers_locked(),
        };
        let mode = self.mode();
        self.cur_anim = self.expand_progress(ctx);
        let fade = self.cur_anim.map(|(_, k)| k);
        let mut to_send: Option<String> = None;
        // Chat replié : l'ancien bouton « Chat » de la barre est remplacé par une flèche en haut du chat
        // (et une languette sur le bord de la vidéo pour le rouvrir), avec un glissement animé.
        let mut chat_open = self.settings.chat_visible;
        if mode == Mode::Normal {
            let left = self.settings.chat_left;
            let panel = if left {
                egui::Panel::left("chat_panel")
            } else {
                egui::Panel::right("chat_panel")
            };
            let mut hide_chat = false;
            // Séparateur du chat : le chat se rétrécit jusqu'à CHAT_MIN_W (messages lisibles), puis le séparateur se bloque ;
            // il faut continuer à tirer sur CHAT_CLOSE_PULL pour que le chat se ferme (il s'estompe pendant ce temps).
            let edge_id = egui::Id::unique("chat_edge_x");
            let drag_id = egui::Id::unique("chat_edge_drag");
            let area_rect = ui.max_rect();
            let mut pull = 0.0_f32;
            if chat_open {
                let (pressed, down, pos) = ctx.input(|i| {
                    (
                        i.pointer.primary_pressed(),
                        i.pointer.primary_down(),
                        i.pointer.interact_pos(),
                    )
                });
                let edge: Option<f32> = ctx.data(|d| d.get_temp(edge_id));
                let mut dragging: bool = ctx.data(|d| d.get_temp(drag_id)).unwrap_or(false);
                if pressed {
                    dragging = matches!((pos, edge), (Some(p), Some(e)) if (p.x - e).abs() <= 6.0);
                }
                if !down {
                    dragging = false;
                }
                if let (true, Some(p)) = (dragging, pos) {
                    let raw = if left {
                        p.x - area_rect.left()
                    } else {
                        area_rect.right() - p.x
                    };
                    pull = (CHAT_MIN_W - raw).max(0.0);
                }
                ctx.data_mut(|d| d.insert_temp(drag_id, dragging));
                if dragging {
                    ctx.request_repaint();
                }
            }
            let pull_k = 1.0 - 0.55 * (pull / CHAT_CLOSE_PULL).clamp(0.0, 1.0);
            let was_open = chat_open;
            // Séparateur d'origine, mais orange (accent) au survol et pendant qu'on modifie la largeur du chat. Les couleurs
            // du contenu du chat restent celles d'avant (rétablies dans le panneau).
            let (old_hover, old_active) = (
                ui.visuals().widgets.hovered.fg_stroke,
                ui.visuals().widgets.active.fg_stroke,
            );
            ui.visuals_mut().widgets.hovered.fg_stroke = egui::Stroke::new(2.0, t::ACCENT());
            ui.visuals_mut().widgets.active.fg_stroke = egui::Stroke::new(3.0, t::ACCENT());
            let out = panel
                .drag_to_open(false)
                .resizable(true)
                .default_size(self.settings.chat_width)
                .size_range(CHAT_MIN_W..=800.0)
                .frame(egui::Frame::NONE.inner_margin(Margin::same(GAP as i8)))
                .show_collapsible(ui, &mut chat_open, |ui| {
                    ui.visuals_mut().widgets.hovered.fg_stroke = old_hover;
                    ui.visuals_mut().widgets.active.fg_stroke = old_active;
                    egui::Frame::new()
                        .fill(t::SURFACE_1())
                        .stroke(egui::Stroke::new(1.0, t::BORDER()))
                        .corner_radius(CornerRadius::same(t::RADIUS_CARD))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_min_size(ui.available_size().max(Vec2::ZERO));
                            let content_k = fade.unwrap_or(1.0) * pull_k;
                            if content_k < 1.0 {
                                ui.set_opacity(content_k);
                            }
                            // Bouton de repli : à gauche du titre si le chat est à droite ; chat à gauche, le titre reste à
                            // gauche et le bouton passe tout à droite (en miroir).
                            // Même hauteur que les boutons de la barre du haut (Retour, Réglages…) : boutons de 32 px centrés à 34 px du haut de la vidéo.
                            ui.add_space(6.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), t::BTN_H),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let icon = if left {
                                        t::Icon::ChevronLeft
                                    } else {
                                        t::Icon::ChevronRight
                                    };
                                    let tip = crate::i18n::tr("Replier le chat");
                                    if left {
                                        ui.label(
                                            RichText::new(crate::i18n::tr("Chat"))
                                                .strong()
                                                .color(t::TEXT()),
                                        );
                                        // Bouton collé au bord droit de la zone : même marge avec la carte que du côté gauche.
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if t::icon_button(
                                                    ui,
                                                    icon,
                                                    t::BTN_H,
                                                    t::IconStyle::Normal,
                                                    tip,
                                                )
                                                .clicked()
                                                {
                                                    hide_chat = true;
                                                }
                                            },
                                        );
                                    } else {
                                        if t::icon_button(
                                            ui,
                                            icon,
                                            t::BTN_H,
                                            t::IconStyle::Normal,
                                            tip,
                                        )
                                        .clicked()
                                        {
                                            hide_chat = true;
                                        }
                                        ui.label(
                                            RichText::new(crate::i18n::tr("Chat"))
                                                .strong()
                                                .color(t::TEXT()),
                                        );
                                    }
                                },
                            );
                            ui.add_space(2.0);
                            to_send = crate::ui::chat::show(
                                ui,
                                &self.chat,
                                &mut self.emotes,
                                &mut self.chat_jump,
                                &mut self.chat_input,
                                &chat_opts,
                            );
                        });
                });
            if let Some(o) = out {
                // Largeur mémorisée seulement quand le panneau est entièrement ouvert.
                if chat_open && self.settings.chat_visible {
                    self.settings.chat_width = o.response.rect.width();
                }
                // Position du séparateur, pour repérer le début d'un tirage à la prochaine image.
                let pr = o.response.rect;
                let x = if left { pr.right() } else { pr.left() };
                ctx.data_mut(|d| d.insert_temp(edge_id, x));
            }
            // Séparateur d'origine rétabli (voir plus haut : couleur d'accent seulement au survol et pendant le tirage).
            ui.visuals_mut().widgets.hovered.fg_stroke = old_hover;
            ui.visuals_mut().widgets.active.fg_stroke = old_active;
            // Le panneau se replie de lui-même dès que le séparateur passe sous la largeur minimale : on s'y oppose tant que
            // l'utilisateur n'a pas tiré CHAT_CLOSE_PULL de plus (le séparateur reste bloqué, le chat s'estompe).
            if was_open && !chat_open && !hide_chat && pull < CHAT_CLOSE_PULL {
                chat_open = true;
            }
            if hide_chat {
                chat_open = false;
            }
            self.settings.chat_visible = chat_open;
        }
        let margin = if mode == Mode::Normal {
            Margin::same(GAP as i8)
        } else {
            Margin::ZERO
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(margin))
            .show(ui, |ui| {
                if let Some(m) = self.watch_ui(ui, ctx, mode) {
                    to_send = Some(m);
                }
            });
        if let Some(m) = to_send {
            self.send_chat(&m);
        }
    }

    /// Dessine la vidéo et ses surcouches ; renvoie un message de chat à envoyer (surimpression).
    fn watch_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, mode: Mode) -> Option<String> {
        let chat_opts = crate::ui::chat::ChatOpts {
            scale: self.settings.chat_font_scale,
            timestamps: self.settings.chat_timestamps,
            badges: self
                .chat_extras
                .clone()
                .filter(|_| self.settings.chat_badges),
            followers_locked: self.followers_locked(),
        };
        let avail = ui.available_rect_before_wrap();
        // Colonne vidéo + infos : même retrait à gauche et en bas que le contenu du chat (bord de la
        // zone de saisie), pour que tout soit aligné ; l'écart avec le chat reste inchangé.
        let avail = if mode == Mode::Normal {
            Rect::from_min_max(
                avail.min + Vec2::new(GAP, 0.0),
                avail.max - Vec2::new(0.0, GAP),
            )
        } else {
            avail
        };
        let (rect, info_rect) = if mode == Mode::Normal {
            split_video_info(avail)
        } else {
            (avail, None)
        };

        // Option « Lecteur au format de la vidéo » : la vidéo prend le format du live, la carte d'infos se colle dessous
        // et l'ensemble est centré dans la place disponible.
        let (rect, info_rect) = match info_rect {
            Some(_) if mode == Mode::Normal && self.settings.fit_video => {
                let v = self.fit_video_rect(ctx, rect);
                let group_h = v.height() + GAP + INFO_HEIGHT;
                let top = (avail.top() + (avail.height() - group_h) / 2.0).max(avail.top());
                let v = Rect::from_min_size(Pos2::new(v.left(), top.round()), v.size());
                let iw = v.width().max(avail.width().min(520.0));
                let ix = (v.center().x - iw / 2.0)
                    .clamp(avail.left(), (avail.right() - iw).max(avail.left()));
                (
                    v,
                    Some(Rect::from_min_size(
                        Pos2::new(ix, v.bottom() + GAP),
                        Vec2::new(iw, INFO_HEIGHT),
                    )),
                )
            }
            _ => (rect, info_rect),
        };
        self.last_video_rect = rect;
        self.shown_video_rect = rect;
        // Agrandissement depuis le mini-lecteur : la vidéo glisse jusqu'à sa place.
        // Les bandes autour de l'image (format du live, taille de la fenêtre) sont dessinées ici, pas par mpv : changer la
        // couleur de fond de mpv recompile ses filtres (environ 200 ms d'arrêt à chaque passage en plein écran). mpv
        // reçoit donc seulement le rectangle de l'image, et son fond reste toujours celui de l'application.
        let ratio = self.video_ratio(ctx);
        if let Some(p) = &self.player {
            let bg_id = egui::Id::unique("video_bg");
            let mpv_bg = t::BG();
            if ctx.data(|d| d.get_temp::<Color32>(bg_id)) != Some(mpv_bg) {
                p.set_background(mpv_bg);
                ctx.data_mut(|d| d.insert_temp(bg_id, mpv_bg));
            }
        }
        // Agrandissement depuis le mini-lecteur ou plein écran : la vidéo glisse jusqu'à sa place.
        if let Some((from, k)) = self.cur_anim {
            let r = Rect::from_min_max(from.min.lerp(rect.min, k), from.max.lerp(rect.max, k));
            self.shown_video_rect = r;
            ui.painter()
                .rect_filled(r, CornerRadius::same(6), Color32::BLACK);
            if let Some(p) = &self.player {
                p.paint(ui, super::lecteur::fit_ratio(r, ratio));
            }
            return None;
        }
        let radius = if mode == Mode::Normal { 12 } else { 0 };
        // Bandes : couleur de fond de l'application ; noir en plein écran et en PiP.
        let bg = if mode == Mode::Normal {
            t::BG()
        } else {
            Color32::BLACK
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius), bg);
        if let Some(p) = &self.player {
            p.paint(ui, super::lecteur::fit_ratio(rect, ratio));
        }
        let w = self.watch.as_ref()?;
        let (login, state, status, on_break) = (
            w.login.clone(),
            w.state.clone(),
            w.status.clone(),
            w.on_break,
        );
        // Connexion, changement de qualité ou de source : grand placeholder animé à la place de l'image.
        // En audio seul (flux lancé), le même cadre reste fixe avec un simple message.
        let audio_only = self.settings.quality == 3;
        let placeholder = match &state {
            WatchState::Checking | WatchState::Starting => {
                Some((crate::i18n::tr("Connexion en cours"), true))
            }
            WatchState::Switching => Some((crate::i18n::tr("Changement de qualité…"), true)),
            WatchState::Refreshing => Some((crate::i18n::tr("Actualisation en cours"), true)),
            WatchState::Playing if audio_only => {
                Some((crate::i18n::tr("Audio du live uniquement"), false))
            }
            WatchState::Error(_) => None,
            WatchState::Playing => None,
        };
        if let Some((text, animated)) = placeholder {
            paint_placeholder(ui, rect, radius, text, animated);
        }

        // Interactions directes avec la vidéo : double-clic, déplacement et bords du PiP.
        let video = ui.interact(
            rect,
            egui::Id::unique("video_area"),
            Sense::click_and_drag(),
        );
        t::focus_ring(ui, &video, rect, 12);
        let mut resizing = false;
        if mode == Mode::Pip {
            self.pip_fit_ratio(ctx);
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                if let Some((dir, icon)) = t::resize_zone(rect, pos, EDGE) {
                    ctx.set_cursor_icon(icon);
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                        resizing = true;
                    }
                }
            }
            if video.drag_started() && !resizing {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
        }
        // Clic = pause : le pointeur devient une main sur la vidéo (curseur normal si l'option est désactivée).
        if self.settings.click_pause
            && video.hovered()
            && ctx.output(|o| o.cursor_icon) == CursorIcon::Default
        {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if video.double_clicked() {
            match mode {
                Mode::Pip => self.exit_pip(ctx),
                _ => self.toggle_fullscreen(ctx, !self.fullscreen),
            }
        }

        // Activité de la souris sur la vidéo.
        let hovered = ctx
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| rect.contains(p));

        // Molette = volume (une cran de molette = 5 %).
        if hovered && self.settings.wheel_volume {
            let d = ctx.input(|i| i.smooth_scroll_delta.y);
            let speed = ctx.options(|o| o.input_options.line_scroll_speed);
            if d != 0.0 && speed > 0.0 {
                self.change_volume(d / speed * 5.0);
            }
        }
        // Clic simple = pause ; attendu un instant pour ne pas gêner le double-clic (plein écran).
        if video.double_clicked() {
            self.pending_click = None;
        } else if video.clicked() && self.settings.click_pause {
            self.pending_click = Some(Instant::now());
        }
        if let Some(t0) = self.pending_click {
            if t0.elapsed() >= Duration::from_millis(280) {
                self.pending_click = None;
                self.toggle_pause();
            } else {
                ctx.request_repaint_after(Duration::from_millis(60));
            }
        }
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO || i.pointer.any_pressed());
        // Réglages ouverts par-dessus le live : barres de contrôle masquées, curseur toujours visible. À la fermeture,
        // les contrôles réapparaissent puis reprennent leur délai normal.
        let under_settings = self.view == View::Settings;
        if (hovered && moved) || under_settings {
            self.controls_seen = Instant::now();
        }
        let since = self.controls_seen.elapsed();
        let playing = state == WatchState::Playing;
        // En PiP les contrôles se masquent plus vite (la fenêtre est petite).
        let hide_after = self.settings.controls_hide_after().map(|h| {
            if mode == Mode::Pip {
                h.min(Duration::from_millis(1200))
            } else {
                h
            }
        });
        let shown = hide_after.is_none_or(|h| since < h);
        let active = !under_settings && (!playing || shown);
        if let Some(h) = hide_after {
            if playing && since < h {
                ctx.request_repaint_after(h - since + Duration::from_millis(20));
            }
        }
        if playing
            && !active
            && !under_settings
            && hovered
            && mode != Mode::Pip
            && hide_after.is_some()
        {
            ctx.set_cursor_icon(CursorIcon::None);
        }
        let alpha = t::motion::eased(
            ctx,
            egui::Id::unique("controls_alpha"),
            active,
            if under_settings {
                0.0
            } else {
                t::motion::CONTROLS
            },
        );

        let (mut back, mut retry, mut settings, mut restart) = (false, false, false, false);
        let mut stop = false;
        let mut refresh = false;
        let mut open_chat = false;
        let (mut fs_toggle, mut pip_toggle) = (false, false);
        let mut pin_toggle = false;
        let mut volume_changed = false;
        let mut pause_toggled = false;
        // Menu « Options du lecteur » : faible latence (relance le flux) et minuteur de veille (décompte remis à zéro).
        let (mut latency_changed, mut sleep_changed) = (false, false);
        let sleep_left = self
            .sleep_deadline
            .map(|d| d.saturating_duration_since(Instant::now()));
        let mut mute_toggled = false;
        let bar_w = (rect.width() - 36.0).max(120.0);

        if alpha > 0.0 {
            // Dégradés sombres sous les barres : les contrôles restent lisibles sur une image claire.
            let fade = |a: f32| (a * alpha).round() as u8;
            let top = Rect::from_min_size(
                rect.left_top(),
                Vec2::new(rect.width(), 84.0_f32.min(rect.height() * 0.4)),
            );
            let bottom = Rect::from_min_max(
                Pos2::new(
                    rect.left(),
                    rect.bottom() - 110.0_f32.min(rect.height() * 0.5),
                ),
                rect.right_bottom(),
            );
            scrim(ctx, "watch_scrim_top", top, fade(120.0), 0);
            scrim(ctx, "watch_scrim_bottom", bottom, 0, fade(150.0));
            // Barre du haut.
            egui::Area::new(egui::Id::unique("watch_top"))
                .fixed_pos(rect.left_top() + Vec2::splat(10.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_opacity(alpha);
                    let _dark = t::DarkScope::enter(ui);
                    egui::Frame::new()
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            if mode != Mode::Pip {
                                ui.set_width(bar_w);
                            }
                            // Barre trop étroite pour ses boutons (fenêtre minimale, zoom élevé) : Retour, Arrêter, Réglages et
                            // Actualiser passent en icône seule ; le bouton du chat reste à sa place habituelle.
                            let gap = ui.spacing().item_spacing.x;
                            let reopen_w = if mode == Mode::Normal && !self.settings.chat_visible {
                                t::BTN_H + gap
                            } else {
                                0.0
                            };
                            let mut labels = vec!["Retour"];
                            if self.settings.mini_player {
                                labels.push("Arrêter");
                            }
                            if mode == Mode::Normal {
                                labels.extend(["Réglages", "Actualiser"]);
                            }
                            let full_w: f32 = labels
                                .iter()
                                .map(|l| t::icon_text_button_width(ui, crate::i18n::tr(l)) + gap)
                                .sum::<f32>()
                                + reopen_w
                                + if on_break { 130.0 } else { 0.0 };
                            let icon_only = mode != Mode::Pip && full_w > bar_w;
                            let body = |ui: &mut egui::Ui| {
                                let top_button = |ui: &mut egui::Ui,
                                                  kind: t::BtnKind,
                                                  icon: t::NavIcon,
                                                  label: &str|
                                 -> bool {
                                    if icon_only {
                                        t::icon_only_button(ui, kind, icon, label).clicked()
                                    } else {
                                        t::icon_text_button(ui, kind, icon, label).clicked()
                                    }
                                };
                                if mode == Mode::Pip {
                                    pip_toggle = t::icon_text_button(
                                        ui,
                                        t::BtnKind::Secondary,
                                        t::NavIcon::Pip,
                                        crate::i18n::tr("Quitter le PiP"),
                                    )
                                    .clicked();
                                    // Punaise : accent = gardé au premier plan (comme toujours), barrée = fenêtre ordinaire.
                                    let (kind, icon) = if self.pip_on_top {
                                        (t::BtnKind::Primary, t::NavIcon::Pin)
                                    } else {
                                        (t::BtnKind::Secondary, t::NavIcon::PinOff)
                                    };
                                    pin_toggle = t::icon_only_button(
                                        ui,
                                        kind,
                                        icon,
                                        crate::i18n::tr("Toujours au premier plan"),
                                    )
                                    .clicked();
                                    if on_break {
                                        t::pill(
                                            ui,
                                            None,
                                            crate::i18n::tr("Reprise dans un instant…"),
                                            t::TEXT(),
                                            t::ACCENT_SOFT(),
                                        );
                                    }
                                    return;
                                }
                                // Chat replié : flèche pour le rouvrir, du côté où se trouvait le chat (même masquage et mêmes
                                // couleurs que les autres boutons). Chat à gauche : devant « Retour » ; à droite : au bout de la barre.
                                let reopen = mode == Mode::Normal && !self.settings.chat_visible;
                                let chat_left = self.settings.chat_left;
                                let reopen_tip = crate::i18n::tr("Afficher le chat");
                                if reopen && chat_left {
                                    open_chat = t::icon_button(
                                        ui,
                                        t::Icon::ChevronRight,
                                        t::BTN_H,
                                        t::IconStyle::Normal,
                                        reopen_tip,
                                    )
                                    .clicked();
                                }
                                // Retour, Arrêter et Réglages côte à côte à gauche ; la pastille « Reprise dans un instant… » reste à droite.
                                back = top_button(
                                    ui,
                                    t::BtnKind::Secondary,
                                    t::NavIcon::Back,
                                    crate::i18n::tr("Retour"),
                                );
                                // Sans mini-lecteur, « Retour » arrête déjà le live : le bouton « Arrêter » ferait double emploi.
                                if self.settings.mini_player {
                                    stop = top_button(
                                        ui,
                                        t::BtnKind::SecondaryDanger,
                                        t::NavIcon::Stop,
                                        crate::i18n::tr("Arrêter"),
                                    );
                                }
                                if mode == Mode::Normal {
                                    settings = top_button(
                                        ui,
                                        t::BtnKind::Secondary,
                                        t::NavIcon::Settings,
                                        crate::i18n::tr("Réglages"),
                                    );
                                    refresh = top_button(
                                        ui,
                                        t::BtnKind::Secondary,
                                        t::NavIcon::Refresh,
                                        crate::i18n::tr("Actualiser"),
                                    );
                                }
                                let right = |ui: &mut egui::Ui| {
                                    if reopen && !chat_left {
                                        open_chat = t::icon_button(
                                            ui,
                                            t::Icon::ChevronLeft,
                                            t::BTN_H,
                                            t::IconStyle::Normal,
                                            reopen_tip,
                                        )
                                        .clicked();
                                    }
                                    if on_break {
                                        t::pill(
                                            ui,
                                            None,
                                            crate::i18n::tr("Reprise dans un instant…"),
                                            t::TEXT(),
                                            t::ACCENT_SOFT(),
                                        );
                                    }
                                };
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    right,
                                );
                            };
                            ui.horizontal(body);
                        });
                });

            if open_chat {
                self.settings.chat_visible = true;
            }

            // Barre du bas.
            egui::Area::new(egui::Id::unique("watch_bottom"))
                .fixed_pos(Pos2::new(rect.left() + 10.0, rect.bottom() - 58.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_opacity(alpha);
                    let _dark = t::DarkScope::enter(ui);
                    egui::Frame::new()
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            if mode != Mode::Pip {
                                ui.set_width(bar_w);
                            }
                            // Largeur nécessaire pour : Pause, Son, curseur de volume (120) + pastille, PiP, plein écran et la
                            // qualité repliée (~100). En dessous : volume en barre verticale ; plus étroit encore : qualité en icône.
                            let pill_w = ui
                                .painter()
                                .layout_no_wrap(
                                    format!("{} %", self.settings.volume),
                                    egui::FontId::proportional(13.0),
                                    Color32::WHITE,
                                )
                                .size()
                                .x
                                + 24.0;
                            let needed_inline =
                                5.0 * BAR_H + 120.0 + pill_w + 8.0 * 7.0 + 100.0 + 16.0;
                            let vertical_volume = mode != Mode::Pip && bar_w < needed_inline;
                            let quality_icon_only =
                                vertical_volume && bar_w < 5.0 * BAR_H + 100.0 + 8.0 * 5.0 + 16.0;
                            // Hauteur fixe + centrage vertical ; qualité, chat, PiP et plein écran collés à droite.
                            ui.allocate_ui_with_layout(
                                Vec2::new(if mode == Mode::Pip { 176.0 } else { bar_w }, BAR_H),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let (pi, ptip) = if self.paused {
                                        (t::Icon::Play, crate::i18n::tr("Lecture"))
                                    } else {
                                        (t::Icon::Pause, "Pause")
                                    };
                                    pause_toggled =
                                        t::icon_button(ui, pi, BAR_H, t::IconStyle::Normal, ptip)
                                            .clicked();
                                    let (mi, mtip) = if self.muted {
                                        (t::Icon::Muted, crate::i18n::tr("Réactiver le son"))
                                    } else {
                                        (t::Icon::Speaker, crate::i18n::tr("Couper le son"))
                                    };
                                    let mute_resp =
                                        t::icon_button(ui, mi, BAR_H, t::IconStyle::Normal, mtip);
                                    mute_toggled = mute_resp.clicked();
                                    if mode == Mode::Pip {
                                        // Curseur de volume compact à côté de Pause et Muet.
                                        if t::volume_slider_scaled(
                                            ui,
                                            &mut self.settings.volume,
                                            self.settings.volume_max,
                                            80.0,
                                            1.4,
                                        ) {
                                            volume_changed = true;
                                        }
                                        return;
                                    }
                                    if vertical_volume {
                                        // Barre étroite : le volume se règle avec une barre verticale qui s'ouvre au-dessus du bouton
                                        // son (survol ou focus), au lieu du curseur horizontal qui déborderait sur la qualité.
                                        let pop_id = egui::Id::unique("volume_popup_until");
                                        let now = ctx.input(|i| i.time);
                                        if mute_resp.hovered() || mute_resp.has_focus() {
                                            ctx.data_mut(|d| d.insert_temp(pop_id, now + 0.35));
                                        }
                                        let until: f64 =
                                            ctx.data(|d| d.get_temp(pop_id)).unwrap_or(0.0);
                                        if until > now {
                                            let (sh, margin) = (120.0, 6.0);
                                            let pos = Pos2::new(
                                                mute_resp.rect.center().x - 17.0 - margin,
                                                (mute_resp.rect.top() - 6.0 - sh - 2.0 * margin)
                                                    .max(rect.top() + 4.0),
                                            );
                                            let area =
                                                egui::Area::new(egui::Id::unique("volume_popup"))
                                                    .order(egui::Order::Foreground)
                                                    .fixed_pos(pos)
                                                    .show(ctx, |ui| {
                                                        ui.set_opacity(alpha);
                                                        let _dark = t::DarkScope::enter(ui);
                                                        egui::Frame::new()
                                                            .fill(Color32::from_black_alpha(200))
                                                            .corner_radius(CornerRadius::same(10))
                                                            .inner_margin(margin)
                                                            .show(ui, |ui| {
                                                                t::volume_slider_vertical(
                                                                    ui,
                                                                    &mut self.settings.volume,
                                                                    self.settings.volume_max,
                                                                    sh,
                                                                )
                                                            })
                                                            .inner
                                                    });
                                            if area.inner {
                                                volume_changed = true;
                                            }
                                            let over =
                                                ctx.input(|i| i.pointer.hover_pos()).is_some_and(
                                                    |p| area.response.rect.expand(8.0).contains(p),
                                                );
                                            if over {
                                                ctx.data_mut(|d| d.insert_temp(pop_id, now + 0.35));
                                            }
                                            self.controls_seen = Instant::now();
                                            ctx.request_repaint_after(Duration::from_millis(60));
                                        }
                                    } else {
                                        if t::volume_slider(
                                            ui,
                                            &mut self.settings.volume,
                                            self.settings.volume_max,
                                            120.0,
                                        ) {
                                            volume_changed = true;
                                        }
                                        t::pill(
                                            ui,
                                            None,
                                            &format!("{} %", self.settings.volume),
                                            Color32::WHITE,
                                            Color32::from_white_alpha(22),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (fi, ftip) = if mode == Mode::Fullscreen {
                                                (
                                                    t::Icon::ExitFullscreen,
                                                    crate::i18n::tr("Quitter le plein écran"),
                                                )
                                            } else {
                                                (
                                                    t::Icon::Fullscreen,
                                                    crate::i18n::tr("Plein écran"),
                                                )
                                            };
                                            fs_toggle = t::icon_button(
                                                ui,
                                                fi,
                                                BAR_H,
                                                t::IconStyle::Normal,
                                                ftip,
                                            )
                                            .clicked();
                                            if t::icon_button(
                                                ui,
                                                t::Icon::Pip,
                                                BAR_H,
                                                t::IconStyle::Normal,
                                                "PiP",
                                            )
                                            .clicked()
                                            {
                                                pip_toggle = true;
                                            }
                                            // Engrenage : options du lecteur (mêmes réglages que la page Réglages).
                                            let gear = t::icon_button(
                                                ui,
                                                t::Icon::Gear,
                                                BAR_H,
                                                t::IconStyle::Normal,
                                                crate::i18n::tr("Options du lecteur"),
                                            );
                                            (latency_changed, sleep_changed) =
                                                super::settings::player_options_menu(
                                                    ui,
                                                    &gear,
                                                    rect,
                                                    &mut self.settings.low_latency,
                                                    &mut self.settings.sleep_timer,
                                                    &mut self.settings.sleep_custom_min,
                                                    sleep_left,
                                                );
                                            if egui::Popup::is_any_open(ctx) {
                                                self.controls_seen = Instant::now();
                                            }
                                            // Sélecteur de qualité de la même hauteur (34 px) que les boutons ronds.
                                            // Barre trop étroite pour tout afficher : un seul bouton qui déploie la liste.
                                            let needed = 5.0 * BAR_H
                                                + 120.0
                                                + pill_w
                                                + 8.0 * 8.0
                                                + super::settings::quality_segmented_width(ui)
                                                + 16.0;
                                            if bar_w < needed {
                                                restart = super::settings::quality_dropdown(
                                                    ui,
                                                    &mut self.settings.quality,
                                                    BAR_H,
                                                    quality_icon_only,
                                                );
                                                if egui::Popup::is_any_open(ctx) {
                                                    self.controls_seen = Instant::now();
                                                }
                                            } else {
                                                ui.spacing_mut().interact_size.y = BAR_H - 8.0;
                                                restart = super::settings::quality_segmented(
                                                    ui,
                                                    &mut self.settings.quality,
                                                    false,
                                                );
                                            }
                                        },
                                    );
                                },
                            );
                        });
                });
        }

        // Chat en surimpression (plein écran, si l'option est activée).
        let mut to_send = None;
        if mode == Mode::Fullscreen
            && self.settings.fullscreen_chat_overlay
            && self.settings.chat_visible
        {
            let w = self.settings.chat_width.min(rect.width() * 0.4);
            let h = (rect.height() - 160.0).max(120.0);
            egui::Area::new(egui::Id::unique("fs_chat"))
                .fixed_pos(Pos2::new(rect.right() - w - 12.0, rect.top() + 64.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let _dark = t::DarkScope::enter(ui);
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(150))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new((w - 20.0).max(0.0), h));
                            ui.set_max_size(Vec2::new((w - 20.0).max(0.0), h));
                            to_send = crate::ui::chat::show(
                                ui,
                                &self.chat,
                                &mut self.emotes,
                                &mut self.chat_jump,
                                &mut self.chat_input,
                                &chat_opts,
                            );
                        });
                });
        }

        // Indication du volume, quelques instants après chaque changement.
        if let Some(t0) = self.volume_toast {
            let age = t0.elapsed().as_secs_f32();
            if age < 1.3 {
                let a = t::motion::toast_alpha(ctx, age, 1.3, false);
                egui::Area::new(egui::Id::unique("volume_toast"))
                    .pivot(egui::Align2::CENTER_TOP)
                    .fixed_pos(Pos2::new(rect.center().x, rect.top() + 78.0))
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.set_opacity(a);
                        let text =
                            format!("{} {} %", crate::i18n::tr("Volume"), self.settings.volume);
                        t::pill(
                            ui,
                            None,
                            &text,
                            Color32::WHITE,
                            Color32::from_black_alpha(190),
                        );
                    });
            } else {
                self.volume_toast = None;
            }
        }

        // Message bref (capture d'écran…), sous l'indication du volume.
        if let Some((t0, text)) = &self.info_toast {
            let age = t0.elapsed().as_secs_f32();
            if age < 2.5 {
                let a = t::motion::toast_alpha(ctx, age, 2.5, true);
                let text = text.clone();
                egui::Area::new(egui::Id::unique("info_toast"))
                    .pivot(egui::Align2::CENTER_TOP)
                    .fixed_pos(Pos2::new(rect.center().x, rect.top() + 118.0))
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .fade_in(false)
                    .show(ctx, |ui| {
                        ui.set_opacity(a);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        t::pill(
                            ui,
                            None,
                            &text,
                            Color32::WHITE,
                            Color32::from_black_alpha(190),
                        );
                    });
            } else {
                self.info_toast = None;
            }
        }

        // Minuteur de veille : compte à rebours pendant la dernière minute.
        if let Some(left) = self
            .sleep_deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
        {
            if left < Duration::from_secs(60) {
                let text = format!(
                    "{} {} s",
                    crate::i18n::tr("Arrêt du live dans"),
                    left.as_secs() + 1
                );
                egui::Area::new(egui::Id::unique("sleep_toast"))
                    .pivot(egui::Align2::CENTER_BOTTOM)
                    .fixed_pos(Pos2::new(rect.center().x, rect.bottom() - 74.0))
                    // Sous les menus et les barres (Foreground) : le menu des options du lecteur passe par-dessus.
                    .order(egui::Order::Middle)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        // Avertissement lisible sans écraser l'image : texte de 17 px sur fond sombre.
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(205))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(18, 8))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&text)
                                        .size(17.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }

        // Panneau d'infos techniques (touche I).
        if self.tech_info && state == WatchState::Playing {
            let rows = self.tech_values();
            egui::Area::new(egui::Id::unique("tech_info"))
                .pivot(egui::Align2::LEFT_TOP)
                .fixed_pos(Pos2::new(rect.left() + 12.0, rect.top() + 64.0))
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    let _dark = t::DarkScope::enter(ui);
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(190))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            ui.label(
                                RichText::new(crate::i18n::tr("Infos techniques"))
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.add_space(4.0);
                            egui::Grid::new(egui::Id::unique("tech_grid"))
                                .num_columns(2)
                                .spacing([14.0, 3.0])
                                .show(ui, |ui| {
                                    for (label, value) in &rows {
                                        ui.label(
                                            RichText::new(crate::i18n::tr(label))
                                                .size(12.0)
                                                .color(t::MUTED()),
                                        );
                                        ui.label(
                                            RichText::new(value).size(12.0).color(Color32::WHITE),
                                        );
                                        ui.end_row();
                                    }
                                });
                        });
                });
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        // Messages d'état toujours visibles.
        let message = match &state {
            WatchState::Error(m) => Some((m.clone(), true)),
            // Les autres états sont montrés par le placeholder qui remplace l'image.
            _ => None,
        };
        // Live hors ligne ou terminé : on propose d'autres chaînes (même jeu en priorité).
        let offline_error = matches!(
            &state,
            WatchState::Error(m) if m.as_str() == crate::i18n::tr("La chaîne est hors ligne.")
                || m.starts_with(crate::i18n::tr("Le flux s'est interrompu :"))
        );
        let known_game = match &status {
            Some(LiveStatus::Live { game, .. }) if !game.is_empty() => Some(game.clone()),
            _ => None,
        };
        let rec_on = offline_error && mode != Mode::Pip && self.settings.recommendations;
        let rec_rows: Vec<(crate::twitch::StreamRec, Option<egui::TextureId>)> = if rec_on {
            let recs = self.recommendations(ctx, &login, known_game.as_deref());
            recs.into_iter()
                .map(|r| {
                    let photo = self.avatars.get(&r.login);
                    (r, photo)
                })
                .collect()
        } else {
            Vec::new()
        };
        // Recherche en cours : des lignes factices tiennent la place, puis les vraies apparaissent de haut en bas.
        let rec_loading = rec_on && !self.recs.as_ref().is_some_and(|r| r.login == login);
        let rec_slots = self.settings.recs_limit();
        let rec_placeholders = if rec_loading { rec_slots } else { 0 };
        let rec_reveal_id = egui::Id::unique("recs_reveal").with(login.as_str());
        let rec_reveal = if rec_on && !rec_loading {
            let now = ctx.input(|i| i.time);
            let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(rec_reveal_id, || now));
            (now - start) as f32
        } else {
            // Pendant un chargement, on oublie l'apparition précédente ; hors de l'écran d'erreur on la garde
            // (« Réessayer » ne rejoue pas l'animation).
            if rec_loading {
                ctx.data_mut(|d| d.remove::<f64>(rec_reveal_id));
            }
            0.0
        };
        let mut rec_pick: Option<String> = None;
        if let Some((text, is_error)) = message {
            egui::Area::new(egui::Id::unique("watch_state"))
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(rect.center())
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let _dark = t::DarkScope::enter(ui);
                    t::card(ui, |ui| {
                        ui.set_max_width((rect.width() - 80.0).max(160.0));
                        if is_error {
                            ui.label(RichText::new(text).color(t::NEGATIVE()));
                            ui.add_space(t::SP_S);
                            ui.horizontal(|ui| {
                                // Même taille pour « Retour » et « Réessayer » (la plus large des deux).
                                let (back_l, retry_l) =
                                    (crate::i18n::tr("Retour"), crate::i18n::tr("Réessayer"));
                                let w = t::icon_text_button_width(ui, back_l)
                                    .max(t::icon_text_button_width(ui, retry_l));
                                if t::icon_text_button_min(
                                    ui,
                                    t::BtnKind::Secondary,
                                    t::NavIcon::Back,
                                    back_l,
                                    w,
                                )
                                .clicked()
                                {
                                    stop = true;
                                }
                                retry = t::icon_text_button_min(
                                    ui,
                                    t::BtnKind::Primary,
                                    t::NavIcon::Refresh,
                                    retry_l,
                                    w,
                                )
                                .clicked();
                            });
                            if !rec_rows.is_empty() || rec_placeholders > 0 {
                                ui.add_space(t::SP_L);
                                ui.label(
                                    RichText::new(crate::i18n::tr("Chaînes recommandées"))
                                        .size(13.0)
                                        .color(t::MUTED()),
                                );
                                ui.add_space(4.0);
                                let list_w = (rect.width() - 80.0).clamp(160.0, 440.0);
                                ui.set_min_width(list_w);
                                ui.set_max_width(list_w);
                                // Hauteur de la carte limitée : marge de sécurité sous les boutons du haut et au-dessus de la
                                // barre du bas ; la liste défile à l'intérieur (comme la liste « Favoris »).
                                let three_rows =
                                    3.0 * 40.0 + 2.0 * ui.spacing().item_spacing.y + 24.0;
                                let max_list_h =
                                    ((rect.height() - 130.0) - 48.0 - ui.min_rect().height())
                                        .max(three_rows);
                                let n_rows =
                                    rec_rows.len().max(rec_placeholders).max(rec_slots) as f32;
                                let content_h = n_rows * 40.0
                                    + (n_rows - 1.0).max(0.0) * ui.spacing().item_spacing.y;
                                let list_h = content_h.min(max_list_h);
                                // Hauteur imposée à la zone (et non déduite de la place restante, trop petite dans la fenêtre flottante).
                                ui.allocate_ui(Vec2::new(list_w, list_h), |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("recs_scroll")
                                        .max_height(list_h)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            if rec_placeholders > 0 {
                                                let time = ctx.input(|i| i.time);
                                                for i in 0..rec_placeholders {
                                                    let (rrect, _) = ui.allocate_exact_size(
                                                        Vec2::new(ui.available_width(), 40.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    let p = ui.painter();
                                                    let bar = |y: f32, f: f32, h: f32| {
                                                        Rect::from_min_size(
                                                            Pos2::new(
                                                                rrect.left() + 42.0,
                                                                rrect.top() + y,
                                                            ),
                                                            Vec2::new(
                                                                (rrect.width() - 100.0) * f,
                                                                h,
                                                            ),
                                                        )
                                                    };
                                                    p.circle_filled(
                                                        Pos2::new(
                                                            rrect.left() + 20.0,
                                                            rrect.center().y,
                                                        ),
                                                        14.0,
                                                        t::SURFACE_3(),
                                                    );
                                                    p.rect_filled(
                                                        bar(8.0, 0.55, 10.0),
                                                        CornerRadius::same(4),
                                                        t::SURFACE_3(),
                                                    );
                                                    p.rect_filled(
                                                        bar(24.0, 0.35, 8.0),
                                                        CornerRadius::same(4),
                                                        t::SURFACE_3(),
                                                    );
                                                    if t::moves(ctx) {
                                                        let phase = ((time * 0.9 - i as f64 * 0.08)
                                                            % 1.0)
                                                            as f32;
                                                        let a = (1.0 - (phase * 2.0 - 1.0).abs())
                                                            .max(0.0)
                                                            * 14.0;
                                                        p.rect_filled(
                                                            rrect,
                                                            CornerRadius::same(8),
                                                            Color32::from_white_alpha(a as u8),
                                                        );
                                                    }
                                                }
                                                ctx.request_repaint();
                                            }
                                            // Colonne fixe : le point vert est au même endroit pour toutes les lignes (largeur de la plus large).
                                            let units = (
                                                crate::i18n::tr("spectateur"),
                                                crate::i18n::tr("spectateurs"),
                                            );
                                            let viewers_text = |r: &crate::twitch::StreamRec| {
                                                format!(
                                                    "{} {}",
                                                    crate::login::format_viewers(r.viewers),
                                                    if r.viewers == 1 { units.0 } else { units.1 }
                                                )
                                            };
                                            let viewers_w = rec_rows
                                                .iter()
                                                .map(|(r, _)| {
                                                    ui.painter()
                                                        .layout_no_wrap(
                                                            viewers_text(r),
                                                            egui::FontId::proportional(12.5),
                                                            Color32::WHITE,
                                                        )
                                                        .size()
                                                        .x
                                                })
                                                .fold(0.0_f32, f32::max);
                                            for (i, (r, photo)) in rec_rows.iter().enumerate() {
                                                // Apparition en fondu, ligne après ligne (aucune animation si elles sont réduites).
                                                let k = t::motion::stagger_in(
                                                    ctx,
                                                    ctx.input(|i| i.time) - f64::from(rec_reveal),
                                                    i,
                                                );
                                                ui.scope(|ui| {
                                                    ui.set_opacity(k);
                                                    let (rrect, resp) = ui.allocate_exact_size(
                                                        Vec2::new(ui.available_width(), 40.0),
                                                        egui::Sense::click(),
                                                    );
                                                    if resp.hovered() {
                                                        ui.painter().rect_filled(
                                                            rrect,
                                                            CornerRadius::same(8),
                                                            t::SURFACE_3(),
                                                        );
                                                    }
                                                    t::focus_ring(ui, &resp, rrect, 8);
                                                    resp.widget_info(|| {
                                                        egui::WidgetInfo::labeled(
                                                            egui::Role::Button,
                                                            true,
                                                            r.login.as_str(),
                                                        )
                                                    });
                                                    t::avatar(
                                                        ui,
                                                        Pos2::new(
                                                            rrect.left() + 20.0,
                                                            rrect.center().y,
                                                        ),
                                                        14.0,
                                                        &r.login,
                                                        *photo,
                                                    );
                                                    let text_w =
                                                        (rrect.width() - 44.0 - viewers_w - 30.0)
                                                            .max(40.0);
                                                    t::paint_name_line(
                                                        ui.painter(),
                                                        Pos2::new(
                                                            rrect.left() + 42.0,
                                                            rrect.top() + 5.0,
                                                        ),
                                                        &r.login,
                                                        13.5,
                                                        t::TEXT(),
                                                        text_w,
                                                    );
                                                    t::paint_one_line(
                                                        ui.painter(),
                                                        Pos2::new(
                                                            rrect.left() + 42.0,
                                                            rrect.top() + 22.0,
                                                        ),
                                                        &r.game,
                                                        11.5,
                                                        t::ACCENT_TEXT(),
                                                        text_w,
                                                    );
                                                    // Point vert, nombre de spectateurs (blanc) puis libellé, alignés à droite.
                                                    let galley = ui.painter().layout_no_wrap(
                                                        viewers_text(r),
                                                        egui::FontId::proportional(12.5),
                                                        Color32::WHITE,
                                                    );
                                                    let gx = rrect.right() - 10.0 - viewers_w;
                                                    ui.painter().circle_filled(
                                                        Pos2::new(gx - 8.0, rrect.center().y),
                                                        3.5,
                                                        t::POSITIVE(),
                                                    );
                                                    ui.painter().galley(
                                                        Pos2::new(
                                                            gx,
                                                            rrect.center().y
                                                                - galley.size().y / 2.0,
                                                        ),
                                                        galley,
                                                        Color32::WHITE,
                                                    );
                                                    if resp.clicked() {
                                                        rec_pick = Some(r.login.clone());
                                                    }
                                                });
                                            }
                                            // Moins de chaînes que de lignes factices : la place reste réservée, rien ne bouge.
                                            let missing =
                                                rec_slots.saturating_sub(rec_rows.len()) as f32;
                                            if !rec_rows.is_empty() && missing > 0.0 {
                                                let gap = ui.spacing().item_spacing.y;
                                                ui.allocate_exact_size(
                                                    Vec2::new(
                                                        1.0,
                                                        missing * 40.0 + (missing - 1.0) * gap,
                                                    ),
                                                    Sense::hover(),
                                                );
                                            }
                                        });
                                });
                            }
                        } else {
                            ui.add(
                                egui::Label::new(RichText::new(text).color(t::TEXT()))
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            );
                        }
                    });
                });
        }

        // Infos du stream sous la vidéo (mode normal).
        let mut toggle_follow = false;
        if let Some(info_rect) = info_rect {
            let photo = self.avatars.get(&login);
            let mut info_ui = ui.new_child(egui::UiBuilder::new().max_rect(info_rect));
            // Même carte (88 px de haut, mêmes bords) ; marges verticales de 10 px pour loger 3 lignes (68 px).
            egui::Frame::new()
                .fill(t::SURFACE_1())
                .stroke(t::hairline())
                .corner_radius(CornerRadius::same(t::RADIUS_CARD))
                // Marge droite de 28 px = marge haut / bas du bouton (carte 88 px, bouton 32 px).
                .inner_margin(Margin {
                    left: 20,
                    right: 28,
                    top: 10,
                    bottom: 10,
                })
                .show(&mut info_ui, |ui| {
                    let content_h = (info_rect.height() - 20.0).max(0.0);
                    ui.set_min_size(Vec2::new((info_rect.width() - 48.0).max(0.0), content_h));
                    // Bouton court (« Favori », étoile pleine ou vide) : il reste petit même à la taille minimale ; le détail
                    // de l'action est dans l'infobulle.
                    let btn_w = t::icon_text_button_width(ui, crate::i18n::tr("Favori"));
                    let (avatar_d, gap_x) = (40.0, 16.0);
                    // Marge de 16 px : le bouton ne doit jamais déborder de la carte (fenêtre étroite, zoom élevé).
                    let text_w =
                        (ui.available_width() - avatar_d - gap_x * 2.0 - btn_w - 16.0).max(60.0);
                    // Avatar, texte et bouton centrés verticalement sur toute la hauteur de la carte.
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), content_h),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = gap_x;
                            let (arect, _) =
                                ui.allocate_exact_size(Vec2::splat(avatar_d), egui::Sense::hover());
                            t::avatar(ui, arect.center(), avatar_d / 2.0, &login, photo);
                            ui.allocate_ui_with_layout(
                                Vec2::new(text_w, content_h),
                                egui::Layout::top_down(egui::Align::Min)
                                    .with_main_align(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(text_w);
                                    ui.spacing_mut().item_spacing.y = 4.0;
                                    let live = matches!(&status, Some(LiveStatus::Live { .. }));
                                    let offline = !live
                                        && (offline_error
                                            || matches!(&status, Some(LiveStatus::Offline)));
                                    let offline_l = crate::i18n::tr("Hors ligne");
                                    let offline_w = if offline {
                                        ui.painter()
                                            .layout_no_wrap(
                                                offline_l.to_string(),
                                                egui::FontId::proportional(13.0),
                                                Color32::WHITE,
                                            )
                                            .size()
                                            .x
                                            + 12.0
                                    } else {
                                        0.0
                                    };
                                    // Sans live : une seule ligne, centrée sur la hauteur de la carte (alignée avec la photo).
                                    if !live {
                                        ui.add_space(((content_h - 24.0) / 2.0).max(0.0));
                                    }
                                    // Ligne 1 : nom de la chaîne (gras) puis pastille LIVE.
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(text_w, 24.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.x = 12.0;
                                            ui.scope(|ui| {
                                                ui.set_max_width(if live {
                                                    (text_w - 76.0).max(60.0)
                                                } else {
                                                    (text_w - offline_w).max(60.0)
                                                });
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&login)
                                                            .font(t::name_font(20.0))
                                                            .color(t::TEXT()),
                                                    )
                                                    .truncate(),
                                                );
                                            });
                                            if live {
                                                t::live_pill(ui);
                                            } else if offline {
                                                ui.label(
                                                    RichText::new(offline_l)
                                                        .size(13.0)
                                                        .color(t::NEGATIVE()),
                                                );
                                            }
                                        },
                                    );
                                    if let Some(LiveStatus::Live {
                                        title,
                                        viewers,
                                        game,
                                        started_at,
                                        ..
                                    }) = &status
                                    {
                                        // Ligne 2 : titre du stream, sur une seule ligne.
                                        if !title.is_empty() {
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(text_w, 20.0),
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(title)
                                                                .size(15.0)
                                                                .color(t::TEXT()),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            );
                                        }
                                        // Ligne 3 : jeu, spectateurs (en vert), durée.
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(text_w, 16.0),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 8.0;
                                                // Colonne étroite : la durée, puis le jeu, disparaissent au lieu de pousser le bouton Favori hors de la carte.
                                                if !game.is_empty() && text_w >= 200.0 {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(game)
                                                                .size(13.0)
                                                                .color(t::ACCENT_TEXT()),
                                                        )
                                                        .truncate(),
                                                    );
                                                    meta_dot(ui);
                                                }
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} {}",
                                                        crate::login::format_viewers(*viewers),
                                                        crate::i18n::tr("spectateurs")
                                                    ))
                                                    .size(13.0)
                                                    .color(t::POSITIVE()),
                                                );
                                                if let Some(s) =
                                                    started_at.as_ref().filter(|_| text_w >= 300.0)
                                                {
                                                    meta_dot(ui);
                                                    let now = std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .map(|d| d.as_secs())
                                                        .unwrap_or(*s);
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} {}",
                                                            crate::i18n::tr("depuis"),
                                                            format_duration(now.saturating_sub(*s))
                                                        ))
                                                        .size(13.0)
                                                        .color(t::MUTED()),
                                                    );
                                                    ctx.request_repaint_after(Duration::from_secs(
                                                        30,
                                                    ));
                                                }
                                            },
                                        );
                                    }
                                },
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let is_fav = self.settings.favorites.contains(&login);
                                    let (kind, icon, tip) = if is_fav {
                                        (
                                            t::BtnKind::Secondary,
                                            t::NavIcon::StarFilled,
                                            crate::i18n::tr("Retirer des favoris"),
                                        )
                                    } else {
                                        (
                                            t::BtnKind::Primary,
                                            t::NavIcon::StarPlus,
                                            crate::i18n::tr("Ajouter aux favoris"),
                                        )
                                    };
                                    let resp = t::icon_text_button(
                                        ui,
                                        kind,
                                        icon,
                                        crate::i18n::tr("Favori"),
                                    );
                                    toggle_follow = resp.clicked();
                                    t::show_tip_centered(resp, tip);
                                },
                            );
                        },
                    );
                });
        }
        if toggle_follow {
            crate::history::toggle_favorite(&mut self.settings.favorites, &login);
            if self.settings.favorites.contains(&login) {
                self.refresh_after_follow(ctx, &login);
            }
        }

        if volume_changed {
            // Bouger le curseur alors que le son est coupé le réactive au niveau choisi.
            if self.muted {
                self.toggle_mute();
            }
            self.apply_volume();
        }
        if pause_toggled {
            self.toggle_pause();
        }
        if mute_toggled {
            self.toggle_mute();
        }
        if sleep_changed {
            self.reset_sleep_timer();
        }
        // Faible latence changée depuis le menu : le flux est relancé tout de suite (comme un changement de qualité).
        restart |= latency_changed;
        if fs_toggle {
            self.toggle_fullscreen(ctx, !self.fullscreen);
        }
        if pip_toggle {
            self.toggle_pip(ctx);
        }
        if pin_toggle {
            self.pip_on_top = !self.pip_on_top;
            super::apply_on_top(ctx, self.pip_on_top);
        }
        if stop {
            self.stop_watch(ctx);
        } else if back {
            self.minimize_watch(ctx, View::Home);
        } else if settings {
            self.minimize_watch(ctx, View::Settings);
        } else if let Some(l) = rec_pick {
            self.start_watch(ctx, l);
        } else if retry {
            self.start_watch(ctx, login);
        } else if restart {
            self.restart_stream(ctx);
        } else if refresh {
            self.refresh_stream(ctx);
        }
        to_send
    }
}

/// Point séparateur entre les éléments d'une ligne d'informations (jeu · spectateurs · durée).
fn meta_dot(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(4.0, 13.0), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), 1.8, t::FAINT());
}

/// Partage de la zone du lecteur (mode normal) entre la vidéo et la carte d'infos dessous. La vidéo garde au moins
/// 120 px ; s'il ne reste pas la place de la carte (fenêtre minuscule, par exemple pendant sa restauration depuis la
/// barre des tâches), la carte n'est pas dessinée plutôt que de recevoir une hauteur négative.
fn split_video_info(avail: Rect) -> (Rect, Option<Rect>) {
    let video_bottom = (avail.bottom() - INFO_HEIGHT - GAP).max(avail.top() + 120.0);
    let video = Rect::from_min_max(avail.min, Pos2::new(avail.right(), video_bottom));
    let info = Rect::from_min_max(Pos2::new(avail.left(), video_bottom + GAP), avail.max);
    // Hauteur minimale de la carte : ses marges verticales (20 px) ; largeur minimale : ses marges horizontales.
    let fits = info.height() >= 20.0 && info.width() >= 48.0;
    (video, fits.then_some(info))
}

#[cfg(test)]
mod tests_disposition {
    use super::*;

    #[test]
    fn carte_d_infos_jamais_de_hauteur_negative() {
        let grande = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let (video, info) = split_video_info(grande);
        let info = info.expect("carte d'infos présente");
        assert!((info.height() - INFO_HEIGHT).abs() < 0.5, "{info:?}");
        assert!(video.bottom() + GAP <= info.top() + 0.5);
        // Fenêtre minuscule (restauration depuis la barre des tâches) : pas de carte, aucune taille négative.
        for h in [0.0, 24.0, 120.0, 150.0] {
            let petite = Rect::from_min_size(Pos2::ZERO, Vec2::new(160.0, h));
            let (video, info) = split_video_info(petite);
            assert!(video.height() >= 0.0);
            assert!(
                info.is_none_or(|r| r.height() >= 20.0 && r.width() >= 48.0),
                "hauteur {h} : {info:?}"
            );
        }
    }
}
