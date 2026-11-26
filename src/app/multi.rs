//! Multistream : jusqu'à 4 lives en même temps sur une seule page, chacun avec son lecteur (libmpv) et son Streamlink.
//! Un seul live a le son à la fois (clic sur une vignette pour l'écouter) ; les autres sont muets.
use super::*;
use crate::ui::theme as t;
use eframe::egui::{Color32, CornerRadius, Pos2, Rect, RichText, Sense, Vec2};

/// Nombre maximal de lives affichés ensemble.
pub(super) const MULTI_MAX: usize = 4;
/// Écart entre deux vignettes (points).
const TILE_GAP: f32 = 12.0;
/// Arrondi du fond des vignettes (chargement, message) ; la vidéo et son contour restent à angles droits.
const TILE_RADIUS: u8 = 10;
/// Durée d'affichage d'un message sous le champ (chaîne déjà affichée, maximum atteint).
const MSG_SHOWN: Duration = Duration::from_secs(4);
/// Chaînes en direct proposées sous le champ, au plus.
const QUICK_MAX: usize = 10;
/// Durée d'affichage du contour d'accent après un changement de son, puis durée de son fondu.
const OUTLINE_SHOWN: f32 = 5.0;
const OUTLINE_FADE: f32 = 0.4;
/// Hauteur des boutons de la barre d'une vignette.
const TILE_BTN: f32 = 30.0;
/// Largeur du curseur de volume d'une vignette (masqué si la vignette est trop étroite).
const TILE_SLIDER_W: f32 = 90.0;
/// Déplacement du pointeur (points), bouton maintenu sur un live, au-delà duquel le live s'attrape (en deçà, le
/// relâchement est un clic : il donne le son).
const DRAG_SLOP: f32 = 6.0;
/// Décalage du poing dessiné par rapport au point du pointeur : le curseur habituel (main qui pointe) a son point au
/// bout du doigt, en haut de l'icône ; le poing, centré sur sa paume, paraissait sinon remonter au moment d'attraper.
const FIST_OFFSET: Vec2 = Vec2::new(3.0, 8.0);
/// Durée pendant laquelle les vignettes glissent vers leur nouvelle place après un échange.
const SWAP_GLIDE: Duration = Duration::from_millis(700);

/// État d'une vignette.
#[derive(PartialEq, Clone)]
pub(super) enum SlotState {
    /// Statut de la chaîne demandé à Twitch.
    Checking,
    /// Streamlink lancé, flux pas encore prêt.
    Starting,
    /// Flux relancé après un changement de qualité.
    Switching,
    Playing,
    Error(String),
}

/// Un live du multistream.
pub(super) struct Slot {
    /// Numéro unique, jamais réutilisé : les messages d'un live déjà retiré sont ignorés.
    id: u64,
    login: String,
    state: SlotState,
    player: Option<Player>,
    /// `Drop` arrête Streamlink.
    stream: Option<Stream>,
    /// Volume propre à ce live (0 à `volume_max`), au départ celui des Réglages.
    volume: u8,
    /// Format de l'image (largeur / hauteur) et heure de sa dernière lecture dans mpv.
    ratio: f32,
    ratio_at: Option<Instant>,
}

/// Action demandée depuis une vignette (appliquée après le dessin).
enum TileAction {
    Remove,
    /// Donne le son à ce live (en plus d'un autre si le réglage le permet).
    Listen,
    /// Coupe le son de ce live.
    Mute,
    Volume(u8),
}

/// Lives qui ont le son après un clic sur `id` : `id` seul, ou ajouté à ceux qui l'ont déjà (réglage « Cumuler le son
/// des lives en Multistream », jusqu'à tous les lives affichés).
fn sounds_after_listen(current: &[u64], id: u64, mix: bool) -> Vec<u64> {
    if !mix {
        return vec![id];
    }
    let mut v: Vec<u64> = current.iter().copied().filter(|x| *x != id).collect();
    v.push(id);
    v
}

/// Rectangles des vignettes pour `n` lives dans `area` : 1 = toute la place ; 2 = côte à côte (l'un sous l'autre si la
/// zone est haute) ; 3 ou 4 = grille 2 x 2 (avec 3, le dernier est centré sur sa ligne). Toutes les vignettes ont la
/// même taille, au format 16:9, et l'ensemble est centré dans la zone.
pub(super) fn tile_rects(area: Rect, n: usize) -> Vec<Rect> {
    let (cols, rows): (usize, usize) = match n {
        0 => return Vec::new(),
        1 => (1, 1),
        2 if area.width() >= area.height() * 1.2 => (2, 1),
        2 => (1, 2),
        _ => (2, 2),
    };
    let (cf, rf) = (cols as f32, rows as f32);
    let cell = Vec2::new(
        ((area.width() - TILE_GAP * (cf - 1.0)) / cf).max(1.0),
        ((area.height() - TILE_GAP * (rf - 1.0)) / rf).max(1.0),
    );
    let ratio = 16.0 / 9.0;
    let size = if cell.x / cell.y > ratio {
        Vec2::new(cell.y * ratio, cell.y)
    } else {
        Vec2::new(cell.x, cell.x / ratio)
    };
    let total = Vec2::new(
        size.x * cf + TILE_GAP * (cf - 1.0),
        size.y * rf + TILE_GAP * (rf - 1.0),
    );
    let origin = area.center() - total / 2.0;
    (0..n.min(cols * rows))
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            // Ligne incomplète (3 lives) : ses vignettes sont centrées.
            let on_row = (n - r * cols).min(cols);
            let shift = (cols - on_row) as f32 * (size.x + TILE_GAP) / 2.0;
            let min = origin
                + Vec2::new(
                    shift + c as f32 * (size.x + TILE_GAP),
                    r as f32 * (size.y + TILE_GAP),
                );
            Rect::from_min_size(min.round(), size)
        })
        .collect()
}

/// Place affichée d'une vignette. Juste après un échange (`gliding`), elle glisse de sa place précédente vers la
/// nouvelle ; sinon (et animations coupées, ou taille changée : fenêtre, live ajouté ou retiré) elle y est directement.
fn glide(ctx: &egui::Context, id: u64, target: Rect, gliding: bool) -> Rect {
    let key = egui::Id::unique(("multi_glide", id));
    let prev: Option<Rect> = ctx.data(|d| d.get_temp(key));
    let shown = match prev {
        Some(p)
            if gliding
                && t::moves(ctx)
                && (p.size() - target.size()).length() < 0.5
                && p.min.distance(target.min) > 0.5 =>
        {
            let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
            let k = 1.0 - (-dt * 14.0).exp();
            ctx.request_repaint();
            Rect::from_min_size(p.min + (target.min - p.min) * k, target.size())
        }
        _ => target,
    };
    ctx.data_mut(|d| d.insert_temp(key, shown));
    shown
}

/// Rectangle calé sur les pixels de l'écran : coin sur un pixel, taille en pixels entiers. Sans cela, pendant un
/// glissement (et avec une taille d'interface de 110 %), la taille en pixels de la vidéo varie d'un pixel d'une image à
/// l'autre selon la position (arrondis), et libmpv recalcule ses filtres à chaque variation : petites saccades.
fn snap(r: Rect, ppp: f32) -> Rect {
    if ppp <= 0.0 {
        return r;
    }
    let min = (r.min.to_vec2() * ppp).round() / ppp;
    let size = (r.size() * ppp).floor() / ppp;
    Rect::from_min_size(min.to_pos2(), size)
}

/// Vignette visée pendant un déplacement : celle sous le pointeur, autre que le live tenu.
fn drop_target(tiles: &[(u64, Rect)], held: u64, pointer: Pos2) -> Option<(u64, Rect)> {
    tiles
        .iter()
        .copied()
        .find(|(id, r)| *id != held && r.contains(pointer))
}

impl VelkaApp {
    /// Libère des vignettes sans figer l'interface : chaque lecteur est coupé tout de suite (son et image), l'arrêt des
    /// Streamlink (qui attend la fin de chaque processus) se fait dans un fil à part, et les lecteurs sont détruits un
    /// par image (`multi_free_one`), la fin de leur destruction se faisant elle aussi dans un fil à part.
    fn multi_release(&mut self, slots: Vec<Slot>) {
        let mut streams: Vec<Stream> = Vec::new();
        for mut s in slots {
            if let Some(st) = s.stream.take() {
                streams.push(st);
            }
            if let Some(mut p) = s.player.take() {
                let _ = p.set_property("mute", "yes");
                p.stop();
                p.destroy_in_background();
                self.multi_dying.push(p);
            }
        }
        if !streams.is_empty() {
            std::thread::spawn(move || drop(streams));
        }
    }

    /// Fermeture de l'application : arrête tout de suite les Streamlink des vignettes (sans toucher aux lecteurs).
    pub(super) fn multi_kill_streams(&mut self) {
        for s in &mut self.multi {
            s.stream = None;
        }
    }

    /// Détruit un lecteur libéré (au plus un par image, contexte OpenGL courant : début de `ui`).
    pub(super) fn multi_free_one(&mut self, ctx: &egui::Context) {
        if self.multi_dying.pop().is_some() && !self.multi_dying.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Ajoute une chaîne au multistream (au plus `MULTI_MAX`). Un live du lecteur principal ou du mini-lecteur est
    /// arrêté d'abord : sinon deux sons joueraient ensemble.
    pub(super) fn multi_add(&mut self, ctx: &egui::Context, login: String) {
        if self.multi.iter().any(|s| s.login == login) {
            self.multi_msg = Some((Instant::now(), "Cette chaîne est déjà affichée."));
            return;
        }
        if self.multi.len() >= MULTI_MAX {
            self.multi_msg = Some((Instant::now(), "4 lives au maximum."));
            return;
        }
        if self.watch.is_some() {
            self.stop_watch(ctx);
        }
        self.multi_msg = None;
        self.multi_next += 1;
        let id = self.multi_next;
        let mut slot = Slot {
            id,
            login: login.clone(),
            state: SlotState::Checking,
            player: None,
            stream: None,
            volume: self.settings.volume.min(self.settings.volume_max),
            ratio: 16.0 / 9.0,
            ratio_at: None,
        };
        match self.multi_player(ctx) {
            Ok(p) => {
                slot.player = Some(p);
                let (tx, ctx) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let st = twitch::fetch_status(std::slice::from_ref(&login))
                        .remove(&login)
                        .unwrap_or(LiveStatus::Unknown);
                    if st == LiveStatus::Unknown && !crate::net::internet_up() {
                        let _ = tx.send(Msg::MultiStream(
                            id,
                            StreamEvent::Failed(StreamError::NoInternet),
                        ));
                    } else {
                        let _ = tx.send(Msg::MultiStatus(id, st));
                    }
                    ctx.request_repaint();
                });
            }
            Err(e) => slot.state = SlotState::Error(e),
        }
        crate::logging::log(&format!("Multistream : {} ajoutée", slot.login));
        // Le premier live a le son ; les suivants arrivent muets.
        if self.multi.is_empty() {
            self.multi_audio = vec![id];
        }
        self.multi.push(slot);
    }

    /// Lecteur d'une vignette (contexte OpenGL courant : appelé pendant `ui`). Pas de journal `mpv.log` ici.
    fn multi_player(&self, ctx: &egui::Context) -> Result<Player, String> {
        let dll = tools::detect(Tool::Libmpv, &self.settings.libmpv_path).ok_or(
            crate::i18n::tr("libmpv est introuvable. Réinstallez les outils depuis les réglages."),
        )?;
        let get_proc = self
            .get_proc
            .clone()
            .ok_or(crate::i18n::tr("Le rendu OpenGL n'est pas disponible."))?;
        let player = Player::new(get_proc, ctx, &dll, None, &crate::mpv::default_options())?;
        player.set_background(Color32::BLACK);
        Ok(player)
    }

    /// Statut reçu : hors ligne ou inexistante = message dans la vignette ; sinon Streamlink est lancé.
    pub(super) fn on_multi_status(&mut self, ctx: &egui::Context, id: u64, st: LiveStatus) {
        let error = match st {
            LiveStatus::Offline => Some(crate::i18n::tr("La chaîne est hors ligne.")),
            LiveStatus::Missing => Some(crate::i18n::tr("Cette chaîne n'existe pas.")),
            _ => None,
        };
        if let Some(m) = error {
            if let Some(s) = self.multi.iter_mut().find(|s| s.id == id) {
                s.state = SlotState::Error(m.to_string());
            }
            return;
        }
        self.multi_begin_stream(ctx, id, SlotState::Starting);
    }

    /// Lance le Streamlink d'une vignette (mêmes arguments que le lecteur principal, qualité choisie sur la page).
    /// `shown` : état affiché pendant le démarrage (connexion ou changement de qualité).
    fn multi_begin_stream(&mut self, ctx: &egui::Context, id: u64, shown: SlotState) {
        let Some(login) = self
            .multi
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.login.clone())
        else {
            return;
        };
        let result: Result<Stream, String> = (|| {
            let exe = tools::detect(Tool::Streamlink, &self.settings.streamlink_path).ok_or(
                crate::i18n::tr(
                    "Streamlink est introuvable. Réinstallez les outils depuis les réglages.",
                ),
            )?;
            let port =
                stream::free_port().ok_or(crate::i18n::tr("Aucun port local disponible."))?;
            let s = &self.settings;
            let args = stream::build_args(&login, s.multi_quality, s.low_latency, port);
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            Stream::start(&exe, &args, port, move |ev| {
                let _ = tx.send(Msg::MultiStream(id, ev));
                ctx.request_repaint();
            })
        })();
        if let Some(s) = self.multi.iter_mut().find(|s| s.id == id) {
            match result {
                Ok(st) => {
                    s.stream = Some(st);
                    s.state = shown;
                }
                Err(e) => s.state = SlotState::Error(e),
            }
        }
    }

    /// Qualité changée sur la page : les lives en cours (ou en démarrage) sont relancés dans la nouvelle qualité. Ceux
    /// dont le statut est encore attendu la prendront à leur démarrage ; une chaîne hors ligne reste telle quelle.
    fn multi_restart_streams(&mut self, ctx: &egui::Context) {
        let ids: Vec<u64> = self
            .multi
            .iter()
            .filter(|s| {
                matches!(
                    s.state,
                    SlotState::Starting | SlotState::Switching | SlotState::Playing
                )
            })
            .map(|s| s.id)
            .collect();
        for id in ids {
            if let Some(s) = self.multi.iter_mut().find(|s| s.id == id) {
                if let Some(old) = s.stream.take() {
                    std::thread::spawn(move || drop(old));
                }
                if let Some(p) = &s.player {
                    p.stop();
                }
            }
            self.multi_begin_stream(ctx, id, SlotState::Switching);
        }
    }

    /// Événement du Streamlink d'une vignette : flux prêt (lecture), pub (ignorée ici), arrêt (message).
    pub(super) fn on_multi_stream(&mut self, id: u64, ev: StreamEvent) {
        let audible = self.multi_audio.contains(&id);
        let (low_latency, hw) = (self.settings.low_latency, self.settings.hw_decode.min(2));
        let Some(s) = self.multi.iter_mut().find(|s| s.id == id) else {
            return;
        };
        match ev {
            StreamEvent::Ready(port) => {
                let Some(p) = &s.player else {
                    return;
                };
                p.set_volume(s.volume);
                let _ = p.set_property("mute", if audible { "no" } else { "yes" });
                let _ = p.set_property(
                    "profile",
                    if low_latency {
                        "low-latency"
                    } else {
                        "default"
                    },
                );
                // Décodage matériel : « Automatique » garde la valeur de `mpv::MPV_OPTIONS` (comme le lecteur principal).
                match hw {
                    1 => {
                        let _ = p.set_property("hwdec", "auto");
                    }
                    2 => {
                        let _ = p.set_property("hwdec", "no");
                    }
                    _ => {}
                }
                s.state = match p.load(&format!("http://127.0.0.1:{port}/")) {
                    Ok(()) => SlotState::Playing,
                    Err(e) => SlotState::Error(e),
                };
            }
            StreamEvent::Break(_) => {}
            StreamEvent::Failed(e) => {
                s.stream = None;
                if let Some(p) = &s.player {
                    p.stop();
                }
                let was_playing = s.state == SlotState::Playing;
                s.state = SlotState::Error(if e == StreamError::NoInternet && was_playing {
                    crate::i18n::tr("La connexion Internet s'est coupée.").to_string()
                } else {
                    e.message()
                });
            }
        }
    }

    /// Retire une vignette (Streamlink arrêté, lecteur libéré). Si elle avait le son, il passe à la première restante.
    fn multi_remove(&mut self, id: u64) {
        let (gone, kept) = std::mem::take(&mut self.multi)
            .into_iter()
            .partition(|s| s.id == id);
        self.multi = kept;
        self.multi_release(gone);
        let had_sound = self.multi_audio.contains(&id);
        self.multi_audio.retain(|x| *x != id);
        // Plus aucun son : il passe au premier live restant.
        if had_sound && self.multi_audio.is_empty() {
            if let Some(n) = self.multi.first().map(|s| s.id) {
                self.multi_audio = vec![n];
                self.multi_apply_sounds();
            }
        }
    }

    /// Arrête tous les lives du multistream et oublie ceux mis de côté (« Tout arrêter », redémarrage, désinstallation).
    pub(super) fn multi_stop_all(&mut self) {
        if !self.multi.is_empty() {
            crate::logging::log("Multistream : tous les lives arrêtés");
        }
        let all = std::mem::take(&mut self.multi);
        self.multi_release(all);
        self.multi_audio.clear();
        self.multi_saved.clear();
        self.multi_saved_audio.clear();
    }

    /// Arrête les lives du multistream en les mettant de côté (live ouvert seul, fenêtre cachée) : ils sont relancés
    /// tous ensemble au retour sur la page « Multistream », le son sur le même live.
    pub(super) fn multi_suspend(&mut self) {
        if self.multi.is_empty() {
            return;
        }
        crate::logging::log("Multistream mis de côté : relancé au retour sur la page");
        self.multi_saved = self.multi.iter().map(|s| s.login.clone()).collect();
        self.multi_saved_audio = self
            .multi
            .iter()
            .filter(|s| self.multi_audio.contains(&s.id))
            .map(|s| s.login.clone())
            .collect();
        let all = std::mem::take(&mut self.multi);
        self.multi_release(all);
        self.multi_audio.clear();
    }

    /// Relance les lives mis de côté (page « Multistream » rouverte), un par image pour ne pas figer l'interface ; une
    /// fois le dernier relancé, le son revient sur les mêmes lives qu'avant.
    fn multi_resume_step(&mut self, ctx: &egui::Context) {
        if self.multi_saved.is_empty() {
            return;
        }
        let login = self.multi_saved.remove(0);
        self.multi_add(ctx, login);
        if self.multi_saved.is_empty() {
            let audio = std::mem::take(&mut self.multi_saved_audio);
            self.multi_audio = self
                .multi
                .iter()
                .filter(|s| audio.contains(&s.login))
                .map(|s| s.id)
                .collect();
            self.multi_apply_sounds();
        } else {
            ctx.request_repaint();
        }
    }

    /// Coupe ou rétablit le son de chaque lecteur selon `multi_audio`.
    fn multi_apply_sounds(&self) {
        for s in &self.multi {
            if let Some(p) = &s.player {
                let _ = p.set_property(
                    "mute",
                    if self.multi_audio.contains(&s.id) {
                        "no"
                    } else {
                        "yes"
                    },
                );
            }
        }
    }

    /// Donne le son à une vignette : elle seule, ou en plus des autres avec « Cumuler le son des lives en Multistream ».
    /// Le contour d'accent s'affiche quelques secondes.
    fn multi_listen(&mut self, id: u64) {
        self.multi_audio =
            sounds_after_listen(&self.multi_audio, id, self.settings.multi_mix_sounds);
        self.multi_audio_at = Some(Instant::now());
        self.multi_apply_sounds();
    }

    /// Bouton du haut de la page : coupe le son de tous les lives (en retenant lesquels l'avaient), ou le rétablit sur
    /// ces mêmes lives (le premier live si aucun n'est retenu).
    fn multi_toggle_all_sound(&mut self) {
        if self.multi_audio.is_empty() {
            let mut back: Vec<u64> = std::mem::take(&mut self.multi_muted)
                .into_iter()
                .filter(|id| self.multi.iter().any(|s| s.id == *id))
                .collect();
            if back.is_empty() {
                back.extend(self.multi.first().map(|s| s.id));
            }
            self.multi_audio = back;
        } else {
            self.multi_muted = std::mem::take(&mut self.multi_audio);
        }
        self.multi_apply_sounds();
    }

    /// Coupe le son d'une vignette.
    fn multi_mute(&mut self, id: u64) {
        self.multi_audio.retain(|x| *x != id);
        self.multi_apply_sounds();
    }

    /// Volume d'un live. Le régler lui donne aussi le son (un live muet n'aurait rien à régler).
    fn multi_set_volume(&mut self, id: u64, volume: u8) {
        let volume = volume.min(self.settings.volume_max);
        if let Some(s) = self.multi.iter_mut().find(|s| s.id == id) {
            s.volume = volume;
            if let Some(p) = &s.player {
                p.set_volume(volume);
            }
        }
        if !self.multi_audio.contains(&id) {
            self.multi_listen(id);
        }
    }

    /// Format de l'image de chaque live, relu dans mpv au plus deux fois par seconde (contour et image ajustés).
    fn multi_read_ratios(&mut self) {
        for s in &mut self.multi {
            if s.state != SlotState::Playing
                || s.ratio_at
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
            {
                continue;
            }
            s.ratio_at = Some(Instant::now());
            let Some(p) = &s.player else {
                continue;
            };
            let num = |name: &str| {
                p.get_property(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| *v > 0.0)
            };
            if let (Some(w), Some(h)) = (num("dwidth"), num("dheight")) {
                s.ratio = w / h;
            }
        }
    }

    /// En fin d'image : les lecteurs non dessinés (autre page affichée) continuent d'avancer, pour rester au direct.
    pub(super) fn multi_keep_live(&self, ctx: &egui::Context) {
        for s in &self.multi {
            if let Some(p) = &s.player {
                p.keep_live(ctx);
            }
        }
    }

    /// Page « Multistream » : champ pour ajouter une chaîne (avec autocomplétion Twitch), qualité des lives, chaînes en
    /// direct proposées, puis les vignettes.
    pub(super) fn multi_main(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let tr = crate::i18n::tr;
        // Relance des lives mis de côté seulement si la page est toujours celle affichée : un live ouvert depuis la barre
        // latérale pendant cette image a déjà changé de page (sinon il serait aussitôt arrêté par la relance).
        let on_page = self.view == View::Multi
            || (self.view == View::Settings && self.overlay_base == View::Multi);
        if on_page {
            self.multi_resume_step(ctx);
        }
        // Réglage « Cumuler le son » coupé : seul le dernier live choisi garde le son.
        if !self.settings.multi_mix_sounds && self.multi_audio.len() > 1 {
            let last = self.multi_audio.split_off(self.multi_audio.len() - 1);
            self.multi_audio = last;
            self.multi_apply_sounds();
        }
        self.multi_read_ratios();
        let mut add: Option<String> = None;
        let mut stop_all = false;
        let mut quality_changed = false;
        let mut toggle_sound = false;
        let full = self.multi.len() >= MULTI_MAX;

        ui.horizontal(|ui| {
            t::heading(ui, tr("Multistream"));
            ui.add_space(t::SP_M);
            ui.label(
                RichText::new(tr("Regardez jusqu'à 4 lives en même temps.")).color(t::MUTED()),
            );
        });
        ui.add_space(t::SP_M);

        // Champ, bouton « Ajouter », qualité, puis « Tout arrêter » à droite.
        let field_id = egui::Id::unique("multi_input");
        let mut field = Rect::NOTHING;
        let mut focused = false;
        let mut enter = false;
        ui.horizontal(|ui| {
            let (edit, frame) = ui
                .allocate_ui(Vec2::new(280.0, t::BTN_H), |ui| {
                    ui.add_enabled_ui(!full, |ui| {
                        t::search_input(
                            ui,
                            &mut self.multi_input,
                            tr("Ajouter une chaîne"),
                            field_id,
                            "",
                        )
                    })
                    .inner
                })
                .inner;
            field = frame;
            focused = edit.has_focus();
            enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let typed = crate::login::normalize(&self.multi_input);
            ui.add_space(t::SP_S);
            let add_btn = ui
                .add_enabled_ui(!full && typed.is_some(), |ui| {
                    t::icon_text_button(ui, t::BtnKind::Primary, t::NavIcon::Plus, tr("Ajouter"))
                })
                .inner;
            if add_btn.clicked() {
                add = typed;
            }
            ui.add_space(t::SP_S);
            // Qualité : même largeur que « Ajouter ».
            quality_changed = super::settings::quality_dropdown_min(
                ui,
                &mut self.settings.multi_quality,
                t::BTN_H,
                add_btn.rect.width(),
            );
            if !self.multi.is_empty() {
                // De droite à gauche : « Tout arrêter » au bord, le son de tous les lives à sa gauche.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if t::icon_text_button(
                        ui,
                        t::BtnKind::SecondaryDanger,
                        t::NavIcon::Stop,
                        tr("Tout arrêter"),
                    )
                    .clicked()
                    {
                        stop_all = true;
                    }
                    let (icon, label) = if self.multi_audio.is_empty() {
                        (t::NavIcon::Sound, tr("Réactiver le son"))
                    } else {
                        (t::NavIcon::SoundOff, tr("Couper tous les sons"))
                    };
                    if t::icon_text_button(ui, t::BtnKind::Secondary, icon, label).clicked() {
                        toggle_sound = true;
                    }
                });
            }
        });
        // Autocomplétion : liste sous le champ (flèches haut / bas, Entrée, clic).
        let (selected, clicked) = self.multi_dropdown(ctx, field, focused || enter);
        if enter {
            if let Some(l) = selected.or_else(|| crate::login::normalize(&self.multi_input)) {
                add = Some(l);
                // Entrée : le curseur reste dans le champ pour enchaîner les chaînes.
                ctx.memory_mut(|m| m.request_focus(field_id));
            }
        } else if clicked.is_some() {
            add = clicked;
        }

        // Message bref (chaîne déjà affichée, maximum atteint).
        if let Some((at, msg)) = self.multi_msg {
            if at.elapsed() < MSG_SHOWN {
                ui.add_space(t::SP_XS);
                ui.label(RichText::new(tr(msg)).size(12.5).color(t::MUTED()));
                ctx.request_repaint_after(MSG_SHOWN.saturating_sub(at.elapsed()));
            } else {
                self.multi_msg = None;
            }
        }

        // Chaînes en direct (favoris, puis chaînes suivies) pas encore affichées : un clic les ajoute.
        if !full {
            let mut quick: Vec<String> = Vec::new();
            let live = |l: &String| {
                matches!(
                    self.status_cache.get(l).map(|(_, s)| s),
                    Some(LiveStatus::Live { .. })
                )
            };
            for l in self
                .settings
                .favorites
                .iter()
                .filter(|l| live(l))
                .cloned()
                .chain(self.live_followed_with(QUICK_MAX, true))
            {
                if !quick.contains(&l) && !self.multi.iter().any(|s| s.login == l) {
                    quick.push(l);
                }
            }
            quick.truncate(QUICK_MAX);
            if !quick.is_empty() {
                ui.add_space(t::SP_M);
                ui.label(RichText::new(tr("En direct")).size(13.0).color(t::MUTED()));
                ui.add_space(t::SP_S);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    for l in &quick {
                        if super::home::chip(ui, l, true).clicked() {
                            add = Some(l.clone());
                        }
                    }
                });
            }
        }
        ui.add_space(t::SP_M);

        // Vignettes : toute la place restante.
        let area = ui.available_rect_before_wrap();
        let mut actions: Vec<(u64, TileAction)> = Vec::new();
        if self.multi.is_empty() {
            ui.painter()
                .rect_filled(area, CornerRadius::same(t::RADIUS_CARD), t::SURFACE_1());
            ui.put(
                area,
                egui::Label::new(
                    RichText::new(tr("Ajoutez une chaîne pour commencer.")).color(t::MUTED()),
                ),
            );
        } else {
            let ppp = ctx.pixels_per_point();
            let rects = tile_rects(area, self.multi.len());
            let gliding = self
                .multi_swap_at
                .is_some_and(|at| at.elapsed() < SWAP_GLIDE);
            let mut tiles: Vec<(u64, Rect, egui::Response)> = Vec::new();
            for (i, target) in rects.into_iter().enumerate() {
                let Some(id) = self.multi.get(i).map(|s| s.id) else {
                    continue;
                };
                let target = snap(target, ppp);
                let shown = snap(glide(ctx, id, target, gliding), ppp);
                if let Some(resp) = self.multi_tile(ui, ctx, i, shown, &mut actions) {
                    tiles.push((id, target, resp));
                }
            }
            ui.allocate_rect(area, Sense::hover());
            self.multi_drag_ui(ui, ctx, &tiles);
        }

        for (id, a) in actions {
            match a {
                TileAction::Remove => self.multi_remove(id),
                TileAction::Listen => self.multi_listen(id),
                TileAction::Mute => self.multi_mute(id),
                TileAction::Volume(v) => self.multi_set_volume(id, v),
            }
        }
        if quality_changed {
            self.multi_restart_streams(ctx);
        }
        if toggle_sound {
            self.multi_toggle_all_sound();
        }
        if stop_all {
            self.multi_stop_all();
        }
        if let Some(l) = add {
            self.multi_input.clear();
            self.multi_sel = None;
            self.multi_add(ctx, l);
        }
    }

    /// Glisser-déposer : bouton maintenu sur un live puis souris déplacée (au-delà de `DRAG_SLOP`) : le live s'attrape
    /// tout de suite et suit le pointeur (même taille que sa vignette, avec une ombre, poing fermé à la place du
    /// curseur) ; la vignette visée s'éclaire ; au relâchement les deux échangent leur place et glissent vers elle.
    /// Échap ou relâchement ailleurs : rien ne change. Un clic sans bouger garde son rôle (donner le son), et le curseur
    /// reste le curseur habituel tant que le live n'est pas attrapé.
    fn multi_drag_ui(
        &mut self,
        ui: &egui::Ui,
        ctx: &egui::Context,
        tiles: &[(u64, Rect, egui::Response)],
    ) {
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        if self.multi_drag.is_none() {
            let down = ctx.input(|i| i.pointer.primary_down());
            match (self.multi_press, pointer) {
                // Bouton maintenu sur un live : attrapé dès que la souris bouge.
                (Some((id, at)), Some(p)) if down => {
                    if p.distance(at) > DRAG_SLOP {
                        self.multi_press = None;
                        if let Some((_, rect, _)) = tiles.iter().find(|(i, ..)| *i == id) {
                            self.multi_drag = Some((id, at - rect.min));
                        }
                    }
                }
                (Some(_), _) => self.multi_press = None,
                (None, Some(p)) => {
                    if let Some((id, ..)) = tiles.iter().find(|(_, _, r)| {
                        r.is_pointer_button_down_on() && ctx.input(|i| i.pointer.primary_pressed())
                    }) {
                        self.multi_press = Some((*id, p));
                    }
                }
                (None, None) => {}
            }
        }
        let Some((held, grab)) = self.multi_drag else {
            return;
        };
        let Some(from) = tiles
            .iter()
            .find(|(id, ..)| *id == held)
            .map(|(_, r, _)| *r)
        else {
            self.multi_drag = None;
            return;
        };
        let places: Vec<(u64, Rect)> = tiles.iter().map(|(id, r, _)| (*id, *r)).collect();
        let target = pointer.and_then(|p| drop_target(&places, held, p));
        // Même taille que la vignette : la vidéo garde sa taille de rendu (un changement de taille fait recalculer ses
        // filtres par libmpv, d'où des saccades).
        let ppp = ctx.pixels_per_point();
        let float = pointer.map(|p| snap(Rect::from_min_size(p - grab, from.size()), ppp));
        let cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if cancel || !ctx.input(|i| i.pointer.primary_down()) {
            self.multi_drag = None;
            if let (false, Some((other, _))) = (cancel, target) {
                let a = self.multi.iter().position(|s| s.id == held);
                let b = self.multi.iter().position(|s| s.id == other);
                if let (Some(a), Some(b)) = (a, b) {
                    self.multi.swap(a, b);
                    self.multi_swap_at = Some(Instant::now());
                    // Le live lâché part de là où il a été relâché ; l'autre part de sa place actuelle.
                    if let Some(f) = float {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::unique(("multi_glide", held)), f));
                    }
                }
            }
            ctx.request_repaint();
            return;
        }
        // Vignette visée : voile et contour d'accent.
        if let Some((_, r)) = target {
            ui.painter().rect_filled(
                r,
                CornerRadius::same(TILE_RADIUS),
                t::ACCENT().gamma_multiply(0.18),
            );
            ui.painter().rect_stroke(
                r,
                CornerRadius::same(TILE_RADIUS),
                egui::Stroke::new(2.0, t::ACCENT()),
                egui::StrokeKind::Inside,
            );
        }
        // Live tenu : au premier plan, sous le pointeur.
        if let (Some(f), Some(s)) = (float, self.multi.iter().find(|s| s.id == held)) {
            egui::Area::new(egui::Id::unique("multi_drag"))
                .order(egui::Order::Foreground)
                .fixed_pos(f.min)
                .interactable(false)
                .show(ctx, |ui| {
                    let p = ui.painter();
                    p.rect_filled(
                        f.translate(Vec2::new(0.0, 8.0)).expand(4.0),
                        CornerRadius::same(TILE_RADIUS + 4),
                        Color32::from_black_alpha(90),
                    );
                    p.rect_filled(f, CornerRadius::same(TILE_RADIUS), Color32::BLACK);
                    match (&s.state, &s.player) {
                        (SlotState::Playing, Some(player)) if self.settings.multi_quality != 3 => {
                            player.paint(ui, snap(super::lecteur::fit_ratio(f, s.ratio), ppp));
                        }
                        _ => {
                            p.rect_filled(f, CornerRadius::same(TILE_RADIUS), t::SURFACE_1());
                        }
                    }
                    let pill = Rect::from_min_size(
                        f.min + Vec2::splat(8.0),
                        Vec2::new(f.width() - 16.0, TILE_BTN),
                    );
                    let mut pill_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(pill)
                            .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    );
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(150))
                        .corner_radius(CornerRadius::same(t::RADIUS_PILL))
                        .inner_margin(egui::Margin::symmetric(10, 3))
                        .show(&mut pill_ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&s.login).size(12.5).color(Color32::WHITE),
                                )
                                .truncate(),
                            );
                        });
                });
        }
        if let Some(p) = pointer {
            ctx.set_cursor_icon(egui::CursorIcon::None);
            super::mini::paint_hand(ctx, p + FIST_OFFSET, true);
        }
        ctx.request_repaint();
    }

    /// Liste sous le champ « Ajouter une chaîne » : favoris et récents qui correspondent, puis les chaînes proposées par
    /// Twitch, et la saisie elle-même. Renvoie la ligne choisie au clavier et la ligne cliquée.
    fn multi_dropdown(
        &mut self,
        ctx: &egui::Context,
        field: Rect,
        focused: bool,
    ) -> (Option<String>, Option<String>) {
        self.multi_editing = focused || self.multi_hover;
        let needle = self
            .multi_input
            .trim()
            .trim_start_matches(['@', '#'])
            .to_lowercase();
        if needle.is_empty() || !self.multi_editing {
            self.multi_hover = false;
            self.multi_sel = None;
            return (None, None);
        }
        let shown = |l: &String| self.multi.iter().any(|s| s.login == *l);
        let mut rows: Vec<String> = Vec::new();
        for l in self
            .settings
            .favorites
            .iter()
            .chain(self.history.items.iter())
        {
            if !rows.contains(l) && !shown(l) && l.contains(&needle) {
                rows.push(l.clone());
            }
        }
        rows.truncate(4);
        if self.suggest_for == self.suggest_want {
            for s in self.suggestions.iter().take(6) {
                if !rows.contains(&s.login) && !shown(&s.login) {
                    rows.push(s.login.clone());
                }
            }
        }
        if let Some(typed) = crate::login::normalize(&self.multi_input) {
            if !rows.contains(&typed) && !shown(&typed) {
                rows.push(typed);
            }
        }
        if rows.is_empty() {
            self.multi_hover = false;
            self.multi_sel = None;
            return (None, None);
        }
        if focused {
            let (down, up) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                )
            });
            let last = rows.len() - 1;
            if down {
                self.multi_sel = Some(self.multi_sel.map_or(0, |s| (s + 1).min(last)));
            }
            if up {
                self.multi_sel = Some(self.multi_sel.map_or(last, |s| s.saturating_sub(1)));
            }
        }
        let width = field.width();
        let pos = Pos2::new(field.left(), field.bottom() + 6.0);
        let mut clicked: Option<String> = None;
        let sel = self.multi_sel;
        let area = egui::Area::new(egui::Id::unique("multi_suggest"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t::SURFACE_1())
                    .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.set_width(width - 14.0);
                        for (i, l) in rows.iter().enumerate() {
                            let viewers = match self.status_cache.get(l).map(|(_, s)| s) {
                                Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
                                _ => self
                                    .suggestions
                                    .iter()
                                    .find(|s| s.login == *l)
                                    .and_then(|s| s.viewers),
                            };
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), t::BTN_H),
                                Sense::click(),
                            );
                            if sel == Some(i) || resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius::same(8),
                                    t::SURFACE_3(),
                                );
                            }
                            t::paint_pressed(ui, &resp, rect, 8);
                            t::focus_ring(ui, &resp, rect, 8);
                            resp.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, l.as_str())
                            });
                            let photo = self.avatars.get(l);
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
                                rect.width() - 34.0 - 64.0,
                            );
                            if let Some(v) = viewers {
                                ui.painter().text(
                                    Pos2::new(rect.right() - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    crate::login::format_viewers(v),
                                    egui::FontId::proportional(12.0),
                                    t::POSITIVE(),
                                );
                            }
                            if resp.clicked() {
                                clicked = Some(l.clone());
                            }
                        }
                    });
            });
        self.multi_hover = clicked.is_none()
            && ctx
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| area.response.rect.contains(p));
        (sel.and_then(|i| rows.get(i).cloned()), clicked)
    }

    /// Une vignette : vidéo (ou chargement, ou message). Au survol : nom de la chaîne, son et volume, retrait.
    /// Un clic sur la vignette lui donne le son ; celle qui l'a est entourée de la couleur d'accent (contour à angles
    /// droits, exactement sur l'image).
    fn multi_tile(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        idx: usize,
        rect: Rect,
        actions: &mut Vec<(u64, TileAction)>,
    ) -> Option<egui::Response> {
        let tr = crate::i18n::tr;
        let s = self.multi.get(idx)?;
        let audible = self.multi_audio.contains(&s.id);
        let resp = ui.interact(rect, egui::Id::unique(("multi_tile", s.id)), Sense::click());
        let dragging = self.multi_drag.is_some();
        // Live tenu à la souris : sa place reste vide (il est dessiné sous le pointeur, voir `multi_drag_ui`).
        if self.multi_drag.is_some_and(|(id, _)| id == s.id) {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(TILE_RADIUS), t::SURFACE_1());
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(TILE_RADIUS),
                egui::Stroke::new(1.0, t::BORDER_HOVER()),
                egui::StrokeKind::Inside,
            );
            return Some(resp);
        }
        let video = snap(
            super::lecteur::fit_ratio(rect, s.ratio),
            ctx.pixels_per_point(),
        );
        let audio_only = self.settings.multi_quality == 3;
        match &s.state {
            SlotState::Playing if audio_only => {
                super::watch::paint_placeholder(
                    ui,
                    rect,
                    TILE_RADIUS,
                    tr("Audio du live uniquement"),
                    false,
                );
            }
            SlotState::Playing => {
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
                if let Some(p) = &s.player {
                    p.paint(ui, video);
                }
            }
            SlotState::Checking | SlotState::Starting => {
                super::watch::paint_placeholder(
                    ui,
                    rect,
                    TILE_RADIUS,
                    tr("Connexion en cours"),
                    true,
                );
            }
            SlotState::Switching => {
                super::watch::paint_placeholder(
                    ui,
                    rect,
                    TILE_RADIUS,
                    tr("Changement de qualité…"),
                    true,
                );
            }
            SlotState::Error(msg) => {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(TILE_RADIUS), t::SURFACE_1());
                ui.put(
                    rect.shrink(24.0),
                    egui::Label::new(RichText::new(msg).color(t::MUTED())).wrap(),
                );
            }
        }
        // Contour d'accent : quelques secondes après un changement de son, puis il s'efface en fondu.
        let outline_k = self.multi_audio_at.map_or(0.0, |at| {
            let left = OUTLINE_SHOWN - at.elapsed().as_secs_f32();
            let fade = t::anim_secs(ctx, OUTLINE_FADE);
            if left <= 0.0 {
                0.0
            } else {
                ctx.request_repaint_after(Duration::from_secs_f32(
                    (left - fade).max(0.0).max(1.0 / 60.0),
                ));
                if fade > 0.0 {
                    (left / fade).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            }
        });
        if audible && self.multi.len() > 1 && outline_k > 0.0 {
            let (outline, radius) = if s.state == SlotState::Playing && !audio_only {
                (video, CornerRadius::ZERO)
            } else {
                (rect, CornerRadius::same(TILE_RADIUS))
            };
            ui.painter().rect_stroke(
                outline,
                radius,
                egui::Stroke::new(2.0, t::ACCENT().gamma_multiply(outline_k)),
                egui::StrokeKind::Inside,
            );
        }
        if resp.clicked() && !audible {
            actions.push((s.id, TileAction::Listen));
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, s.login.as_str()));

        // Molette au-dessus d'une vignette : son volume (réglage « Molette de la souris sur la vidéo = volume »).
        let hovered = !dragging && ui.rect_contains_pointer(rect);
        if hovered && self.settings.wheel_volume {
            let d = ctx.input(|i| i.smooth_scroll_delta.y);
            let speed = ctx.options(|o| o.input_options.line_scroll_speed);
            if d != 0.0 && speed > 0.0 {
                // Le défilement lissé arrive par petits morceaux : ils s'additionnent jusqu'à un point de volume entier.
                let acc_id = egui::Id::unique(("multi_wheel", s.id));
                let acc = ctx.data(|m| m.get_temp::<f32>(acc_id)).unwrap_or(0.0) + d / speed * 5.0;
                let whole = acc.trunc();
                ctx.data_mut(|m| m.insert_temp(acc_id, acc - whole));
                if whole != 0.0 {
                    let v = (f32::from(s.volume) + whole)
                        .clamp(0.0, f32::from(self.settings.volume_max));
                    actions.push((s.id, TileAction::Volume(v as u8)));
                }
            }
        }

        // Surcouches au survol (toujours sombres, quel que soit le thème) : en haut à gauche le retrait, puis le son et le
        // volume ; en haut à droite le nom de la chaîne.
        let k = t::motion::eased(
            ctx,
            egui::Id::unique(("multi_bar", s.id)),
            hovered,
            t::motion::CONTROLS,
        );
        if k <= 0.0 {
            return Some(resp);
        }
        let bar = Rect::from_min_size(
            rect.min + Vec2::splat(8.0),
            Vec2::new(rect.width() - 16.0, TILE_BTN),
        );
        let mut bar_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        bar_ui.set_opacity(k);
        let _dark = t::DarkScope::enter(&mut bar_ui);
        bar_ui.spacing_mut().item_spacing.x = 6.0;
        if t::icon_button(
            &mut bar_ui,
            t::Icon::Close,
            TILE_BTN,
            t::IconStyle::AccentDanger,
            tr("Retirer ce live"),
        )
        .clicked()
        {
            actions.push((s.id, TileAction::Remove));
        }
        let (icon, tip) = if audible {
            (t::Icon::Speaker, tr("Couper le son"))
        } else {
            (t::Icon::Muted, tr("Écouter ce live"))
        };
        if t::icon_button(&mut bar_ui, icon, TILE_BTN, t::IconStyle::Overlay, tip).clicked() {
            actions.push((
                s.id,
                if audible {
                    TileAction::Mute
                } else {
                    TileAction::Listen
                },
            ));
        }
        if rect.width() >= 300.0 {
            let mut v = s.volume;
            if t::volume_slider(&mut bar_ui, &mut v, self.settings.volume_max, TILE_SLIDER_W) {
                actions.push((s.id, TileAction::Volume(v)));
            }
        }
        bar_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(150))
                .corner_radius(CornerRadius::same(t::RADIUS_PILL))
                .inner_margin(egui::Margin::symmetric(10, 3))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&s.login).size(12.5).color(t::TEXT()))
                            .truncate(),
                    );
                });
        });
        Some(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1600.0, 900.0))
    }

    #[test]
    fn grille_des_vignettes() {
        assert!(tile_rects(area(), 0).is_empty());
        for n in 1..=MULTI_MAX {
            let r = tile_rects(area(), n);
            assert_eq!(r.len(), n);
            for (i, a) in r.iter().enumerate() {
                assert!(
                    area().expand(0.5).contains_rect(*a),
                    "{n} lives : vignette {i} hors de la zone"
                );
                assert!(
                    (a.width() / a.height() - 16.0 / 9.0).abs() < 0.02,
                    "format 16:9"
                );
                for b in r.iter().skip(i + 1) {
                    assert!(
                        !a.shrink(1.0).intersects(*b),
                        "{n} lives : vignettes superposées"
                    );
                }
            }
        }
        // Deux lives côte à côte dans une zone large, l'un sous l'autre dans une zone haute.
        let wide = tile_rects(area(), 2);
        assert!((wide[0].top() - wide[1].top()).abs() < 1.0);
        let tall = tile_rects(
            Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(700.0, 900.0)),
            2,
        );
        assert!((tall[0].left() - tall[1].left()).abs() < 1.0);
        // Trois lives : le troisième est centré sous les deux premiers.
        let three = tile_rects(area(), 3);
        assert!((three[2].center().x - area().center().x).abs() < 1.0);
    }

    /// Pendant un glissement, la taille en pixels de la vidéo ne change jamais (sinon libmpv recalcule ses filtres).
    #[test]
    fn taille_en_pixels_stable_pendant_un_glissement() {
        for ppp in [1.0_f32, 1.1, 1.25, 1.5, 1.65] {
            let size = Vec2::new(640.37, 360.21);
            let first = snap(Rect::from_min_size(egui::pos2(0.0, 0.0), size), ppp);
            let px = (first.width() * ppp).round();
            for i in 0..400 {
                let x = i as f32 * 0.37;
                let r = snap(
                    Rect::from_min_size(egui::pos2(17.0 + x, 9.0 + x * 0.5), size),
                    ppp,
                );
                assert!(
                    (r.width() * ppp - px).abs() < 0.01,
                    "ppp {ppp}, décalage {x} : {} au lieu de {px}",
                    r.width() * ppp
                );
                assert!(
                    (r.min.x * ppp - (r.min.x * ppp).round()).abs() < 0.01,
                    "coin sur un pixel"
                );
            }
        }
    }

    #[test]
    fn vignette_visee_au_lacher() {
        let r = tile_rects(area(), 4);
        let places: Vec<(u64, Rect)> = r
            .iter()
            .enumerate()
            .map(|(i, x)| (i as u64 + 1, *x))
            .collect();
        // Pointeur sur la 4e vignette en tenant la 1re : la 4e est visée.
        assert_eq!(
            drop_target(&places, 1, r[3].center()).map(|(id, _)| id),
            Some(4)
        );
        // Pointeur sur sa propre place ou entre deux vignettes : rien.
        assert_eq!(drop_target(&places, 1, r[0].center()), None);
        let gap = Pos2::new((r[0].right() + r[1].left()) / 2.0, r[0].center().y);
        assert_eq!(drop_target(&places, 1, gap), None);
    }

    #[test]
    fn son_d_un_ou_de_plusieurs_lives() {
        // Réglage coupé : le live cliqué a seul le son.
        assert_eq!(sounds_after_listen(&[1], 2, false), [2]);
        assert_eq!(sounds_after_listen(&[1, 2], 3, false), [3]);
        // Réglage activé : le son s'ajoute, jusqu'aux quatre lives.
        assert_eq!(sounds_after_listen(&[1], 2, true), [1, 2]);
        assert_eq!(sounds_after_listen(&[1, 2], 3, true), [1, 2, 3]);
        assert_eq!(sounds_after_listen(&[1, 2, 3], 4, true), [1, 2, 3, 4]);
        assert_eq!(sounds_after_listen(&[], 4, true), [4]);
        // Un live qui a déjà le son passe en dernier (le plus récent), sans doublon.
        assert_eq!(sounds_after_listen(&[1, 2], 1, true), [2, 1]);
    }
}
