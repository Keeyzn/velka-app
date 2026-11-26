//! Photos de profil des chaînes : téléchargement en arrière-plan, cache disque (7 jours) et textures.
//! Les adresses viennent de `twitch.rs` ; une photo n'est demandée qu'une fois par session.
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, SystemTime};

const SIZE: u32 = 64;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(8);
const VALIDITY: Duration = Duration::from_secs(7 * 24 * 3600);

pub struct Avatars {
    textures: HashMap<String, egui::TextureHandle>,
    requested: HashSet<String>,
    pending: Vec<String>,
    request: Sender<Vec<String>>,
    done: Receiver<(String, egui::ColorImage)>,
    ctx: egui::Context,
}

fn cache_dir() -> PathBuf {
    crate::logging::data_dir().join("cache").join("avatars")
}

fn cache_path(login: &str) -> PathBuf {
    cache_dir().join(format!("{login}.png"))
}

/// Décode une photo (PNG ou JPEG) et la réduit à 64×64.
fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?;
    Some(
        img.resize_to_fill(SIZE, SIZE, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    )
}

fn to_color_image(img: &image::RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    )
}

fn read_cache(login: &str) -> Option<egui::ColorImage> {
    let bytes = fs::read(cache_path(login)).ok()?;
    decode(&bytes).map(|i| to_color_image(&i))
}

fn cache_is_fresh(login: &str) -> bool {
    fs::metadata(cache_path(login))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < VALIDITY)
}

fn download(agent: &ureq::Agent, url: &str) -> Option<image::RgbaImage> {
    if !url.starts_with("https://") {
        return None;
    }
    let mut resp = agent.get(url).call().ok()?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .ok()?;
    decode(&bytes)
}

fn worker(rx: Receiver<Vec<String>>, tx: Sender<(String, egui::ColorImage)>, ctx: egui::Context) {
    let agent = crate::net::agent_https(TIMEOUT);
    let send = |login: &str, img: egui::ColorImage| {
        let ok = tx.send((login.to_string(), img)).is_ok();
        ctx.request_repaint();
        ok
    };
    while let Ok(batch) = rx.recv() {
        let mut stale: Vec<String> = Vec::new();
        for login in batch {
            // Photo en cache : affichée tout de suite, même périmée (on la garde si le réseau échoue).
            if let Some(img) = read_cache(&login) {
                if !send(&login, img) {
                    return;
                }
            }
            if !cache_is_fresh(&login) {
                stale.push(login);
            }
        }
        if stale.is_empty() {
            continue;
        }
        let urls = crate::twitch::fetch_avatar_urls(&stale);
        for login in &stale {
            let Some(img) = urls.get(login).and_then(|u| download(&agent, u)) else {
                continue;
            };
            let _ = fs::create_dir_all(cache_dir());
            if let Err(e) = img.save_with_format(cache_path(login), image::ImageFormat::Png) {
                crate::logging::log(&format!("Photo de {login} non enregistrée : {e}"));
            }
            if !send(login, to_color_image(&img)) {
                return;
            }
        }
    }
}

impl Avatars {
    pub fn new(ctx: &egui::Context) -> Self {
        let (request, req_rx) = channel::<Vec<String>>();
        let (done_tx, done) = channel();
        let thread_ctx = ctx.clone();
        std::thread::spawn(move || worker(req_rx, done_tx, thread_ctx));
        Self {
            textures: HashMap::new(),
            requested: HashSet::new(),
            pending: Vec::new(),
            request,
            done,
            ctx: ctx.clone(),
        }
    }

    /// Demande la photo d'une chaîne (une seule fois par session).
    pub fn request(&mut self, login: &str) {
        if crate::login::normalize(login).as_deref() == Some(login)
            && self.requested.insert(login.to_string())
        {
            self.pending.push(login.to_string());
            self.ctx.request_repaint();
        }
    }

    /// À appeler à chaque image : envoie les demandes groupées, charge les photos arrivées.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if !self.pending.is_empty() {
            let _ = self.request.send(std::mem::take(&mut self.pending));
        }
        while let Ok((login, img)) = self.done.try_recv() {
            let tex =
                ctx.load_texture(format!("avatar-{login}"), img, egui::TextureOptions::LINEAR);
            self.textures.insert(login, tex);
        }
    }

    /// Texture de la photo si elle est prête ; sinon la demande et renvoie `None` (lettre de la chaîne).
    pub fn get(&mut self, login: &str) -> Option<egui::TextureId> {
        self.request(login);
        self.textures.get(login).map(|t| t.id())
    }
}
