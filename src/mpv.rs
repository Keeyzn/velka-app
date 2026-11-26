//! Bindings minimaux de libmpv (chargée dynamiquement) et rendu OpenGL dans egui.
//!
//! Principe : mpv dessine dans un FBO propre à la taille de la zone vidéo, puis ce FBO est
//! copié (blit) dans la zone prévue de la fenêtre pendant le `PaintCallback` d'egui. Les
//! contrôles egui peuvent donc être superposés à la vidéo.
use eframe::egui;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Handle = *mut c_void;

/// Fonction de chargement des symboles OpenGL fournie par eframe.
pub type GetProc = Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>;

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct OpenGlInitParams {
    get_proc_address: extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
struct OpenGlFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

/// `MPV_ERROR_OPTION_NOT_FOUND` (client.h) : option inconnue de cette libmpv.
const MPV_ERROR_OPTION_NOT_FOUND: c_int = -5;

const PARAM_API_TYPE: c_int = 1;
const PARAM_OPENGL_INIT_PARAMS: c_int = 2;
const PARAM_OPENGL_FBO: c_int = 3;
const PARAM_FLIP_Y: c_int = 4;

/// Fonctions de libmpv ; la bibliothèque reste chargée tant que cette structure existe.
struct Lib {
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char,
    free: unsafe extern "C" fn(*mut c_void),
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    terminate_destroy: unsafe extern "C" fn(Handle),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    rc_create: unsafe extern "C" fn(*mut Handle, Handle, *mut RenderParam) -> c_int,
    rc_render: unsafe extern "C" fn(Handle, *mut RenderParam) -> c_int,
    rc_update: unsafe extern "C" fn(Handle) -> u64,
    rc_set_update_callback:
        unsafe extern "C" fn(Handle, Option<extern "C" fn(*mut c_void)>, *mut c_void),
    rc_free: unsafe extern "C" fn(Handle),
    _lib: libloading::Library,
}

impl Lib {
    fn load(path: &Path) -> Result<Self, String> {
        // SAFETY : chargement d'une DLL choisie par l'utilisateur ou installée par l'application.
        unsafe {
            let lib = libloading::Library::new(path).map_err(|e| {
                format!("{} {e}", crate::i18n::tr("Impossible de charger libmpv :"))
            })?;
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| {
                        format!(
                            "{} {} ({e})",
                            crate::i18n::tr("Fonction libmpv manquante :"),
                            $name
                        )
                    })?
                };
            }
            Ok(Self {
                create: sym!("mpv_create"),
                initialize: sym!("mpv_initialize"),
                set_option_string: sym!("mpv_set_option_string"),
                set_property_string: sym!("mpv_set_property_string"),
                get_property_string: sym!("mpv_get_property_string"),
                free: sym!("mpv_free"),
                command: sym!("mpv_command"),
                terminate_destroy: sym!("mpv_terminate_destroy"),
                error_string: sym!("mpv_error_string"),
                rc_create: sym!("mpv_render_context_create"),
                rc_render: sym!("mpv_render_context_render"),
                rc_update: sym!("mpv_render_context_update"),
                rc_set_update_callback: sym!("mpv_render_context_set_update_callback"),
                rc_free: sym!("mpv_render_context_free"),
                _lib: lib,
            })
        }
    }

    fn err(&self, code: c_int) -> String {
        // SAFETY : mpv_error_string renvoie une chaîne statique.
        unsafe {
            let p = (self.error_string)(code);
            if p.is_null() {
                format!("erreur {code}")
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }
}

/// Ressources OpenGL de la zone vidéo (FBO + texture), créées à la demande.
#[derive(Default)]
struct GlRes {
    fbo: Option<glow::Framebuffer>,
    tex: Option<glow::Texture>,
    size: (i32, i32),
    /// Dernière taille rendue à l'exacte dimension de la zone (référence pendant un redimensionnement).
    rendered: Option<(i32, i32)>,
    /// Vrai une fois le lecteur détruit : un rendu déjà programmé par egui ne doit plus toucher au contexte libéré.
    closed: bool,
}

/// FBO et texture d'un lecteur détruit, à supprimer côté OpenGL.
type Leftover = (Option<glow::Framebuffer>, Option<glow::Texture>);

/// Ressources laissées par les lecteurs détruits : `Player::drop` n'a pas le contexte OpenGL sous la
/// main, il les dépose ici et `free_dropped` les supprime à l'image suivante.
static DROPPED: Mutex<Vec<Leftover>> = Mutex::new(Vec::new());

/// Retire de `res` le FBO et la texture, et les met en file pour suppression.
fn queue_for_release(res: &mut GlRes, queue: &Mutex<Vec<Leftover>>) {
    let left: Leftover = (res.fbo.take(), res.tex.take());
    if left.0.is_some() || left.1.is_some() {
        if let Ok(mut q) = queue.lock() {
            q.push(left);
        }
    }
}

/// Vide la file et rend son contenu.
fn take_queue(queue: &Mutex<Vec<Leftover>>) -> Vec<Leftover> {
    queue
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// Supprime les FBO et textures des lecteurs détruits. À appeler à chaque image, contexte OpenGL courant
/// (début de `ui`) ; sans effet quand rien n'attend.
pub fn free_dropped(gl: &glow::Context) {
    for (fbo, tex) in take_queue(&DROPPED) {
        // SAFETY : contexte OpenGL courant (voir ci-dessus) ; ces objets n'appartiennent plus à aucun lecteur.
        unsafe {
            if let Some(f) = fbo {
                gl.delete_framebuffer(f);
            }
            if let Some(t) = tex {
                gl.delete_texture(t);
            }
        }
    }
}

struct Shared {
    lib: Arc<Lib>,
    rc: Handle,
    gl: Mutex<GlRes>,
}

// SAFETY : les pointeurs mpv ne sont utilisés que depuis le fil d'interface (rendu) ;
// mpv gère lui-même ses accès concurrents.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

pub struct Player {
    lib: Arc<Lib>,
    handle: Handle,
    shared: Arc<Shared>,
    update_ctx: *mut egui::Context,
    /// Dernière taille de la zone vidéo (pixels) et moment de son dernier changement.
    track: Mutex<((i32, i32), Instant)>,
    /// Numéro de la dernière image d'interface où la vidéo a été dessinée (voir `keep_live`).
    last_paint: AtomicU64,
    /// Fin de la destruction (arrêt du cœur de mpv) dans un fil à part : voir `destroy_in_background`.
    destroy_async: bool,
}

extern "C" fn on_update(ctx: *mut c_void) {
    // SAFETY : `ctx` est le Box<egui::Context> créé dans `Player::new`, libéré après rc_free.
    let ctx = unsafe { &*(ctx as *const egui::Context) };
    ctx.request_repaint();
}

extern "C" fn get_proc(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY : `ctx` pointe vers la fonction de chargement d'eframe pendant la création.
    unsafe {
        let f = &*(ctx as *const &dyn Fn(&CStr) -> *const c_void);
        f(CStr::from_ptr(name)) as *mut c_void
    }
}

/// Options de libmpv posées avant l'initialisation, dans cet ordre.
pub const MPV_OPTIONS: &[(&str, &str)] = &[
    ("vo", "libmpv"),
    ("hwdec", "auto-safe"),
    ("keep-open", "no"),
    ("osc", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("terminal", "no"),
    ("idle", "yes"),
    ("msg-level", "all=warn"),
];

/// `MPV_OPTIONS` sous la forme attendue par `Player::new`.
pub fn default_options() -> Vec<(String, String)> {
    MPV_OPTIONS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl Player {
    /// À appeler sur le fil d'interface, contexte OpenGL courant (création de l'application).
    pub fn new(
        get_addr: GetProc,
        egui_ctx: &egui::Context,
        dll: &Path,
        log_file: Option<&Path>,
        opts: &[(String, String)],
    ) -> Result<Self, String> {
        let lib = Arc::new(Lib::load(dll)?);
        // SAFETY : appels C documentés de l'API client de libmpv.
        unsafe {
            let handle = (lib.create)();
            if handle.is_null() {
                return Err(crate::i18n::tr("Le lecteur n'a pas pu démarrer.").into());
            }
            let mut p = Self {
                lib: lib.clone(),
                handle,
                shared: Arc::new(Shared {
                    lib: lib.clone(),
                    rc: ptr::null_mut(),
                    gl: Mutex::default(),
                }),
                update_ctx: ptr::null_mut(),
                track: Mutex::new(((0, 0), Instant::now())),
                last_paint: AtomicU64::new(0),
                destroy_async: false,
            };
            for (k, v) in opts {
                // Option absente de cette libmpv (ex. `osc`, script Lua, sur la libmpv Mac compilée sans Lua) : notée
                // dans le journal et ignorée ; toute autre erreur (valeur refusée) bloque toujours le démarrage.
                match p.set_option_code(k, v)? {
                    0.. => {}
                    MPV_ERROR_OPTION_NOT_FOUND => crate::logging::log(&format!(
                        "Option du lecteur absente de cette libmpv, ignorée : {k}"
                    )),
                    code => {
                        return Err(format!(
                            "{} {k} : {}",
                            crate::i18n::tr("Option du lecteur refusée :"),
                            lib.err(code)
                        ))
                    }
                }
            }
            // Journal détaillé de mpv : seulement en mode diagnostic (`--diag`) ; sans lui, mpv n'écrit rien.
            if let Some(f) = log_file {
                let _ = p.set_option("log-file", &f.to_string_lossy());
            }
            let code = (lib.initialize)(handle);
            if code < 0 {
                return Err(format!(
                    "{} {}",
                    crate::i18n::tr("Initialisation du lecteur impossible :"),
                    lib.err(code)
                ));
            }

            let api = CString::new("opengl").unwrap_or_default();
            let getter: &dyn Fn(&CStr) -> *const c_void = &*get_addr;
            let mut init = OpenGlInitParams {
                get_proc_address: get_proc,
                get_proc_address_ctx: &getter as *const _ as *mut c_void,
            };
            let mut params = [
                RenderParam {
                    kind: PARAM_API_TYPE,
                    data: api.as_ptr() as *mut c_void,
                },
                RenderParam {
                    kind: PARAM_OPENGL_INIT_PARAMS,
                    data: &mut init as *mut _ as *mut c_void,
                },
                RenderParam {
                    kind: 0,
                    data: ptr::null_mut(),
                },
            ];
            let mut rc: Handle = ptr::null_mut();
            let code = (lib.rc_create)(&mut rc, handle, params.as_mut_ptr());
            if code < 0 {
                return Err(format!(
                    "{} {}",
                    crate::i18n::tr("Rendu vidéo impossible :"),
                    lib.err(code)
                ));
            }
            p.shared = Arc::new(Shared {
                lib: lib.clone(),
                rc,
                gl: Mutex::default(),
            });
            let ctx = Box::into_raw(Box::new(egui_ctx.clone()));
            p.update_ctx = ctx;
            (lib.rc_set_update_callback)(rc, Some(on_update), ctx as *mut c_void);
            Ok(p)
        }
    }

    /// Code renvoyé par libmpv pour une option (0 ou plus : acceptée).
    fn set_option_code(&self, name: &str, value: &str) -> Result<c_int, String> {
        let (n, v) = (cstr(name)?, cstr(value)?);
        // SAFETY : handle valide tant que `self` existe.
        Ok(unsafe { (self.lib.set_option_string)(self.handle, n.as_ptr(), v.as_ptr()) })
    }

    pub fn set_option(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (cstr(name)?, cstr(value)?);
        // SAFETY : handle valide tant que `self` existe.
        let code = unsafe { (self.lib.set_option_string)(self.handle, n.as_ptr(), v.as_ptr()) };
        if code < 0 {
            Err(format!(
                "{} {name} : {}",
                crate::i18n::tr("Option du lecteur refusée :"),
                self.lib.err(code)
            ))
        } else {
            Ok(())
        }
    }

    pub fn set_property(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (cstr(name)?, cstr(value)?);
        // SAFETY : voir `set_option`.
        let code = unsafe { (self.lib.set_property_string)(self.handle, n.as_ptr(), v.as_ptr()) };
        if code < 0 {
            Err(format!(
                "{} {name} : {}",
                crate::i18n::tr("Propriété du lecteur refusée :"),
                self.lib.err(code)
            ))
        } else {
            Ok(())
        }
    }

    /// Valeur d'une propriété sous forme de texte (`None` si elle n'est pas disponible).
    pub fn get_property(&self, name: &str) -> Option<String> {
        let n = cstr(name).ok()?;
        // SAFETY : handle valide ; la chaîne renvoyée par mpv est copiée puis libérée avec `mpv_free`.
        unsafe {
            let p = (self.lib.get_property_string)(self.handle, n.as_ptr());
            if p.is_null() {
                return None;
            }
            let v = CStr::from_ptr(p).to_string_lossy().into_owned();
            (self.lib.free)(p.cast());
            Some(v)
        }
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| cstr(a)).collect::<Result<_, _>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(ptr::null());
        // SAFETY : tableau terminé par NULL, chaînes vivantes pendant l'appel.
        let code = unsafe { (self.lib.command)(self.handle, ptrs.as_mut_ptr()) };
        if code < 0 {
            Err(format!(
                "{} {} : {}",
                crate::i18n::tr("Commande du lecteur refusée :"),
                args.first().copied().unwrap_or("?"),
                self.lib.err(code)
            ))
        } else {
            Ok(())
        }
    }

    /// Couleur des bandes autour de l'image (selon le format du live ou la taille de la fenêtre).
    /// Selon la version de libmpv l'option s'appelle `background-color` ou `background` : on essaie les deux.
    pub fn set_background(&self, c: egui::Color32) {
        let hex = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
        if self.set_property("background-color", &hex).is_err() {
            let _ = self.set_property("background", &hex);
        }
    }

    pub fn set_volume(&self, percent: u8) {
        let _ = self.set_property("volume", &percent.to_string());
    }

    pub fn set_pause(&self, paused: bool) {
        let _ = self.set_property("pause", if paused { "yes" } else { "no" });
    }

    pub fn stop(&self) {
        let _ = self.command(&["stop"]);
    }

    /// À la destruction, le contexte de rendu est toujours libéré sur ce fil (il demande le contexte OpenGL), mais
    /// l'arrêt du cœur de mpv (`mpv_terminate_destroy`, qui attend ses fils de lecture) se fait dans un fil à part :
    /// l'interface ne se fige pas quand plusieurs lecteurs s'arrêtent (multistream).
    pub fn destroy_in_background(&mut self) {
        self.destroy_async = true;
    }

    pub fn load(&self, url: &str) -> Result<(), String> {
        self.command(&["loadfile", url])
    }

    /// Dessine la vidéo dans `rect` ; les widgets ajoutés ensuite passent par-dessus.
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        self.last_paint
            .store(ui.ctx().cumulative_pass_nr(), Ordering::Relaxed);
        let shared = self.shared.clone();
        // Redimensionnement en cours (chat tiré à la souris, fenêtre) : mpv garde la taille de rendu d'avant et l'image
        // est étirée à la volée, ce qui évite de recalculer les filtres à chaque pixel (saccade puis rattrapage).
        let ppp = ui.ctx().pixels_per_point();
        let now_size = (
            (rect.width() * ppp).round() as i32,
            (rect.height() * ppp).round() as i32,
        );
        let settle = {
            let mut t = self.track.lock().unwrap_or_else(|e| e.into_inner());
            if t.0 != now_size {
                *t = (now_size, Instant::now());
            }
            t.1.elapsed() < Duration::from_millis(150)
        };
        if settle {
            ui.ctx().request_repaint_after(Duration::from_millis(160));
        }
        let cb = egui_glow::CallbackFn::new(move |info, painter| {
            let vp = info.viewport_in_pixels();
            // SAFETY : appelé par egui avec le contexte OpenGL courant.
            unsafe {
                render(
                    &shared,
                    &info,
                    painter,
                    (vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px),
                    settle,
                    true,
                )
            };
        });
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        });
    }

    /// À appeler en fin d'image : si la vidéo n'a pas été dessinée (Réglages ouverts par-dessus l'accueil, mini-lecteur
    /// masqué), mpv est quand même rendu, dans une toute petite zone et sans rien afficher. Sans cela mpv n'avance
    /// plus ses images, prend du retard sur le son et les rattrape en accéléré au retour.
    pub fn keep_live(&self, ctx: &egui::Context) {
        if self.last_paint.load(Ordering::Relaxed) == ctx.cumulative_pass_nr() {
            return;
        }
        let shared = self.shared.clone();
        let cb = egui_glow::CallbackFn::new(move |info, painter| {
            let vp = info.viewport_in_pixels();
            // SAFETY : appelé par egui avec le contexte OpenGL courant.
            unsafe {
                render(
                    &shared,
                    &info,
                    painter,
                    (vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px),
                    false,
                    false,
                )
            };
        });
        let rect = egui::Rect::from_min_size(ctx.content_rect().min, egui::vec2(32.0, 18.0));
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::unique("mpv_keep_live"),
        ))
        .add(egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        });
    }
}

fn cstr(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| "Texte invalide pour mpv.".to_string())
}

/// # Safety
/// Doit être appelée avec le contexte OpenGL de `painter` courant.
unsafe fn render(
    sh: &Shared,
    _info: &egui::PaintCallbackInfo,
    painter: &egui_glow::Painter,
    (x, y, w, h): (i32, i32, i32, i32),
    settle: bool,
    show: bool,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let gl = painter.gl();
    let Ok(mut res) = sh.gl.lock() else { return };
    if res.closed {
        return;
    }
    // Pendant le redimensionnement (chat, fenêtre), la taille change à chaque image : la texture n'est
    // réallouée que si elle devient trop petite ou beaucoup trop grande (marge de 128 px), pas à chaque pixel.
    // Taille de rendu : celle de la zone, sauf pendant un redimensionnement modéré (±35 %) où l'on garde la précédente.
    let close = |a: i32, b: i32| (a as f32 - b as f32).abs() <= 0.35 * b as f32;
    let (rw, rh) = match res.rendered {
        Some((pw, ph)) if settle && close(w, pw) && close(h, ph) => (pw, ph),
        _ => (w, h),
    };
    let (cw, ch) = res.size;
    let fits = res.fbo.is_some() && rw <= cw && rh <= ch && cw <= rw * 2 && ch <= rh * 2;
    if !fits {
        let (aw, ah) = ((rw + 127) / 128 * 128, (rh + 127) / 128 * 128);
        if let Some(f) = res.fbo.take() {
            gl.delete_framebuffer(f);
        }
        if let Some(t) = res.tex.take() {
            gl.delete_texture(t);
        }
        let (Ok(tex), Ok(fbo)) = (gl.create_texture(), gl.create_framebuffer()) else {
            return;
        };
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            aw,
            ah,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );
        res.tex = Some(tex);
        res.fbo = Some(fbo);
        res.size = (aw, ah);
    }
    let Some(fbo) = res.fbo else { return };

    (sh.lib.rc_update)(sh.rc);
    let mut target = OpenGlFbo {
        fbo: fbo.0.get() as c_int,
        w: rw,
        h: rh,
        internal_format: 0,
    };
    let mut flip: c_int = 1;
    let mut params = [
        RenderParam {
            kind: PARAM_OPENGL_FBO,
            data: &mut target as *mut _ as *mut c_void,
        },
        RenderParam {
            kind: PARAM_FLIP_Y,
            data: &mut flip as *mut _ as *mut c_void,
        },
        RenderParam {
            kind: 0,
            data: ptr::null_mut(),
        },
    ];
    (sh.lib.rc_render)(sh.rc, params.as_mut_ptr());

    let dst = painter.intermediate_fbo();
    if !show {
        gl.bind_framebuffer(glow::FRAMEBUFFER, dst);
        return;
    }
    gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
    gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, dst);
    if (rw, rh) == (w, h) {
        res.rendered = Some((w, h));
        gl.blit_framebuffer(
            0,
            0,
            w,
            h,
            x,
            y,
            x + w,
            y + h,
            glow::COLOR_BUFFER_BIT,
            glow::NEAREST,
        );
    } else {
        gl.blit_framebuffer(
            0,
            0,
            rw,
            rh,
            x,
            y,
            x + w,
            y + h,
            glow::COLOR_BUFFER_BIT,
            glow::LINEAR,
        );
    }
    gl.bind_framebuffer(glow::FRAMEBUFFER, dst);
}

impl Drop for Player {
    fn drop(&mut self) {
        // Verrou pris pendant la destruction : un rendu en cours finit avant, un rendu suivant voit `closed`.
        let mut gl = self.shared.gl.lock().unwrap_or_else(|e| e.into_inner());
        gl.closed = true;
        // Le FBO et la texture de la zone vidéo sont supprimés à l'image suivante (`free_dropped`).
        queue_for_release(&mut gl, &DROPPED);
        // SAFETY : ordre imposé par libmpv : contexte de rendu, puis lecteur.
        unsafe {
            if !self.shared.rc.is_null() {
                (self.lib.rc_set_update_callback)(self.shared.rc, None, ptr::null_mut());
                (self.lib.rc_free)(self.shared.rc);
            }
            if self.destroy_async {
                // Le pointeur passe d'un fil à l'autre sous forme de nombre ; `lib` garde la bibliothèque chargée
                // jusqu'à la fin. Plus rien d'autre n'utilise ce lecteur (le contexte de rendu est déjà libéré).
                let (lib, handle) = (self.lib.clone(), self.handle as usize);
                // SAFETY (bloc `unsafe` englobant) : l'API cliente de libmpv accepte cet appel depuis n'importe quel fil.
                std::thread::spawn(move || (lib.terminate_destroy)(handle as Handle));
            } else {
                (self.lib.terminate_destroy)(self.handle);
            }
            if !self.update_ctx.is_null() {
                drop(Box::from_raw(self.update_ctx));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    fn fbo(n: u32) -> glow::Framebuffer {
        glow::NativeFramebuffer(NonZeroU32::new(n).unwrap())
    }

    fn tex(n: u32) -> glow::Texture {
        glow::NativeTexture(NonZeroU32::new(n).unwrap())
    }

    /// à la destruction d'un lecteur, son FBO et sa texture passent dans la file de suppression, une seule
    /// fois ; un lecteur qui n'a jamais rendu (rien d'alloué) n'ajoute rien.
    #[test]
    fn ressources_d_un_lecteur_detruit_mises_en_file() {
        let queue = Mutex::new(Vec::new());

        let mut res = GlRes {
            fbo: Some(fbo(3)),
            tex: Some(tex(5)),
            ..GlRes::default()
        };
        queue_for_release(&mut res, &queue);
        assert!(
            res.fbo.is_none() && res.tex.is_none(),
            "plus rien ne reste dans le lecteur"
        );
        queue_for_release(&mut res, &queue);
        assert_eq!(queue.lock().unwrap().len(), 1, "supprimé une seule fois");

        // Un seul des deux alloués (création interrompue) : l'autre est quand même supprimé.
        let mut partiel = GlRes {
            tex: Some(tex(9)),
            ..GlRes::default()
        };
        queue_for_release(&mut partiel, &queue);
        queue_for_release(&mut GlRes::default(), &queue);

        let attente = take_queue(&queue);
        assert_eq!(
            attente,
            vec![(Some(fbo(3)), Some(tex(5))), (None, Some(tex(9)))]
        );
        assert!(
            take_queue(&queue).is_empty(),
            "la file est vidée par la suppression"
        );
    }
}
