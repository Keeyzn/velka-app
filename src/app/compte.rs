//! Compte Twitch : connexion, jeton et chaînes suivies.
use super::*;

impl VelkaApp {
    /// Lance « Se connecter avec Twitch » : la page d'autorisation s'ouvre dans le navigateur.
    pub(super) fn start_login(&mut self, ctx: &egui::Context) {
        if self.auth_cancel.is_some() || !auth::available() {
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.auth_cancel = Some(cancel.clone());
        self.auth_error = None;
        self.auth_cancelled_at = None;
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            auth::run_device_flow(&cancel, &|ev| {
                // Tentative annulée entre-temps : ses résultats tardifs (page à ouvrir, échec, connexion) sont ignorés,
                // pour qu'un nouvel essai reparte de zéro.
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let _ = tx.send(Msg::Auth(ev));
                ctx.request_repaint();
            });
        });
    }

    /// Annule la connexion en cours (bouton « Annuler ») : on peut aussitôt réessayer. L'annulation est annoncée comme
    /// telle (« Connexion annulée »), pas comme une erreur.
    pub(super) fn cancel_login(&mut self) {
        if let Some(c) = self.auth_cancel.take() {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
            self.auth_cancelled_at = Some(Instant::now());
            self.auth_error = None;
        }
        self.auth_code = None;
    }

    pub(super) fn logout(&mut self) {
        // Jeton révoqué chez Twitch en arrière-plan, puis effacé ici.
        let token = std::mem::take(&mut self.settings.oauth_token);
        crate::auth::forget_identity();
        self.identity_at = None;
        std::thread::spawn(move || crate::auth::revoke(&token));
        self.settings.oauth_token.clear();
        self.settings.oauth_refresh.clear();
        self.settings.oauth_expires_at = 0;
        self.settings.account_login.clear();
        self.followed.clear();
        self.follows_state = FollowsState::Idle;
        // Les deux boutons « Se déconnecter » repartent désarmés (une reconnexion rapide ne les trouve pas déjà armés).
        self.logout_armed = None;
        self.logout_armed_account = None;
    }

    pub(super) fn connected(&self) -> bool {
        !self.settings.oauth_token.is_empty() && !self.settings.account_login.is_empty()
    }

    /// Charge les chaînes suivies au démarrage, après la connexion, puis toutes les 10 minutes.
    pub(super) fn tick_follows(&mut self, ctx: &egui::Context) {
        // Rien en arrière-plan pendant qu'on regarde un live : on actualise seulement sur l'accueil.
        if self.view != View::Home
            || !self.settings.show_follows
            || !self.connected()
            || matches!(
                self.follows_state,
                FollowsState::Loading | FollowsState::NeedsReconnect
            )
        {
            return;
        }
        if let Some(t) = self.follows_at {
            match self.poll_interval(ctx) {
                Some(iv) if t.elapsed() < iv => {
                    ctx.request_repaint_after(iv - t.elapsed());
                    return;
                }
                None => return, // actualisation manuelle : le bouton remet `follows_at` à zéro
                _ => {}
            }
        }
        self.start_token_refresh(ctx);
        // Jeton expiré : on attend son renouvellement au lieu d'interroger Twitch avec un jeton mort.
        if !self.connected() || !self.token_usable() {
            return;
        }
        self.follows_state = FollowsState::Loading;
        self.follows_at = Some(Instant::now());
        let with_names = self.followed.is_empty()
            || self
                .follows_names_at
                .is_none_or(|t| t.elapsed() >= FOLLOWS_NAMES_REFRESH);
        let (tx, ctx, token) = (
            self.tx.clone(),
            ctx.clone(),
            self.settings.oauth_token.clone(),
        );
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Follows(crate::follows::fetch_snapshot(
                &token, with_names,
            )));
            ctx.request_repaint();
        });
    }

    /// Chaînes suivies sur Twitch en direct, hors favoris, les plus regardées d'abord.
    pub(super) fn live_followed(&self, limit: usize) -> Vec<String> {
        self.live_followed_with(limit, false)
    }

    /// Comme `live_followed`, avec `with_favorites` : garde aussi les chaînes qui sont aussi dans les favoris
    /// (barre latérale : une chaîne suivie sur Twitch passe devant, même si elle est en favori).
    pub(super) fn live_followed_with(&self, limit: usize, with_favorites: bool) -> Vec<String> {
        if !self.settings.show_follows {
            return Vec::new();
        }
        let viewers = |l: &String| match self.status_cache.get(l).map(|(_, s)| s) {
            Some(LiveStatus::Live { viewers, .. }) => Some(*viewers),
            _ => None,
        };
        let mut v: Vec<(u64, &String)> = self
            .followed
            .iter()
            .filter(|l| with_favorites || !self.settings.favorites.contains(l))
            .filter_map(|l| viewers(l).map(|n| (n, l)))
            .collect();
        v.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
        v.into_iter().take(limit).map(|(_, l)| l.clone()).collect()
    }

    /// Jeton utilisé pour le chat : celui du compte connecté, sinon aucun (lecture anonyme).
    /// (Le jeton du compte n'est jamais transmis à Streamlink.)
    pub(super) fn chat_token(&self) -> &str {
        if self.settings.account_login.is_empty() {
            ""
        } else {
            &self.settings.oauth_token
        }
    }

    /// Lance en arrière-plan le renouvellement du jeton s'il expire bientôt (jamais sur le fil d'interface).
    /// Après une coupure réseau, on réessaie au plus une fois par `TOKEN_RETRY`.
    pub(super) fn start_token_refresh(&mut self, ctx: &egui::Context) {
        let s = &self.settings;
        if s.oauth_refresh.is_empty() || !auth::needs_refresh(s.oauth_expires_at, auth::now_secs())
        {
            return;
        }
        if self.token_refresh_inflight {
            return;
        }
        if let Some(t) = self.token_retry_at {
            if t.elapsed() < TOKEN_RETRY {
                ctx.request_repaint_after(TOKEN_RETRY - t.elapsed());
                return;
            }
        }
        self.token_refresh_inflight = true;
        let sent = s.oauth_refresh.clone();
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let res = auth::refresh(&sent);
            let _ = tx.send(Msg::TokenRefreshed(sent, res));
            ctx.request_repaint();
        });
    }

    /// Le jeton du compte peut encore servir (pas expiré) : sinon on attend le renouvellement.
    pub(super) fn token_usable(&self) -> bool {
        auth::is_valid(self.settings.oauth_expires_at, auth::now_secs())
    }

    /// Résultat du renouvellement : seul un refus explicite de Twitch déconnecte le compte.
    pub(super) fn on_token_refreshed(
        &mut self,
        sent: String,
        res: Result<auth::Tokens, auth::RefreshError>,
    ) {
        self.token_refresh_inflight = false;
        // Déconnexion ou nouvelle connexion pendant l'appel : cette réponse ne concerne plus le compte actuel.
        if self.settings.oauth_refresh != sent {
            return;
        }
        match res {
            Ok(t) => {
                self.settings.oauth_token = t.access_token;
                self.settings.oauth_refresh = t.refresh_token;
                self.settings.oauth_expires_at = t.expires_at;
                self.token_retry_at = None;
            }
            Err(auth::RefreshError::Rejected) => {
                crate::logging::log(
                    "Compte Twitch : le renouvellement de la connexion a été refusé.",
                );
                // La connexion n'est plus valable : il faudra se reconnecter.
                self.logout();
            }
            Err(auth::RefreshError::Network) => {
                crate::logging::log("Compte Twitch : renouvellement impossible pour l'instant, nouvel essai plus tard.");
                self.token_retry_at = Some(Instant::now());
            }
        }
    }
}
