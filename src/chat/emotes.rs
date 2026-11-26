//! Emotes (Twitch, 7TV, BTTV, FFZ) et badges du chat : téléchargement en arrière-plan, cache disque et
//! mémoire, animation des GIF (option « Emotes animées »).
use eframe::egui;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Nombre d'images gardées au plus pour une emote animée (les autres sont fusionnées).
const MAX_FRAMES: usize = 60;
/// Images lues au plus dans un GIF.
const MAX_DECODED: usize = 240;

/// Images décodées et durée de chacune (ms ; 0 = image fixe).
type Frames = Vec<(egui::ColorImage, u32)>;

/// Mémoire des textures : au plus `MAX_EMOTES` emotes et `MAX_TEXTURE_BYTES` d'images en mémoire vidéo ;
/// au-delà, les emotes les moins récemment affichées sont libérées (rechargées depuis le cache disque si elles
/// reviennent dans le chat). Une emote affichée à l'image précédente n'est jamais libérée.
const MAX_EMOTES: usize = 400;
const MAX_TEXTURE_BYTES: u64 = 128 * 1024 * 1024;

struct Anim {
    /// Texture et instant de fin (ms, cumulé) de chaque image.
    frames: Vec<(egui::TextureHandle, u32)>,
    total: u32,
    /// Mémoire occupée par toutes les images (octets).
    bytes: u64,
    /// Dernier usage (valeur de `Emotes::clock`).
    used: u64,
}

enum Slot {
    Loading,
    Ready(Anim),
    Failed,
}

pub struct Emotes {
    slots: HashMap<String, Slot>,
    request: Sender<(String, bool)>,
    done: Receiver<(String, bool, Option<Frames>)>,
    animate: bool,
    /// Compteur d'usages : chaque emote affichée ou ajoutée y prend un numéro, le plus petit est le plus ancien.
    clock: u64,
    /// Valeur de `clock` au début de l'image précédente : ce qui a servi depuis est protégé de l'éviction.
    floor: u64,
}

fn cache_dir() -> PathBuf {
    crate::logging::data_dir().join("cache").join("emotes")
}

/// Une clé n'est faite que de caractères sûrs (pas de chemin ni d'URL arbitraire).
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Adresse de l'image et nom du fichier de cache pour une clé (voir `chat::extra`) ; sans préfixe = emote Twitch.
fn source(key: &str, animate: bool) -> (String, String) {
    let (kind, id) = match key.split_once('_') {
        Some((k, id)) if matches!(k, "bttv" | "bttva" | "ffz" | "7tv" | "7tva" | "badge") => {
            (k, id)
        }
        _ => ("", key),
    };
    match kind {
        "bttv" | "bttva" => (
            format!("https://cdn.betterttv.net/emote/{id}/2x"),
            format!("{key}.img"),
        ),
        "ffz" => (
            format!("https://cdn.betterttv.net/frankerfacez_emote/{id}/2"),
            format!("{key}.img"),
        ),
        // Les emotes 7TV animées n'existent pas en PNG : sans animation, seule la première image est gardée.
        "7tva" => (
            format!("https://cdn.7tv.app/emote/{id}/2x.gif"),
            format!("{key}.gif"),
        ),
        "7tv" => (
            format!("https://cdn.7tv.app/emote/{id}/2x.png"),
            format!("{key}.png"),
        ),
        "badge" => (
            format!("https://static-cdn.jtvnw.net/badges/v1/{id}/2"),
            format!("{key}.png"),
        ),
        // « default » renvoie le GIF si l'emote Twitch est animée, sinon l'image fixe.
        _ if animate => (
            format!("https://static-cdn.jtvnw.net/emoticons/v2/{key}/default/dark/2.0"),
            format!("{key}.anim"),
        ),
        _ => (
            format!("https://static-cdn.jtvnw.net/emoticons/v2/{key}/static/dark/2.0"),
            format!("{key}.png"),
        ),
    }
}

/// Limites de décodage : les emotes 7TV/BTTV/FFZ sont téléversées par n'importe qui. Une image géante
/// ou une « bombe » de décompression ne doit pas faire exploser la mémoire de tous les spectateurs d'une chaîne.
const MAX_SIDE: u32 = 512;
const MAX_ALLOC: u64 = 32 * 1024 * 1024;
/// Mémoire totale des images décodées d'un GIF animé.
const MAX_GIF_BYTES: u64 = 64 * 1024 * 1024;
/// Taille maximale d'un fichier d'emote téléchargé.
const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024;

fn limits() -> image::Limits {
    let mut l = image::Limits::default();
    l.max_image_width = Some(MAX_SIDE);
    l.max_image_height = Some(MAX_SIDE);
    l.max_alloc = Some(MAX_ALLOC);
    l
}

fn still(bytes: &[u8]) -> Option<Frames> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits());
    let img = reader.decode().ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(vec![(
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        0,
    )])
}

/// Décode une image ; un GIF animé donne plusieurs images si `animate`.
fn decode(bytes: &[u8], animate: bool) -> Option<Frames> {
    if !(animate && bytes.starts_with(b"GIF")) {
        return still(bytes);
    }
    use image::AnimationDecoder;
    use image::ImageDecoder;
    let Ok(mut dec) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)) else {
        return still(bytes);
    };
    let (w, h) = dec.dimensions();
    if w > MAX_SIDE || h > MAX_SIDE || dec.set_limits(limits()).is_err() {
        return None;
    }
    let per_frame = u64::from(w) * u64::from(h) * 4;
    let max_frames = (MAX_GIF_BYTES / per_frame.max(1)).max(1) as usize;
    let mut frames: Frames = Vec::new();
    for f in dec.into_frames().take(MAX_DECODED.min(max_frames)) {
        let Ok(f) = f else { break };
        let (n, d) = f.delay().numer_denom_ms();
        let ms = n.checked_div(d).unwrap_or(0);
        // Comme les navigateurs : une durée trop courte vaut 100 ms.
        let ms = if ms < 20 { 100 } else { ms };
        let buf = f.into_buffer();
        let size = [buf.width() as usize, buf.height() as usize];
        frames.push((
            egui::ColorImage::from_rgba_unmultiplied(size, buf.as_raw()),
            ms,
        ));
    }
    if frames.len() <= 1 {
        return still(bytes);
    }
    if frames.len() > MAX_FRAMES {
        let step = frames.len().div_ceil(MAX_FRAMES);
        let mut merged: Frames = Vec::new();
        for (i, (img, ms)) in frames.into_iter().enumerate() {
            if i % step == 0 {
                merged.push((img, ms));
            } else if let Some(last) = merged.last_mut() {
                last.1 += ms;
            }
        }
        frames = merged;
    }
    Some(frames)
}

/// Emotes à libérer parmi `entries` (clé, dernier usage, octets) : les plus anciennes d'abord, jusqu'à repasser sous
/// les deux plafonds. Tout ce qui a servi après `floor` reste, même si les plafonds sont dépassés.
fn eviction(
    mut entries: Vec<(&str, u64, u64)>,
    max_count: usize,
    max_bytes: u64,
    floor: u64,
) -> Vec<String> {
    let mut count = entries.len();
    let mut bytes: u64 = entries.iter().map(|e| e.2).sum();
    entries.sort_by_key(|e| e.1);
    let mut out = Vec::new();
    for (key, used, size) in entries {
        if (count <= max_count && bytes <= max_bytes) || used > floor {
            break;
        }
        out.push(key.to_string());
        count -= 1;
        bytes = bytes.saturating_sub(size);
    }
    out
}

fn fetch(agent: &ureq::Agent, key: &str, animate: bool) -> Option<Frames> {
    let (url, file) = source(key, animate);
    let path = cache_dir().join(file);
    if let Ok(bytes) = fs::read(&path) {
        if let Some(img) = decode(&bytes, animate) {
            return Some(img);
        }
    }
    let mut resp = agent.get(&url).call().ok()?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .read_to_vec()
        .ok()?;
    let img = decode(&bytes, animate)?;
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(&path, &bytes);
    Some(img)
}

impl Emotes {
    pub fn new(ctx: &egui::Context) -> Self {
        let (request, req_rx) = channel::<(String, bool)>();
        let (done_tx, done) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let agent = crate::net::agent(std::time::Duration::from_secs(10));
            while let Ok((id, animate)) = req_rx.recv() {
                let img = fetch(&agent, &id, animate);
                if done_tx.send((id, animate, img)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            slots: HashMap::new(),
            request,
            done,
            animate: false,
            clock: 0,
            floor: 0,
        }
    }

    /// Active ou coupe l'animation (option « Emotes animées ») ; les images sont alors rechargées.
    pub fn set_animate(&mut self, on: bool) {
        if on != self.animate {
            self.animate = on;
            self.slots.clear();
        }
    }

    /// À appeler à chaque image : transforme les téléchargements terminés en textures.
    pub fn poll(&mut self, ctx: &egui::Context) {
        // Ce qui a servi depuis le début de l'image précédente (et ce qui arrive maintenant) n'est pas libérable.
        let floor = std::mem::replace(&mut self.floor, self.clock);
        let mut added = false;
        while let Ok((id, animate, img)) = self.done.try_recv() {
            if animate != self.animate {
                self.slots.remove(&id); // demandé avant un changement d'option : redemandé au besoin
                continue;
            }
            let slot = match img {
                Some(frames) => {
                    let (mut end, mut bytes) = (0, 0u64);
                    let frames: Vec<(egui::TextureHandle, u32)> = frames
                        .into_iter()
                        .enumerate()
                        .map(|(i, (img, ms))| {
                            end += ms;
                            bytes += (img.size[0] * img.size[1] * 4) as u64;
                            (
                                ctx.load_texture(
                                    format!("emote-{id}-{i}"),
                                    img,
                                    egui::TextureOptions::LINEAR,
                                ),
                                end,
                            )
                        })
                        .collect();
                    self.clock += 1;
                    added = true;
                    Slot::Ready(Anim {
                        frames,
                        total: end,
                        bytes,
                        used: self.clock,
                    })
                }
                None => Slot::Failed,
            };
            self.slots.insert(id, slot);
        }
        if added {
            self.evict(floor);
        }
    }

    /// Libère les emotes les moins récemment affichées si les plafonds sont dépassés.
    fn evict(&mut self, floor: u64) {
        let ready: Vec<(&str, u64, u64)> = self
            .slots
            .iter()
            .filter_map(|(k, s)| match s {
                Slot::Ready(a) => Some((k.as_str(), a.used, a.bytes)),
                _ => None,
            })
            .collect();
        for key in eviction(ready, MAX_EMOTES, MAX_TEXTURE_BYTES, floor) {
            self.slots.remove(&key); // les textures sont libérées avec l'emote ; redemandée si elle revient dans le chat
        }
    }

    /// Texture à afficher maintenant (`time` en secondes) et `true` si l'image est animée ;
    /// sinon la demande (une seule fois).
    pub fn get(&mut self, id: &str, time: f64) -> Option<(egui::TextureId, bool)> {
        if !self.slots.contains_key(id) {
            if !safe_id(id) {
                self.slots.insert(id.to_string(), Slot::Failed);
            } else {
                self.slots.insert(id.to_string(), Slot::Loading);
                let _ = self.request.send((id.to_string(), self.animate));
            }
        }
        self.clock += 1;
        let clock = self.clock;
        let Some(Slot::Ready(anim)) = self.slots.get_mut(id) else {
            return None;
        };
        anim.used = clock;
        if anim.total == 0 || anim.frames.len() < 2 {
            return anim.frames.first().map(|(t, _)| (t.id(), false));
        }
        let t = ((time * 1000.0) as u64 % u64::from(anim.total)) as u32;
        let frame = anim
            .frames
            .iter()
            .find(|(_, end)| t < *end)
            .or(anim.frames.last());
        frame.map(|(tex, _)| (tex.id(), true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(w, h)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// une image trop grande est refusée au décodage (pas d'allocation géante).
    #[test]
    fn image_geante_refusee() {
        assert!(decode(&png(64, 64), false).is_some());
        assert!(decode(&png(64, 64), true).is_some());
        assert!(decode(&png(4000, 4000), false).is_none());
        assert!(decode(&png(MAX_SIDE + 1, 8), true).is_none());
        assert!(decode(b"GIF89a pas une image", true).is_none());
    }

    /// les plus anciennes sont libérées d'abord, jusqu'à repasser sous les deux plafonds ; ce qui a servi
    /// depuis `floor` reste, même au-delà des plafonds.
    #[test]
    fn eviction_des_plus_anciennes() {
        // (clé, dernier usage, octets) dans le désordre : l'ordre d'insertion ne compte pas.
        let e = vec![
            ("c", 30, 10),
            ("a", 10, 10),
            ("e", 50, 10),
            ("b", 20, 10),
            ("d", 40, 10),
        ];
        assert!(
            eviction(e.clone(), 5, 1000, 100).is_empty(),
            "sous les plafonds : rien à libérer"
        );
        assert_eq!(
            eviction(e.clone(), 3, 1000, 100),
            ["a", "b"],
            "trop d'emotes"
        );
        assert_eq!(
            eviction(e.clone(), 100, 30, 100),
            ["a", "b"],
            "trop d'octets (50 > 30 : deux libérées suffisent)"
        );
        assert_eq!(
            eviction(e.clone(), 3, 1000, 15),
            ["a"],
            "b, c, d, e ont servi après le seuil : gardées"
        );
        assert!(
            eviction(e, 0, 0, 0).is_empty(),
            "tout a servi après le seuil : rien n'est libéré"
        );
    }

    fn emote_prete(id: &str) -> (String, bool, Option<Frames>) {
        (
            id.to_string(),
            false,
            Some(vec![(
                egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                0,
            )]),
        )
    }

    /// De bout en bout (sans réseau ni fenêtre) : au-delà de `MAX_EMOTES`, les emotes qui ne servent plus
    /// sont libérées, celles qui sont affichées restent, et une libérée est redemandée si elle revient.
    #[test]
    fn emotes_liberees_au_dela_du_plafond() {
        let ctx = egui::Context::default();
        let (request, req_rx) = channel::<(String, bool)>();
        let (done_tx, done) = channel();
        let mut e = Emotes {
            slots: HashMap::new(),
            request,
            done,
            animate: false,
            clock: 0,
            floor: 0,
        };
        let ready = |e: &Emotes| {
            e.slots
                .values()
                .filter(|s| matches!(s, Slot::Ready(_)))
                .count()
        };

        for i in 0..MAX_EMOTES + 10 {
            done_tx.send(emote_prete(&format!("e{i}"))).unwrap();
        }
        e.poll(&ctx);
        assert_eq!(
            ready(&e),
            MAX_EMOTES + 10,
            "tout vient d'arriver : rien n'est libéré"
        );

        // Les emotes e200 à e409 restent affichées ; chaque image apporte une nouvelle emote.
        for round in 1..=2 {
            for i in 200..MAX_EMOTES + 10 {
                assert!(e.get(&format!("e{i}"), 0.0).is_some());
            }
            done_tx.send(emote_prete(&format!("n{round}"))).unwrap();
            e.poll(&ctx);
        }
        assert_eq!(ready(&e), MAX_EMOTES, "ramené au plafond");
        assert!(
            !e.slots.contains_key("e0")
                && !e.slots.contains_key("e11")
                && e.slots.contains_key("e12"),
            "les plus anciennes d'abord, seulement ce qu'il faut"
        );
        assert!(
            (200..MAX_EMOTES + 10).all(|i| e.slots.contains_key(&format!("e{i}"))),
            "les emotes affichées restent"
        );
        assert!(e.slots.contains_key("n2"));

        // Une emote libérée qui revient dans le chat est redemandée (cache disque).
        while req_rx.try_recv().is_ok() {}
        assert!(e.get("e0", 0.0).is_none());
        assert_eq!(req_rx.try_recv().unwrap(), ("e0".to_string(), false));
    }

    #[test]
    fn identifiants() {
        assert!(safe_id("25"));
        assert!(safe_id("emotesv2_abc123"));
        assert!(safe_id("badge_e804d310-f35d"));
        assert!(!safe_id("../x"));
        assert!(!safe_id("a/b"));
        assert!(!safe_id(""));
    }

    #[test]
    fn adresses() {
        // Sans l'option d'animation, une emote Twitch garde l'adresse et le cache d'origine.
        assert_eq!(
            source("25", false),
            (
                "https://static-cdn.jtvnw.net/emoticons/v2/25/static/dark/2.0".to_string(),
                "25.png".to_string()
            )
        );
        assert!(source("emotesv2_ab", true)
            .0
            .ends_with("/emotesv2_ab/default/dark/2.0"));
        assert_eq!(
            source("bttv_5e41", false).0,
            "https://cdn.betterttv.net/emote/5e41/2x"
        );
        assert_eq!(
            source("ffz_9", false).0,
            "https://cdn.betterttv.net/frankerfacez_emote/9/2"
        );
        assert!(source("7tva_01G", true).0.ends_with("/01G/2x.gif"));
        assert!(source("7tva_01G", false).0.ends_with("/01G/2x.gif"));
        assert!(source("7tv_01G", false).0.ends_with("/01G/2x.png"));
        assert_eq!(
            source("badge_e8-04", false).0,
            "https://static-cdn.jtvnw.net/badges/v1/e8-04/2"
        );
    }
}
