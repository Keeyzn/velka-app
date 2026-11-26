//! Statuts des chaînes (en direct, spectateurs), suggestions de recherche et notifications.
use super::*;

/// Intervalle minimal quand la fenêtre est réduite ou qu'une autre application est au premier plan.
const BACKGROUND_INTERVAL: Duration = Duration::from_secs(120);
/// Intervalle minimal quand l'application est au premier plan mais sans action depuis `IDLE_AFTER`.
const IDLE_INTERVAL: Duration = Duration::from_secs(60);
const IDLE_AFTER: Duration = Duration::from_secs(180);

/// Intervalle effectif d'actualisation : le réglage choisi quand on utilise l'application, au moins
/// 60 s au repos, au moins 120 s en arrière-plan ou fenêtre réduite. `None` (actualisation manuelle) reste `None`.
pub(super) fn effective_interval(
    base: Option<Duration>,
    focused: bool,
    minimized: bool,
    idle: Duration,
) -> Option<Duration> {
    let base = base?;
    Some(if minimized || !focused {
        base.max(BACKGROUND_INTERVAL)
    } else if idle >= IDLE_AFTER {
        base.max(IDLE_INTERVAL)
    } else {
        base
    })
}

impl VelkaApp {
    /// Intervalle d'actualisation pour l'état actuel de la fenêtre.
    pub(super) fn poll_interval(&self, ctx: &egui::Context) -> Option<Duration> {
        let (focused, minimized) = ctx.input(|i| {
            (
                i.viewport().focused.unwrap_or(true),
                i.viewport().minimized.unwrap_or(false),
            )
        });
        effective_interval(
            self.settings.refresh_interval(),
            focused,
            minimized,
            self.last_activity.elapsed(),
        )
    }

    /// Validation du jeton Twitch au plus une fois par heure, quelle que soit la vue (exigence Twitch).
    pub(super) fn tick_identity(&mut self) {
        if !self.connected()
            || !self.token_usable()
            || self
                .identity_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(3600))
        {
            return;
        }
        self.identity_at = Some(Instant::now());
        let token = self.settings.oauth_token.clone();
        if !crate::auth::identity_due(&token) {
            return;
        }
        std::thread::spawn(move || {
            if crate::auth::cached_identity(&token).is_none() {
                crate::logging::log("Compte Twitch : validation horaire du jeton impossible");
            }
        });
    }

    /// Enregistre un statut. L'apparition ou la fin d'un direct est prise en compte tout de suite ;
    /// le nombre de spectateurs d'un live déjà affiché n'est renouvelé qu'une fois par minute.
    pub(super) fn set_status(&mut self, login: String, mut st: LiveStatus) {
        let now = Instant::now();
        // Un favori qui passe de « hors ligne » à « en direct » : notification (jamais au premier relevé).
        if self.settings.notify_live
            && self.settings.favorites.contains(&login)
            && matches!(
                self.status_cache.get(&login),
                Some((_, LiveStatus::Offline))
            )
            && self.watching_login() != Some(login.as_str())
        {
            if let LiveStatus::Live { game, title, .. } = &st {
                let head = format!("{} {}", login, crate::i18n::tr("est en direct"));
                let body = match (game.is_empty(), title.is_empty()) {
                    (false, false) => format!("{game} · {title}"),
                    (false, true) => game.clone(),
                    (true, false) => title.clone(),
                    (true, true) => String::new(),
                };
                crate::procs::notify(&head, &body, &login);
            }
        }
        if let LiveStatus::Live { viewers, .. } = &mut st {
            let old = match self.status_cache.get(&login) {
                Some((_, LiveStatus::Live { viewers: v, .. })) => Some(*v),
                _ => None,
            };
            match (old, self.viewer_stamp.get(&login)) {
                (Some(v), Some(t)) if t.elapsed() < VIEWERS_REFRESH => *viewers = v,
                _ => {
                    self.viewer_stamp.insert(login.clone(), now);
                }
            }
        }
        self.status_cache.insert(login, (now, st));
    }

    /// Chaîne ouverte dans le lecteur (en grand ou en mini-lecteur).
    pub(super) fn watching_login(&self) -> Option<&str> {
        self.watch.as_ref().map(|w| w.login.as_str())
    }

    /// Notifications actives hors de l'accueil : relevé des favoris au plus une fois par minute.
    pub(super) fn tick_notify_poll(&mut self, ctx: &egui::Context) {
        if !self.settings.notify_live || self.settings.favorites.is_empty() || self.status_inflight
        {
            return;
        }
        let every = self
            .poll_interval(ctx)
            .unwrap_or(Duration::from_secs(60))
            .max(Duration::from_secs(60));
        if self.last_status_req.is_none_or(|t| t.elapsed() >= every) {
            self.request_statuses(ctx, self.settings.favorites.clone());
        }
    }

    /// Demande les chaînes qui commencent comme la saisie, 250 ms après la dernière touche.
    pub(super) fn tick_suggest(&mut self, ctx: &egui::Context) {
        let source = if self.startup_editing {
            &self.startup_draft
        } else if self.multi_editing {
            &self.multi_input
        } else {
            &self.input
        };
        let fragment = twitch::suggest_fragment(source).unwrap_or_default();
        let now = Instant::now();
        if fragment != self.suggest_want {
            self.suggest_want = fragment.clone();
            self.suggest_want_at = now;
        }
        if fragment.is_empty() {
            self.suggestions.clear();
            self.suggest_for.clear();
            return;
        }
        if fragment == self.suggest_for || self.suggest_inflight {
            return;
        }
        let wait = SUGGEST_DEBOUNCE.saturating_sub(now.duration_since(self.suggest_want_at));
        if !wait.is_zero() {
            ctx.request_repaint_after(wait);
            return;
        }
        self.suggest_for = fragment.clone();
        self.suggest_inflight = true;
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Suggest(
                fragment.clone(),
                twitch::fetch_suggestions(&fragment),
            ));
            ctx.request_repaint();
        });
    }

    pub(super) fn request_statuses(&mut self, ctx: &egui::Context, logins: Vec<String>) {
        if self.status_inflight || logins.is_empty() {
            return;
        }
        self.status_inflight = true;
        self.last_status_req = Some(Instant::now());
        self.poll_jitter = self
            .settings
            .refresh_interval()
            .map_or(Duration::ZERO, |iv| crate::net::random_delay(iv / 10));
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let mut all = HashMap::new();
            for chunk in logins.chunks(10) {
                all.extend(twitch::fetch_status(chunk));
            }
            let _ = tx.send(Msg::Statuses(all));
            ctx.request_repaint();
        });
    }

    /// Une chaîne vient d'être suivie : son statut est demandé tout de suite (sans décaler l'actualisation périodique).
    pub(super) fn refresh_after_follow(&mut self, ctx: &egui::Context, login: &str) {
        if self.status_inflight {
            // Une requête est en cours : la suivante (immédiate) redemandera tout, y compris cette chaîne.
            self.last_status_req = None;
            return;
        }
        let previous = self.last_status_req;
        self.request_statuses(ctx, vec![login.to_string()]);
        self.last_status_req = previous;
    }

    /// Vérification différée de la saisie, cache de 25 s, rafraîchissement toutes les 60 s.
    pub(super) fn tick_status(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if self.input != self.last_input {
            self.last_input = self.input.clone();
            self.input_changed = now;
        }
        let typed = crate::login::normalize(&self.input);
        let mut wanted: Vec<String> = Vec::new();
        for l in self
            .settings
            .favorites
            .iter()
            .chain(self.history.items.iter())
        {
            if !wanted.contains(l) {
                wanted.push(l.clone());
            }
        }
        if let Some(t) = &typed {
            if !wanted.contains(t) {
                wanted.push(t.clone());
            }
        }
        let since_input = now.duration_since(self.input_changed);
        let debounced = since_input >= DEBOUNCE;
        let typed_stale = typed
            .as_ref()
            .map(|t| {
                self.status_cache
                    .get(t)
                    .map(|(at, _)| at.elapsed() > CACHE_TTL)
                    .unwrap_or(true)
            })
            .unwrap_or(false);
        let since_req = self.last_status_req.map(|t| now.duration_since(t));
        let interval = self.poll_interval(ctx).map(|iv| iv + self.poll_jitter);
        let started = now >= self.status_start;
        let periodic = match interval {
            Some(iv) => since_req.map(|d| d >= iv).unwrap_or(started),
            None => since_req.is_none() && started, // actualisation manuelle : seulement la première fois
        };
        if !started && since_req.is_none() {
            ctx.request_repaint_after(self.status_start - now);
        }

        if (periodic || (typed_stale && debounced)) && !self.status_inflight {
            self.request_statuses(ctx, wanted);
        } else if typed_stale && !debounced {
            ctx.request_repaint_after(DEBOUNCE - since_input);
        }
        if let (Some(d), Some(iv)) = (since_req, interval) {
            ctx.request_repaint_after(iv.saturating_sub(d).max(Duration::from_millis(200)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S20: Option<Duration> = Some(Duration::from_secs(20));

    #[test]
    fn intervalle_adaptatif() {
        let actif = Duration::from_secs(5);
        let repos = Duration::from_secs(600);
        assert_eq!(
            effective_interval(S20, true, false, actif),
            S20,
            "réglage respecté pendant l'utilisation"
        );
        assert_eq!(
            effective_interval(S20, true, false, repos),
            Some(IDLE_INTERVAL)
        );
        assert_eq!(
            effective_interval(S20, false, false, actif),
            Some(BACKGROUND_INTERVAL),
            "autre fenêtre devant"
        );
        assert_eq!(
            effective_interval(S20, true, true, actif),
            Some(BACKGROUND_INTERVAL),
            "fenêtre réduite"
        );
        let m5 = Some(Duration::from_secs(300));
        assert_eq!(
            effective_interval(m5, false, true, repos),
            m5,
            "un intervalle plus long n'est jamais raccourci"
        );
        assert_eq!(effective_interval(None, true, false, actif), None, "manuel");
    }

    /// Requêtes par minute vers Twitch d'un client resté sur l'accueil (cas mesuré : 15 favoris =
    /// 2 requêtes GraphQL, 1 requête Helix pour les suivis, réglage 20 s). Avant : validation du jeton à chaque
    /// actualisation, 20 s fixes = 12 req/min (mesuré). Après : validation horaire, 60 s au repos, 120 s en arrière-plan.
    #[test]
    fn volume_au_repos_sur_l_accueil() {
        let per_min = |iv: Duration, validate_each: bool| {
            let per_cycle = 3.0 + if validate_each { 1.0 } else { 0.0 };
            60.0 / iv.as_secs_f64() * per_cycle + if validate_each { 0.0 } else { 1.0 / 60.0 }
        };
        assert_eq!(
            per_min(Duration::from_secs(20), true),
            12.0,
            "avant (mesure)"
        );
        let repos = effective_interval(S20, true, false, Duration::from_secs(600)).unwrap();
        let fond = effective_interval(S20, false, false, Duration::ZERO).unwrap();
        let actif = effective_interval(S20, true, false, Duration::ZERO).unwrap();
        assert!(
            per_min(repos, false) < 3.1,
            "au repos : {}",
            per_min(repos, false)
        );
        assert!(
            per_min(fond, false) < 1.6,
            "en arrière-plan : {}",
            per_min(fond, false)
        );
        assert!(
            per_min(actif, false) < 9.1,
            "en utilisation : {}",
            per_min(actif, false)
        );
    }
}
