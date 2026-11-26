//! Raccourcis clavier, géométrie de la fenêtre et sauvegarde des réglages.
use super::*;

impl VelkaApp {
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.view != View::Watch || ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::{Key, Modifiers};
        let none = Modifiers::NONE;
        let keys: Vec<(&'static str, Option<Key>)> = SHORTCUTS
            .iter()
            .map(|(id, _, _)| (*id, self.shortcut_key(id)))
            .collect();
        // Un menu ouvert (qualité, options du lecteur) garde l'Échap : il se ferme avant que le plein écran ne le soit.
        let menu_open = egui::Popup::is_any_open(ctx);
        let (fired, esc) = ctx.input_mut(|i| {
            let fired: Vec<&'static str> = keys
                .iter()
                .filter(|(_, k)| k.is_some_and(|k| i.consume_key(none, k)))
                .map(|(id, _)| *id)
                .collect();
            (fired, !menu_open && i.consume_key(none, Key::Escape))
        });
        for action in fired {
            match action {
                "pause" => self.toggle_pause(),
                "mute" => self.toggle_mute(),
                "volume_up" => self.change_volume(5.0),
                "volume_down" => self.change_volume(-5.0),
                "fullscreen" => self.toggle_fullscreen(ctx, !self.fullscreen),
                "chat" => self.settings.chat_visible = !self.settings.chat_visible,
                "pip" => self.toggle_pip(ctx),
                "screenshot" => self.take_screenshot(),
                "tech_info" => {
                    self.tech_info = !self.tech_info;
                    self.tech_cache = None;
                }
                _ => {}
            }
        }
        if esc {
            if self.fullscreen {
                self.toggle_fullscreen(ctx, false);
            } else if self.pip {
                self.exit_pip(ctx);
            } else if matches!(
                self.watch.as_ref().map(|w| &w.state),
                Some(WatchState::Error(_))
            ) {
                self.stop_watch(ctx);
            } else {
                self.minimize_watch(ctx, View::Home);
            }
        }
    }

    /// Barre de titre intégrée (la fenêtre n'a pas de bordure Windows) : zone de déplacement (double-clic = agrandir
    /// ou restaurer) et boutons Réduire, Agrandir / Restaurer, Fermer en haut à droite, comme ceux de Windows.
    /// `salt` distingue les identifiants quand la barre est aussi dessinée au-dessus de l'overlay des Réglages.
    pub(super) fn title_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, salt: &str) {
        let tr = crate::i18n::tr;
        theme::window_title_bar(
            ui,
            ctx,
            salt,
            [tr("Réduire"), tr("Agrandir"), tr("Restaurer"), tr("Fermer")],
        );
    }

    /// Barre de titre posée en haut de la zone restante (toute la largeur, ou à droite de la barre latérale).
    pub(super) fn title_bar_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("title_bar")
            .exact_size(TITLE_BAR_H)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.title_bar(ui, ctx, "bar"));
    }

    /// Bords de la fenêtre sans bordure : redimensionnement à la souris (sauf agrandie), et liseré fin quand elle
    /// flotte (Windows 11 dessine en plus ses coins arrondis).
    pub(super) fn window_edges(&mut self, ctx: &egui::Context) {
        if self.fullscreen || self.pip {
            return;
        }
        theme::window_edges(ctx);
    }

    /// Mémorise taille, position et état maximisé (hors plein écran et PiP).
    pub(super) fn track_window(&mut self, ctx: &egui::Context) {
        if self.fullscreen || self.pip || Instant::now() < self.geometry_hold {
            return;
        }
        let vp = ctx.input(|i| i.viewport().clone());
        if vp.minimized == Some(true) {
            return;
        }
        let maximized = vp.maximized.unwrap_or(false);
        let s = &mut self.settings;
        s.window_maximized = maximized;
        if !maximized {
            if let (Some(inner), Some(outer)) = (vp.inner_rect, vp.outer_rect) {
                // Points d'écran réels (ceux qu'on repasse à l'ouverture) : les points d'egui sont divisés par le zoom de
                // l'interface, la fenêtre rétrécissait et glissait à chaque lancement.
                let zoom = ctx.zoom_factor();
                let size = [
                    (inner.width() * zoom).round(),
                    (inner.height() * zoom).round(),
                ];
                let pos = [(outer.min.x * zoom).round(), (outer.min.y * zoom).round()];
                if s.window_size != Some(size) {
                    s.window_size = Some(size);
                }
                if s.window_pos != Some(pos) {
                    s.window_pos = Some(pos);
                }
                s.window_ppp = vp.native_pixels_per_point.unwrap_or(1.0);
            }
        }
    }

    /// Option « Réduire dans la zone de notification à la fermeture » : icône permanente, menu de l'icône, et la croix
    /// de fermeture qui cache la fenêtre au lieu de quitter. Sans l'option, rien de tout cela n'existe.
    pub(super) fn tick_tray(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::tr;
        let want = self.settings.close_to_tray;
        if want && (!self.tray_on || self.tray_lang != self.settings.language) {
            crate::procs::tray_enable(tr("Ouvrir Velka"), tr("Quitter"));
            self.tray_on = true;
            self.tray_lang = self.settings.language;
        } else if !want && self.tray_on {
            crate::procs::tray_disable();
            self.tray_on = false;
            if self.hidden_in_tray.is_some() {
                self.restore_from_tray(ctx);
            }
        }
        // Fenêtre réaffichée par un autre moyen (seconde instance lancée) : on n'est plus cachés.
        if self
            .hidden_in_tray
            .is_some_and(|t| t.elapsed() > Duration::from_secs(1))
            && crate::procs::own_window_visible()
        {
            self.hidden_in_tray = None;
        }
        match crate::procs::take_tray_command() {
            crate::procs::TRAY_OPEN => self.restore_from_tray(ctx),
            crate::procs::TRAY_QUIT => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
        if self.tray_on && !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_in_tray(ctx);
        }
    }

    /// Cache la fenêtre dans la zone de notification : le live est coupé, l'application reste active (notifications).
    fn hide_in_tray(&mut self, ctx: &egui::Context) {
        if self.watch.is_some() {
            self.stop_watch(ctx);
        }
        self.multi_suspend();
        self.player = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.hidden_in_tray = Some(Instant::now());
        crate::logging::log("Fenêtre cachée dans la zone de notification");
        // Première fois seulement : on explique où l'application est passée.
        if !self.settings.tray_hint_shown {
            self.settings.tray_hint_shown = true;
            crate::procs::notify(
                "Velka",
                crate::i18n::tr("Velka reste actif dans la zone de notification. Clic droit sur l'icône pour quitter."),
                "",
            );
        }
    }

    /// Réaffiche la fenêtre (icône de la zone de notification, clic sur une notification de live).
    pub(super) fn restore_from_tray(&mut self, ctx: &egui::Context) {
        self.hidden_in_tray = None;
        crate::procs::show_own_window();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// Réglages à écrire sur le disque : les options de la ligne de commande, tant que
    /// l'utilisateur ne les a pas changées lui-même, ne sont pas mémorisées.
    pub(super) fn persistable(&self) -> Settings {
        let mut s = self.settings.clone();
        if let Some((before, after)) = &self.cli_guard {
            macro_rules! guard {
                ($($f:ident),*) => { $( if s.$f == after.$f { s.$f = before.$f.clone(); } )* };
            }
            guard!(
                volume,
                quality,
                on_top,
                chat_left,
                chat_visible,
                low_latency
            );
        }
        s
    }

    /// Enregistre les réglages modifiés (sans écrire à chaque image pendant un glissement).
    pub(super) fn save_settings_if_changed(&mut self, ctx: &egui::Context) {
        if super::apercu_outils() {
            return; // aperçu : aucun réglage écrit
        }
        if self.settings.on_top != self.saved_settings.on_top {
            apply_on_top(ctx, self.settings.on_top);
        }
        if self.settings != self.saved_settings
            && !ctx.input(|i| i.pointer.any_down())
            && self.last_save.elapsed() > Duration::from_millis(800)
        {
            self.last_save = Instant::now();
            match self.persistable().save() {
                Ok(()) => self.saved_settings = self.settings.clone(),
                Err(e) => {
                    crate::logging::log(&format!("Réglages non enregistrés : {e}"));
                    self.saved_settings = self.settings.clone(); // inutile de réessayer à chaque image
                }
            }
        }
    }
}

/// Hauteur de la barre de titre intégrée (définie dans `theme.rs`, partagée avec l'outil d'admin).
pub(super) const TITLE_BAR_H: f32 = theme::TITLE_BAR_H;
