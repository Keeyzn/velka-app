//! Aperçus des lives (image du direct) : téléchargés en arrière-plan, gardés en mémoire seulement
//! et renouvelés toutes les 5 minutes tant qu'ils sont affichés.
use eframe::egui;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(8);
const REFRESH: Duration = Duration::from_secs(300);

pub struct Thumbs {
    textures: HashMap<String, egui::TextureHandle>,
    requested: HashMap<String, Instant>,
    request: Sender<String>,
    done: Receiver<(String, egui::ColorImage)>,
    ctx: egui::Context,
}

/// Adresse de l'aperçu d'un live (CDN public de Twitch) ; `None` si le nom est invalide.
pub fn preview_url(login: &str) -> Option<String> {
    (crate::login::normalize(login).as_deref() == Some(login))
        .then(|| format!("https://static-cdn.jtvnw.net/previews-ttv/live_user_{login}-440x248.jpg"))
}

fn download(agent: &ureq::Agent, login: &str) -> Option<egui::ColorImage> {
    let url = preview_url(login)?;
    let mut resp = agent.get(&url).call().ok()?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    ))
}

impl Thumbs {
    pub fn new(ctx: &egui::Context) -> Self {
        let (request, req_rx) = channel::<String>();
        let (done_tx, done) = channel();
        let thread_ctx = ctx.clone();
        std::thread::spawn(move || {
            let agent = crate::net::agent_https(TIMEOUT);
            while let Ok(login) = req_rx.recv() {
                if let Some(img) = download(&agent, &login) {
                    if done_tx.send((login, img)).is_err() {
                        break;
                    }
                    thread_ctx.request_repaint();
                }
            }
        });
        Self {
            textures: HashMap::new(),
            requested: HashMap::new(),
            request,
            done,
            ctx: ctx.clone(),
        }
    }

    /// À appeler à chaque image : charge les aperçus arrivés.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((login, img)) = self.done.try_recv() {
            let tex = ctx.load_texture(format!("thumb-{login}"), img, egui::TextureOptions::LINEAR);
            self.textures.insert(login, tex);
        }
    }

    /// Texture de l'aperçu s'il est prêt (l'ancien reste affiché pendant le renouvellement).
    pub fn get(&mut self, login: &str) -> Option<egui::TextureId> {
        let stale = self
            .requested
            .get(login)
            .is_none_or(|t| t.elapsed() >= REFRESH);
        if stale && preview_url(login).is_some() {
            self.requested.insert(login.to_string(), Instant::now());
            let _ = self.request.send(login.to_string());
            self.ctx.request_repaint_after(REFRESH);
        }
        self.textures.get(login).map(|t| t.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_apercu() {
        assert_eq!(
            preview_url("xqc").as_deref(),
            Some("https://static-cdn.jtvnw.net/previews-ttv/live_user_xqc-440x248.jpg")
        );
        assert!(preview_url("../x").is_none());
        assert!(preview_url("Mauvais Nom").is_none());
    }
}
