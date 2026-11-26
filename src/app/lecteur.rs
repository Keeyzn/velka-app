//! Lecteur : démarrage et arrêt du flux, son, plein écran et picture-in-picture.
use super::*;

impl VelkaApp {
    /// Crée le lecteur au premier besoin (le contexte OpenGL est courant pendant `ui`).
    pub(super) fn ensure_player(&mut self, ctx: &egui::Context) -> Result<(), String> {
        if self.player.is_some() {
            return Ok(());
        }
        let dll = tools::detect(Tool::Libmpv, &self.settings.libmpv_path).ok_or(
            crate::i18n::tr("libmpv est introuvable. Réinstallez les outils depuis les réglages."),
        )?;
        let get_proc = self
            .get_proc
            .clone()
            .ok_or(crate::i18n::tr("Le rendu OpenGL n'est pas disponible."))?;
        // Journal du lecteur seulement avec `--diag` ; sinon celui d'une session précédente est supprimé.
        let log = crate::logging::mpv_log(crate::cli::current().diag);
        let player = Player::new(
            get_proc,
            ctx,
            &dll,
            log.as_deref(),
            &crate::mpv::default_options(),
        )?;
        player.set_background(crate::ui::theme::BG());
        self.player = Some(player);
        Ok(())
    }

    pub(super) fn start_watch(&mut self, ctx: &egui::Context, login: String) {
        // Un live ouvert dans le lecteur principal met le multistream de côté : un seul ensemble de lives joue à la fois,
        // et les lives du multistream sont relancés au retour sur sa page.
        self.multi_suspend();
        self.start_token_refresh(ctx);
        // Chaîne déjà connue pour ne pas exister (faute de frappe) : un live en cours (page ou mini-lecteur)
        // continue, seul un message est affiché. Sans live en cours, la page du lecteur affichera l'erreur.
        if self.watch.is_some()
            && matches!(
                self.status_cache.get(&login),
                Some((_, LiveStatus::Missing))
            )
        {
            self.info_toast = Some((
                Instant::now(),
                crate::i18n::tr("Cette chaîne n'existe pas.").to_string(),
            ));
            return;
        }
        // La recherche a servi : le champ ne garde pas le nom saisi quand on revient du lecteur.
        self.input.clear();
        // Même chaîne déjà ouverte (mini-lecteur) : on la rouvre simplement en grand.
        if let Some(w) = &self.watch {
            if w.login == login && !matches!(w.state, WatchState::Error(_)) {
                if self.view != View::Watch {
                    self.restore_watch(ctx);
                }
                return;
            }
        }
        self.stop_stream();
        // Recommandations déjà chargées pour cette chaîne (« Réessayer ») : on les garde, sans nouvelle attente.
        if self.recs.as_ref().is_some_and(|r| r.login != login) {
            self.recs = None;
        }
        self.generation += 1;
        self.chat_epoch += 1;
        let (gen, epoch) = (self.generation, self.chat_epoch);
        self.paused = false;
        self.controls_seen = Instant::now();
        // Nouveau live : le minuteur de veille repart sur « Désactivé » (il ne se garde pas d'un live à l'autre).
        self.settings.sleep_timer = 0;
        self.sleep_deadline = None;
        // L'historique (« Récents ») n'est complété qu'une fois le statut de la chaîne connu (`remember_channel`).
        self.chat = ChatState::new();
        self.chat_extras = None;
        {
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            self.chat_client = Some(ChatClient::start(&login, self.chat_token(), move |ev| {
                let _ = tx.send(Msg::Chat(epoch, ev));
                // Rafraîchissement limité : le chat ne doit jamais ralentir la vidéo.
                ctx.request_repaint_after(Duration::from_millis(50));
            }));
        }
        let mut watch = Watch {
            login: login.clone(),
            state: WatchState::Checking,
            status: None,
            on_break: false,
        };
        if let Err(e) = self.ensure_player(ctx) {
            watch.state = WatchState::Error(e);
        } else {
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let st = twitch::fetch_status(std::slice::from_ref(&login))
                    .remove(&login)
                    .unwrap_or(LiveStatus::Unknown);
                // Statut introuvable et Internet injoignable : message de connexion, pas d'essai de flux voué à l'échec.
                if st == LiveStatus::Unknown && !crate::net::internet_up() {
                    let _ = tx.send(Msg::Stream(
                        gen,
                        StreamEvent::Failed(StreamError::NoInternet),
                    ));
                } else {
                    let _ = tx.send(Msg::Status(gen, st));
                }
                ctx.request_repaint();
            });
        }
        self.watch = Some(watch);
        self.view = View::Watch;
    }

    /// Ajoute la chaîne aux récents une fois son statut confirmé (jamais pour une chaîne inexistante).
    pub(super) fn remember_channel(&mut self, login: &str) {
        self.history.push(login);
        if let Err(e) = self.history.save() {
            crate::logging::log(&format!("Historique non enregistré : {e}"));
        }
    }

    /// Relance uniquement le flux (changement de qualité), le chat et le volume restent.
    pub(super) fn restart_stream(&mut self, ctx: &egui::Context) {
        self.relaunch_stream(ctx, WatchState::Switching);
    }

    /// Actualisation demandée depuis la barre du haut : mêmes réglages, nouvelle connexion au flux.
    pub(super) fn refresh_stream(&mut self, ctx: &egui::Context) {
        self.relaunch_stream(ctx, WatchState::Refreshing);
    }

    fn relaunch_stream(&mut self, ctx: &egui::Context, shown: WatchState) {
        self.stop_stream();
        self.generation += 1;
        self.paused = false;
        if let Some(w) = &mut self.watch {
            w.on_break = false;
        }
        self.begin_stream(ctx);
        if self.stream.is_some() {
            self.set_state(shown);
        }
    }

    pub(super) fn begin_stream(&mut self, ctx: &egui::Context) {
        let Some(login) = self.watch.as_ref().map(|w| w.login.clone()) else {
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
            let args = stream::build_args(&login, s.quality, s.low_latency, port);
            let (tx, ctx, gen) = (self.tx.clone(), ctx.clone(), self.generation);
            Stream::start(&exe, &args, port, move |ev| {
                let _ = tx.send(Msg::Stream(gen, ev));
                ctx.request_repaint();
            })
        })();
        match result {
            Ok(st) => {
                self.stream = Some(st);
                self.set_state(WatchState::Starting);
            }
            Err(e) => self.set_state(WatchState::Error(e)),
        }
    }

    pub(super) fn set_state(&mut self, state: WatchState) {
        if let Some(w) = &mut self.watch {
            w.state = state;
        }
    }

    pub(super) fn stop_stream(&mut self) {
        self.stream = None; // `Drop` tue Streamlink
        if let Some(p) = &self.player {
            p.stop();
        }
    }

    /// Emotes 7TV/BTTV/FFZ et badges : chargés une fois par live (et de nouveau si l'option change).
    pub(super) fn tick_chat_extras(&mut self, ctx: &egui::Context) {
        let (emotes, badges) = (self.settings.chat_third_party, self.settings.chat_badges);
        if !(emotes || badges) {
            return;
        }
        let Some(login) = self.watch.as_ref().map(|w| w.login.clone()) else {
            return;
        };
        let req = (self.chat_epoch, emotes, badges);
        if self.extras_req == Some(req) {
            return;
        }
        self.extras_req = Some(req);
        let (tx, ctx, epoch) = (self.tx.clone(), ctx.clone(), self.chat_epoch);
        std::thread::spawn(move || {
            let extras = crate::chat::extra::fetch(&login, emotes, badges);
            crate::logging::log(&format!(
                "Chat : {} emotes externes, {} badges",
                extras.emotes.len(),
                extras.badges.len()
            ));
            let _ = tx.send(Msg::ChatExtras(epoch, extras));
            ctx.request_repaint();
        });
    }

    /// Pendant un live : sonde la connexion Internet toutes les 3 s. Après 3 échecs de suite (environ 10 s), le flux
    /// est arrêté et le message de coupure s'affiche au milieu de l'écran ; la sonde continue et le live est relancé
    /// tout seul dès que la connexion revient (comme « Réessayer »).
    pub(super) fn tick_connection(&mut self, ctx: &egui::Context) {
        const EVERY: Duration = Duration::from_secs(3);
        // Live en cours (surveillance) ou live arrêté faute d'Internet (attente du retour de la connexion).
        if !self
            .watch
            .as_ref()
            .is_some_and(|w| w.state == WatchState::Playing || w.net_lost())
        {
            self.net_probe_fails = 0;
            return;
        }
        if self.net_probe_busy {
            return;
        }
        let due = self
            .net_probe_at
            .map_or(Duration::ZERO, |t| EVERY.saturating_sub(t.elapsed()));
        if !due.is_zero() {
            ctx.request_repaint_after(due);
            return;
        }
        self.net_probe_at = Some(Instant::now());
        self.net_probe_busy = true;
        let (tx, ctx, gen) = (self.tx.clone(), ctx.clone(), self.generation);
        std::thread::spawn(move || {
            let _ = tx.send(Msg::NetProbe(gen, crate::net::internet_up()));
            ctx.request_repaint();
        });
    }

    /// Minuteur de veille : arrête le live une fois le délai écoulé.
    /// Le décompte du minuteur de veille repart de zéro pour le live en cours (changement de durée, depuis les Réglages
    /// ou le menu du lecteur : même fonction, même réglage `sleep_timer`).
    pub(super) fn reset_sleep_timer(&mut self) {
        self.sleep_deadline = self
            .watch
            .as_ref()
            .and(self.settings.sleep_after())
            .map(|d| Instant::now() + d);
    }

    pub(super) fn tick_sleep(&mut self, ctx: &egui::Context) {
        let Some(deadline) = self.sleep_deadline else {
            return;
        };
        if self.watch.is_none() {
            self.sleep_deadline = None;
            return;
        }
        let now = Instant::now();
        if now >= deadline {
            crate::logging::log("Minuteur de veille : lecture arrêtée");
            self.sleep_deadline = None;
            self.stop_watch(ctx);
        } else {
            ctx.request_repaint_after(deadline - now);
        }
    }

    /// Enregistre l'image en cours dans le dossier des captures (par défaut « Images\Velka », touche S).
    pub(super) fn take_screenshot(&mut self) {
        let playing = self
            .watch
            .as_ref()
            .is_some_and(|w| w.state == WatchState::Playing);
        if self.player.is_none() || !playing {
            return;
        }
        let login = self
            .watch
            .as_ref()
            .map(|w| w.login.clone())
            .unwrap_or_default();
        let dir = self.screenshot_dir();
        let Some(player) = &self.player else {
            return;
        };
        let file = dir.join(format!("{login}_{}.png", crate::procs::local_stamp()));
        let res = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                player.command(&[
                    "async",
                    "screenshot-to-file",
                    &file.to_string_lossy(),
                    "video",
                ])
            });
        let text = match res {
            Ok(()) => {
                crate::logging::log("Capture d'écran demandée");
                self.shot_check = Some((file, Instant::now()));
                format!(
                    "{} {}",
                    crate::i18n::tr("Capture enregistrée :"),
                    dir.display()
                )
            }
            Err(e) => {
                crate::logging::log(&format!("Capture d'écran impossible : {e}"));
                crate::i18n::tr("Capture impossible.").to_string()
            }
        };
        self.info_toast = Some((Instant::now(), text));
    }

    /// Dossier des captures : celui des Réglages s'il existe et accepte l'écriture, sinon le dossier par défaut (le
    /// réglage est alors vidé, avec un message clair).
    pub(super) fn screenshot_dir(&mut self) -> PathBuf {
        if self.settings.screenshot_dir.is_empty() {
            return default_screenshot_dir();
        }
        let chosen = PathBuf::from(&self.settings.screenshot_dir);
        if dir_writable(&chosen) {
            return chosen;
        }
        crate::logging::log(&format!(
            "Dossier des captures indisponible : {}",
            chosen.display()
        ));
        self.settings.screenshot_dir.clear();
        self.shot_dir_lost = true;
        self.info_toast = Some((Instant::now(), crate::i18n::tr(SHOT_DIR_LOST).to_string()));
        default_screenshot_dir()
    }

    /// Réglages > Lecture vidéo > Captures d'écran : contrôle du dossier choisi toutes les 2 s tant que la page est visible.
    pub(super) fn check_screenshot_dir(&mut self, ctx: &egui::Context) {
        if self.settings.screenshot_dir.is_empty() {
            return;
        }
        const EVERY: Duration = Duration::from_secs(2);
        if self.shot_dir_check.elapsed() >= EVERY {
            self.shot_dir_check = Instant::now();
            let _ = self.screenshot_dir();
        }
        ctx.request_repaint_after(EVERY);
    }

    /// Ouvre la fenêtre de choix d'un dossier (fil dédié) ; la réponse est reprise par `tick_screenshot_dir`.
    pub(super) fn browse_screenshot_dir(&mut self) {
        if self.shot_pick.is_some() {
            return;
        }
        let start = if self.settings.screenshot_dir.is_empty() {
            default_screenshot_dir()
        } else {
            PathBuf::from(&self.settings.screenshot_dir)
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let start = if start.is_dir() {
                Some(start)
            } else {
                start.parent().filter(|p| p.is_dir()).map(PathBuf::from)
            };
            let _ = tx.send(crate::procs::pick_folder(start.as_deref()));
        });
        self.shot_pick = Some(rx);
    }

    /// Reprend le dossier choisi dans la fenêtre : il n'est enregistré que s'il accepte l'écriture.
    pub(super) fn tick_screenshot_dir(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.shot_pick else {
            return;
        };
        match rx.try_recv() {
            Ok(Some(path)) => {
                self.shot_pick = None;
                if dir_writable(&path) {
                    let same_as_default = path
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&default_screenshot_dir().to_string_lossy());
                    self.settings.screenshot_dir = if same_as_default {
                        String::new()
                    } else {
                        path.to_string_lossy().into_owned()
                    };
                    self.shot_dir_err = false;
                    self.shot_dir_lost = false;
                } else {
                    crate::logging::log(&format!(
                        "Dossier des captures refusé (écriture impossible) : {}",
                        path.display()
                    ));
                    self.shot_dir_err = true;
                }
            }
            Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => self.shot_pick = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(200))
            }
        }
    }

    /// La capture est écrite en arrière-plan : si le fichier n'existe toujours pas après 4 s, on le signale.
    pub(super) fn tick_screenshot(&mut self, ctx: &egui::Context) {
        let Some((file, t0)) = &self.shot_check else {
            return;
        };
        if file.exists() {
            self.shot_check = None;
        } else if t0.elapsed() >= Duration::from_secs(4) {
            crate::logging::log("Capture d'écran : fichier absent");
            self.shot_check = None;
            self.info_toast = Some((
                Instant::now(),
                crate::i18n::tr("Capture impossible.").to_string(),
            ));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// Valeurs du panneau d'infos techniques (libellé, valeur), relues au plus deux fois par seconde.
    pub(super) fn tech_values(&mut self) -> Vec<(&'static str, String)> {
        if let Some((t0, v)) = &self.tech_cache {
            if t0.elapsed() < Duration::from_millis(500) {
                return v.clone();
            }
        }
        let Some(p) = &self.player else {
            return Vec::new();
        };
        let num = |name: &str| p.get_property(name).and_then(|v| v.parse::<f64>().ok());
        let dec = crate::i18n::decimal;
        let dash = || "-".to_string();
        let mut out = Vec::new();
        let res = match (num("video-params/w"), num("video-params/h")) {
            (Some(w), Some(h)) => format!("{w} x {h}"),
            _ => dash(),
        };
        out.push(("Résolution", res));
        let fps = num("estimated-vf-fps").or_else(|| num("container-fps"));
        if let Some(f) = fps {
            out.push(("Images par seconde", dec(f, 1)));
        }
        out.push((
            "Débit vidéo",
            num("video-bitrate").map_or_else(dash, |b| format!("{} Mb/s", dec(b / 1_000_000.0, 2))),
        ));
        out.push((
            "Débit audio",
            num("audio-bitrate").map_or_else(dash, |b| format!("{} kb/s", dec(b / 1000.0, 0))),
        ));
        out.push((
            "Images perdues",
            num("frame-drop-count").map_or_else(dash, |n| format!("{n}")),
        ));
        out.push((
            "Images perdues au décodage",
            num("decoder-frame-drop-count").map_or_else(dash, |n| format!("{n}")),
        ));
        out.push((
            "Mémoire tampon",
            num("demuxer-cache-duration").map_or_else(dash, |s| format!("{} s", dec(s, 1))),
        ));
        out.push((
            "Format vidéo",
            p.get_property("video-format").unwrap_or_else(dash),
        ));
        let hw = p
            .get_property("hwdec-current")
            .filter(|h| !h.is_empty() && h != "no");
        out.push((
            "Décodage",
            hw.unwrap_or_else(|| crate::i18n::tr("logiciel").to_string()),
        ));
        self.tech_cache = Some((Instant::now(), out.clone()));
        out
    }

    pub(super) fn stop_watch(&mut self, ctx: &egui::Context) {
        self.leave_immersive(ctx);
        self.generation += 1;
        self.chat_epoch += 1;
        self.stop_stream();
        self.chat_client = None;
        self.watch = None;
        self.anim = None;
        if self.view == View::Watch {
            self.view = View::Home;
        }
    }

    pub(super) fn apply_volume(&self) {
        if let Some(p) = &self.player {
            p.set_volume(self.settings.volume);
        }
    }

    /// Netteté et agrandissement de l'image (Réglages). Tant qu'aucune des deux options n'a été activée,
    /// le lecteur n'est pas modifié (comportement d'origine).
    pub(super) fn apply_video_options(&mut self) {
        let (sharpen, upscale) = (
            self.settings.video_sharpen.min(3),
            self.settings.video_upscale,
        );
        if sharpen == 0 && !upscale && !self.video_opts_set {
            return;
        }
        let Some(p) = &self.player else {
            return;
        };
        self.video_opts_set = true;
        let base = self
            .scale_default
            .get_or_insert_with(|| {
                p.get_property("scale")
                    .unwrap_or_else(|| "bilinear".to_string())
            })
            .clone();
        let scale = if upscale {
            "ewa_lanczossharp"
        } else {
            base.as_str()
        };
        if let Err(e) = p.set_property("scale", scale) {
            crate::logging::log(&format!("Agrandissement : {e}"));
        }
        let shader = if sharpen == 0 {
            Some(String::new())
        } else {
            sharpen_shader(sharpen)
        };
        match shader {
            Some(path) => {
                if let Err(e) = p.set_property("glsl-shaders", &path) {
                    crate::logging::log(&format!("Netteté : {e}"));
                }
            }
            None => crate::logging::log("Netteté : fichier du filtre impossible à écrire"),
        }
    }

    /// Décodage vidéo matériel (Réglages > Lecture vidéo). « Automatique » ne touche à rien : le lecteur garde la valeur
    /// de `mpv::MPV_OPTIONS` (`auto-safe`). « Activé » = `auto` (essaie tous les décodeurs de la carte graphique ; mpv revient seul
    /// au décodage normal s'ils échouent), « Désactivé » = `no`. Appelé avant chaque chargement de flux ; les Réglages
    /// relancent le live en cours quand ce réglage change. Revenir sur « Automatique » restaure la valeur d'origine relue au premier changement.
    pub(super) fn apply_hwdec(&mut self) {
        let mode = self.settings.hw_decode.min(2);
        if mode == 0 && !self.hwdec_set {
            return;
        }
        let Some(p) = &self.player else {
            return;
        };
        let base = self
            .hwdec_default
            .get_or_insert_with(|| {
                p.get_property("hwdec")
                    .unwrap_or_else(|| "auto-safe".to_string())
            })
            .clone();
        let value = match mode {
            1 => "auto",
            2 => "no",
            _ => base.as_str(),
        };
        match p.set_property("hwdec", value) {
            Ok(()) => self.hwdec_set = mode != 0,
            Err(e) => crate::logging::log(&format!("Décodage matériel : {e}")),
        }
    }

    /// Relit la liste des périphériques de sortie connus de mpv ; `false` si le lecteur n'existe pas ou ne répond pas.
    fn refresh_audio_devices(&mut self) -> bool {
        let Some(list) = self
            .player
            .as_ref()
            .and_then(|p| p.get_property("audio-device-list"))
        else {
            return false;
        };
        self.audio_devices = parse_audio_devices(&list);
        self.audio_dev_check = Instant::now();
        true
    }

    /// Applique le périphérique de sortie choisi (Réglages > Lecture vidéo > Audio). Vide = périphérique par défaut :
    /// rien n'est touché tant que l'utilisateur n'a rien choisi (comportement d'origine), puis `auto` est remis si besoin.
    /// Un périphérique absent de la liste (casque débranché) est abandonné : retour au choix par défaut et message discret.
    pub(super) fn apply_audio_device(&mut self) {
        if self.settings.audio_device.is_empty() && !self.audio_dev_set {
            return;
        }
        if !self.settings.audio_device.is_empty()
            && self.refresh_audio_devices()
            && !self
                .audio_devices
                .iter()
                .any(|(n, _)| *n == self.settings.audio_device)
        {
            self.settings.audio_device.clear();
            self.audio_dev_lost = true;
            self.info_toast = Some((
                Instant::now(),
                crate::i18n::tr(AUDIO_DEVICE_LOST).to_string(),
            ));
        }
        let Some(p) = &self.player else {
            return;
        };
        let name = if self.settings.audio_device.is_empty() {
            "auto"
        } else {
            self.settings.audio_device.as_str()
        };
        match p.set_property("audio-device", name) {
            Ok(()) => self.audio_dev_set = !self.settings.audio_device.is_empty(),
            Err(e) => crate::logging::log(&format!("Périphérique audio : {e}")),
        }
    }

    /// Surveille la liste des périphériques toutes les 2 s, seulement si un périphérique précis est choisi ou si la page
    /// « Lecture vidéo » des Réglages est ouverte (liste à jour au branchement). Aucun réveil en fond pour le choix par défaut.
    pub(super) fn tick_audio_device(&mut self, ctx: &egui::Context) {
        let settings_open =
            self.view == View::Settings && self.settings_tab == super::settings::TAB_PLAYBACK;
        if self.player.is_none() && settings_open && !self.audio_dev_tried {
            // Le lecteur est créé ici pour pouvoir lister les périphériques sans avoir ouvert de live.
            self.audio_dev_tried = true;
            let _ = self.ensure_player(ctx);
        }
        let chosen = !self.settings.audio_device.is_empty();
        if self.player.is_none() || !(chosen || settings_open) {
            return;
        }
        const EVERY: Duration = Duration::from_secs(2);
        if self.audio_dev_check.elapsed() < EVERY {
            ctx.request_repaint_after(EVERY - self.audio_dev_check.elapsed());
            return;
        }
        if !self.refresh_audio_devices() {
            return;
        }
        if chosen
            && !self
                .audio_devices
                .iter()
                .any(|(n, _)| *n == self.settings.audio_device)
        {
            self.apply_audio_device();
        }
        ctx.request_repaint_after(EVERY);
    }

    /// Applique (ou retire) la normalisation du son sur le flux en cours.
    pub(super) fn apply_audio_filter(&self) {
        if let Some(p) = &self.player {
            if let Err(e) = p.set_property("af", audio_filter(self.settings.normalize_audio)) {
                crate::logging::log(&format!("Filtre audio : {e}"));
            }
        }
    }

    /// Change le volume (molette, flèches) et affiche l'indication sur la vidéo.
    pub(super) fn change_volume(&mut self, delta: f32) {
        self.volume_acc += delta;
        let whole = self.volume_acc.trunc();
        if whole != 0.0 {
            self.volume_acc -= whole;
            let v = (f32::from(self.settings.volume) + whole)
                .clamp(0.0, f32::from(self.settings.volume_max));
            self.settings.volume = v as u8;
            self.apply_volume();
        }
        self.volume_toast = Some(Instant::now());
    }

    pub(super) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if let Some(p) = &self.player {
            p.set_pause(self.paused);
        }
    }

    pub(super) fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if let Some(p) = &self.player {
            let _ = p.set_property("mute", if self.muted { "yes" } else { "no" });
        }
    }

    pub(super) fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        if on && self.pip {
            self.exit_pip(ctx);
        }
        if on {
            self.pip_wait = None; // plein écran redemandé pendant l'attente du PiP (macOS) : le PiP est abandonné
        }
        self.fullscreen = on;
        self.geometry_hold = Instant::now() + Duration::from_millis(1200);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        crate::procs::set_window_corners(!on);
    }

    pub(super) fn toggle_pip(&mut self, ctx: &egui::Context) {
        if self.pip {
            self.exit_pip(ctx);
        } else {
            self.enter_pip(ctx);
        }
    }

    pub(super) fn enter_pip(&mut self, ctx: &egui::Context) {
        self.pip_wait = None;
        // macOS : le plein écran (celui de l'appli comme celui du bouton vert) est un Bureau à part, quitté avec une
        // animation. Réduire la fenêtre pendant celle-ci la laisse dans ce Bureau, sur fond noir, impossible à déplacer :
        // on quitte d'abord le plein écran, et le PiP suit quand la fenêtre a fini de bouger (`tick_pip_wait`).
        if theme::NATIVE_FRAME
            && (self.fullscreen || ctx.input(|i| i.viewport().fullscreen == Some(true)))
        {
            if self.fullscreen {
                self.set_fullscreen(ctx, false);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            let now = Instant::now();
            self.pip_wait = Some((now, egui::Vec2::ZERO, now));
            ctx.request_repaint();
            return;
        }
        if self.fullscreen {
            self.set_fullscreen(ctx, false);
        }
        let (size, pos, monitor, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect.map(|r| r.size()),
                v.outer_rect.map(|r| r.min),
                v.monitor_size,
                v.maximized.unwrap_or(false),
            )
        });
        self.pip_restore = size.map(|s| (s, pos));
        self.pip_was_maximized = maximized;
        // Taille et position de la dernière fois, si elles sont encore raisonnables pour l'écran actuel.
        let pip_size = self
            .settings
            .pip_size
            .map(|s| egui::vec2(s[0].clamp(320.0, 3840.0), s[1].clamp(180.0, 2160.0)))
            .filter(|s| monitor.is_none_or(|m| s.x < m.x * 0.8 && s.y < m.y * 0.8))
            .unwrap_or(egui::vec2(480.0, 270.0));
        let saved_pos = self
            .settings
            .pip_pos
            .map(|p| egui::pos2(p[0], p[1]))
            .filter(|p| {
                monitor.is_some_and(|m| {
                    p.x > -m.x && p.x <= 2.0 * m.x - 100.0 && p.y > -m.y && p.y <= 2.0 * m.y - 100.0
                })
            });
        self.pip = true;
        // Le réglage « au format de la vidéo » attend que la nouvelle taille soit appliquée (sinon il part de l'ancienne).
        self.pip_fit_at = Instant::now();
        self.geometry_hold = Instant::now() + Duration::from_millis(1200);
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        // macOS : le cadre natif reste (boutons ronds au-dessus de la vidéo) ; le retirer puis le remettre perdrait le
        // contenu pleine hauteur (voir `theme::window_frame`). Windows : la fenêtre n'a déjà aucune bordure.
        if !theme::NATIVE_FRAME {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        }
        self.pip_on_top = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            320.0, 180.0,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(pip_size));
        if let Some(p) = saved_pos {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
        } else if let Some(m) = monitor {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                (m.x - pip_size.x - 24.0).max(0.0),
                (m.y - pip_size.y - 64.0).max(0.0),
            )));
        }
    }

    /// Option « Fenêtre PiP au format de la vidéo » : ajuste la hauteur (ou la largeur, au minimum) de la fenêtre au
    /// format du live. Vérifié deux fois par seconde, jamais pendant un déplacement ou un redimensionnement.
    pub(super) fn pip_fit_ratio(&mut self, ctx: &egui::Context) {
        if !self.settings.pip_fit
            || self.pip_fit_at.elapsed() < Duration::from_millis(500)
            || ctx.input(|i| i.pointer.any_down())
        {
            return;
        }
        self.pip_fit_at = Instant::now();
        let Some(p) = &self.player else { return };
        let num = |name: &str| {
            p.get_property(name)
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0)
        };
        let (Some(vw), Some(vh)) = (num("dwidth"), num("dheight")) else {
            return;
        };
        let ratio = vw / vh;
        let cur = ctx.content_rect().size();
        if cur.x < 1.0 || cur.y < 1.0 || (cur.x / cur.y - ratio).abs() / ratio < 0.01 {
            return;
        }
        let (mut w, mut h) = (cur.x, cur.x / ratio);
        if h < 180.0 {
            h = 180.0;
            w = h * ratio;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            w.round(),
            h.round(),
        )));
    }

    /// Option « Lecteur au format de la vidéo » : réduit la zone vidéo au format du live, centrée dans la place
    /// disponible (sans bandes autour de l'image). Sans format connu, la zone reste entière.
    pub(super) fn fit_video_rect(&mut self, ctx: &egui::Context, rect: egui::Rect) -> egui::Rect {
        let ratio = self.video_ratio(ctx);
        fit_ratio(rect, ratio)
    }

    /// Format (largeur / hauteur) de l'image du live : lu dans mpv (au plus deux fois par seconde), sinon le dernier
    /// connu, sinon 16/9.
    pub(super) fn video_ratio(&mut self, ctx: &egui::Context) -> f32 {
        // Le dernier format connu (lu avec les réglages) sert dès l'ouverture : aucun saut quand le flux démarre.
        let mut ratio = if self.settings.video_ratio.is_finite() && self.settings.video_ratio > 0.2
        {
            self.settings.video_ratio
        } else {
            16.0 / 9.0
        };
        if let Some(p) = &self.player {
            let id = egui::Id::unique("video_ratio");
            let now = ctx.input(|i| i.time);
            let cached: Option<(Option<f32>, f64)> = ctx.data(|d| d.get_temp(id));
            let live = match cached {
                Some((r, t0)) if now - t0 < 0.5 => r,
                _ => {
                    let num = |name: &str| {
                        p.get_property(name)
                            .and_then(|v| v.parse::<f32>().ok())
                            .filter(|v| *v > 0.0)
                    };
                    let r = match (num("dwidth"), num("dheight")) {
                        (Some(w), Some(h)) => Some(w / h),
                        _ => None,
                    };
                    ctx.data_mut(|d| d.insert_temp(id, (r, now)));
                    r
                }
            };
            if let Some(r) = live {
                if (r - ratio).abs() / ratio > 0.005 {
                    self.settings.video_ratio = r;
                }
                ratio = r;
            }
        }
        ratio
    }

    pub(super) fn exit_pip(&mut self, ctx: &egui::Context) {
        if !self.pip {
            return;
        }
        self.pip = false;
        // Mémorise la mini-fenêtre telle que l'utilisateur l'a placée.
        let (size, pos) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect.map(|r| r.size()), v.outer_rect.map(|r| r.min))
        });
        // Rien n'est mémorisé si la fenêtre n'a pas changé de taille (le passage en PiP n'a pas eu lieu).
        let unchanged = size
            .zip(self.pip_restore.map(|(s, _)| s))
            .is_some_and(|(a, b)| (a.x - b.x).abs() < 2.0 && (a.y - b.y).abs() < 2.0);
        if let (Some(s), Some(p), false) = (size, pos, unchanged) {
            self.settings.pip_size = Some([s.x.round(), s.y.round()]);
            self.settings.pip_pos = Some([p.x.round(), p.y.round()]);
        }
        self.geometry_hold = Instant::now() + Duration::from_millis(1200);
        apply_on_top(ctx, self.settings.on_top);
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            crate::config::MIN_WINDOW[0],
            crate::config::MIN_WINDOW[1],
        )));
        if let Some((size, pos)) = self.pip_restore.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            if let Some(p) = pos {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
            }
        }
        if std::mem::take(&mut self.pip_was_maximized) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
    }

    /// macOS : PiP demandé depuis le plein écran. Il est appliqué au moins 1 s après la demande de sortie (durée de
    /// l'animation du système, que winit ne signale pas : il annonce la sortie dès la demande) et quand la taille de la
    /// fenêtre n'a plus bougé depuis 0,3 s ; abandonné si le live est fermé, ou si le plein écran n'est toujours pas
    /// quitté au bout de 4 s.
    pub(super) fn tick_pip_wait(&mut self, ctx: &egui::Context) {
        const CALME: Duration = Duration::from_millis(300);
        const ANIMATION: Duration = Duration::from_millis(1000);
        let Some((start, last, changed)) = self.pip_wait else {
            return;
        };
        if self.view != View::Watch || self.watch.is_none() || self.pip {
            self.pip_wait = None;
            return;
        }
        let (fullscreen, size) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.fullscreen.unwrap_or(false),
                v.inner_rect.map_or(last, |r| r.size()),
            )
        });
        let now = Instant::now();
        let changed = if (size - last).length() > 0.5 {
            now
        } else {
            changed
        };
        self.pip_wait = Some((start, size, changed));
        if !fullscreen
            && now.duration_since(changed) >= CALME
            && now.duration_since(start) >= ANIMATION
        {
            self.pip_wait = None;
            self.enter_pip(ctx);
        } else if now.duration_since(start) >= Duration::from_secs(4) {
            self.pip_wait = None;
            crate::logging::log(if fullscreen {
                "PiP abandonné : le plein écran n'a pas été quitté"
            } else {
                "PiP appliqué après 4 s d'attente de la fin du plein écran"
            });
            if !fullscreen {
                self.enter_pip(ctx);
            }
        } else {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    pub(super) fn leave_immersive(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            self.set_fullscreen(ctx, false);
        }
        self.exit_pip(ctx);
    }
}

/// Plus grand rectangle de ce format, centré dans `rect`.
pub(super) fn fit_ratio(rect: egui::Rect, ratio: f32) -> egui::Rect {
    if ratio <= 0.0 || rect.width() < 1.0 || rect.height() < 1.0 {
        return rect;
    }
    let (mut w, mut h) = (rect.width(), rect.width() / ratio);
    if h > rect.height() {
        h = rect.height();
        w = h * ratio;
    }
    egui::Rect::from_center_size(rect.center(), egui::vec2(w.floor(), h.floor()))
}

/// Filtre de netteté (masque flou sur l'image agrandie, calculé par la carte graphique) écrit dans le
/// dossier de l'application ; renvoie son chemin.
fn sharpen_shader(level: u8) -> Option<String> {
    let force = [0.0, 0.15, 0.3, 0.5][usize::from(level.min(3))];
    let code = format!(
        "//!HOOK SCALED\n//!BIND HOOKED\n//!DESC Velka : nettete\n\
         vec4 hook() {{\n\
         vec4 c = HOOKED_tex(HOOKED_pos);\n\
         vec4 n = HOOKED_texOff(vec2(0.0, -1.0)) + HOOKED_texOff(vec2(0.0, 1.0)) + HOOKED_texOff(vec2(-1.0, 0.0)) + HOOKED_texOff(vec2(1.0, 0.0));\n\
         vec3 s = c.rgb + {force:.2} * (4.0 * c.rgb - n.rgb);\n\
         return vec4(clamp(s, 0.0, 1.0), c.a);\n\
         }}\n"
    );
    let dir = crate::logging::data_dir().join("shaders");
    let path = dir.join(format!("nettete-{level}.glsl"));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(code.as_str()) {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, &code).ok()?;
    }
    Some(path.to_string_lossy().into_owned())
}

/// Texte du message (bulle et Réglages) quand le dossier des captures choisi a disparu.
pub(super) const SHOT_DIR_LOST: &str =
    "Le dossier des captures n'est plus disponible : retour au dossier par défaut.";

/// Texte affiché quand le dossier choisi dans la fenêtre n'accepte pas l'écriture (il n'est pas retenu).
pub(super) const SHOT_DIR_REFUSED: &str =
    "Ce dossier n'accepte pas l'écriture : choisissez-en un autre.";

/// Dossier d'origine des captures : « Images\Velka » (à défaut, un dossier « captures » des données de l'application).
pub(super) fn default_screenshot_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|d| d.picture_dir().map(|p| p.join("Velka")))
        .unwrap_or_else(|| crate::logging::data_dir().join("captures"))
}

/// Vrai si le dossier existe et accepte l'écriture (un petit fichier d'essai est créé puis supprimé).
pub(super) fn dir_writable(dir: &std::path::Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".velka-ecriture-{}.tmp", std::process::id()));
    let ok = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Texte du message (bulle et Réglages) quand le périphérique audio choisi a disparu.
pub(super) const AUDIO_DEVICE_LOST: &str =
    "Le périphérique audio choisi n'est plus disponible : retour au périphérique par défaut.";

/// Périphériques réels (nom mpv, description) d'après la propriété `audio-device-list` (JSON). L'entrée `auto` (choix
/// par défaut de mpv) est écartée : elle correspond à « Périphérique par défaut du système ». JSON illisible : liste vide.
pub(super) fn parse_audio_devices(json: &str) -> Vec<(String, String)> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|d| {
            let name = d.get("name")?.as_str()?;
            if name.is_empty() || name == "auto" {
                return None;
            }
            let desc = d
                .get("description")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(name);
            Some((name.to_string(), desc.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{dir_writable, parse_audio_devices};

    #[test]
    fn dossier_des_captures_doit_accepter_l_ecriture() {
        let dir = std::env::temp_dir().join(format!("velka-test-captures-{}", std::process::id()));
        assert!(!dir_writable(&dir), "dossier absent");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(dir_writable(&dir));
        assert!(
            std::fs::read_dir(&dir).unwrap().next().is_none(),
            "le fichier d'essai est supprimé"
        );
        let file = dir.join("fichier.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(!dir_writable(&file), "un fichier n'est pas un dossier");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn liste_des_peripheriques_audio() {
        let json = r#"[{"name":"auto","description":"Autoselect device"},
            {"name":"wasapi/{1}","description":"Haut-parleurs (Realtek)"},
            {"name":"wasapi/{2}","description":""},
            {"description":"sans nom"}]"#;
        let l = parse_audio_devices(json);
        assert_eq!(
            l,
            vec![
                (
                    "wasapi/{1}".to_string(),
                    "Haut-parleurs (Realtek)".to_string()
                ),
                ("wasapi/{2}".to_string(), "wasapi/{2}".to_string())
            ]
        );
        assert!(parse_audio_devices("pas du json").is_empty());
        assert!(parse_audio_devices("{}").is_empty());
        assert!(parse_audio_devices("[]").is_empty());
    }
}
