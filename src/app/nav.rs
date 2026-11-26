//! Navigation entre les pages (Précédent/Suivant) et curseur « main ».
use super::*;

impl VelkaApp {
    pub(super) fn nav_point(&self) -> Option<NavPoint> {
        match self.view {
            View::FirstRun => None,
            View::Home => Some(NavPoint::Home),
            View::Multi => Some(NavPoint::Multi),
            View::Settings => Some(NavPoint::Settings),
            View::Watch => self
                .watch
                .as_ref()
                .map(|w| NavPoint::Watch(w.login.clone())),
        }
    }

    pub(super) fn nav_goto(&mut self, ctx: &egui::Context, target: NavPoint) {
        self.nav_cur = target.clone();
        let view = match target {
            NavPoint::Home => View::Home,
            NavPoint::Multi => View::Multi,
            NavPoint::Settings => View::Settings,
            NavPoint::Watch(login) => return self.start_watch(ctx, login),
        };
        if self.view == View::Watch && self.watch.is_some() {
            self.minimize_watch(ctx, view);
        } else {
            self.view = view;
        }
    }

    /// Enregistre chaque changement de page, et réagit aux boutons Précédent / Suivant de la souris
    /// (ou Alt + flèche gauche / droite), comme dans un navigateur.
    pub(super) fn tick_nav(&mut self, ctx: &egui::Context) {
        if let Some(now) = self.nav_point() {
            if now != self.nav_cur {
                let old = std::mem::replace(&mut self.nav_cur, now);
                self.nav_back.push(old);
                if self.nav_back.len() > NAV_MAX {
                    self.nav_back.remove(0);
                }
                self.nav_fwd.clear();
            }
        }
        if self.fullscreen || self.pip || self.view == View::FirstRun {
            return;
        }
        let (back, forward) = if !self.settings.mouse_nav {
            (false, false)
        } else {
            ctx.input(|i| {
                let alt = i.modifiers.alt;
                (
                    i.pointer.button_pressed(egui::PointerButton::Extra1)
                        || (alt && i.key_pressed(egui::Key::ArrowLeft)),
                    i.pointer.button_pressed(egui::PointerButton::Extra2)
                        || (alt && i.key_pressed(egui::Key::ArrowRight)),
                )
            })
        };
        if back {
            if let Some(target) = self.nav_back.pop() {
                self.nav_fwd.push(self.nav_cur.clone());
                self.nav_goto(ctx, target);
            }
        } else if forward {
            if let Some(target) = self.nav_fwd.pop() {
                self.nav_back.push(self.nav_cur.clone());
                self.nav_goto(ctx, target);
            }
        }
    }

    /// Main pointée sur tout élément cliquable (sauf la vidéo elle-même et les champs de saisie,
    /// qui gardent leur curseur).
    pub(super) fn update_cursor(&self, ctx: &egui::Context) {
        if !self.settings.hand_cursor {
            return;
        }
        if ctx.output(|o| o.cursor_icon) != egui::CursorIcon::Default {
            return;
        }
        let video = self.last_video_rect.shrink(2.0);
        let hovered = ctx.interaction_snapshot(|s| s.hovered.clone());
        let clickable = ctx.viewport(|vp| {
            hovered.iter().any(|id| {
                vp.this_pass.widgets.get(*id).is_some_and(|w| {
                    w.sense.senses_click()
                        && !(self.view == View::Watch
                            && video.is_positive()
                            && w.rect.contains_rect(video))
                })
            })
        });
        if clickable {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}
