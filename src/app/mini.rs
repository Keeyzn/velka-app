//! Mini-lecteur : quand on quitte la page lecteur (Retour, Réglages), le live continue dans
//! une petite fenêtre flottante en bas à droite ; un clic la rouvre en grand, avec animation.
use super::{VelkaApp, View, WatchState};
use crate::ui::theme as t;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Sense, Vec2};
use std::time::{Duration, Instant};

/// Largeur d'origine (et minimale) du mini-lecteur ; il garde le format 16:9.
const MINI_W: f32 = 320.0;
/// Au-delà de cette part de la fenêtre (largeur ou hauteur), le mini-lecteur repasse en lecteur normal.
const MAX_SHARE: f32 = 0.85;
const MINI_MARGIN: f32 = 28.0;
const ANIM: Duration = Duration::from_millis(280);
/// Plein écran : entrée un peu plus longue que la sortie (on est pressé de reprendre la main en sortant).
const FS_IN: Duration = Duration::from_millis(220);
const FS_OUT: Duration = Duration::from_millis(180);
/// Attente maximale du changement de taille de la fenêtre avant de lancer l'animation de plein écran.
const FS_WAIT: Duration = Duration::from_millis(250);

/// Animation de passage entre la page lecteur et le mini-lecteur, ou vers / depuis le plein écran.
pub struct ViewAnim {
    start: Instant,
    dur: Duration,
    from: Rect,
    expanding: bool,
    /// Plein écran : taille de la fenêtre au moment de la demande et instant de la demande. Windows redimensionne la
    /// fenêtre un peu plus tard (parfois avec une saccade) : l'animation ne démarre qu'une fois la nouvelle taille
    /// reçue, pour que cette saccade ne la « mange » pas.
    wait: Option<(Vec2, Instant)>,
}

/// Mini-lecteur hors de sa place de repos : tenu par le pointeur (`held`) ou lancé, puis ramené vers son bord par un
/// ressort (avec l'élan du lancer).
pub struct MiniMotion {
    pos: Pos2,
    vel: Vec2,
    grab: Vec2,
    held: bool,
}

/// Partie du mini-lecteur saisie pour le redimensionner : l'angle, ou l'un des deux bords tournés vers le centre.
#[derive(Clone, Copy, PartialEq)]
enum ResizeMode {
    Corner,
    SideX,
    SideY,
}

/// Épaisseur de la zone de saisie des bords (de part et d'autre du trait) et taille de celle de l'angle.
const EDGE_HIT: f32 = 14.0;
const CORNER_HIT: f32 = 26.0;

/// Redimensionnement en cours : partie saisie, décalage de la prise (pointeur moins bord) et côté fixe, figés au début.
pub struct MiniResize {
    mode: ResizeMode,
    grab: Vec2,
    fixed: Pos2,
    left: bool,
    top: bool,
}

/// Événement de glisser de l'image courante.
enum Drag {
    Start(Vec2),
    /// Début d'un redimensionnement.
    ResizeStart(MiniResize),
}

/// Vrai si le pointeur est sur l'un des boutons : ils gardent le curseur « main qui pointe ».
fn pause_hover(ui: &egui::Ui, r: Rect, with_reset: bool) -> bool {
    let Some(p) = ui.ctx().input(|i| i.pointer.hover_pos()) else {
        return false;
    };
    pause_hover_at(r, p, with_reset)
}

/// Espace vide entre Fermer et Rétablir (même hauteur, sans déborder des deux boutons).
fn gap_rect(r: Rect) -> Rect {
    let (_, close, reset) = button_rects(r);
    Rect::from_min_max(
        Pos2::new(close.right(), close.top()),
        Pos2::new(reset.left(), close.bottom()),
    )
}

/// Vrai si `p` est sur l'un des boutons du mini-lecteur de rectangle `r`.
fn pause_hover_at(r: Rect, p: Pos2, with_reset: bool) -> bool {
    let (pause, close, reset) = button_rects(r);
    pause.contains(p)
        || close.contains(p)
        || (with_reset && (reset.contains(p) || gap_rect(r).contains(p)))
}

/// Côté des boutons Fermer et Rétablir.
const BTN: f32 = 36.0;

/// Boutons du mini-lecteur : pause au centre, fermer puis rétablir côte à côte en haut à gauche.
fn button_rects(r: Rect) -> (Rect, Rect, Rect) {
    let close = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 8.0), Vec2::splat(BTN));
    (
        Rect::from_center_size(r.center(), Vec2::splat(46.0)),
        close,
        close.translate(Vec2::new(BTN + 6.0, 0.0)),
    )
}

/// Adoucit un contour en coupant ses angles (Chaikin) : traits arrondis, comme les autres icônes de l'application.
fn smooth(pts: &[(f32, f32)], iterations: u8, closed: bool) -> Vec<Vec2> {
    let mut cur: Vec<Vec2> = pts.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
    for _ in 0..iterations {
        let n = cur.len();
        let segments = if closed { n } else { n.saturating_sub(1) };
        let mut out = Vec::with_capacity(segments * 2 + 2);
        if !closed {
            out.push(cur[0]);
        }
        for k in 0..segments {
            let (a, b) = (cur[k], cur[(k + 1) % n]);
            out.push(a * 0.75 + b * 0.25);
            out.push(a * 0.25 + b * 0.75);
        }
        if !closed {
            out.push(cur[n - 1]);
        }
        cur = out;
    }
    cur
}

/// Triangulation d'un polygone simple (découpe des « oreilles ») : sert à remplir la main, qui n'est pas convexe.
fn triangulate(poly: &[Vec2]) -> Vec<[u32; 3]> {
    let n = poly.len();
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let area: f32 = (0..n)
        .map(|i| poly[i].x * poly[(i + 1) % n].y - poly[(i + 1) % n].x * poly[i].y)
        .sum();
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let inside = |q: Vec2, a: Vec2, b: Vec2, c: Vec2| {
        cross(a, b, q) >= -1e-4 && cross(b, c, q) >= -1e-4 && cross(c, a, q) >= -1e-4
    };
    let mut tris = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            cross(poly[a], poly[b], poly[c]) > 1e-6
                && !idx.iter().any(|&k| {
                    k != a && k != b && k != c && inside(poly[k], poly[a], poly[b], poly[c])
                })
        });
        let Some(i) = ear else { break };
        tris.push([
            idx[(i + m - 1) % m] as u32,
            idx[i] as u32,
            idx[(i + 1) % m] as u32,
        ]);
        idx.remove(i);
    }
    if idx.len() == 3 {
        tris.push([idx[0] as u32, idx[1] as u32, idx[2] as u32]);
    }
    tris
}

/// Forme d'une main : contour adouci, triangles de remplissage et traits intérieurs (séparations des doigts).
struct HandShape {
    outline: Vec<Vec2>,
    tris: Vec<[u32; 3]>,
    lines: Vec<Vec<Vec2>>,
}

fn hand_shapes() -> &'static (HandShape, HandShape) {
    static SHAPES: std::sync::OnceLock<(HandShape, HandShape)> = std::sync::OnceLock::new();
    SHAPES.get_or_init(|| {
        const OPEN: [(f32, f32); 22] = [
            (-5.0, 10.0),
            (-6.0, 4.0),
            (-9.4, 0.8),
            (-10.0, -1.6),
            (-8.0, -2.2),
            (-6.0, 0.6),
            (-6.0, -8.5),
            (-3.5, -8.5),
            (-3.5, -3.0),
            (-2.83, -3.0),
            (-2.83, -11.0),
            (-0.33, -11.0),
            (-0.33, -3.0),
            (0.33, -3.0),
            (0.33, -10.0),
            (2.83, -10.0),
            (2.83, -3.0),
            (3.5, -3.0),
            (3.5, -7.0),
            (6.0, -7.0),
            (6.0, 4.0),
            (5.0, 10.0),
        ];
        const FIST: [(f32, f32); 13] = [
            (-5.0, 10.0),
            (-7.0, 3.0),
            (-7.6, -2.0),
            (-6.0, -5.6),
            (-3.6, -6.2),
            (-1.8, -5.4),
            (0.0, -6.4),
            (1.8, -5.4),
            (3.6, -6.2),
            (6.0, -5.6),
            (7.6, -2.0),
            (7.0, 3.0),
            (5.0, 10.0),
        ];
        let open = smooth(&OPEN, 2, true);
        let fist = smooth(&FIST, 2, true);
        let fist_lines = vec![
            smooth(&[(-2.6, -5.0), (-2.6, 0.5)], 2, false),
            smooth(&[(0.0, -5.6), (0.0, 0.5)], 2, false),
            smooth(&[(2.6, -5.0), (2.6, 0.5)], 2, false),
            smooth(&[(-7.3, 1.0), (-4.5, 3.4), (-1.5, 3.6)], 2, false),
        ];
        (
            HandShape {
                tris: triangulate(&open),
                outline: open,
                lines: Vec::new(),
            },
            HandShape {
                tris: triangulate(&fist),
                outline: fist,
                lines: fist_lines,
            },
        )
    })
}

/// Main pleine dessinée à la place du curseur système (Windows n'a pas de main qui attrape : « Grab » y donne une
/// croix) : remplissage blanc, contour sombre fin et arrondi. `closed` : poing fermé (le mini-lecteur est attrapé) ;
/// sinon main ouverte. Le point chaud est au centre de la paume.
pub(super) fn paint_hand(ctx: &egui::Context, at: Pos2, closed: bool) {
    let (open, fist) = hand_shapes();
    let shape = if closed { fist } else { open };
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("mini_hand"),
    ));
    let mut mesh = egui::epaint::Mesh::default();
    for q in &shape.outline {
        mesh.colored_vertex(at + *q, Color32::WHITE);
    }
    for t in &shape.tris {
        mesh.add_triangle(t[0], t[1], t[2]);
    }
    p.add(egui::Shape::mesh(mesh));
    let ink = egui::Stroke::new(1.4, Color32::from_black_alpha(235));
    p.add(egui::Shape::closed_line(
        shape.outline.iter().map(|q| at + *q).collect(),
        ink,
    ));
    for l in &shape.lines {
        p.add(egui::Shape::line(l.iter().map(|q| at + *q).collect(), ink));
    }
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t))
}

/// Taille du mini-lecteur pour la largeur demandée : 16:9, entre la taille d'origine et 85 % de la fenêtre.
fn mini_size(ctx: &egui::Context, width: f32) -> Vec2 {
    let scr = ctx.content_rect();
    let cap = (MAX_SHARE * scr.width())
        .min(MAX_SHARE * scr.height() * 16.0 / 9.0)
        .max(MINI_W);
    // Valeur illisible dans les réglages (fichier modifié à la main) : taille d'origine.
    let width = if width.is_finite() { width } else { MINI_W };
    let w = width.clamp(MINI_W, cap).round();
    Vec2::new(w, (w * 9.0 / 16.0).round())
}

/// Zone où peut se trouver le coin haut-gauche du mini-lecteur (marges de `MINI_MARGIN` sur les quatre côtés).
fn mini_travel(ctx: &egui::Context, size: Vec2) -> Rect {
    let screen = ctx.content_rect();
    let min = screen.min + Vec2::splat(MINI_MARGIN);
    let max = Pos2::new(
        screen.right() - MINI_MARGIN - size.x,
        screen.bottom() - MINI_MARGIN - size.y,
    );
    Rect::from_min_max(min, Pos2::new(max.x.max(min.x), max.y.max(min.y)))
}

fn mini_rect(ctx: &egui::Context, frac: [f32; 2], size: Vec2) -> Rect {
    let tr = mini_travel(ctx, size);
    let f = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            1.0
        }
    };
    let pos = Pos2::new(
        tr.left() + tr.width() * f(frac[0]),
        tr.top() + tr.height() * f(frac[1]),
    );
    Rect::from_min_size(pos, size)
}

/// Ramène une position (coin haut-gauche) sur le bord le plus proche : le mini-lecteur ne reste jamais au milieu.
fn snap_to_edge(ctx: &egui::Context, pos: Pos2, size: Vec2) -> [f32; 2] {
    let tr = mini_travel(ctx, size);
    let p = Pos2::new(
        pos.x.clamp(tr.left(), tr.right()),
        pos.y.clamp(tr.top(), tr.bottom()),
    );
    let norm = |v: f32, lo: f32, hi: f32| {
        if hi - lo > 0.5 {
            ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
        } else {
            1.0
        }
    };
    let (mut fx, mut fy) = (
        norm(p.x, tr.left(), tr.right()),
        norm(p.y, tr.top(), tr.bottom()),
    );
    let (dl, dr, dt, db) = (
        p.x - tr.left(),
        tr.right() - p.x,
        p.y - tr.top(),
        tr.bottom() - p.y,
    );
    // Aimant sur les coins : près d'un coin, l'autre coordonnée s'y colle aussi.
    let magnet = |f: f32| {
        if f < 0.12 {
            0.0
        } else if f > 0.88 {
            1.0
        } else {
            f
        }
    };
    fx = magnet(fx);
    fy = magnet(fy);
    let nearest = dl.min(dr).min(dt).min(db);
    if nearest == dl {
        fx = 0.0;
    } else if nearest == dr {
        fx = 1.0;
    } else if nearest == dt {
        fy = 0.0;
    } else {
        fy = 1.0;
    }
    [fx, fy]
}

impl VelkaApp {
    /// Le mini-lecteur est visible quand un live est ouvert mais qu'on est ailleurs.
    pub(super) fn mini_visible(&self) -> bool {
        // Pas pendant l'overlay des Réglages : il passerait par-dessus.
        self.watch.is_some() && matches!(self.view, View::Home | View::Multi)
    }

    /// Quitte la page lecteur sans arrêter le live.
    pub(super) fn minimize_watch(&mut self, ctx: &egui::Context, to: View) {
        if self.watch.is_none() {
            return;
        }
        // Un live interrompu n'a rien à montrer dans un mini-lecteur : on l'arrête.
        if self
            .watch
            .as_ref()
            .is_some_and(|w| matches!(w.state, WatchState::Error(_)))
        {
            self.stop_watch(ctx);
            self.view = to;
            return;
        }
        // Mini-lecteur désactivé : quitter la page arrête le live.
        if !self.settings.mini_player {
            self.stop_watch(ctx);
            self.view = to;
            return;
        }
        self.leave_immersive(ctx);
        // Réglages : simple overlay, la page du live reste fixe (aucune animation de réduction).
        self.anim = (to != View::Settings && crate::ui::theme::moves(ctx)).then(|| ViewAnim {
            start: Instant::now(),
            dur: ANIM,
            from: self.shown_video_rect,
            expanding: false,
            wait: None,
        });
        self.view = to;
    }

    /// Rouvre le live en grand.
    pub(super) fn restore_watch(&mut self, ctx: &egui::Context) {
        if self.watch.is_none() {
            return;
        }
        // Retour depuis les Réglages : le live n'a pas bougé, donc pas d'animation d'agrandissement.
        let from_settings = self.view == View::Settings;
        self.anim = (!from_settings && crate::ui::theme::moves(ctx)).then(|| ViewAnim {
            start: Instant::now(),
            dur: ANIM,
            from: self.mini_last.unwrap_or_else(|| {
                mini_rect(
                    ctx,
                    self.settings.mini_pos,
                    mini_size(ctx, self.settings.mini_width),
                )
            }),
            expanding: true,
            wait: None,
        });
        self.controls_seen = Instant::now();
        self.view = View::Watch;
    }

    /// Plein écran demandé par l'utilisateur (F, Échap, double-clic, bouton) : la vidéo grandit depuis sa place jusqu'à
    /// tout l'écran, ou rétrécit vers sa place en sortant (le chat apparaît en fondu). Les autres chemins (PiP, quitter
    /// la page) appellent `set_fullscreen` directement, sans animation.
    pub(super) fn toggle_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        // Point de départ : la vidéo telle qu'elle est dessinée (si une animation tourne encore, pas sa place finale).
        let rect = self.shown_video_rect;
        let animate = on != self.fullscreen
            && self.view == View::Watch
            && !self.pip
            && self.player.is_some()
            && crate::ui::theme::moves(ctx)
            && rect.width() > 1.0
            && rect.height() > 1.0;
        let wait = Some((ctx.content_rect().size(), Instant::now()));
        self.anim = animate.then(|| ViewAnim {
            start: Instant::now(),
            dur: if on { FS_IN } else { FS_OUT },
            from: rect,
            expanding: true,
            wait,
        });
        self.set_fullscreen(ctx, on);
    }

    /// Progression de l'agrandissement : rectangle de départ et avancement 0..1.
    pub(super) fn expand_progress(&mut self, ctx: &egui::Context) -> Option<(Rect, f32)> {
        let a = self.anim.as_mut().filter(|a| a.expanding)?;
        if let Some((old, asked)) = a.wait {
            if ctx.content_rect().size() == old && asked.elapsed() < FS_WAIT {
                // La fenêtre n'a pas encore changé de taille : la vidéo reste à sa place de départ.
                a.start = Instant::now();
                ctx.request_repaint();
                return Some((a.from, 0.0));
            }
            a.wait = None;
            a.start = Instant::now();
        }
        let k = a.start.elapsed().as_secs_f32() / a.dur.as_secs_f32();
        if k >= 1.0 {
            self.anim = None;
            return None;
        }
        ctx.request_repaint();
        Some((a.from, ease_out(k)))
    }

    /// Applique le redimensionnement tenu : largeur visée d'après le pointeur (moins la prise), format 16:9 conservé,
    /// côté opposé fixe. Plus de 85 % de la fenêtre : retour au lecteur normal.
    fn apply_mini_resize(&mut self, ctx: &egui::Context) {
        let Some(rs) = &self.mini_resize else { return };
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.mini_resize = None;
            return;
        }
        let Some(p) = ctx.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        let (tp, fixed, left, top, mode) = (p - rs.grab, rs.fixed, rs.left, rs.top, rs.mode);
        let scr = ctx.content_rect();
        let w = match mode {
            ResizeMode::Corner => (tp.x - fixed.x)
                .abs()
                .max((tp.y - fixed.y).abs() * 16.0 / 9.0),
            ResizeMode::SideX => (tp.x - fixed.x).abs(),
            ResizeMode::SideY => (tp.y - fixed.y).abs() * 16.0 / 9.0,
        };
        if w > MAX_SHARE * scr.width() || w * 9.0 / 16.0 > MAX_SHARE * scr.height() {
            let h = (w * 9.0 / 16.0).round();
            let min = Pos2::new(
                if left { fixed.x - w } else { fixed.x },
                if top { fixed.y - h } else { fixed.y },
            );
            self.mini_last = Some(Rect::from_min_size(min, Vec2::new(w, h)));
            self.mini_motion = None;
            self.mini_resize = None;
            self.restore_watch(ctx);
        } else {
            let ns = mini_size(ctx, w);
            let min = Pos2::new(
                if left { fixed.x - ns.x } else { fixed.x },
                if top { fixed.y - ns.y } else { fixed.y },
            );
            self.settings.mini_width = ns.x;
            // Place exacte (sans aimant) : le côté fixe ne doit pas bouger pendant qu'on tire.
            let tr = mini_travel(ctx, ns);
            let norm = |v: f32, lo: f32, hi: f32| {
                if hi - lo > 0.5 {
                    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            };
            self.settings.mini_pos = [
                norm(min.x, tr.left(), tr.right()),
                norm(min.y, tr.top(), tr.bottom()),
            ];
            self.mini_motion = None;
            ctx.request_repaint();
        }
    }

    /// Dessine le mini-lecteur (au-dessus de l'accueil ou des réglages).
    pub(super) fn mini_ui(&mut self, ctx: &egui::Context) {
        // Redimensionnement tenu : appliqué dès le début de l'image, d'après la position actuelle du pointeur (aucun retard).
        self.apply_mini_resize(ctx);
        // Retour à la taille d'origine : la largeur glisse vers sa valeur (même ressort que le déplacement).
        let target_w = self.settings.mini_width;
        let cur_w = match self.mini_w_anim {
            Some(w) => {
                let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
                let nw = w + (target_w - w) * (1.0 - (-dt * 12.0).exp());
                if (nw - target_w).abs() < 0.6 {
                    self.mini_w_anim = None;
                    target_w
                } else {
                    self.mini_w_anim = Some(nw);
                    ctx.request_repaint();
                    nw
                }
            }
            None => target_w,
        };
        let size = mini_size(ctx, cur_w);
        let target = mini_rect(ctx, self.settings.mini_pos, size);
        // Pendant la réduction, le rectangle glisse depuis la page lecteur.
        let mut progress = 1.0;
        let mut rect = target;
        if let Some(a) = self.anim.as_ref().filter(|a| !a.expanding) {
            let k = a.start.elapsed().as_secs_f32() / ANIM.as_secs_f32();
            if k >= 1.0 {
                self.anim = None;
            } else {
                progress = ease_out(k);
                rect = lerp_rect(a.from, target, progress);
                ctx.request_repaint();
            }
        }
        // Retour vers le bord après un lâcher : ressort amorti qui repart de la vitesse du lancer (inertie).
        if progress >= 1.0 {
            // Tenu : la position suit le pointeur dès le début de l'image (aucun retard, le curseur reste fixe sur l'image).
            if ctx.input(|i| i.pointer.primary_pressed()) {
                self.mini_dragged = false;
            }
            if self.mini_motion.as_ref().is_some_and(|m| m.held)
                && !ctx.input(|i| i.pointer.primary_down())
            {
                // Lâcher : retour vers le bord le plus proche, avec l'élan du lancer.
                if let Some(m) = self.mini_motion.as_mut() {
                    m.held = false;
                    self.settings.mini_pos = snap_to_edge(ctx, m.pos + m.vel * 0.25, size);
                }
            }
            if let Some(m) = self.mini_motion.as_mut().filter(|m| m.held) {
                m.vel = ctx.input(|i| i.pointer.velocity());
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    let scr = ctx.content_rect();
                    let pos = p - m.grab;
                    m.pos = Pos2::new(
                        pos.x
                            .clamp(scr.left(), (scr.right() - size.x).max(scr.left())),
                        pos.y
                            .clamp(scr.top(), (scr.bottom() - size.y).max(scr.top())),
                    );
                }
            }
            if let Some(m) = self.mini_motion.as_mut().filter(|m| !m.held) {
                let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
                let (k, c) = (170.0_f32, 2.0 * 170.0_f32.sqrt() * 0.78);
                let acc = (target.min - m.pos) * k - m.vel * c;
                m.vel += acc * dt;
                m.pos += m.vel * dt;
                if (target.min - m.pos).length() < 0.4 && m.vel.length() < 8.0 {
                    self.mini_motion = None;
                } else {
                    ctx.request_repaint();
                }
            }
            if let Some(m) = &self.mini_motion {
                rect = Rect::from_min_size(m.pos, size);
            }
        }
        self.mini_last = Some(rect);
        let Some(w) = &self.watch else { return };
        let state = w.state.clone();
        let (mut restore, mut stop, mut pause, mut reset) = (false, false, false, false);
        let at_default = self.settings.mini_pos == [1.0, 1.0]
            && (self.settings.mini_width - MINI_W).abs() < 0.5
            && self.mini_w_anim.is_none()
            && self.mini_motion.is_none();
        let mut drag: Option<Drag> = None;
        let held_now = self.mini_motion.as_ref().is_some_and(|m| m.held);
        let paused = self.paused;

        egui::Area::new(egui::Id::unique("mini_player"))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let _dark = t::DarkScope::enter(ui);
                let (r, resp) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
                t::focus_ring(ui, &resp, r, 6);
                // Ombre portée : le mini-lecteur se détache des cartes de l'accueil qu'il recouvre.
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 1,
                        color: Color32::from_black_alpha(150),
                    }
                    .as_shape(r, CornerRadius::same(6)),
                );
                ui.painter()
                    .rect_filled(r, CornerRadius::same(6), Color32::BLACK);
                if let Some(p) = &self.player {
                    p.paint(ui, r);
                }
                ui.painter().rect_stroke(
                    r.expand(1.0),
                    CornerRadius::same(6),
                    egui::Stroke::new(1.0, t::TEXT().gamma_multiply(0.35)),
                    egui::StrokeKind::Outside,
                );
                // Redimensionnement : bords et angle tournés vers le centre de la fenêtre (le côté opposé reste en place,
                // le format 16:9 est toujours conservé).
                let scr = ctx.content_rect();
                let handle_left = r.center().x > scr.center().x;
                let handle_top = r.center().y > scr.center().y;
                let fixed = Pos2::new(
                    if handle_left { r.right() } else { r.left() },
                    if handle_top { r.bottom() } else { r.top() },
                );
                let hx = if handle_left { r.left() } else { r.right() };
                let hy = if handle_top { r.top() } else { r.bottom() };
                let h = EDGE_HIT / 2.0;
                let zones = [
                    // Bords d'abord (dessous), angle en dernier (dessus).
                    (
                        ResizeMode::SideX,
                        Rect::from_min_max(
                            Pos2::new(hx - h, r.top()),
                            Pos2::new(hx + h, r.bottom()),
                        ),
                        Pos2::new(hx, r.center().y),
                    ),
                    (
                        ResizeMode::SideY,
                        Rect::from_min_max(
                            Pos2::new(r.left(), hy - h),
                            Pos2::new(r.right(), hy + h),
                        ),
                        Pos2::new(r.center().x, hy),
                    ),
                    (
                        ResizeMode::Corner,
                        Rect::from_min_max(
                            Pos2::new(
                                if handle_left { hx - h } else { hx - CORNER_HIT },
                                if handle_top { hy - h } else { hy - CORNER_HIT },
                            ),
                            Pos2::new(
                                if handle_left { hx + CORNER_HIT } else { hx + h },
                                if handle_top { hy + CORNER_HIT } else { hy + h },
                            ),
                        ),
                        Pos2::new(hx, hy),
                    ),
                ];
                let mut resizing = false;
                let mut on_handle = false;
                for (mode, zr, feature) in zones {
                    let id = egui::Id::unique("mini_resize").with(mode as u8);
                    let zresp = ui.interact(zr, id, Sense::drag());
                    if zresp.drag_started() {
                        if let Some(p) = ctx.input(|i| i.pointer.press_origin()) {
                            drag = Some(Drag::ResizeStart(MiniResize {
                                mode,
                                grab: p - feature,
                                fixed,
                                left: handle_left,
                                top: handle_top,
                            }));
                        }
                    }
                    if zresp.dragged() {
                        resizing = true;
                    }
                    if zresp.hovered() || zresp.dragged() {
                        on_handle = true;
                        ctx.set_cursor_icon(match mode {
                            ResizeMode::SideX => egui::CursorIcon::ResizeHorizontal,
                            ResizeMode::SideY => egui::CursorIcon::ResizeVertical,
                            ResizeMode::Corner if handle_left == handle_top => {
                                egui::CursorIcon::ResizeNwSe
                            }
                            ResizeMode::Corner => egui::CursorIcon::ResizeNeSw,
                        });
                        // Repère visible : le bord (ou l'angle) qui va bouger.
                        let ink = egui::Stroke::new(2.0, t::ACCENT());
                        match mode {
                            ResizeMode::SideX => {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(hx, r.top() + 6.0),
                                        Pos2::new(hx, r.bottom() - 6.0),
                                    ],
                                    ink,
                                );
                            }
                            ResizeMode::SideY => {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(r.left() + 6.0, hy),
                                        Pos2::new(r.right() - 6.0, hy),
                                    ],
                                    ink,
                                );
                            }
                            ResizeMode::Corner => {
                                let (dx, dy) = (
                                    if handle_left { 1.0 } else { -1.0 },
                                    if handle_top { 1.0 } else { -1.0 },
                                );
                                ui.painter().line_segment(
                                    [Pos2::new(hx, hy + dy * 18.0), Pos2::new(hx, hy)],
                                    ink,
                                );
                                ui.painter().line_segment(
                                    [Pos2::new(hx, hy), Pos2::new(hx + dx * 18.0, hy)],
                                    ink,
                                );
                            }
                        }
                    }
                }
                // Glisser : le mini-lecteur suit le pointeur partout dans la fenêtre ; au lâcher il revient au bord.
                // (Rien de tout cela pendant le redimensionnement ni sur un bord.)
                // Seuil de 4 px : en dessous, le relâchement reste un clic (retour au grand lecteur) ; au-delà, déplacement.
                if !resizing && !on_handle && !held_now && resp.is_pointer_button_down_on() {
                    let (origin, now) =
                        ctx.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos()));
                    if let (Some(o), Some(p)) = (origin, now) {
                        if r.contains(o)
                            && (p - o).length() > 4.0
                            && !pause_hover_at(r, o, !at_default)
                        {
                            drag = Some(Drag::Start(o - r.min));
                        }
                    }
                }
                // État seulement : le nom de la chaîne n'est plus affiché dans le mini-lecteur.
                let mut label = String::new();
                match &state {
                    WatchState::Checking | WatchState::Starting => {
                        label.push_str(crate::i18n::tr("connexion…"))
                    }
                    WatchState::Switching => {
                        label.push_str(crate::i18n::tr("changement de qualité…"))
                    }
                    WatchState::Refreshing => {
                        label.push_str(crate::i18n::tr("Actualisation en cours"))
                    }
                    WatchState::Error(_) => label.push_str(crate::i18n::tr("interrompu")),
                    WatchState::Playing => {}
                }
                let failed = matches!(state, WatchState::Error(_));
                if let WatchState::Error(msg) = &state {
                    let text_rect = Rect::from_min_size(
                        r.min + Vec2::new(12.0, 44.0),
                        Vec2::new(r.width() - 24.0, r.height() - 56.0),
                    );
                    let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
                    text_ui.add(
                        egui::Label::new(RichText::new(msg).size(12.5).color(t::NEGATIVE())).wrap(),
                    );
                }
                let pill = Rect::from_min_size(
                    r.min + Vec2::new(98.0, 10.0),
                    Vec2::new((r.width() - 98.0 - 8.0).max(10.0), 24.0),
                );
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pill));
                if !label.is_empty() {
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(150))
                        .corner_radius(CornerRadius::same(t::RADIUS_PILL))
                        .inner_margin(egui::Margin::symmetric(10, 3))
                        .show(&mut child, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(label).size(12.0).color(t::TEXT()))
                                    .truncate(),
                            );
                        });
                }
                // Boutons au survol.
                let held = held_now || matches!(drag, Some(Drag::Start(_)));
                if !held && (failed || resp.hovered() || ui.rect_contains_pointer(r)) {
                    // Pause au centre de la vidéo, arrêt (rouge) en haut à droite.
                    let (icon, tip) = if paused {
                        (t::Icon::Play, crate::i18n::tr("Lecture"))
                    } else {
                        (t::Icon::Pause, "Pause")
                    };
                    let pause_rect = Rect::from_center_size(r.center(), Vec2::splat(46.0));
                    let mut pause_ui = ui.new_child(egui::UiBuilder::new().max_rect(pause_rect));
                    // Les actions partent dès l'appui (jamais de début de déplacement depuis un bouton).
                    let press = ctx.input(|i| i.pointer.primary_pressed());
                    pause = {
                        let b =
                            t::icon_button(&mut pause_ui, icon, 46.0, t::IconStyle::Overlay, tip);
                        b.hovered() && press
                    };
                    let (_, close_rect, reset_rect) = button_rects(r);
                    let mut close_ui = ui.new_child(egui::UiBuilder::new().max_rect(close_rect));
                    stop = {
                        let b = t::icon_button(
                            &mut close_ui,
                            t::Icon::Close,
                            BTN,
                            t::IconStyle::AccentDanger,
                            crate::i18n::tr("Arrêter"),
                        );
                        b.hovered() && press
                    };
                    // Retour en un clic à la taille et à la place d'origine (visible seulement s'il y a quelque chose à rétablir).
                    if !at_default {
                        let mut reset_ui =
                            ui.new_child(egui::UiBuilder::new().max_rect(reset_rect));
                        reset = {
                            let b = t::icon_button(
                                &mut reset_ui,
                                t::Icon::Reset,
                                BTN,
                                t::IconStyle::AccentLight,
                                crate::i18n::tr("Taille et place d'origine"),
                            );
                            b.hovered() && press
                        };
                        // Zone tampon invisible entre les deux boutons : ni main de saisie, ni glisser, ni clic.
                        ui.interact(
                            gap_rect(r),
                            egui::Id::unique("mini_gap"),
                            Sense::click_and_drag(),
                        );
                    }
                }
                if resp.clicked() && !pause && !stop && !reset && !self.mini_dragged {
                    restore = true;
                }
                // Curseur : main ouverte au survol, main qui attrape tant qu'il est tenu (même hors du cadre).
                // Le poing se ferme dès l'appui (sans attendre le début du déplacement).
                let over_button = pause_hover(ui, r, !at_default);
                let pressed = resp.is_pointer_button_down_on() && !over_button && !on_handle;
                let over_gap = !at_default
                    && ctx
                        .input(|i| i.pointer.hover_pos())
                        .is_some_and(|p| gap_rect(r).contains(p));
                if over_gap && !held {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                } else if over_button && !held {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                } else if held || pressed {
                    ctx.set_cursor_icon(egui::CursorIcon::None);
                    if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                        paint_hand(ctx, p, true);
                    }
                } else if resp.hovered() && !on_handle {
                    // Sur la vidéo : main ouverte dès le survol (elle se ferme à l'appui).
                    ctx.set_cursor_icon(egui::CursorIcon::None);
                    if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                        paint_hand(ctx, p, false);
                    }
                }
            });
        let _ = progress;

        match drag {
            Some(Drag::Start(grab)) => {
                self.mini_dragged = true;
                let pos = ctx
                    .input(|i| i.pointer.interact_pos())
                    .map_or(rect.min, |p| p - grab);
                self.mini_motion = Some(MiniMotion {
                    pos,
                    vel: Vec2::ZERO,
                    grab,
                    held: true,
                });
            }
            Some(Drag::ResizeStart(rs)) => {
                self.mini_w_anim = None;
                self.mini_motion = None;
                self.mini_resize = Some(rs);
                ctx.request_repaint();
            }
            None => {}
        }
        if reset {
            self.mini_w_anim = Some(size.x);
            self.settings.mini_width = MINI_W;
            self.settings.mini_pos = [1.0, 1.0];
            self.mini_motion = Some(MiniMotion {
                pos: rect.min,
                vel: Vec2::ZERO,
                grab: Vec2::ZERO,
                held: false,
            });
            ctx.request_repaint();
        }
        // Curseur de redimensionnement fixe tant que le bord ou l'angle est tenu (même hors de la zone).
        if let Some(rs) = &self.mini_resize {
            ctx.set_cursor_icon(match rs.mode {
                ResizeMode::SideX => egui::CursorIcon::ResizeHorizontal,
                ResizeMode::SideY => egui::CursorIcon::ResizeVertical,
                ResizeMode::Corner if rs.left == rs.top => egui::CursorIcon::ResizeNwSe,
                ResizeMode::Corner => egui::CursorIcon::ResizeNeSw,
            });
        }
        if pause {
            self.toggle_pause();
        }
        if stop {
            self.stop_watch(ctx);
        } else if restore {
            self.restore_watch(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(p: &[Vec2]) -> f32 {
        (0..p.len())
            .map(|i| p[i].x * p[(i + 1) % p.len()].y - p[(i + 1) % p.len()].x * p[i].y)
            .sum::<f32>()
            .abs()
            / 2.0
    }

    /// Le remplissage de la main (triangles) couvre toute la surface du contour : aucun trou, aucun débordement.
    #[test]
    fn main_remplie_sans_trou() {
        let (open, fist) = hand_shapes();
        for s in [open, fist] {
            let tris: f32 = s
                .tris
                .iter()
                .map(|t| {
                    area(&[
                        s.outline[t[0] as usize],
                        s.outline[t[1] as usize],
                        s.outline[t[2] as usize],
                    ])
                })
                .sum();
            let full = area(&s.outline);
            assert!(
                (tris - full).abs() / full < 0.02,
                "triangles {tris} / contour {full}"
            );
        }
    }
}
