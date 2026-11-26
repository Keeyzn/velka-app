//! Cartes d'options des Réglages (une méthode par carte ; l'ordre d'affichage est dans `settings_content`).
use super::*;

/// Dépôt qui compile les builds libmpv utilisés (scripts de compilation et références exactes de mpv et FFmpeg de chaque
/// build) : lien « Code source et compilation » de la carte « Licences des composants ».
/// Mac et Linux : dépôts publics qui fabriquent ou réemballent une libmpv LGPL, avec leurs sources.
const LGPL_SOURCES_URL: &str = if cfg!(target_os = "macos") {
    "https://github.com/Keeyzn/velka-composants-mac"
} else if cfg!(target_os = "linux") {
    "https://github.com/Keeyzn/velka-composants-linux"
} else {
    "https://github.com/zhongfly/mpv-winbuild"
};

impl VelkaApp {
    // Cartes d'options : chacune est une méthode (voir `settings_content` pour l'ordre d'affichage).
    /// Lit une fois, à l'ouverture des réglages, la taille du cache et l'état du démarrage avec Windows.
    /// Fait en arrière-plan (parcours du cache, registre) pour que l'ouverture ne fige pas l'interface.
    pub(super) fn settings_prepare(&mut self, ctx: &egui::Context) {
        if (self.cache_size.is_none() || self.startup_on.is_none()) && !self.settings_info_inflight
        {
            self.settings_info_inflight = true;
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(super::super::Msg::SettingsInfo(
                    cache_bytes(),
                    crate::procs::startup_enabled(),
                ));
                ctx.request_repaint();
            });
        }
    }

    /// Son : normalisation, volume au lancement, volume maximum. Renvoie `true` si la normalisation a changé.
    pub(super) fn card_sound(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) -> bool {
        let tr = crate::i18n::tr;
        let c = pick(260.0);
        let mut audio_changed = false;
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Audio"));
            ui.add_space(t::SP_S);
            audio_changed = t::switch_row(
                ui,
                tr("Volume stable"),
                Some(tr("Limite les écarts de volume entre les chaînes")),
                &mut self.settings.normalize_audio,
            )
            .changed();
            // Périphérique de sortie : le choix s'applique tout de suite (liste relue toutes les 2 s tant que la page est ouverte).
            let mut device_changed = false;
            t::control_row(ui, tr("Périphérique de sortie"), |ui| {
                device_changed = output_device_dropdown(
                    ui,
                    &mut self.settings.audio_device,
                    &self.audio_devices,
                );
            });
            if device_changed {
                self.audio_dev_lost = false;
                self.apply_audio_device();
            }
            if self.audio_dev_lost {
                ui.label(
                    egui::RichText::new(tr(super::super::lecteur::AUDIO_DEVICE_LOST))
                        .color(t::MUTED())
                        .size(12.5),
                );
            }
            t::switch_row(
                ui,
                tr("Démarrer en muet"),
                None,
                &mut self.settings.start_muted,
            );
            t::switch_row(
                ui,
                tr("Cumuler le son des lives en Multistream"),
                Some(tr(
                    "Un clic sur un live ajoute son son au lieu de le remplacer",
                )),
                &mut self.settings.multi_mix_sounds,
            );
            t::switch_row(
                ui,
                tr("Volume fixe au lancement"),
                Some(tr("Sinon : le dernier volume utilisé")),
                &mut self.settings.fixed_volume_on,
            );
            if self.settings.fixed_volume_on {
                let max = self.settings.volume_max;
                t::control_row(ui, tr("Volume au lancement"), |ui| {
                    t::pill(
                        ui,
                        None,
                        &format!("{} %", self.settings.fixed_volume),
                        t::TEXT(),
                        t::SURFACE_3(),
                    );
                    t::volume_slider(ui, &mut self.settings.fixed_volume, max, 120.0);
                });
            }
            let mut idx = u8::from(self.settings.volume_max > 100);
            let mut max_changed = false;
            t::control_row(ui, tr("Volume maximum"), |ui| {
                max_changed = t::segmented(ui, &mut idx, &["100 %", "130 %"]);
            });
            if max_changed {
                self.settings.volume_max = if idx == 0 { 100 } else { 130 };
                self.settings.volume = self.settings.volume.min(self.settings.volume_max);
                self.settings.fixed_volume =
                    self.settings.fixed_volume.min(self.settings.volume_max);
                self.apply_volume();
            }
        });
        cols[c].add_space(t::SP_M);
        audio_changed
    }

    /// Lecteur : fenêtre, contrôles, souris sur la vidéo, minuteur de veille, mini-lecteur.
    pub(super) fn card_player(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(380.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Lecteur et comportement"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Toujours au premier plan"),
                None,
                &mut self.settings.on_top,
            );
            t::control_row(ui, tr("Masquage des contrôles"), |ui| {
                t::segmented(
                    ui,
                    &mut self.settings.controls_hide,
                    &[tr("2,5 s"), tr("5 s"), tr("10 s"), tr("Jamais")],
                );
            });
            t::switch_row(
                ui,
                tr("Fenêtre PiP au format de la vidéo"),
                None,
                &mut self.settings.pip_fit,
            );
            t::switch_row(
                ui,
                tr("Lecteur au format de la vidéo"),
                None,
                &mut self.settings.fit_video,
            );
            t::switch_row(
                ui,
                tr("Molette de la souris sur la vidéo = volume"),
                None,
                &mut self.settings.wheel_volume,
            );
            t::switch_row(
                ui,
                tr("Clic sur la vidéo = pause"),
                None,
                &mut self.settings.click_pause,
            );
            let mut sleep_changed = false;
            t::control_row(ui, tr("Minuteur de veille"), |ui| {
                sleep_changed =
                    t::segmented(ui, &mut self.settings.sleep_timer, &super::sleep_labels());
            });
            // Durée personnalisée : visible quand « Perso » est choisi.
            if self.settings.sleep_timer == super::SLEEP_CUSTOM {
                t::control_row(ui, tr("Durée personnalisée"), |ui| {
                    sleep_changed |=
                        super::sleep_custom_field(ui, &mut self.settings.sleep_custom_min, 1.0);
                });
            }
            if sleep_changed {
                // Le décompte repart du moment du réglage pour le live en cours.
                self.reset_sleep_timer();
            }
            t::switch_row(
                ui,
                tr("Mini-lecteur"),
                Some(tr("Garder le live dans un coin quand on quitte la page")),
                &mut self.settings.mini_player,
            );
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Captures d'écran » de « Lecture vidéo » : dossier d'enregistrement (par défaut Images\Velka), avec
    /// « Parcourir… » et « Ouvrir le dossier ». Un dossier n'est retenu que s'il accepte l'écriture ; s'il disparaît
    /// plus tard, le réglage revient au dossier par défaut avec un message.
    pub(super) fn card_screenshots(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(200.0);
        self.check_screenshot_dir(&cols[c].ctx().clone());
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Captures d'écran"));
            ui.add_space(t::SP_S);
            let (mut browse, mut open) = (false, false);
            t::control_row(ui, tr("Dossier d'enregistrement"), |ui| {
                // Posés de droite à gauche : « Parcourir… » apparaît à gauche de « Ouvrir le dossier ».
                open = t::icon_text_button(
                    ui,
                    t::BtnKind::Secondary,
                    t::NavIcon::Folder,
                    tr("Ouvrir le dossier"),
                )
                .clicked();
                browse = t::icon_text_button(
                    ui,
                    t::BtnKind::Secondary,
                    t::NavIcon::Folder,
                    tr("Parcourir…"),
                )
                .clicked();
            });
            let dir = if self.settings.screenshot_dir.is_empty() {
                super::super::lecteur::default_screenshot_dir()
            } else {
                std::path::PathBuf::from(&self.settings.screenshot_dir)
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(dir.display().to_string())
                        .color(t::MUTED())
                        .size(13.0),
                )
                .truncate(),
            );
            if self.shot_dir_err {
                ui.add_space(t::SP_S);
                ui.label(
                    egui::RichText::new(tr(super::super::lecteur::SHOT_DIR_REFUSED))
                        .color(t::NEGATIVE())
                        .size(12.5),
                );
            } else if self.shot_dir_lost {
                ui.add_space(t::SP_S);
                ui.label(
                    egui::RichText::new(tr(super::super::lecteur::SHOT_DIR_LOST))
                        .color(t::MUTED())
                        .size(12.5),
                );
            }
            if browse {
                self.browse_screenshot_dir();
            }
            if open {
                // Le dossier par défaut n'existe qu'après la première capture : on le crée pour pouvoir l'ouvrir.
                let _ = std::fs::create_dir_all(&dir);
                crate::procs::open_folder(&dir);
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Accueil et barre latérale » de l'onglet « Général » (sous « Démarrage et fenêtre ») : actualisation,
    /// aperçus, chaînes suivies et leurs limites, jeu mémorisé, clic sur une carte. Les deux limites ne s'affichent que
    /// si « Chaînes suivies sur Twitch » est activé (comme avant le déplacement depuis l'onglet « Accueil »).
    pub(super) fn card_home_sidebar(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(490.0);
        let mut to_account = false;
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Accueil et barre latérale"));
            ui.add_space(t::SP_S);
            t::control_row(ui, tr("Actualisation"), |ui| {
                t::segmented(
                    ui,
                    &mut self.settings.refresh_mode,
                    &["20 s", "1 min", tr("Manuelle")],
                );
            });
            t::switch_row(
                ui,
                tr("Aperçus des lives"),
                Some(tr("Images du direct sur les cartes")),
                &mut self.settings.show_thumbnails,
            );
            // Déconnecté : l'interrupteur et les deux limites sont grisés (sans effet sans compte) ; les valeurs
            // enregistrées sont gardées et réapparaissent à la connexion.
            let connected = self.connected();
            // Lien au-dessus des réglages grisés (lui n'est pas grisé) : mène à Compte, où se trouve « Se connecter ».
            if !connected {
                to_account =
                    t::setting_link(ui, tr("Connectez-vous à Twitch pour activer")).clicked();
            }
            ui.add_enabled_ui(connected, |ui| {
                t::switch_row(
                    ui,
                    tr("Chaînes suivies sur Twitch"),
                    Some(tr(
                        "Dans la barre latérale, sur l'accueil et dans les recommandations",
                    )),
                    &mut self.settings.show_follows,
                );
            });
            if self.settings.show_follows {
                ui.add_enabled_ui(connected, |ui| {
                    t::control_row(ui, tr("Chaînes dans la barre latérale"), |ui| {
                        t::segmented(
                            ui,
                            &mut self.settings.sidebar_follow_limit,
                            &["10", "20", "40"],
                        );
                    });
                    t::control_row(ui, tr("Chaînes sur l'accueil"), |ui| {
                        t::segmented(
                            ui,
                            &mut self.settings.home_follow_limit,
                            &["12", "24", "40"],
                        );
                    });
                });
            }
            if t::switch_row(
                ui,
                tr("Se souvenir du jeu choisi"),
                None,
                &mut self.settings.remember_game,
            )
            .changed()
            {
                self.settings.home_game = if self.settings.remember_game {
                    self.home_game.clone().unwrap_or_default()
                } else {
                    String::new()
                };
            }
            t::control_row(ui, tr("Clic sur une carte"), |ui| {
                t::segmented(ui, &mut self.settings.card_click, &[tr("Ouvrir"), "PiP"]);
            });
            // Historique des chaînes récentes : nombre enregistré et effacement d'un coup. Les favoris ne sont jamais touchés.
            let count = self.history.items.len();
            let mut clear = false;
            t::control_row(ui, tr("Historique des chaînes"), |ui| {
                let label = tr("Effacer l'historique");
                let kind = if count == 0 {
                    t::BtnKind::Status
                } else {
                    t::BtnKind::Secondary
                };
                let resp = t::icon_text_button(ui, kind, t::NavIcon::Trash, label);
                t::paint_highlight(ui, resp.rect, label);
                clear = resp.clicked() && count > 0;
                t::pill(ui, None, &count.to_string(), t::TEXT(), t::SURFACE_3());
            });
            if clear {
                self.history = crate::history::History::default();
                if let Err(e) = self.history.save() {
                    crate::logging::log(&format!("Historique non enregistré : {e}"));
                }
                self.history_cleared = Some(std::time::Instant::now());
            }
            // Message court, quelques secondes.
            if let Some(t0) = self.history_cleared {
                let age = t0.elapsed().as_secs_f32();
                if age < 3.0 {
                    let fade = t::motion::toast_alpha(ui.ctx(), age, 3.0, true);
                    ui.horizontal(|ui| {
                        ui.add_space(t::SP_M);
                        ui.label(
                            RichText::new(tr("Historique effacé."))
                                .size(13.0)
                                .color(t::POSITIVE().gamma_multiply(fade)),
                        );
                    });
                    ui.ctx().request_repaint();
                } else {
                    self.history_cleared = None;
                }
            }
        });
        cols[c].add_space(t::SP_M);
        if to_account {
            self.pick_setting(ctx, "Compte Twitch", super::TAB_ACCOUNT);
        }
    }

    /// Groupe « Affichage et lisibilité » de l'onglet « Chat » : taille du texte, position du chat, surimpression en plein
    /// écran, horodatage.
    pub(super) fn card_chat(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(284.0);
        let mut to_scale = false;
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Affichage et lisibilité"));
            ui.add_space(t::SP_S);
            t::control_row(ui, tr("Taille du texte"), |ui| {
                t::pill(
                    ui,
                    None,
                    &format!("{} %", (self.settings.chat_font_scale * 100.0).round()),
                    t::TEXT(),
                    t::SURFACE_3(),
                );
                t::value_slider(
                    ui,
                    &mut self.settings.chat_font_scale,
                    0.8,
                    1.5,
                    0.05,
                    160.0,
                );
            });
            // Lien vers le réglage voisin : évite de zoomer toute l'interface quand on voulait seulement le chat.
            to_scale =
                t::setting_link(ui, tr("Ajuster la taille globale de l'interface")).clicked();
            t::switch_row(
                ui,
                tr("Chat à gauche de la vidéo"),
                None,
                &mut self.settings.chat_left,
            );
            t::switch_row(
                ui,
                tr("Chat en surimpression en plein écran"),
                None,
                &mut self.settings.fullscreen_chat_overlay,
            );
            t::switch_row(
                ui,
                tr("Horodatage des messages"),
                None,
                &mut self.settings.chat_timestamps,
            );
        });
        cols[c].add_space(t::SP_M);
        if to_scale {
            self.pick_setting(ctx, "Taille de l'interface", super::TAB_APPEARANCE);
        }
    }

    /// Groupe « Filtrage et alertes » de l'onglet « Chat » : bots masqués et mots surlignés.
    pub(super) fn card_chat_filter(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(150.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Filtrage et alertes"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Masquer les bots"),
                Some("Nightbot, StreamElements, Streamlabs…"),
                &mut self.settings.chat_hide_bots,
            );
            ui.add_space(t::SP_XS);
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                t::help_label(ui, tr("Mots surlignés"));
            });
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.chat_words)
                        .hint_text(tr("mot, pseudo… (séparés par des virgules)"))
                        .desired_width(ui.available_width() - 14.0),
                );
            });
        });
        cols[c].add_space(t::SP_M);
    }

    /// Emotes des extensions, emotes animées et badges.
    pub(super) fn card_emotes(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(170.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Émotes et badges"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Émotes 7TV, BTTV et FFZ"),
                None,
                &mut self.settings.chat_third_party,
            );
            t::switch_row(
                ui,
                tr("Émotes animées"),
                None,
                &mut self.settings.chat_animated,
            );
            t::switch_row(
                ui,
                tr("Badges du chat"),
                None,
                &mut self.settings.chat_badges,
            );
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Recommandations » de l'onglet « Général » (sous « Accueil et barre latérale »).
    pub(super) fn card_reco(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        _ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(190.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Recommandations"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Recommandations de chaînes"),
                Some(tr(
                    "Proposer des chaînes quand un live est hors ligne ou terminé",
                )),
                &mut self.settings.recommendations,
            );
            if self.settings.recommendations {
                t::control_row(ui, tr("Nombre de recommandations"), |ui| {
                    t::segmented(ui, &mut self.settings.recs_count, &["3", "5", "8"]);
                });
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Mise à l'échelle » de l'onglet « Apparence » (sous « Couleur d'accent ») : taille de l'interface. La
    /// taille n'est appliquée qu'au relâchement du curseur, pour que celui-ci ne bouge pas sous la souris. Les
    /// raccourcis Ctrl + / Ctrl − / Ctrl 0 (voir `app/mod.rs`) ne changent pas.
    pub(super) fn card_scale(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(124.0);
        let mut to_chat = false;
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Mise à l'échelle"));
            ui.add_space(t::SP_S);
            t::control_row(ui, tr("Taille de l'interface"), |ui| {
                t::pill(
                    ui,
                    None,
                    &format!("{} %", (self.settings.ui_zoom * 100.0).round()),
                    t::TEXT(),
                    t::SURFACE_3(),
                );
                t::value_slider(ui, &mut self.settings.ui_zoom, 0.8, 1.5, 0.05, 160.0);
            });
            // Lien vers le réglage voisin : agrandir seulement le texte du chat, sans zoomer toute l'interface.
            to_chat =
                t::setting_link(ui, tr("Ajuster uniquement la taille du texte du chat")).clicked();
        });
        cols[c].add_space(t::SP_M);
        if to_chat {
            self.pick_setting(ctx, "Taille du texte", super::TAB_CHAT);
        }
    }

    /// Groupe « Mouvement et lisibilité » de l'onglet « Accessibilité » : animations (complet, réduit, désactivé) et
    /// contraste élevé. Même logique qu'avant leur regroupement : `apply_animations` et `apply_access`.
    pub(super) fn card_motion(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(150.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Mouvement et lisibilité"));
            ui.add_space(t::SP_S);
            let mut level = self.settings.motion_level();
            let mut motion_changed = false;
            t::control_row(ui, tr("Animations"), |ui| {
                motion_changed = t::segmented(
                    ui,
                    &mut level,
                    &[tr("Complet"), tr("Réduit"), tr("Désactivé")],
                );
            });
            if motion_changed {
                self.settings.set_motion_level(level);
                super::super::apply_animations(ctx, &self.settings);
            }
            let mut contrast_changed = false;
            t::control_row(ui, tr("Contraste élevé"), |ui| {
                contrast_changed = t::segmented(
                    ui,
                    &mut self.settings.high_contrast,
                    &[tr("Automatique"), tr("Activé"), tr("Désactivé")],
                );
            });
            if contrast_changed {
                super::super::apply_access(ctx, &self.settings);
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Navigation et souris » de l'onglet « Accessibilité » : focus clavier renforcé, boutons Précédent /
    /// Suivant de la souris, main sur les éléments cliquables et vitesse de défilement (aides à la navigation).
    pub(super) fn card_navigation(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(240.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Navigation et souris"));
            ui.add_space(t::SP_S);
            if t::switch_row(
                ui,
                tr("Focus clavier renforcé"),
                None,
                &mut self.settings.focus_strong,
            )
            .changed()
            {
                super::super::apply_access(ctx, &self.settings);
            }
            t::switch_row(
                ui,
                tr("Boutons Précédent / Suivant de la souris"),
                None,
                &mut self.settings.mouse_nav,
            );
            t::switch_row(
                ui,
                tr("Main sur les éléments cliquables"),
                None,
                &mut self.settings.hand_cursor,
            );
            let mut speed_changed = false;
            t::control_row(ui, tr("Vitesse de défilement"), |ui| {
                t::pill(
                    ui,
                    None,
                    &format!(
                        "{} ×",
                        crate::i18n::decimal(f64::from(self.settings.scroll_speed), 1)
                    ),
                    t::TEXT(),
                    t::SURFACE_3(),
                );
                speed_changed =
                    t::value_slider(ui, &mut self.settings.scroll_speed, 0.5, 3.0, 0.1, 160.0);
            });
            if speed_changed {
                super::super::apply_scroll_speed(ctx, self.settings.scroll_speed);
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Couleur d'accent » de l'onglet « Apparence » (sous « Thème ») : palettes prédéfinies, couleur
    /// personnalisée, « Améliorer le contraste » et « Rétablir ». Une couleur dont le texte blanc des boutons aurait
    /// moins de 4,5:1 (7:1 en contraste élevé) n'est jamais appliquée : on le dit en toutes lettres et on propose la
    /// couleur conforme la plus proche. « Contraste élevé » reste dans « Accessibilité ».
    pub(super) fn card_colors(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        use egui::Color32;
        let tr = crate::i18n::tr;
        let c = pick(620.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Couleur d'accent"));
            ui.add_space(t::SP_S);
            let mut changed = false;
            let current = usize::from(self.settings.palette).min(5);
            let custom_name = tr("Personnalisée");
            let default_name = tr("Orange (par défaut)");
            // L'orange d'origine (nouvelles installations) est un choix nommé, pas une « personnalisée ».
            let is_default =
                current == t::PALETTES.len() && self.settings.accent_custom == t::ACCENT_NEW;
            let name_of = |i: usize| {
                if i < t::PALETTES.len() {
                    tr(t::PALETTES[i].0)
                } else {
                    custom_name
                }
            };
            t::control_row(ui, tr("Palette de couleurs"), |ui| {
                t::pill(
                    ui,
                    None,
                    if is_default {
                        default_name
                    } else {
                        name_of(current)
                    },
                    t::TEXT(),
                    t::SURFACE_3(),
                );
            });
            ui.add_space(t::SP_XS);
            ui.spacing_mut().item_spacing.y = 6.0;
            let n_presets = t::PALETTES.len();
            if t::choice_row(ui, default_name, &t::hex_of(t::ACCENT_NEW), is_default).clicked()
                && !is_default
            {
                self.settings.palette = n_presets as u8;
                self.settings.accent_custom = t::ACCENT_NEW;
                self.accent_draft = None;
                self.accent_hsv = None;
                self.colors_open = false;
                changed = true;
            }
            for i in 0..n_presets {
                if t::choice_row(ui, name_of(i), &t::hex_of(t::PALETTES[i].1), current == i)
                    .clicked()
                    && current != i
                {
                    self.settings.palette = i as u8;
                    // Une autre couleur est choisie : le bouton « Personnalisée » se referme en douceur.
                    self.colors_open = false;
                    changed = true;
                }
            }
            // « Personnalisée » : à la place du code couleur, un bouton « Personnaliser » dans la ligne. Il déploie la
            // roue chromatique juste dessous, dans le même bloc.
            let selected_custom = current == n_presets && (!is_default || self.colors_open);
            let row = t::choice_row(ui, name_of(n_presets), "", selected_custom);
            let label = tr("Personnaliser");
            let galley = ui.painter().layout_no_wrap(
                label.to_string(),
                egui::FontId::proportional(14.0),
                t::TEXT(),
            );
            let btn = egui::Rect::from_center_size(
                egui::pos2(
                    row.rect.right() - 12.0 - (galley.size().x + 24.0) / 2.0,
                    row.rect.center().y,
                ),
                egui::vec2(galley.size().x + 24.0, 30.0),
            );
            let bresp = ui.interact(
                btn,
                egui::Id::unique("colors_custom_btn"),
                egui::Sense::click(),
            );
            {
                // Même look que les boutons secondaires de l'application.
                let hov = bresp.hovered();
                let fill = if self.colors_open || hov {
                    t::SURFACE_1()
                } else {
                    t::SURFACE_2()
                };
                let stroke = egui::Stroke::new(
                    1.0,
                    if hov || self.colors_open {
                        t::ACCENT_LINE()
                    } else {
                        t::BORDER_HOVER()
                    },
                );
                let p = ui.painter();
                p.rect(
                    btn,
                    egui::CornerRadius::same(t::RADIUS_INNER),
                    fill,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                p.galley(
                    egui::pos2(btn.left() + 12.0, btn.center().y - galley.size().y / 2.0),
                    galley,
                    t::TEXT(),
                );
            }
            t::focus_ring(ui, &bresp, btn, t::RADIUS_INNER);
            bresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
            if bresp.clicked() {
                self.colors_open = !self.colors_open;
                if self.colors_open {
                    self.advanced_scroll = 4; // la descente suit l'ouverture de la zone (voir plus bas)
                    if !selected_custom {
                        self.settings.palette = n_presets as u8;
                        changed = true;
                    }
                }
            } else if row.clicked() && !selected_custom {
                self.settings.palette = n_presets as u8;
                changed = true;
            }
            let colors_open = self.colors_open;
            let section_t = t::animated_section(ui, "colors_custom", colors_open, |ui| {
                ui.add_space(t::SP_S);
                ui.label(
                    RichText::new(tr("Couleur d'accent personnalisée"))
                        .size(13.0)
                        .color(t::MUTED()),
                );
                ui.add_space(t::SP_XS);

                // Édition : code, puis rouge, vert, bleu (curseurs au clavier : flèches, Origine, Fin).
                let mut rgb = *self.accent_draft.get_or_insert(self.settings.accent_custom);
                let draft_before = rgb;
                // Roue chromatique : teinte autour, saturation du centre vers le bord (la luminosité reste celle de la
                // couleur courante, réglable avec les curseurs ci-dessous).
                let mut hsv = match self.accent_hsv {
                    Some(h) if h.to_srgb().iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 1) => h,
                    _ => egui::ecolor::Hsva::from_srgb(rgb),
                };
                // De gauche à droite : libellés Rouge / Vert / Bleu, leurs curseurs, puis la roue chromatique.
                // Bloc de 168 px de haut : la roue à droite, les libellés + curseurs à gauche, centrés pour que le centre de
                // la roue tombe sur la ligne du milieu (« Vert »). La hauteur du bloc de gauche est mesurée à l'image
                // précédente.
                let wheel = 168.0;
                let gap = ui.spacing().item_spacing.x;
                let (block, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), wheel),
                    egui::Sense::hover(),
                );
                let left_w = (block.width() - wheel - gap).max(120.0);
                let mut wheel_ui =
                    ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                        block.right_top() - egui::vec2(wheel, 0.0),
                        egui::vec2(wheel, wheel),
                    )));
                if t::color_wheel(
                    &mut wheel_ui,
                    &mut hsv,
                    tr("Couleur d'accent personnalisée"),
                ) {
                    rgb = hsv.to_srgb();
                }
                let h_key = egui::Id::unique("rgb_block_height");
                let used: f32 = ctx.data(|d| d.get_temp::<f32>(h_key)).unwrap_or(120.0);
                let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(
                    egui::Rect::from_min_size(block.min, egui::vec2(left_w, wheel)),
                ));
                left_ui.add_space(((wheel - used) / 2.0).max(0.0));
                let top = left_ui.cursor().top();
                let slider_w = (left_w - 130.0).clamp(70.0, 200.0);
                for (i, label) in ["Rouge", "Vert", "Bleu"].into_iter().enumerate() {
                    t::control_row_inline(&mut left_ui, tr(label), |ui| {
                        let mut v = f32::from(rgb[i]);
                        t::value_pill(ui, &rgb[i].to_string(), "255", t::TEXT(), t::SURFACE_3());
                        if t::value_slider(ui, &mut v, 0.0, 255.0, 1.0, slider_w) {
                            rgb[i] = v.round() as u8;
                        }
                    });
                }
                let measured = left_ui.min_rect().bottom() - top;
                ctx.data_mut(|d| d.insert_temp(h_key, measured));
                self.accent_hsv = Some(hsv);
                if rgb != draft_before {
                    self.accent_draft = Some(rgb);
                }

                // Aperçu et verdict : toujours en toutes lettres (jamais la couleur seule).
                let min = t::required_ratio(t::high_contrast_active());
                let chosen = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                // L'exemple montre la couleur réellement appliquée : corrigée si « Améliorer le contraste » est activé.
                let col = if self.settings.accent_boost {
                    t::nearest_compliant_fill(chosen, min)
                } else {
                    chosen
                };
                let ratio = t::contrast(Color32::WHITE, col);
                let ok = ratio >= min;
                let shown = (ratio * 10.0).floor() / 10.0;
                ui.add_space(t::SP_S);
                ui.horizontal(|ui| {
                    t::pill(ui, None, tr("Exemple"), Color32::WHITE, col);
                    let verdict = if ok {
                        tr("Conforme").to_string()
                    } else {
                        format!(
                            "{} {}:1",
                            tr("Insuffisant, minimum"),
                            crate::i18n::decimal(f64::from(min), 1)
                        )
                    };
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "{} : {}:1 : {}",
                                tr("Contraste du texte sur l'accent"),
                                crate::i18n::decimal(f64::from(shown), 1),
                                verdict
                            ))
                            .size(12.5)
                            .color(if ok {
                                t::POSITIVE()
                            } else {
                                t::NEGATIVE()
                            }),
                        )
                        .wrap(),
                    );
                });
                // Couleur libre : appliquée telle quelle dès qu'elle change. Le contraste n'est qu'indiqué ; l'option
                // « Améliorer le contraste » (à côté de « Rétablir… ») la corrige si on le souhaite.
                if rgb != self.settings.accent_custom {
                    self.settings.accent_custom = rgb;
                    self.settings.palette = t::PALETTES.len() as u8;
                    changed = true;
                }
            });
            // Ouverture et descente vont à la même vitesse que la fermeture (0,35 s) : la page suit la zone qui s'ouvre.
            if colors_open && section_t < 1.0 {
                self.advanced_scroll = self.advanced_scroll.max(4);
            }

            ui.add_space(t::SP_M);
            // Deux éléments côte à côte : rétablir, et (facultatif) améliorer le contraste de la couleur choisie. Le
            // commutateur n'existe que tant que la zone de personnalisation est dépliée (fondu avec elle) : il suit
            // son ouverture et sa fermeture, jamais visible ailleurs.
            ui.horizontal_wrapped(|ui| {
                if t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Refresh, tr("Rétablir les couleurs par défaut")).clicked() {
                    self.settings.palette = 5;
                    self.settings.accent_custom = crate::ui::theme::ACCENT_NEW;
                    self.accent_draft = None;
                    self.accent_hsv = None;
                    changed = true;
                }
                if section_t > 0.0 && usize::from(self.settings.palette) == t::PALETTES.len() {
                    ui.add_space(t::SP_S);
                    ui.scope(|ui| {
                        ui.set_opacity(section_t);
                        let boost = t::inline_switch(ui, tr("Améliorer le contraste"), &mut self.settings.accent_boost);
                        let boost = t::show_tip(boost, tr("Éclaircit ou assombrit la couleur choisie pour que le texte reste lisible."));
                        if boost.changed() {
                            changed = true;
                        }
                    });
                }
            });
            if changed {
                super::super::apply_access(ctx, &self.settings);
            }
        });
        cols[c].add_space(t::SP_M);
    }

    pub(super) fn card_shortcuts(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(430.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Raccourcis clavier"));
            ui.add_space(t::SP_S);
            for (id, label, _) in super::super::SHORTCUTS {
                let current = self.shortcut_key(id);
                let capturing = self.capture_shortcut == Some(id);
                let mut clicked = false;
                let mut cleared = false;
                t::control_row(ui, tr(label), |ui| {
                    // Posé d'abord = tout à droite (ligne de droite à gauche). Grisé quand le raccourci est déjà « Aucun ».
                    let kind = if current.is_some() {
                        t::BtnKind::Secondary
                    } else {
                        t::BtnKind::Status
                    };
                    cleared = t::icon_only_button(
                        ui,
                        kind,
                        t::NavIcon::Close,
                        tr("Effacer un raccourci"),
                    )
                    .clicked()
                        && current.is_some();
                    let text = if capturing {
                        tr("Appuyez sur une touche…").to_string()
                    } else {
                        current.map_or_else(|| tr("Aucun").to_string(), key_label)
                    };
                    clicked = t::toggle_button(ui, &text, capturing).clicked();
                });
                if cleared {
                    // « Aucun » : valeur vide mémorisée (absente = touche par défaut, vide = désactivé).
                    self.settings
                        .shortcuts
                        .insert(id.to_string(), String::new());
                    if capturing {
                        self.capture_shortcut = None;
                    }
                } else if clicked {
                    self.capture_shortcut = if capturing { None } else { Some(id) };
                }
            }
            if let Some(action) = self.capture_shortcut {
                let key = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Key {
                            key,
                            pressed: true,
                            repeat: false,
                            ..
                        } => Some(*key),
                        _ => None,
                    })
                });
                if let Some(k) = key {
                    self.capture_shortcut = None;
                    if k != egui::Key::Escape {
                        // Une touche ne sert qu'à une seule action.
                        for (other, _, _) in super::super::SHORTCUTS {
                            if other != action && self.shortcut_key(other) == Some(k) {
                                self.settings
                                    .shortcuts
                                    .insert(other.to_string(), String::new());
                            }
                        }
                        self.settings
                            .shortcuts
                            .insert(action.to_string(), k.name().to_string());
                    }
                }
            }
            ui.add_space(t::SP_S);
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                if t::icon_text_button(
                    ui,
                    t::BtnKind::Secondary,
                    t::NavIcon::Refresh,
                    tr("Rétablir les raccourcis"),
                )
                .clicked()
                {
                    self.settings.shortcuts.clear();
                    self.capture_shortcut = None;
                }
            });
        });
        cols[c].add_space(t::SP_M);
    }

    /// Encart informatif « Raccourcis non modifiables » (sous la liste des raccourcis) : touches fixes de l'application.
    /// Chaque ligne est indexée dans la recherche des Réglages.
    pub(super) fn card_shortcuts_fixed(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(230.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Raccourcis non modifiables"));
            ui.add_space(t::SP_S);
            let history = format!(
                "Alt + {} / {}",
                tr("Flèche gauche").to_lowercase(),
                tr("Flèche droite").to_lowercase()
            );
            let rows: [(&str, String); 4] = [
                (
                    "Quitter le plein écran, le PiP ou le live",
                    tr("Échap").to_string(),
                ),
                ("Rechercher une chaîne", "Ctrl K".to_string()),
                (
                    "Zoom de l'interface",
                    "Ctrl + / Ctrl - / Ctrl 0".to_string(),
                ),
                ("Page précédente / suivante", history),
            ];
            for (label, keys) in rows {
                t::control_row(ui, tr(label), |ui| {
                    t::pill(ui, None, &keys, t::TEXT(), t::SURFACE_3());
                });
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Version de l'application, licence, dépôt, non-affiliation et soutien (section « À propos »).
    pub(super) fn card_about(&mut self, cols: &mut [egui::Ui], pick: &mut dyn FnMut(f32) -> usize) {
        let tr = crate::i18n::tr;
        let c = pick(280.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            let title = tr("Version de l'application");
            let title_resp = t::card_title(ui, title);
            t::paint_highlight(ui, title_resp.rect, title);
            ui.add_space(t::SP_S);
            // Même retrait que les lignes de réglage : le texte s'aligne sur leurs libellés.
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                ui.label(
                    RichText::new(format!("Velka {}", env!("CARGO_PKG_VERSION"))).color(t::TEXT()),
                );
            });
            ui.add_space(t::SP_XS);
            t::card_body(ui, |ui| {
                ui.label(
                    RichText::new(tr("Logiciel libre sous licence GPL-3.0 ou ultérieure."))
                        .size(13.0)
                        .color(t::MUTED()),
                );
                ui.label(
                    RichText::new(tr("Application non affiliée à Twitch."))
                        .size(13.0)
                        .color(t::MUTED()),
                );
            });
            ui.add_space(t::SP_XS);
            if t::setting_link(ui, tr("Code source sur GitHub")).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(format!(
                    "https://github.com/{}",
                    crate::update::GITHUB_REPO
                )));
            }
            ui.add_space(t::SP_S);
            let label = tr("Soutenir le projet");
            let resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Star, label);
            t::paint_highlight(ui, resp.rect, label);
            if resp.clicked() {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(crate::app::soutien::SUPPORT_URL));
            }
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Rappel pour soutenir le projet"),
                None,
                &mut self.settings.support_reminder,
            );
        });
        cols[c].add_space(t::SP_M);
    }

    /// Mises à jour : recherche automatique (section « À propos », sous la version).
    pub(super) fn card_updates(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        use super::super::maj::UpdateCheck;
        let tr = crate::i18n::tr;
        let c = pick(190.0);
        let (mut search, mut install) = (false, false);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Mises à jour"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr("Vérification des mises à jour"),
                None,
                &mut self.settings.check_updates,
            );
            ui.add_space(t::SP_S);
            // Bouton : même vérification que l'automatique (`request_update_check`) ; grisé pendant une recherche ou un
            // téléchargement (un seul contrôle à la fois).
            let busy = self.update_inflight || self.update_progress.is_some();
            let label = tr("Rechercher des mises à jour");
            let kind = if busy {
                t::BtnKind::Status
            } else {
                t::BtnKind::Secondary
            };
            let resp = t::icon_text_button(ui, kind, t::NavIcon::Refresh, label);
            t::paint_highlight(ui, resp.rect, label);
            search = resp.clicked() && !busy;
            ui.add_space(t::SP_XS);
            t::card_body(ui, |ui| {
                let small = |text: String, color: egui::Color32| {
                    RichText::new(text).size(13.0).color(color)
                };
                if let Some(f) = self.update_progress {
                    ui.label(small(
                        format!(
                            "{} {:.0} %",
                            tr("Téléchargement de la mise à jour…"),
                            f * 100.0
                        ),
                        t::TEXT(),
                    ));
                } else if self.update_inflight {
                    ui.label(small(tr("Vérification…").to_string(), t::MUTED()));
                } else if let Some(info) = &self.update_info {
                    ui.label(small(
                        tr("Mise à jour {version} disponible").replace("{version}", &info.version),
                        t::TEXT(),
                    ));
                    ui.add_space(t::SP_XS);
                    let go = if self.update_error.is_some() {
                        tr("Réessayer")
                    } else {
                        tr("Mettre à jour")
                    };
                    install =
                        t::icon_text_button(ui, t::BtnKind::Primary, t::NavIcon::Download, go)
                            .clicked();
                    if let Some(e) = &self.update_error {
                        ui.label(small(
                            format!("{} {e}", tr("Mise à jour impossible :")),
                            t::NEGATIVE(),
                        ));
                    }
                } else {
                    match &self.update_check {
                        UpdateCheck::Idle => {}
                        UpdateCheck::Done => {
                            ui.label(small(
                                tr("Vous utilisez la dernière version.").to_string(),
                                t::POSITIVE(),
                            ));
                        }
                        UpdateCheck::Failed(e) => {
                            let text = if e.starts_with("réseau") {
                                tr("Impossible de se connecter. Vérifiez votre connexion internet.")
                            } else {
                                tr("La recherche de mise à jour a échoué.")
                            };
                            ui.label(small(text.to_string(), t::NEGATIVE()));
                        }
                    }
                }
            });
        });
        cols[c].add_space(t::SP_M);
        if search {
            self.request_update_check(ctx, true);
        }
        if install {
            self.start_update(ctx);
        }
    }

    /// Aide : relancer le didacticiel (section « À propos », sous les mises à jour). Le clic ferme les Réglages (voir l'appelant).
    pub(super) fn card_help(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        replay: &mut bool,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(110.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Aide"));
            ui.add_space(t::SP_S);
            let label = tr("Revoir le didacticiel");
            let resp = t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Play, label);
            t::paint_highlight(ui, resp.rect, label);
            if resp.clicked() {
                *replay = true;
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Licences des composants (section « À propos », sous l'aide) : libmpv et FFmpeg sont sous LGPL. La
    /// licence demande de prévenir l'utilisateur, de pouvoir remplacer la bibliothèque (chemin personnalisé de
    /// « Avancé et Système ») et de donner accès aux sources : le lien mène au dépôt qui compile les builds utilisés.
    pub(super) fn card_licenses(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(160.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            let title = tr("Licences des composants");
            let title_resp = t::card_title(ui, title);
            t::paint_highlight(ui, title_resp.rect, title);
            ui.add_space(t::SP_S);
            t::card_body(ui, |ui| {
                let text = tr("Le lecteur vidéo utilise libmpv (mpv) et FFmpeg, sous licence LGPL version 2.1 ou ultérieure. Vous pouvez remplacer libmpv par votre propre version dans {section}.")
                    .replace("{section}", tr("Avancé et Système"));
                ui.label(RichText::new(text).size(13.0).color(t::MUTED()));
            });
            ui.add_space(t::SP_XS);
            if t::setting_link(ui, tr("Code source et compilation")).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(LGPL_SOURCES_URL));
            }
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Démarrage et fenêtre » de l'onglet « Général » (sous « Langue ») : réglages d'ouverture de l'application,
    /// « Démarrer avec Windows » et « Chaîne ouverte au démarrage ».
    pub(super) fn card_startup(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(230.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Démarrage et fenêtre"));
            ui.add_space(t::SP_S);
            let mut on = self.startup_on.unwrap_or(false);
            if t::switch_row(
                ui,
                tr(crate::i18n::os::STARTUP),
                Some(tr("Ouvre l'application à l'ouverture de session")),
                &mut on,
            )
            .changed()
                && !self.startup_busy
            {
                // Écriture dans le registre en arrière-plan : l'interface n'attend pas. Le résultat arrive par
                // `Msg::StartupSet` (l'interrupteur revient en arrière si Windows a refusé).
                self.startup_busy = true;
                self.startup_on = Some(on);
                let (tx, ctx) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let ok = crate::procs::set_startup(on);
                    let _ = tx.send(super::super::Msg::StartupSet(on, ok));
                    ctx.request_repaint();
                });
            }
            // La croix cache la fenêtre au lieu de quitter ; `tick_tray` (fenetre.rs) crée l'icône et intercepte la fermeture.
            // Pas d'icône de zone de notification sur Mac ni sous Linux pour l'instant (pistes Mac et Linux) : réglage
            // masqué (sans icône, la fenêtre cachée ne pourrait plus être rouverte).
            #[cfg(windows)]
            t::switch_row(
                ui,
                tr("Réduire dans la zone de notification à la fermeture"),
                Some(tr(
                    "L'application reste active en fond pour les notifications",
                )),
                &mut self.settings.close_to_tray,
            );
            // Chaîne ouverte au démarrage : champ avec autocomplétion à gauche, interrupteur à droite. L'interrupteur
            // active le champ (focus immédiat) ; le désactiver vide le champ et supprime la chaîne enregistrée.
            let edit_id = egui::Id::unique("startup_channel_edit");
            let label = tr("Chaîne ouverte au démarrage");
            let mut on = self.startup_armed || !self.settings.startup_channel.is_empty();
            let mut toggled = false;
            let mut submit = false;
            let mut edit_rect = egui::Rect::NOTHING;
            let mut focused = false;
            // Le champ rétrécit (de 200 à 110) avant que la ligne ne passe sous le libellé : le calcul reprend la règle
            // de `control_row_impl` (libellé limité à 170, « ? » 36, retraits 14 + 12) pour que la ligne ne se range
            // dessous que s'il n'y a vraiment pas assez de place, même au minimum.
            let label_min = ui
                .painter()
                .layout_no_wrap(
                    label.to_string(),
                    egui::FontId::proportional(14.5),
                    t::TEXT(),
                )
                .size()
                .x
                .min(170.0)
                + 4.0;
            let field_w = (ui.available_width()
                - 44.0
                - ui.spacing().item_spacing.x
                - 14.0
                - 12.0
                - 36.0
                - label_min
                - 2.0)
                .clamp(110.0, 200.0);
            t::control_row(ui, label, |ui| {
                // Disposition de droite à gauche : l'interrupteur d'abord (au bord), le champ à sa gauche.
                toggled = t::switch_only(ui, label, &mut on).changed();
                if toggled && on {
                    self.startup_focus = true;
                }
                // Hors saisie, le champ reflète la chaîne enregistrée (une saisie abandonnée est annulée).
                if !(self.startup_editing) {
                    self.startup_draft = self.settings.startup_channel.clone();
                }
                let edit = ui.add_enabled(
                    on,
                    egui::TextEdit::singleline(&mut self.startup_draft)
                        .id(edit_id)
                        .hint_text(tr("Nom de la chaîne"))
                        .char_limit(60)
                        .desired_width(field_w),
                );
                // Un clic sur le champ grisé active l'option (comme l'interrupteur) et lui donne le focus.
                if !on
                    && ui
                        .interact(edit.rect, edit_id.with("off_click"), egui::Sense::click())
                        .clicked()
                {
                    on = true;
                    toggled = true;
                    self.startup_focus = true;
                }
                // Focus donné au champ dès l'activation : redemandé à chaque image tant qu'il n'est pas obtenu.
                if self.startup_focus {
                    if !on || edit.has_focus() {
                        self.startup_focus = false;
                    } else {
                        edit.request_focus();
                        ui.ctx().request_repaint();
                    }
                }
                edit_rect = edit.rect;
                focused = edit.has_focus();
                submit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                // Retour visuel sans texte : le contour du champ passe au vert (chaîne enregistrée) ou au rouge
                // (chaîne supprimée), puis s'efface en fondu.
                if let Some((t0, ok)) = self.startup_feedback {
                    let age = t0.elapsed().as_secs_f32();
                    if age < 2.0 {
                        let fade = t::motion::toast_alpha(ui.ctx(), age, 2.0, true);
                        let col =
                            if ok { t::POSITIVE() } else { t::NEGATIVE() }.gamma_multiply(fade);
                        ui.painter().rect_stroke(
                            edit.rect,
                            egui::CornerRadius::same(t::RADIUS_INNER),
                            egui::Stroke::new(1.5, col),
                            egui::StrokeKind::Inside,
                        );
                    } else {
                        self.startup_feedback = None;
                    }
                }
            });
            if toggled {
                if on {
                    self.startup_armed = true;
                } else {
                    // Désactivé : champ vidé, chaîne oubliée, contour rouge. Sans chaîne enregistrée au départ, il n'y
                    // a rien à signaler.
                    if !self.settings.startup_channel.is_empty() {
                        self.startup_feedback = Some((std::time::Instant::now(), false));
                    }
                    self.startup_armed = false;
                    self.startup_draft.clear();
                    self.settings.startup_channel.clear();
                }
            }
            if submit {
                if let Some(l) = crate::login::normalize(&self.startup_draft) {
                    self.startup_draft = l.clone();
                    self.settings.startup_channel = l.clone();
                    self.startup_feedback = Some((std::time::Instant::now(), true));
                }
            }
            self.startup_editing = focused || self.startup_hover;
            self.startup_dropdown(ui.ctx(), edit_rect, edit_id, focused);
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                ui.add(
                    egui::Label::new(
                        RichText::new(tr("Son live s'ouvre au lancement si elle est en direct. Sinon, l'accueil s'affiche normalement."))
                            .size(12.0)
                            .color(t::MUTED()),
                    )
                    .wrap(),
                );
            });
        });
        cols[c].add_space(t::SP_M);
    }

    /// Section « Notifications » : alerte quand un favori passe en direct, et notification de test.
    pub(super) fn card_notifications(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
    ) {
        let tr = crate::i18n::tr;
        let c = pick(200.0);
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Notifications"));
            ui.add_space(t::SP_S);
            t::switch_row(
                ui,
                tr(crate::i18n::os::NOTIFY),
                Some(tr("Prévient quand un favori passe en direct")),
                &mut self.settings.notify_live,
            );
            if self.settings.notify_live {
                ui.horizontal(|ui| {
                    ui.add_space(t::SP_M);
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr("Activé : vos favoris sont vérifiés toutes les minutes en arrière-plan, même pendant un live. Cela utilise un peu plus d'internet et de mémoire."))
                                .size(12.0)
                                .color(t::MUTED()),
                        )
                        .wrap(),
                    );
                });
                ui.add_space(t::SP_XS);
            }
            // Le test est toujours disponible, même quand les notifications sont désactivées.
            ui.horizontal(|ui| {
                ui.add_space(t::SP_M);
                if t::icon_text_button(
                    ui,
                    t::BtnKind::Secondary,
                    t::NavIcon::Play,
                    tr("Envoyer une notification de test"),
                )
                .clicked()
                {
                    crate::procs::notify(
                        "Velka",
                        tr("Notification de test : tout fonctionne."),
                        "",
                    );
                }
            });
        });
        cols[c].add_space(t::SP_M);
    }

    /// Groupe « Stockage et configuration » de la section « Avancé et Système » : cache des images et « Redémarrer
    /// l'application ». Renvoie `true` quand le redémarrage est demandé (les notifications ont leur propre section, le
    /// démarrage est dans « Général »).
    pub(super) fn card_system(
        &mut self,
        cols: &mut [egui::Ui],
        pick: &mut dyn FnMut(f32) -> usize,
        ctx: &egui::Context,
    ) -> bool {
        let tr = crate::i18n::tr;
        let c = pick(170.0);
        let mut restart = false;
        t::card(&mut cols[c], |ui| {
            ui.set_width(ui.available_width());
            t::card_title(ui, tr("Stockage et configuration"));
            ui.add_space(t::SP_S);
            let size = self.cache_size.unwrap_or(0);
            let mut clear = false;
            t::control_row(ui, tr("Cache des images"), |ui| {
                clear =
                    t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Trash, tr("Vider"))
                        .clicked();
                t::pill(ui, None, &format_bytes(size), t::TEXT(), t::SURFACE_3());
            });
            if clear && !self.cache_clearing {
                // Des milliers de fichiers : suppression en arrière-plan, la taille est relue à la fin.
                self.cache_clearing = true;
                let (tx, ctx) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    clear_cache();
                    let _ = tx.send(super::super::Msg::CacheCleared);
                    ctx.request_repaint();
                });
            }
            ui.add_space(t::SP_XS);
            // Les deux boutons côte à côte (à la ligne si la place manque). Le dossier est celui de `settings.json`
            // (sauvegarde des réglages à la main) ; rien n'est exporté ni importé.
            ui.horizontal_wrapped(|ui| {
                ui.add_space(t::SP_M);
                let label = tr("Ouvrir le dossier de configuration");
                let resp =
                    t::icon_text_button(ui, t::BtnKind::Secondary, t::NavIcon::Folder, label);
                t::paint_highlight(ui, resp.rect, label);
                if resp.clicked() {
                    let dir = crate::config::settings_path()
                        .parent()
                        .map_or_else(crate::logging::data_dir, std::path::Path::to_path_buf);
                    crate::procs::open_folder(&dir);
                }
                restart = t::icon_text_button(
                    ui,
                    t::BtnKind::Secondary,
                    t::NavIcon::Power,
                    tr("Redémarrer l'application"),
                )
                .clicked();
            });
        });
        cols[c].add_space(t::SP_M);
        restart
    }

    /// Liste déroulante sous le champ « chaîne au démarrage » : favoris et récents qui correspondent, puis les chaînes
    /// proposées par Twitch, avec photo, pseudo et spectateurs. Un clic enregistre la chaîne.
    fn startup_dropdown(
        &mut self,
        ctx: &egui::Context,
        field: egui::Rect,
        edit_id: egui::Id,
        focused: bool,
    ) {
        let needle = self
            .startup_draft
            .trim()
            .trim_start_matches(['@', '#'])
            .to_lowercase();
        if needle.is_empty() || !(focused || self.startup_hover) {
            self.startup_hover = false;
            return;
        }
        let mut rows: Vec<String> = Vec::new();
        for l in self
            .settings
            .favorites
            .iter()
            .chain(self.history.items.iter())
        {
            if !rows.contains(l) && l.contains(&needle) {
                rows.push(l.clone());
            }
        }
        rows.truncate(4);
        if self.suggest_for == self.suggest_want {
            for s in self.suggestions.iter().take(6) {
                if !rows.contains(&s.login) {
                    rows.push(s.login.clone());
                }
            }
        }
        if rows.is_empty() {
            self.startup_hover = false;
            return;
        }
        // Exactement la largeur du champ, alignée sous lui (comme la recherche de la barre latérale).
        let width = field.width();
        let pos = egui::Pos2::new(field.left(), field.bottom() + 6.0);
        let mut picked: Option<String> = None;
        let area = egui::Area::new(egui::Id::unique("startup_suggest"))
            .fixed_pos(pos)
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t::SURFACE_1())
                    .stroke(egui::Stroke::new(1.0, t::BORDER_HOVER()))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.set_width(width - 14.0);
                        for l in &rows {
                            let viewers = match self.status_cache.get(l).map(|(_, s)| s) {
                                Some(crate::twitch::LiveStatus::Live { viewers, .. }) => {
                                    Some(*viewers)
                                }
                                _ => self
                                    .suggestions
                                    .iter()
                                    .find(|s| s.login == *l)
                                    .and_then(|s| s.viewers),
                            };
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::Vec2::new(ui.available_width(), t::BTN_H),
                                egui::Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(8),
                                    t::SURFACE_3(),
                                );
                            }
                            t::focus_ring(ui, &resp, rect, 8);
                            resp.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, l.as_str())
                            });
                            let photo = self.avatars.get(l);
                            t::avatar(
                                ui,
                                egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                                11.0,
                                l,
                                photo,
                            );
                            t::paint_name_line(
                                ui.painter(),
                                egui::Pos2::new(rect.left() + 34.0, rect.top() + 8.0),
                                l,
                                13.0,
                                t::TEXT(),
                                rect.width() - 34.0 - 64.0,
                            );
                            if let Some(v) = viewers {
                                ui.painter().text(
                                    egui::Pos2::new(rect.right() - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    crate::login::format_viewers(v),
                                    egui::FontId::proportional(12.0),
                                    t::POSITIVE(),
                                );
                            }
                            if resp.clicked() {
                                picked = Some(l.clone());
                            }
                        }
                    });
            });
        self.startup_hover = ctx
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| area.response.rect.contains(p));
        if let Some(l) = picked {
            self.startup_draft = l.clone();
            self.settings.startup_channel = l.clone();
            self.startup_armed = true;
            self.startup_feedback = Some((std::time::Instant::now(), true));
            ctx.memory_mut(|m| m.surrender_focus(edit_id));
            self.startup_hover = false;
            self.startup_editing = false;
        }
    }
}
