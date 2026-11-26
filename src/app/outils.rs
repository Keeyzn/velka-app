//! Outils (Streamlink, libmpv) : installation, détection et contrôle des versions.
use super::*;

/// Une dernière version connue reste valable ce temps (évite d'interroger GitHub à chaque ouverture de « Avancé »).
const LATEST_FRESH_SECS: u64 = 300;

impl VelkaApp {
    /// Installe les composants qui manquent (écran de premier lancement).
    pub(super) fn start_install(&mut self, ctx: &egui::Context) {
        let todo: Vec<Tool> = self
            .rows
            .iter()
            .filter(|r| !matches!(r.status, ToolStatus::Ready(_)))
            .map(|r| r.tool)
            .collect();
        self.run_installs(ctx, todo);
    }

    /// Met à jour TOUS les composants qui en ont besoin (d'après la dernière vérification), l'un après l'autre.
    pub(super) fn start_tools_update(&mut self, ctx: &egui::Context) {
        if self.busy {
            return; // pas de mises à jour simultanées ni de double clic
        }
        let todo = tools::update_targets(&self.tools_check, self.player.is_some());
        self.tool_errors.clear();
        self.run_installs(ctx, todo);
    }

    fn run_installs(&mut self, ctx: &egui::Context, todo: Vec<Tool>) {
        if self.busy || todo.is_empty() {
            return;
        }
        self.busy = true;
        for r in &mut self.rows {
            if todo.contains(&r.tool) {
                r.status = ToolStatus::Installing(0.0, crate::i18n::tr("Préparation…").into());
            }
        }
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        if super::apercu_outils() {
            // Aperçu du design : progression simulée (environ 6 s par composant), aucun téléchargement, aucun fichier
            // écrit ; chaque composant se termine par un échec factice, l'écran reste donc affiché.
            std::thread::spawn(move || {
                for tool in todo {
                    for step in 0..=60u32 {
                        let f = step as f32 / 60.0;
                        let _ = tx.send(Msg::Progress(
                            tool,
                            f,
                            crate::i18n::tr("Préparation…").to_string(),
                        ));
                        ctx.request_repaint();
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    let _ = tx.send(Msg::Done(
                        tool,
                        Err("Aperçu : rien n'a été téléchargé".to_string()),
                    ));
                }
                let _ = tx.send(Msg::Finished);
                ctx.request_repaint();
            });
            return;
        }
        std::thread::spawn(move || {
            // Chaque composant est indépendant : l'échec de l'un n'empêche pas la mise à jour de l'autre, et garde
            // sa version précédente (voir `tools::install`).
            for tool in todo {
                let (txp, ctxp) = (tx.clone(), ctx.clone());
                let res = tools::install(tool, &move |f, m| {
                    let _ = txp.send(Msg::Progress(tool, f, m.to_string()));
                    ctxp.request_repaint();
                });
                if let Err(e) = &res {
                    crate::logging::log(&format!("Installation {tool:?} échouée : {e}"));
                }
                let _ = tx.send(Msg::Done(tool, res));
                ctx.request_repaint();
            }
            let _ = tx.send(Msg::Finished);
            ctx.request_repaint();
        });
    }

    /// Vérifie l'état des composants : version installée (relue sur le disque) et dernière version disponible.
    /// `force` : interroge GitHub même si une version récente est déjà connue.
    pub(super) fn start_tools_check(&mut self, ctx: &egui::Context, force: bool) {
        if self.busy || matches!(self.tools_check, tools::ToolsCheck::Checking) {
            return;
        }
        self.tools_check = tools::ToolsCheck::Checking;
        let fresh = self
            .tools_latest_at
            .is_some_and(|t| t.elapsed().as_secs() < LATEST_FRESH_SECS);
        let known = if force || !fresh {
            Vec::new()
        } else {
            self.tools_latest.clone()
        };
        let customs: Vec<(Tool, String)> = Tool::ALL
            .iter()
            .map(|&t| (t, custom_path(&self.settings, t).to_string()))
            .collect();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = tools::check_all(&customs, &known, &tools::latest_tag);
            if let Err(e) = &res {
                crate::logging::log(&format!("Vérification des composants échouée : {e}"));
            }
            let _ = tx.send(Msg::ToolsChecked(res));
            ctx.request_repaint();
        });
    }

    pub(super) fn on_tools_checked(
        &mut self,
        ctx: &egui::Context,
        res: Result<Vec<tools::ToolReport>, String>,
    ) {
        match res {
            Ok(reports) => {
                self.tools_latest = reports.iter().map(|r| (r.tool, r.latest.clone())).collect();
                self.tools_latest_at = Some(Instant::now());
                self.tools_check = tools::ToolsCheck::Done(reports);
                // Redémarrage demandé pour mettre à jour : libmpv n'est plus chargée, on met tout à jour maintenant.
                if std::mem::take(&mut self.tools_update_after_check) {
                    self.start_tools_update(ctx);
                }
            }
            Err(e) => {
                self.tools_update_after_check = false;
                self.tools_check = tools::ToolsCheck::Failed(e);
            }
        }
    }

    /// Bouton « Redémarrage nécessaire » : note la mise à jour à faire, ferme l'application et la relance ; la mise à
    /// jour se fait au démarrage suivant. En cas d'impossibilité, on le dit et on ne ferme rien.
    pub(super) fn restart_to_update_tools(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let res = tools::mark_update_pending().and_then(|()| crate::update::relaunch());
        match res {
            Ok(()) => {
                crate::logging::log("Redémarrage pour mettre à jour les composants.");
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => {
                let _ = tools::take_update_pending(); // pas de relance prévue : on retire la marque
                self.tool_errors.clear();
                self.tool_errors.push((
                    Tool::Libmpv,
                    format!("{} {e}", crate::i18n::tr("Installation impossible :")),
                ));
            }
        }
    }

    pub(super) fn redetect_tools(&mut self) {
        for r in &mut self.rows {
            if matches!(r.status, ToolStatus::Installing(..)) {
                continue;
            }
            r.status = match tools::detect(r.tool, custom_path(&self.settings, r.tool)) {
                Some(p) => ToolStatus::Ready(p.display().to_string()),
                None => ToolStatus::Missing,
            };
        }
    }

    pub(super) fn set_tool(&mut self, tool: Tool, status: ToolStatus) {
        if let Some(r) = self.rows.iter_mut().find(|r| r.tool == tool) {
            r.status = status;
        }
    }
}
