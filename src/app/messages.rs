//! Messages des fils d'arrière-plan (flux, chat, réseau) et envoi dans le chat.
use super::*;

impl VelkaApp {
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Progress(t, f, m) => self.set_tool(t, ToolStatus::Installing(f, m)),
                Msg::Done(t, Ok(p)) => {
                    // Le composant vient d'être installé : on ne le montre plus « à mettre à jour » en attendant le
                    // contrôle qui relira les versions sur le disque (fin de l'opération).
                    if let tools::ToolsCheck::Done(reports) = &mut self.tools_check {
                        if let Some(r) = reports.iter_mut().find(|r| r.tool == t) {
                            r.state = tools::UpdateState::UpToDate;
                            r.installed = Some(r.latest.clone());
                            r.path = Some(p.clone());
                        }
                    }
                    self.set_tool(t, ToolStatus::Ready(p.display().to_string()));
                }
                Msg::Done(t, Err(e)) => {
                    self.tool_errors.push((t, e.clone()));
                    self.set_tool(t, ToolStatus::Failed(e));
                }
                Msg::Finished => {
                    self.busy = false;
                    if super::apercu_outils() {
                        continue; // aperçu : pas de relecture du disque ni de contrôle de versions
                    }
                    if self.view == View::FirstRun {
                        if self
                            .rows
                            .iter()
                            .all(|r| matches!(r.status, ToolStatus::Ready(_)))
                        {
                            self.view = View::Home;
                        }
                    } else {
                        // Un composant dont la mise à jour a échoué garde sa version précédente : on relit le disque.
                        self.redetect_tools();
                    }
                    // Après chaque mise à jour : on relit les versions réellement installées et on les compare.
                    self.tools_check = tools::ToolsCheck::Idle;
                    self.start_tools_check(ctx, true);
                }
                Msg::ToolsChecked(res) => self.on_tools_checked(ctx, res),
                Msg::Diagnostic(text) => {
                    ctx.copy_text(text);
                    self.diag_copied = true;
                }
                Msg::TokenRefreshed(sent, res) => self.on_token_refreshed(sent, res),
                Msg::UpdateInfo(res) => {
                    self.update_inflight = false;
                    let manual = std::mem::take(&mut self.update_manual);
                    match res {
                        Ok(info) => {
                            self.update_info = info;
                            if manual {
                                self.update_check = maj::UpdateCheck::Done;
                            }
                        }
                        Err(e) => {
                            crate::logging::log(&format!("Recherche de mise à jour échouée : {e}"));
                            if manual {
                                self.update_check = maj::UpdateCheck::Failed(e);
                            }
                        }
                    }
                }
                Msg::UpdateProgress(f) => self.update_progress = Some(f),
                Msg::UpdateReady(Ok(path)) => match crate::update::apply(&path) {
                    Ok(()) => {
                        crate::logging::log("Mise à jour installée, redémarrage.");
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => {
                        crate::logging::log(&format!("Mise à jour non appliquée : {e}"));
                        self.update_progress = None;
                        self.update_error = Some(e);
                    }
                },
                Msg::UpdateReady(Err(e)) => {
                    crate::logging::log(&format!("Mise à jour non téléchargée : {e}"));
                    self.update_progress = None;
                    self.update_error = Some(e);
                }
                Msg::Follows(res) => match res {
                    Ok(snap) => {
                        if let Some(names) = snap.names {
                            self.followed = names;
                            self.follows_names_at = Some(Instant::now());
                        }
                        // Une chaîne suivie qui n'est plus dans la liste des directs est hors ligne
                        // (les favoris ont leur propre rafraîchissement).
                        let ended: Vec<String> = self
                            .followed
                            .iter()
                            .filter(|l| {
                                !self.settings.favorites.contains(l)
                                    && !snap.live.iter().any(|(n, _)| n == *l)
                            })
                            .filter(|l| {
                                matches!(
                                    self.status_cache.get(*l).map(|(_, s)| s),
                                    Some(LiveStatus::Live { .. })
                                )
                            })
                            .cloned()
                            .collect();
                        for l in ended {
                            self.set_status(l, LiveStatus::Offline);
                        }
                        for (l, st) in snap.live {
                            // Chaîne suivie depuis Twitch après le dernier chargement de la liste complète (toutes les
                            // 10 minutes) : elle est en direct, donc forcément suivie, on l'ajoute tout de suite.
                            if !self.followed.contains(&l) {
                                self.followed.push(l.clone());
                            }
                            self.set_status(l, st);
                        }
                        self.follows_state = FollowsState::Ready;
                    }
                    Err(crate::follows::FollowsError::NeedsReconnect) => {
                        self.followed.clear();
                        self.follows_state = FollowsState::NeedsReconnect;
                    }
                    Err(crate::follows::FollowsError::Failed(m)) => {
                        crate::logging::log(&format!("Chaînes suivies : {m}"));
                        self.follows_state = FollowsState::Failed;
                    }
                    // Recul en cours (429) : la liste déjà affichée est gardée telle quelle.
                    Err(crate::follows::FollowsError::Paused) => {
                        self.follows_state = if self.followed.is_empty() {
                            FollowsState::Idle
                        } else {
                            FollowsState::Ready
                        };
                    }
                },
                Msg::SettingsInfo(cache, startup) => {
                    self.settings_info_inflight = false;
                    self.cache_size = Some(cache);
                    self.startup_on = Some(startup);
                }
                Msg::StartupSet(on, ok) => {
                    self.startup_busy = false;
                    self.startup_on = Some(if ok { on } else { !on });
                }
                Msg::CacheCleared => {
                    self.cache_clearing = false;
                    self.cache_size = None; // relue en arrière-plan par `settings_prepare`
                }
                Msg::Recs(login, game, same_game, top) => {
                    self.recs_inflight = false;
                    self.recs = Some(RecsData {
                        login,
                        game,
                        same_game,
                        top,
                    });
                }
                Msg::Suggest(fragment, list) => {
                    self.suggest_inflight = false;
                    if fragment == self.suggest_want {
                        self.suggestions = list;
                    }
                }
                Msg::Statuses(map) => {
                    self.status_inflight = false;
                    for (l, st) in map {
                        self.set_status(l, st);
                    }
                }
                Msg::Status(g, st) if g == self.generation => {
                    let error = match &st {
                        LiveStatus::Offline => Some(crate::i18n::tr("La chaîne est hors ligne.")),
                        LiveStatus::Missing => Some(crate::i18n::tr("Cette chaîne n'existe pas.")),
                        _ => None,
                    };
                    let remember = if st == LiveStatus::Missing {
                        None
                    } else {
                        self.watch.as_ref().map(|w| w.login.clone())
                    };
                    if let Some(w) = &mut self.watch {
                        w.status = Some(st);
                    }
                    if let Some(login) = remember {
                        self.remember_channel(&login);
                    }
                    match error {
                        Some(m) => self.set_state(WatchState::Error(m.into())),
                        None => self.begin_stream(ctx),
                    }
                }
                Msg::Stream(g, ev) if g == self.generation => match ev {
                    StreamEvent::Ready(port) => {
                        self.apply_video_options();
                        self.apply_hwdec();
                        self.apply_audio_device();
                        if let Some(p) = &self.player {
                            p.set_volume(self.settings.volume);
                            let _ = p.set_property("mute", if self.muted { "yes" } else { "no" });
                            let _ =
                                p.set_property("af", audio_filter(self.settings.normalize_audio));
                            let profile = if self.settings.low_latency {
                                "low-latency"
                            } else {
                                "default"
                            };
                            let _ = p.set_property("profile", profile);
                            match p.load(&format!("http://127.0.0.1:{port}/")) {
                                Ok(()) => self.set_state(WatchState::Playing),
                                Err(e) => self.set_state(WatchState::Error(e)),
                            }
                        }
                    }
                    StreamEvent::Break(on) => {
                        if let Some(w) = &mut self.watch {
                            w.on_break = on;
                        }
                    }
                    StreamEvent::Failed(e) => {
                        let was_playing = self
                            .watch
                            .as_ref()
                            .is_some_and(|w| w.state == WatchState::Playing);
                        self.stop_stream();
                        let mut msg = e.message();
                        // Internet coupé pendant un live : message de coupure, pas celui de l'ouverture d'un live.
                        if e == StreamError::NoInternet && was_playing {
                            msg = crate::i18n::tr("La connexion Internet s'est coupée.").into();
                        }
                        self.set_state(WatchState::Error(msg));
                    }
                },
                Msg::NetProbe(g, up) => {
                    self.net_probe_busy = false;
                    let playing = self
                        .watch
                        .as_ref()
                        .is_some_and(|w| w.state == WatchState::Playing);
                    let lost = self.watch.as_ref().is_some_and(Watch::net_lost);
                    if g == self.generation && lost && up {
                        // Connexion revenue : le live se relance seul, comme avec « Réessayer ».
                        self.net_probe_fails = 0;
                        if let Some(login) = self.watch.as_ref().map(|w| w.login.clone()) {
                            crate::logging::log("Connexion Internet revenue : le live est relancé");
                            self.start_watch(ctx, login);
                        }
                    } else if g != self.generation || !playing || up {
                        self.net_probe_fails = 0;
                    } else {
                        self.net_probe_fails += 1;
                        if self.net_probe_fails >= NET_PROBE_MAX_FAILS {
                            self.net_probe_fails = 0;
                            crate::logging::log("Connexion Internet coupée pendant le live");
                            self.stop_stream();
                            self.set_state(WatchState::Error(
                                crate::i18n::tr("La connexion Internet s'est coupée.").into(),
                            ));
                        }
                    }
                }
                Msg::Auth(ev) => match ev {
                    auth::AuthEvent::Code(d) => {
                        ctx.open_url(egui::OpenUrl::new_tab(d.verification_uri.clone()));
                        self.auth_code = Some(d);
                    }
                    auth::AuthEvent::Done { tokens, login } => {
                        self.settings.oauth_token = tokens.access_token;
                        self.settings.oauth_refresh = tokens.refresh_token;
                        self.settings.oauth_expires_at = tokens.expires_at;
                        self.settings.account_login = login;
                        self.follows_state = FollowsState::Idle;
                        self.follows_at = None;
                        self.follows_names_at = None;
                        self.auth_code = None;
                        self.auth_cancel = None;
                        self.auth_error = None;
                        // Compte récupéré : l'interface s'actualise tout de suite (statuts et chaînes suivies).
                        self.last_status_req = None;
                        ctx.request_repaint();
                        // Connexion établie depuis les Réglages : ils se referment.
                        if self.view == View::Settings {
                            self.go_back(ctx);
                        }
                    }
                    auth::AuthEvent::Failed(m) => {
                        self.auth_code = None;
                        self.auth_cancel = None;
                        self.auth_error = Some(m);
                    }
                },
                Msg::Chat(g, ev) if g == self.chat_epoch => match ev {
                    ChatEvent::State(s) => self.chat.conn = s,
                    ChatEvent::Auth(a) => self.chat.auth = a,
                    ChatEvent::FollowersOnly(on) => self.chat.followers_only = on,
                    ChatEvent::Exempt(e) => self.chat.exempt = e,
                    ChatEvent::Message(mut m) => {
                        if !(self.settings.chat_hide_bots && crate::chat::parse::is_bot(&m.user)) {
                            if let (true, Some(x)) =
                                (self.settings.chat_third_party, &self.chat_extras)
                            {
                                crate::chat::extra::apply(&x.emotes, &mut m.segments);
                            }
                            m.highlight =
                                crate::chat::parse::contains_word(&self.settings.chat_words, &m);
                            m.time = crate::procs::local_hhmm();
                            self.chat.push(m);
                        }
                    }
                },
                Msg::ChatExtras(e, x) if e == self.chat_epoch => {
                    self.chat_extras = Some(std::sync::Arc::new(x))
                }
                // Multistream : un live déjà retiré n'a plus de vignette, son message est ignoré.
                Msg::MultiStatus(id, st) => self.on_multi_status(ctx, id, st),
                Msg::MultiStream(id, ev) => self.on_multi_stream(id, ev),
                // Messages d'une session précédente : ignorés.
                Msg::Status(..) | Msg::Stream(..) | Msg::Chat(..) | Msg::ChatExtras(..) => {}
            }
        }
    }

    /// Envoie un message dans le chat (jeton valide requis) et l'affiche tout de suite.
    /// Chat en mode followers-only alors que le compte ne suit pas la chaîne : Twitch refuserait le message (il
    /// s'afficherait ici sans être publié). Faux si la liste des chaînes suivies n'est pas chargée (on ne sait pas).
    pub(super) fn followers_locked(&self) -> bool {
        if !self.chat.followers_only
            || self.chat.exempt
            || self.follows_state != FollowsState::Ready
        {
            return false;
        }
        let Some(w) = &self.watch else {
            return false;
        };
        let login = w.login.to_lowercase();
        let own =
            matches!(&self.chat.auth, AuthState::Authenticated(me) if me.to_lowercase() == login);
        !own && !self.followed.iter().any(|f| f.to_lowercase() == login)
    }

    pub(super) fn send_chat(&mut self, text: &str) {
        let AuthState::Authenticated(me) = self.chat.auth.clone() else {
            return;
        };
        if self.followers_locked() {
            return;
        }
        let Some(clean) = crate::chat::client::sanitize_outgoing(text) else {
            return;
        };
        if let Some(c) = &self.chat_client {
            c.send(&clean);
            // Twitch ne renvoie pas nos propres messages : on les ajoute localement.
            self.chat.push(ChatMessage {
                user: me,
                color: None,
                segments: vec![Segment::Text(clean)],
                is_action: false,
                is_reply: false,
                is_mention: false,
                highlight: false,
                time: crate::procs::local_hhmm(),
                badges: Vec::new(),
            });
            self.chat_jump = true;
        }
    }
}
