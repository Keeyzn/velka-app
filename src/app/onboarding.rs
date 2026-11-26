//! Didacticiel de bienvenue : quatre courtes étapes (présentation, apparence, lecture, notifications), affichées une
//! seule fois sur l'accueil d'une nouvelle installation. « Passer » (ou Échap) le ferme à tout moment ; chaque
//! réglage proposé s'applique tout de suite et reste modifiable dans les Réglages.
use super::*;

/// Nombre d'étapes.
const STEPS: usize = 4;
const CARD_W: f32 = 560.0;
/// Hauteur utile de chaque étape ; la carte prend la plus grande pour ne pas déplacer les boutons.
const PAGE_H: [f32; STEPS] = [540.0, 390.0, 530.0, 500.0];

/// Sortie douce : démarrage vif, arrivée lente.
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

/// Légère surélévation à l'arrivée du logo (dépasse un peu 1 puis se pose).
fn ease_out_back(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let (c1, c3) = (1.2_f32, 2.2_f32);
    1.0 + c3 * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}

impl VelkaApp {
    /// Le didacticiel doit-il s'afficher maintenant ?
    pub(super) fn onboarding_active(&self) -> bool {
        !self.settings.onboarding_done && self.view == View::Home && !self.busy
    }

    /// Relance le didacticiel depuis les Réglages : retour à l'accueil, première étape, entrée animée comme neuve.
    pub(super) fn restart_onboarding(&mut self, ctx: &egui::Context) {
        self.settings.onboarding_done = false;
        self.onb_step = 0;
        self.onb_closing = false;
        ctx.data_mut(|d| d.remove::<bool>(egui::Id::unique("onb_open_seeded")));
        ctx.animate_value_with_time(egui::Id::unique("onb_pos"), 0.0, 0.0);
    }

    fn finish_onboarding(&mut self) {
        self.onb_closing = true;
    }

    pub(super) fn onboarding_ui(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let bar_h = fenetre::TITLE_BAR_H;
        let backdrop_id = egui::Id::unique("onb_backdrop");
        let bar_id = egui::Id::unique("onb_titlebar");
        let modal_id = egui::Id::unique("onb_modal");
        ctx.memory_mut(|m| {
            m.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, backdrop_id))
        });

        // Apparition : le voile, puis la carte qui monte en fondu.
        let closing = self.onb_closing;
        // Départ à 0 à chaque ouverture (egui renvoie sinon directement la cible au premier appel : pas d'animation).
        let open_id = egui::Id::unique("onb_open");
        let seeded_id = egui::Id::unique("onb_open_seeded");
        if ctx.data_mut(|d| d.get_temp::<bool>(seeded_id)).is_none() {
            ctx.animate_bool_with_time(open_id, false, 0.0);
            ctx.data_mut(|d| d.insert_temp(seeded_id, true));
        }
        let open = theme::motion::eased(
            ctx,
            open_id,
            !closing,
            if closing {
                theme::motion::INTRO_OUT
            } else {
                theme::motion::INTRO_IN
            },
        );
        if closing && open <= 0.001 {
            ctx.data_mut(|d| d.remove::<bool>(seeded_id));
            self.settings.onboarding_done = true;
            self.onb_closing = false;
            self.onb_step = 0;
            return;
        }
        if closing {
            ctx.request_repaint();
        }
        // Position animée entre les étapes (0.0, 1.0, 2.0… en régime établi).
        let pos = ctx.animate_value_with_time(
            egui::Id::unique("onb_pos"),
            self.onb_step as f32,
            theme::move_secs(ctx, 0.5),
        );
        if (pos - self.onb_step as f32).abs() > 0.001 || open < 1.0 {
            ctx.request_repaint();
        }

        egui::Area::new(backdrop_id)
            .order(egui::Order::Foreground)
            .sense(egui::Sense::hover())
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(screen.size(), egui::Sense::hover());
                ui.painter().rect_filled(
                    r,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_black_alpha((165.0 * open) as u8),
                );
            });
        let bar = egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), bar_h));
        egui::Area::new(bar_id)
            .order(egui::Order::Foreground)
            .sense(egui::Sense::hover())
            .fixed_pos(bar.min)
            .show(ctx, |ui| {
                ui.set_min_size(bar.size());
                ui.set_max_size(bar.size());
                self.title_bar(ui, ctx, "onboarding");
            });

        let avail_h = screen.height() - bar_h;
        // Hauteur unique (celle de la page la plus haute) : « Suivant » et « Retour » restent au même endroit d'une étape à l'autre.
        let card_h = PAGE_H.iter().copied().fold(0.0_f32, f32::max);
        let size = egui::vec2(
            CARD_W.min(screen.width() - 24.0),
            card_h.min(avail_h - 16.0),
        );
        // Animations réduites : la carte apparaît en fondu, sans monter.
        let rise = if theme::moves(ctx) {
            (1.0 - open) * 18.0
        } else {
            0.0
        };
        let center = egui::pos2(
            screen.center().x,
            screen.top() + bar_h + avail_h / 2.0 + rise,
        );
        let rect = egui::Rect::from_center_size(center, size);

        let mut skip = false;
        let mut next = false;
        let mut back = false;
        egui::Area::new(modal_id)
            .order(egui::Order::Foreground)
            .sense(egui::Sense::hover())
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_opacity(open);
                // Carte dessinée à taille et position fixes (pas de `Frame`, qui épouse le contenu) : le contenu des pages qui
                // glissent ne peut ni agrandir ni décaler la carte, et il est coupé à ses bords.
                let (inner, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect(
                    inner,
                    egui::CornerRadius::same(theme::RADIUS_CARD),
                    theme::BG(),
                    egui::Stroke::new(1.0, theme::BORDER_HOVER()),
                    egui::StrokeKind::Inside,
                );
                {
                    {
                        let pad = 28.0;
                        // En-tête : points de progression à gauche, « Passer » à droite.
                        let head = egui::Rect::from_min_size(
                            inner.min + egui::vec2(pad, 22.0),
                            egui::vec2(size.x - 2.0 * pad, 28.0),
                        );
                        self.onb_dots(ui, head, pos);
                        let skip_label = crate::i18n::tr("Passer");
                        let skip_w = ui
                            .painter()
                            .layout_no_wrap(
                                skip_label.to_string(),
                                egui::FontId::proportional(14.0),
                                theme::MUTED(),
                            )
                            .size()
                            .x
                            + 24.0;
                        let skip_rect = egui::Rect::from_min_size(
                            egui::pos2(head.right() - skip_w, head.top()),
                            egui::vec2(skip_w, 28.0),
                        );
                        if self.onb_ghost_button(ui, skip_rect, skip_label, "onb_skip") {
                            skip = true;
                        }
                        // Corps : chaque page glisse et s'efface selon sa distance à la position animée.
                        let body = egui::Rect::from_min_max(
                            egui::pos2(inner.left() + pad, head.bottom() + 18.0),
                            egui::pos2(inner.right() - pad, inner.bottom() - 82.0),
                        );
                        for page in 0..STEPS {
                            let d = page as f32 - pos;
                            if d.abs() >= 1.0 {
                                continue;
                            }
                            let e = 1.0 - d.abs();
                            let shift = egui::vec2(d * 70.0, 0.0);
                            let current = page == self.onb_step && d.abs() < 0.02;
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(body.translate(shift))
                                    .id_salt(("onb_page", page)),
                                |ui| {
                                    // L'ancienne page s'efface avant que la nouvelle n'apparaisse : jamais deux pages lisibles en même temps.
                                    ui.set_opacity(ease_out((e - 0.6) / 0.4));
                                    // Coupé aux bords de la carte et à la zone de contenu : une page qui s'efface ne passe pas sous les boutons
                                    // quand la carte change de hauteur.
                                    ui.shrink_clip_rect(egui::Rect::from_min_max(
                                        egui::pos2(inner.left() + 1.0, body.top()),
                                        egui::pos2(inner.right() - 1.0, body.bottom()),
                                    ));
                                    ui.set_width(body.width());
                                    self.onb_page(ui, ctx, page, e * open);
                                    // Page en transition : une zone « glisser » (jamais cliquable, donc ni main ni clic)
                                    // la rend inerte sans l'estomper, ce qui évitait un saut de couleurs à l'arrivée.
                                    if !current {
                                        ui.interact(
                                            body.translate(shift),
                                            egui::Id::unique(("onb_inert", page)),
                                            egui::Sense::drag(),
                                        );
                                    }
                                },
                            );
                        }
                        // Pied : Retour (dès la 2e étape) et bouton principal.
                        let foot_y = inner.bottom() - 28.0 - 36.0;
                        let last = self.onb_step + 1 == STEPS;
                        let primary_label = if last {
                            crate::i18n::tr("C'est parti")
                        } else {
                            crate::i18n::tr("Suivant")
                        };
                        let pw = theme::icon_text_button_width(ui, primary_label).max(150.0);
                        let primary = egui::Rect::from_min_size(
                            egui::pos2(inner.right() - pad - pw, foot_y),
                            egui::vec2(pw, 36.0),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(primary), |ui| {
                            let icon = if last {
                                theme::NavIcon::Check
                            } else {
                                theme::NavIcon::Play
                            };
                            if theme::icon_text_button_min(
                                ui,
                                theme::BtnKind::Primary,
                                icon,
                                primary_label,
                                pw,
                            )
                            .clicked()
                            {
                                next = true;
                            }
                        });
                        if self.onb_step > 0 {
                            let bl = crate::i18n::tr("Retour");
                            let bw = theme::icon_text_button_width(ui, bl).max(110.0);
                            let brect = egui::Rect::from_min_size(
                                egui::pos2(inner.left() + pad, foot_y),
                                egui::vec2(bw, 36.0),
                            );
                            ui.scope_builder(egui::UiBuilder::new().max_rect(brect), |ui| {
                                if theme::icon_text_button_min(
                                    ui,
                                    theme::BtnKind::Secondary,
                                    theme::NavIcon::Back,
                                    bl,
                                    bw,
                                )
                                .clicked()
                                {
                                    back = true;
                                }
                            });
                        }
                        // Mention discrète sous les boutons.
                        let note =
                            crate::i18n::tr("Tout se règle aussi plus tard dans les Réglages.");
                        ui.painter().text(
                            egui::pos2(inner.center().x, inner.bottom() - 16.0),
                            egui::Align2::CENTER_CENTER,
                            note,
                            egui::FontId::proportional(12.0),
                            theme::FAINT(),
                        );
                    }
                }
            });
        for id in [backdrop_id, bar_id, modal_id] {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        }

        if closing {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            skip = true;
        }
        if skip {
            self.finish_onboarding();
        } else if next {
            if self.onb_step + 1 >= STEPS {
                self.finish_onboarding();
            } else {
                self.onb_step += 1;
            }
        } else if back && self.onb_step > 0 {
            self.onb_step -= 1;
        }
    }

    /// Points de progression : le point actif s'étire en pastille d'accent au fil de la position animée.
    fn onb_dots(&self, ui: &mut egui::Ui, head: egui::Rect, pos: f32) {
        let p = ui.painter();
        let mut x = head.left();
        let cy = head.center().y;
        for i in 0..STEPS {
            let k = (1.0 - (i as f32 - pos).abs()).clamp(0.0, 1.0);
            let w = 8.0 + 18.0 * k;
            let r = egui::Rect::from_min_size(egui::pos2(x, cy - 4.0), egui::vec2(w, 8.0));
            let done = (i as f32) < pos - 0.5;
            let base = if done {
                theme::ACCENT_GFX().gamma_multiply(0.55)
            } else {
                theme::SURFACE_3()
            };
            p.rect_filled(
                r,
                egui::CornerRadius::same(4),
                base.lerp_to_gamma(theme::ACCENT_GFX(), k),
            );
            x += w + 6.0;
        }
    }

    /// Bouton sans fond (texte discret) ; renvoie `true` au clic.
    fn onb_ghost_button(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        label: &str,
        salt: &str,
    ) -> bool {
        let resp = ui.interact(rect, egui::Id::unique(salt), egui::Sense::click());
        let k = theme::hover_k(ui, resp.id, resp.hovered());
        let col = theme::MUTED().lerp_to_gamma(theme::TEXT(), k);
        if k > 0.0 {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(theme::RADIUS_INNER),
                theme::SURFACE_2().gamma_multiply(k),
            );
        }
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            col,
        );
        theme::focus_ring(ui, &resp, rect, theme::RADIUS_INNER);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        resp.clicked()
    }

    /// Titre et sous-titre d'une page, avec leur entrée en fondu.
    fn onb_title(ui: &mut egui::Ui, title: &str, sub: &str) {
        ui.label(
            egui::RichText::new(title)
                .size(24.0)
                .strong()
                .color(theme::TEXT()),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new(sub).size(14.0).color(theme::MUTED()));
        ui.add_space(18.0);
    }

    /// Une ligne qui apparaît en décalé : `e` est la progression de la page (0 à 1), `k` le rang de la ligne.
    fn onb_row(ui: &mut egui::Ui, e: f32, k: usize, add: impl FnOnce(&mut egui::Ui)) {
        let a = ease_out((e - 0.6) / 0.4 * 2.2 - 0.25 * k as f32);
        ui.scope(|ui| {
            ui.set_opacity(a);
            add(ui);
        });
    }

    fn onb_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, page: usize, e: f32) {
        let tr = crate::i18n::tr;
        match page {
            0 => {
                let s = ease_out_back(e);
                ui.add_space(4.0);
                let (slot, _) =
                    ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
                let logo = egui::Rect::from_center_size(
                    slot.center(),
                    egui::Vec2::splat(60.0 * s.max(0.01)),
                );
                crate::ui::logo::paint(ui, logo, theme::ACCENT_GFX());
                ui.add_space(14.0);
                Self::onb_title(
                    ui,
                    tr("Bienvenue dans Velka"),
                    tr("Lecteur Twitch léger avec multistream, chat et notifications."),
                );
                let items = [
                    (theme::NavIcon::Play, tr("Multistream")),
                    (theme::NavIcon::Panel, tr("Chat avec émotes")),
                    (theme::NavIcon::Pip, tr("Mini-lecteur et PiP")),
                    (
                        theme::NavIcon::Feather,
                        tr("Ultra légère et optimisée, parfaite pour jouer en même temps."),
                    ),
                ];
                for (k, (icon, text)) in items.into_iter().enumerate() {
                    Self::onb_row(ui, e, k + 1, |ui| {
                        ui.horizontal(|ui| {
                            let (chip, _) = ui
                                .allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(chip.center(), 18.0, theme::ACCENT_SOFT());
                            theme::paint_nav_icon(
                                ui.painter(),
                                chip.center(),
                                icon,
                                theme::ACCENT_TEXT(),
                            );
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(text).size(15.5).color(theme::TEXT()));
                        });
                        ui.add_space(8.0);
                    });
                }
            }
            1 => {
                Self::onb_title(
                    ui,
                    tr("Choisissez votre style"),
                    tr("Thème et couleur de l'application."),
                );
                Self::onb_row(ui, e, 0, |ui| {
                    theme::control_row(ui, tr("Thème"), |ui| {
                        if theme::theme_segmented(
                            ui,
                            &mut self.settings.theme,
                            &theme::theme_names(),
                        ) {
                            self.begin_theme_change(ctx);
                        }
                    });
                });
                ui.add_space(14.0);
                Self::onb_row(ui, e, 1, |ui| {
                    ui.label(
                        egui::RichText::new(tr("Couleur d'accent"))
                            .size(14.5)
                            .color(theme::TEXT()),
                    );
                    ui.add_space(10.0);
                    self.onb_swatches(ui, ctx);
                });
            }
            2 => {
                Self::onb_title(
                    ui,
                    tr("Réglez la lecture"),
                    tr("Les réglages les plus utiles, en un coup d'œil."),
                );
                Self::onb_row(ui, e, 0, |ui| {
                    theme::control_row(ui, tr("Qualité"), |ui| {
                        let order = settings::QUALITY_ORDER;
                        let labels: Vec<&str> = order
                            .iter()
                            .map(|&q| settings::QUALITIES[usize::from(q)])
                            .collect();
                        let mut idx = order
                            .iter()
                            .position(|&q| q == self.settings.quality)
                            .unwrap_or(0) as u8;
                        if theme::segmented(ui, &mut idx, &labels) {
                            self.settings.quality = order[usize::from(idx)];
                        }
                    });
                });
                Self::onb_row(ui, e, 1, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Démarrer en muet"),
                        None,
                        &mut self.settings.start_muted,
                    );
                });
                Self::onb_row(ui, e, 2, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Mini-lecteur"),
                        Some(tr("Garder le live dans un coin quand on quitte la page")),
                        &mut self.settings.mini_player,
                    );
                });
                Self::onb_row(ui, e, 3, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Faible latence"),
                        None,
                        &mut self.settings.low_latency,
                    );
                });
                Self::onb_row(ui, e, 4, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Molette de la souris sur la vidéo = volume"),
                        None,
                        &mut self.settings.wheel_volume,
                    );
                });
                Self::onb_row(ui, e, 5, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Clic sur la vidéo = pause"),
                        None,
                        &mut self.settings.click_pause,
                    );
                });
            }
            _ => {
                Self::onb_title(
                    ui,
                    tr("Restez informé"),
                    tr("Ne ratez plus vos streamers préférés."),
                );
                let (mut login, mut cancel) = (false, false);
                Self::onb_row(ui, e, 0, |ui| self.onb_twitch(ui, &mut login, &mut cancel));
                ui.add_space(6.0);
                Self::onb_row(ui, e, 1, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Chaînes suivies sur Twitch"),
                        None,
                        &mut self.settings.show_follows,
                    );
                });
                Self::onb_row(ui, e, 2, |ui| {
                    theme::switch_row(
                        ui,
                        tr(crate::i18n::os::NOTIFY),
                        None,
                        &mut self.settings.notify_live,
                    );
                });
                Self::onb_row(ui, e, 3, |ui| {
                    theme::switch_row(
                        ui,
                        tr("Vérification des mises à jour"),
                        None,
                        &mut self.settings.check_updates,
                    );
                });
                if login {
                    self.start_login(ctx);
                }
                if cancel {
                    self.cancel_login();
                }
            }
        }
    }

    /// Connexion Twitch (même flux que dans les Réglages) : bouton, code à vérifier, ou compte connecté.
    fn onb_twitch(&self, ui: &mut egui::Ui, login: &mut bool, cancel: &mut bool) {
        let tr = crate::i18n::tr;
        theme::card(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.connected() {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    theme::paint_nav_icon(
                        ui.painter(),
                        r.center(),
                        theme::NavIcon::Check,
                        theme::POSITIVE(),
                    );
                    ui.label(egui::RichText::new(tr("Connecté en tant que")).color(theme::MUTED()));
                    ui.label(
                        egui::RichText::new(&self.settings.account_login)
                            .strong()
                            .color(theme::TEXT()),
                    );
                });
            } else if let Some(code) = &self.auth_code {
                ui.label(
                    egui::RichText::new(tr("Une page Twitch s'est ouverte. Vérifiez ce code, puis cliquez sur « Autoriser » :"))
                        .size(13.0)
                        .color(theme::MUTED()),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&code.user_code)
                            .size(22.0)
                            .strong()
                            .color(theme::TEXT()),
                    );
                    ui.add_space(8.0);
                    if theme::icon_text_button(
                        ui,
                        theme::BtnKind::Secondary,
                        theme::NavIcon::Close,
                        tr("Annuler"),
                    )
                    .clicked()
                    {
                        *cancel = true;
                    }
                });
            } else if self.auth_cancel.is_some() {
                ui.label(egui::RichText::new(tr("Connexion à Twitch…")).color(theme::MUTED()));
            } else {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(crate::auth::available(), |ui| {
                        if theme::icon_text_button(
                            ui,
                            theme::BtnKind::Primary,
                            theme::NavIcon::Login,
                            tr("Se connecter avec Twitch"),
                        )
                        .clicked()
                        {
                            *login = true;
                        }
                    });
                });
                if let Some(e) = &self.auth_error {
                    ui.label(
                        egui::RichText::new(e.as_str())
                            .size(12.0)
                            .color(theme::NEGATIVE()),
                    );
                } else if self
                    .auth_cancelled_at
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(8))
                {
                    ui.label(
                        egui::RichText::new(tr("Connexion annulée"))
                            .size(12.0)
                            .color(theme::MUTED()),
                    );
                    ui.ctx().request_repaint_after(Duration::from_secs(1));
                }
            }
        });
    }

    /// Pastilles de couleur d'accent : la couleur d'origine, puis quatre palettes. Le survol les grossit, la
    /// sélection est cerclée.
    fn onb_swatches(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // (palette, couleur) : l'orange des nouvelles installations est la palette « personnalisée » (5).
        let choices: [(u8, [u8; 3]); 5] = [
            (5, theme::ACCENT_NEW),
            (0, theme::PALETTES[0].1),
            (1, theme::PALETTES[1].1),
            (2, theme::PALETTES[2].1),
            (4, theme::PALETTES[4].1),
        ];
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for (palette, rgb) in choices {
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
                let selected = self.settings.palette == palette
                    && (palette != 5 || self.settings.accent_custom == theme::ACCENT_NEW);
                let k = theme::hover_k(ui, resp.id, resp.hovered());
                let sel = ctx.animate_bool_with_time(
                    resp.id.with("sel"),
                    selected,
                    theme::anim_secs(ctx, 0.18),
                );
                let col = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                let r = 14.0 + 2.0 * k + 1.5 * sel;
                let p = ui.painter();
                p.circle_filled(rect.center(), r, col);
                if sel > 0.0 {
                    p.circle_stroke(
                        rect.center(),
                        18.5,
                        egui::Stroke::new(2.0 * sel, theme::TEXT().gamma_multiply(sel)),
                    );
                    theme::paint_nav_icon(
                        p,
                        rect.center(),
                        theme::NavIcon::Check,
                        egui::Color32::WHITE.gamma_multiply(sel),
                    );
                }
                theme::focus_ring(ui, &resp, rect, 20);
                let name = if palette == 5 {
                    crate::i18n::tr("Orange (par défaut)")
                } else {
                    crate::i18n::tr(theme::PALETTES[usize::from(palette)].0)
                };
                resp.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, name)
                });
                if resp.clicked() && !selected {
                    self.settings.palette = palette;
                    if palette == 5 {
                        self.settings.accent_custom = theme::ACCENT_NEW;
                    }
                    changed = true;
                }
            }
        });
        if changed {
            apply_access(ctx, &self.settings);
        }
    }
}
