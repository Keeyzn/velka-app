//! Rappel de soutien : de temps en temps, un bandeau sur l'accueil propose de soutenir le projet (lien de don).
//! Jamais aux premiers lancements, et au plus une fois tous les `INTERVALLE_JOURS`.
use super::*;

/// Formulaire de don de la page Ko-fi du projet (dons sans contrepartie), ouvert directement, sans le reste de la
/// page.
pub(crate) const SUPPORT_URL: &str =
    "https://ko-fi.com/velkaapp/?hidefeed=true&widget=true&embed=true";

/// Nombre de lancements avant le premier rappel.
const LANCEMENTS_MIN: u32 = 5;
/// Jours d'utilisation avant le premier rappel.
const JOURS_MIN: u32 = 7;
/// Jours entre deux rappels.
const INTERVALLE_JOURS: u32 = 30;

/// Le rappel est-il dû ce jour (`today`, jours depuis 1970) ?
fn support_due(launches: u32, first_day: u32, last_day: u32, today: u32) -> bool {
    launches >= LANCEMENTS_MIN
        && first_day > 0
        && today >= first_day.saturating_add(JOURS_MIN)
        && (last_day == 0 || today >= last_day.saturating_add(INTERVALLE_JOURS))
}

impl VelkaApp {
    /// Au démarrage : compte le lancement et décide si le bandeau s'affiche pendant cette session. S'il s'affiche, la
    /// date est retenue tout de suite : un bandeau ignoré ne revient qu'au prochain intervalle.
    pub(super) fn init_support_reminder(&mut self) {
        if crate::smoke::enabled() {
            return;
        }
        let today = commun::today_days();
        let s = &mut self.settings;
        if s.first_use_day == 0 {
            s.first_use_day = today;
        }
        s.launch_count = s.launch_count.saturating_add(1);
        self.support_visible =
            support_due(s.launch_count, s.first_use_day, s.support_last_day, today);
        if self.support_visible {
            s.support_last_day = today;
        }
    }

    /// Bandeau de soutien (accueil et multistream), avec le bouton vers la page de don.
    pub(super) fn support_banner(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.support_visible {
            return;
        }
        let tr = crate::i18n::tr;
        let (mut support, mut later) = (false, false);
        egui::Frame::new()
            .fill(theme::SURFACE_2())
            .stroke(egui::Stroke::new(1.0, theme::ACCENT_LINE()))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_INNER))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(tr("Velka est gratuit et le restera. Si l'application vous plaît, un petit don aide à la faire vivre."))
                            .color(theme::TEXT()),
                    );
                    support = theme::icon_text_button(ui, theme::BtnKind::Primary, theme::NavIcon::Star, tr("Soutenir le projet")).clicked();
                    later = theme::icon_text_button(ui, theme::BtnKind::Secondary, theme::NavIcon::Close, tr("Plus tard")).clicked();
                });
            });
        ui.add_space(theme::SP_M);
        if support {
            ctx.open_url(egui::OpenUrl::new_tab(SUPPORT_URL));
        }
        if support || later {
            self.support_visible = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::support_due;

    #[test]
    fn rappel_de_temps_en_temps_seulement() {
        let jour = 20_000;
        // Premiers lancements ou première semaine : jamais.
        assert!(!support_due(1, jour, 0, jour));
        assert!(!support_due(4, jour, 0, jour + 60));
        assert!(!support_due(10, jour, 0, jour + 6));
        // Ensuite, une première fois…
        assert!(support_due(5, jour, 0, jour + 7));
        // …puis pas avant 30 jours.
        assert!(!support_due(50, jour, jour + 7, jour + 36));
        assert!(support_due(50, jour, jour + 7, jour + 37));
        // Date de première utilisation inconnue : jamais.
        assert!(!support_due(50, 0, 0, jour + 100));
    }
}
