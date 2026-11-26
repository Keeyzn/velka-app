//! Mise à jour automatique : vérification, téléchargement et bandeau.
use super::*;

/// Résultat de la dernière recherche demandée avec le bouton (la recherche automatique ne l'affecte pas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum UpdateCheck {
    /// Rien à afficher.
    Idle,
    /// Recherche terminée (nouvelle version ou non : voir `update_info`).
    Done,
    /// Recherche échouée : message de l'erreur.
    Failed(String),
}

impl VelkaApp {
    /// Recherche d'une nouvelle version : au démarrage puis toutes les 6 heures.
    pub(super) fn tick_update(&mut self, ctx: &egui::Context) {
        let due = self
            .update_checked
            .is_none_or(|t| t.elapsed() > Duration::from_secs(6 * 3600));
        if !self.settings.check_updates || !due || self.update_progress.is_some() {
            return;
        }
        self.request_update_check(ctx, false);
    }

    /// Lance la vérification (la seule : l'automatique et le bouton « Rechercher des mises à jour » passent ici). Un
    /// seul contrôle à la fois : refusé tant qu'un autre est en cours. `manual` : le résultat est affiché dans les
    /// Réglages (À propos).
    pub(super) fn request_update_check(&mut self, ctx: &egui::Context, manual: bool) -> bool {
        if self.update_inflight {
            return false;
        }
        self.update_checked = Some(Instant::now());
        self.update_inflight = true;
        self.update_manual = manual;
        if manual {
            self.update_check = UpdateCheck::Idle;
        }
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::UpdateInfo(crate::update::check()));
            ctx.request_repaint();
        });
        true
    }

    /// Télécharge la version proposée ; à la fin, `Msg::UpdateReady` remplace l'exécutable.
    pub(super) fn start_update(&mut self, ctx: &egui::Context) {
        // Plateforme sans remplacement automatique : le bandeau ouvre la page de téléchargement.
        if !crate::update::replaces_itself() {
            ctx.open_url(egui::OpenUrl::new_tab(crate::update::releases_page()));
            return;
        }
        let Some(info) = self.update_info.clone() else {
            return;
        };
        if self.update_progress.is_some() {
            return;
        }
        self.update_progress = Some(0.0);
        self.update_error = None;
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let (txp, ctxp) = (tx.clone(), ctx.clone());
            let res = crate::update::download(&info, &move |f| {
                let _ = txp.send(Msg::UpdateProgress(f));
                ctxp.request_repaint();
            });
            let _ = tx.send(Msg::UpdateReady(res));
            ctx.request_repaint();
        });
    }

    /// Bandeau de mise à jour (accueil et réglages).
    pub(super) fn update_banner(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(info) = self.update_info.clone() else {
            return;
        };
        let tr = crate::i18n::tr;
        let mut go = false;
        egui::Frame::new()
            .fill(theme::SURFACE_2())
            .stroke(egui::Stroke::new(1.0, theme::ACCENT_LINE()))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_INNER))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if let Some(f) = self.update_progress {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {:.0} %",
                                tr("Téléchargement de la mise à jour…"),
                                f * 100.0
                            ))
                            .color(theme::TEXT()),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                tr("Nouvelle version disponible :"),
                                info.version
                            ))
                            .color(theme::TEXT()),
                        );
                        let label = if self.update_error.is_some() {
                            tr("Réessayer")
                        } else {
                            tr("Mettre à jour")
                        };
                        go = theme::icon_text_button(
                            ui,
                            theme::BtnKind::Primary,
                            theme::NavIcon::Download,
                            label,
                        )
                        .clicked();
                    }
                });
                if let Some(e) = &self.update_error {
                    ui.label(
                        egui::RichText::new(format!("{} {e}", tr("Mise à jour impossible :")))
                            .size(12.0)
                            .color(theme::NEGATIVE()),
                    );
                }
            });
        ui.add_space(theme::SP_M);
        if go {
            self.start_update(ctx);
        }
    }
}
