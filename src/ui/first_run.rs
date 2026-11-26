//! Écran de premier lancement : état des outils et installation.
use crate::tools::Tool;
use crate::ui::theme as t;
use eframe::egui::{self, RichText};

#[derive(Debug, Clone)]
pub enum ToolStatus {
    Missing,
    Installing(f32, String),
    Ready(String),
    Failed(String),
}

pub struct ToolRow {
    pub tool: Tool,
    pub status: ToolStatus,
}

/// Dessine l'écran ; renvoie `true` si l'utilisateur demande l'installation.
pub fn show(ui: &mut egui::Ui, rows: &[ToolRow], busy: bool) -> bool {
    let mut install = false;
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        crate::ui::logo::show(ui, 56.0, t::ACCENT_GFX(), egui::Sense::hover());
        ui.add_space(14.0);
        ui.label(
            RichText::new(crate::i18n::tr("Bienvenue dans Velka"))
                .size(26.0)
                .strong()
                .color(t::TEXT()),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(crate::i18n::tr(
                "Quelques composants sont nécessaires pour lire les lives Twitch.",
            ))
            .color(t::MUTED()),
        );
        ui.label(
            RichText::new(crate::i18n::tr(
                "Ils sont installés dans votre dossier utilisateur, sans droits administrateur.",
            ))
            .color(t::MUTED()),
        );
        ui.add_space(24.0);
        ui.allocate_ui(egui::vec2(520.0_f32.min(ui.available_width()), 0.0), |ui| {
            for row in rows {
                t::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(row.tool.label()).strong().color(t::TEXT()));
                    ui.add_space(4.0);
                    match &row.status {
                        ToolStatus::Missing => {
                            ui.label(
                                RichText::new(crate::i18n::tr("Non installé")).color(t::MUTED()),
                            );
                        }
                        ToolStatus::Installing(f, msg) => {
                            t::progress_bar(ui, *f, msg);
                        }
                        ToolStatus::Ready(path) => {
                            ui.label(RichText::new(crate::i18n::tr("Prêt")).color(t::POSITIVE()));
                            ui.label(RichText::new(path.as_str()).size(11.5).color(t::FAINT()));
                        }
                        ToolStatus::Failed(err) => {
                            ui.label(RichText::new(err.as_str()).color(t::NEGATIVE()));
                        }
                    }
                });
                ui.add_space(10.0);
            }
        });
        ui.add_space(12.0);
        let any_failed = rows
            .iter()
            .any(|r| matches!(r.status, ToolStatus::Failed(_)));
        let label = if any_failed {
            crate::i18n::tr("Réessayer")
        } else {
            crate::i18n::tr("Tout installer")
        };
        if busy {
            ui.label(RichText::new(crate::i18n::tr("Installation en cours…")).color(t::MUTED()));
        } else if t::icon_text_button(
            ui,
            t::BtnKind::Primary,
            if any_failed {
                t::NavIcon::Refresh
            } else {
                t::NavIcon::Download
            },
            label,
        )
        .clicked()
        {
            install = true;
        }
    });
    install
}
