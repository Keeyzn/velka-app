//! Réglages `settings.json` : valeurs par défaut, lecture tolérante, écriture atomique.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Taille minimale de la fenêtre principale (points) : la taille d'ouverture choisie, pas plus petite.
pub const MIN_WINDOW: [f32; 2] = [1007.0, 657.0];

/// Taille de l'interface d'une nouvelle installation (110 %) ; Ctrl 0 y revient. Un fichier de réglages qui contient déjà
/// une taille la garde. Bornes : 80 % à 150 %.
pub const DEFAULT_UI_ZOOM: f32 = 1.1;

/// Version du format de `settings.json`. À augmenter seulement quand une version change le
/// sens d'un champ existant ou fait une migration ; ajouter un champ ne demande pas d'augmentation. Ne jamais
/// réutiliser un nom de champ pour un autre sens : créer un nouveau champ.
pub const SETTINGS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 0 = Source, 1 = 720p, 2 = 480p, 3 = Audio.
    pub quality: u8,
    /// Qualité des lives de la page « Multistream » (mêmes valeurs que `quality`), choisie sur la page ; 720p par
    /// défaut pour ménager réseau et processeur avec quatre lives.
    pub multi_quality: u8,
    /// Multistream : un clic sur un live ajoute son son à ceux déjà écoutés (jusqu'aux quatre lives) au lieu de le remplacer.
    pub multi_mix_sounds: bool,
    /// 0 à 130.
    pub volume: u8,
    pub low_latency: bool,
    pub on_top: bool,
    /// 0 = noir (affiché « Dark »), 1 = bleu nuit (affiché « Sombre »), 2 = Clair, 3 = Système (suit Windows).
    pub theme: u8,
    /// Indice dans `i18n::LANGUAGES` (0 = français, 1 = anglais…).
    pub language: u8,
    pub chat_visible: bool,
    /// Molette de la souris sur la vidéo = volume.
    pub wheel_volume: bool,
    /// Clic simple sur la vidéo = pause / lecture.
    pub click_pause: bool,
    /// Égalise le volume entre les streamers (filtre audio de mpv).
    pub normalize_audio: bool,
    /// Multiplicateur de la vitesse de défilement (0,5 à 3).
    pub scroll_speed: f32,
    /// Tri des lives de l'accueil : 0 = spectateurs décroissants, 1 = spectateurs croissants.
    pub home_sort: u8,
    /// Barre latérale réduite : seulement les icônes et les photos de chaînes.
    pub sidebar_collapsed: bool,
    /// Actualisation de l'accueil : 0 = 20 s, 1 = 1 min (défaut des nouvelles installations), 2 = manuelle
    /// (bouton « Actualiser »). Un `settings.json` existant garde sa valeur enregistrée (0 pour qui n'a rien changé).
    pub refresh_mode: u8,
    /// Volume maximal du curseur et des raccourcis : 100 ou 130.
    pub volume_max: u8,
    pub start_muted: bool,
    /// Si `true`, le volume au lancement est `fixed_volume` au lieu du dernier volume utilisé.
    pub fixed_volume_on: bool,
    pub fixed_volume: u8,
    /// Masquage des contrôles du lecteur : 0 = 2,5 s, 1 = 5 s, 2 = 10 s, 3 = jamais.
    pub controls_hide: u8,
    /// Quitter un live sans l'arrêter le garde dans un mini-lecteur.
    pub mini_player: bool,
    /// Clic sur une carte de live : 0 = ouvrir en grand, 1 = ouvrir en PiP.
    pub card_click: u8,
    /// Échelle du texte du chat (0,8 à 1,5).
    pub chat_font_scale: f32,
    pub chat_hide_bots: bool,
    /// Mots à surligner dans le chat, séparés par des virgules.
    pub chat_words: String,
    pub chat_timestamps: bool,
    /// Netteté de l'image : 0 = désactivée, 1 = légère, 2 = moyenne, 3 = forte.
    pub video_sharpen: u8,
    /// Agrandissement de haute qualité (utile en 480p et 720p).
    pub video_upscale: bool,
    /// Décodage vidéo matériel (GPU) : 0 = automatique (le lecteur garde son réglage d'origine), 1 = activé, 2 = désactivé.
    pub hw_decode: u8,
    /// Périphérique de sortie audio (nom mpv, ex. `wasapi/{…}`) ; vide = périphérique par défaut du système.
    pub audio_device: String,
    /// Dossier des captures d'écran ; vide = `Images\Velka` (dossier d'origine).
    pub screenshot_dir: String,
    /// Emotes des extensions 7TV, BTTV et FFZ dans le chat.
    pub chat_third_party: bool,
    /// Emotes animées (GIF) dans le chat.
    pub chat_animated: bool,
    /// Badges (abonné, modérateur…) devant les pseudos.
    pub chat_badges: bool,
    pub show_thumbnails: bool,
    pub remember_game: bool,
    pub home_game: String,
    /// Favoris masqués de la liste de recherche (sans être retirés des favoris).
    pub search_hidden: Vec<String>,
    /// Affiche les chaînes suivies sur le compte Twitch (barre latérale, accueil, recommandations).
    pub show_follows: bool,
    /// Nombre de chaînes Twitch dans la barre latérale : 0 = 10, 1 = 20, 2 = 40.
    pub sidebar_follow_limit: u8,
    /// Nombre de chaînes Twitch sur l'accueil : 0 = 12, 1 = 24, 2 = 40.
    pub home_follow_limit: u8,
    pub recommendations: bool,
    /// Nombre de recommandations : 0 = 3, 1 = 5, 2 = 8.
    pub recs_count: u8,
    pub mouse_nav: bool,
    /// Recherche d'une nouvelle version au démarrage et toutes les 6 heures.
    pub check_updates: bool,
    pub hand_cursor: bool,
    /// Fenêtre PiP au format de la vidéo (pas de bandes noires).
    pub pip_fit: bool,
    /// Lecteur principal au format de la vidéo (pas de bandes autour de l'image).
    pub fit_video: bool,
    /// Chaîne dont le live s'ouvre au lancement si elle est en direct (vide = aucune).
    pub startup_channel: String,
    /// Dernier format d'image connu (largeur / hauteur) : le lecteur s'ouvre directement à ces proportions.
    pub video_ratio: f32,
    /// Animations : faux = aucune. Voir `motion_level` (le réglage de Windows n'est jamais lu).
    pub animations: bool,
    /// Animations réduites (avec `animations` vrai) : fondus et couleurs gardés, déplacements coupés.
    pub reduced_motion: bool,
    /// Taille de l'interface (0,8 à 1,5, défaut 1,1) : zoom de toute l'application.
    pub ui_zoom: f32,
    /// Contraste élevé : 0 = automatique (suit le thème de contraste de Windows), 1 = activé, 2 = désactivé.
    pub high_contrast: u8,
    /// Focus clavier renforcé (anneau épais, toujours visible).
    pub focus_strong: bool,
    /// Version de l'application au dernier contrôle des composants (Streamlink, libmpv) : vide tant qu'aucun contrôle
    /// n'a eu lieu. Un lancement avec une autre version déclenche un contrôle.
    pub tools_checked_version: String,
    /// Palette de couleurs : 0 à 4 = palettes prédéfinies (`theme::PALETTES`), 5 = couleur d'accent personnalisée
    /// (`accent_custom`, orange par défaut).
    pub palette: u8,
    /// Couleur d'accent personnalisée (RVB) ; n'est appliquée que si `palette` vaut 5 et seulement si elle est lisible.
    pub accent_custom: [u8; 3],
    /// Corrige la couleur personnalisée pour que le texte blanc des boutons y soit lisible (facultatif).
    pub accent_boost: bool,
    /// Notification Windows quand un favori passe en direct (désactivée par défaut).
    pub notify_live: bool,
    /// La croix de fermeture cache la fenêtre dans la zone de notification au lieu de quitter (désactivé par défaut).
    pub close_to_tray: bool,
    /// La bulle « l'application reste active » a déjà été montrée (une seule fois).
    pub tray_hint_shown: bool,
    /// Minuteur de veille : arrêt du live après 0 = jamais, 1 = 15 min, 2 = 30 min, 3 = 1 h, 4 = 2 h.
    pub sleep_timer: u8,
    /// Durée du minuteur « Perso » (valeur 5 de `sleep_timer`), en minutes, de 1 à 720.
    pub sleep_custom_min: u32,
    /// Raccourcis modifiés : action → nom de touche (vide = désactivé). Absent = touche par défaut.
    pub shortcuts: BTreeMap<String, String>,
    pub chat_left: bool,
    pub chat_width: f32,
    pub fullscreen_chat_overlay: bool,
    pub favorites: Vec<String>,
    /// Jeton du compte connecté avec « Se connecter avec Twitch » (chat seulement).
    /// Les jetons sont enregistrés chiffrés dans `secrets.dat` (DPAPI, Trousseau ou trousseau du bureau), jamais dans
    /// `settings.json` ; ils y sont seulement relus s'ils s'y trouvent encore (ancienne version), puis déplacés au
    /// prochain enregistrement.
    #[serde(skip_serializing)]
    pub oauth_token: String,
    /// Jeton de renouvellement (connexion « Se connecter avec Twitch »).
    #[serde(skip_serializing)]
    pub oauth_refresh: String,
    /// Fin de validité du jeton (secondes depuis 1970), 0 si inconnue.
    pub oauth_expires_at: u64,
    /// Pseudo du compte connecté (affichage).
    pub account_login: String,
    pub streamlink_path: String,
    pub libmpv_path: String,
    pub window_size: Option<[f32; 2]>,
    pub window_pos: Option<[f32; 2]>,
    /// Taille et position de la mini-fenêtre PiP à la dernière sortie du PiP (réutilisées à la prochaine entrée).
    pub pip_size: Option<[f32; 2]>,
    pub pip_pos: Option<[f32; 2]>,
    /// Place du mini-lecteur dans la fenêtre : part (0 à 1) du trajet disponible en x et en y ; toujours sur un bord
    /// (au moins une valeur à 0 ou 1). `[1, 1]` = en bas à droite (d'origine).
    pub mini_pos: [f32; 2],
    /// Largeur du mini-lecteur (320 = d'origine ; agrandissable jusqu'à 85 % de la fenêtre).
    pub mini_width: f32,
    /// Échelle d'affichage au moment de l'enregistrement (pour vérifier que la position est visible).
    pub window_ppp: f32,
    pub window_maximized: bool,
    /// Le didacticiel de bienvenue a été vu (ou passé). Un fichier de réglages existant sans ce champ vient d'une
    /// version antérieure : pas de didacticiel ; une nouvelle installation (valeur par défaut) l'affiche une fois.
    #[serde(default = "onboarding_seen")]
    pub onboarding_done: bool,
    /// Jour (depuis 1970) de la première utilisation ; 0 = pas encore connu.
    pub first_use_day: u32,
    /// Jour du dernier rappel de soutien affiché ; 0 = jamais.
    pub support_last_day: u32,
    /// Nombre de lancements de l'application.
    pub launch_count: u32,
    /// `window_size` est enregistrée en points d'écran réels (celle qu'on passe à l'ouverture), et la migration
    /// `grow_window_once` (conversion + 20 %) est faite. Un fichier de réglages existant sans ce champ vient d'une
    /// version antérieure : sa taille était enregistrée divisée par le zoom de l'interface, elle est convertie et
    /// agrandie une seule fois ; une nouvelle installation (valeur par défaut) n'a rien à migrer.
    #[serde(default = "not_grown")]
    pub window_native_size: bool,
    /// Version du format du fichier lu : 0 = fichier d'avant ce champ. L'enregistrement écrit la plus grande
    /// des deux valeurs (fichier lu, `SETTINGS_SCHEMA`) : une version plus ancienne relancée après une plus récente
    /// (retour arrière S7) ne fait pas croire à la récente que ses migrations restent à faire.
    #[serde(default)]
    pub schema: u32,
    /// Champs du fichier que cette version ne connaît pas (écrits par une version plus récente) : gardés tels quels et
    /// réécrits à l'enregistrement, pour qu'un retour à une version plus ancienne ne fasse perdre aucun réglage.
    #[serde(skip)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn onboarding_seen() -> bool {
    true
}

fn not_grown() -> bool {
    false
}

/// Taille d'ouverture d'une nouvelle installation (points) : 20 % de plus que l'ancienne (1100 x 650). Jamais plus petite
/// que `MIN_WINDOW`.
pub const DEFAULT_WINDOW: [f32; 2] = [1320.0, 780.0];

/// Agrandissement de la taille de fenêtre mémorisée par les versions antérieures.
const WINDOW_GROWTH: f32 = 1.2;

impl Settings {
    /// Convertit une seule fois la taille de fenêtre mémorisée par une version antérieure (enregistrée en points
    /// divisés par le zoom de l'interface : elle rétrécissait à chaque lancement) en points réels, puis l'agrandit de
    /// 20 %, sans dépasser `limit` (taille utile de l'écran principal, en points) et sans jamais la réduire. Renvoie
    /// vrai si le réglage a changé (à enregistrer).
    pub fn grow_window_once(&mut self, limit: [f32; 2]) -> bool {
        if self.window_native_size {
            return false;
        }
        self.window_native_size = true;
        if let Some(size) = self.window_size {
            let real = size.map(|v| v * self.ui_zoom);
            self.window_size = Some([0, 1].map(|i| {
                (real[i] * WINDOW_GROWTH)
                    .round()
                    .min(limit[i])
                    .max(real[i].round())
            }));
        }
        if let Some(pos) = self.window_pos {
            self.window_pos = Some(pos.map(|v| (v * self.ui_zoom).round()));
        }
        true
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: 0,
            multi_quality: 1,
            multi_mix_sounds: false,
            // Nouvelle installation : volume modéré au premier live ; ensuite le dernier volume est mémorisé.
            volume: 30,
            low_latency: true,
            on_top: false,
            // Nouvelle installation : thème et langue de Windows (les réglages déjà enregistrés ne changent pas).
            theme: crate::ui::theme::THEME_SYSTEM,
            language: crate::i18n::default_index(),
            chat_visible: true,
            wheel_volume: false,
            click_pause: false,
            normalize_audio: false,
            scroll_speed: 1.0,
            home_sort: 0,
            sidebar_collapsed: false,
            refresh_mode: 1,
            volume_max: 130,
            start_muted: false,
            fixed_volume_on: false,
            fixed_volume: 50,
            controls_hide: 0,
            mini_player: true,
            card_click: 0,
            chat_font_scale: 1.0,
            chat_hide_bots: false,
            chat_words: String::new(),
            chat_timestamps: false,
            video_sharpen: 0,
            video_upscale: false,
            hw_decode: 0,
            audio_device: String::new(),
            screenshot_dir: String::new(),
            chat_third_party: true,
            chat_animated: true,
            chat_badges: true,
            show_thumbnails: true,
            remember_game: false,
            home_game: String::new(),
            search_hidden: Vec::new(),
            show_follows: true,
            sidebar_follow_limit: 1,
            home_follow_limit: 2,
            recommendations: true,
            recs_count: 1,
            mouse_nav: true,
            check_updates: true,
            hand_cursor: true,
            pip_fit: false,
            fit_video: false,
            startup_channel: String::new(),
            video_ratio: 16.0 / 9.0,
            animations: true,
            reduced_motion: false,
            ui_zoom: DEFAULT_UI_ZOOM,
            high_contrast: 0,
            focus_strong: false,
            tools_checked_version: String::new(),
            // Nouvelle installation : couleur d'accent orange (#CA4F24).
            palette: 5,
            accent_custom: crate::ui::theme::ACCENT_NEW,
            accent_boost: false,
            notify_live: false,
            close_to_tray: false,
            tray_hint_shown: false,
            sleep_timer: 0,
            sleep_custom_min: 45,
            shortcuts: BTreeMap::new(),
            chat_left: false,
            chat_width: 340.0,
            fullscreen_chat_overlay: false,
            favorites: Vec::new(),
            oauth_token: String::new(),
            oauth_refresh: String::new(),
            oauth_expires_at: 0,
            account_login: String::new(),
            streamlink_path: String::new(),
            libmpv_path: String::new(),
            window_size: None,
            window_pos: None,
            pip_size: None,
            pip_pos: None,
            mini_pos: [1.0, 1.0],
            mini_width: 320.0,
            window_ppp: 1.0,
            window_maximized: false,
            onboarding_done: false,
            first_use_day: 0,
            support_last_day: 0,
            launch_count: 0,
            window_native_size: true,
            schema: SETTINGS_SCHEMA,
            extra: serde_json::Map::new(),
        }
    }
}

impl Settings {
    pub fn sidebar_limit(&self) -> usize {
        [10, 20, 40][self.sidebar_follow_limit.min(2) as usize]
    }

    pub fn home_limit(&self) -> usize {
        [12, 24, 40][self.home_follow_limit.min(2) as usize]
    }

    pub fn recs_limit(&self) -> usize {
        [3, 5, 8][self.recs_count.min(2) as usize]
    }

    /// Durée du minuteur de veille (`None` = désactivé). 0 = désactivé, 1 à 4 = 15 min, 30 min, 1 h, 2 h, 5 = durée
    /// personnalisée (`sleep_custom_min`).
    pub fn sleep_after(&self) -> Option<std::time::Duration> {
        let minutes = match self.sleep_timer {
            0 => return None,
            1 => 15,
            2 => 30,
            3 => 60,
            5 => u64::from(self.sleep_custom_min.clamp(1, 720)),
            _ => 120,
        };
        Some(std::time::Duration::from_secs(minutes * 60))
    }

    /// Délai avant que les contrôles du lecteur se cachent (`None` = ils restent affichés).
    pub fn controls_hide_after(&self) -> Option<std::time::Duration> {
        [Some(2500u64), Some(5000), Some(10_000), None][self.controls_hide.min(3) as usize]
            .map(std::time::Duration::from_millis)
    }

    /// Intervalle d'actualisation de l'accueil (`None` = manuelle).
    pub fn refresh_interval(&self) -> Option<std::time::Duration> {
        [Some(20u64), Some(60), None][self.refresh_mode.min(2) as usize]
            .map(std::time::Duration::from_secs)
    }

    /// Couleur d'accent à appliquer.
    pub fn accent_choice(&self) -> Option<[u8; 3]> {
        match self.palette {
            i @ 0..=4 => Some(crate::ui::theme::PALETTES[usize::from(i)].1),
            _ => Some(self.accent_custom),
        }
    }

    /// Contraste élevé réellement appliqué : le réglage, ou (en automatique) le thème de contraste de Windows.
    /// Niveau des animations : 0 = complètes (défaut), 1 = réduites, 2 = désactivées.
    pub fn motion_level(&self) -> u8 {
        if !self.animations {
            2
        } else if self.reduced_motion {
            1
        } else {
            0
        }
    }

    pub fn set_motion_level(&mut self, level: u8) {
        self.animations = level < 2;
        self.reduced_motion = level == 1;
    }

    pub fn high_contrast_on(&self, system: bool) -> bool {
        match self.high_contrast {
            1 => true,
            2 => false,
            _ => system,
        }
    }

    /// Remet les valeurs hors limites dans leur plage.
    fn sanitize(mut self) -> Self {
        self.quality = self.quality.min(4);
        self.multi_quality = self.multi_quality.min(4);
        self.volume_max = if self.volume_max <= 100 { 100 } else { 130 };
        self.volume = self.volume.min(self.volume_max);
        self.fixed_volume = self.fixed_volume.min(self.volume_max);
        self.refresh_mode = self.refresh_mode.min(2);
        self.controls_hide = self.controls_hide.min(3);
        self.card_click = self.card_click.min(1);
        self.chat_font_scale = self.chat_font_scale.clamp(0.8, 1.5);
        self.ui_zoom = if self.ui_zoom.is_finite() {
            self.ui_zoom.clamp(0.8, 1.5)
        } else {
            DEFAULT_UI_ZOOM
        };
        self.high_contrast = self.high_contrast.min(2);
        self.palette = self.palette.min(5);
        self.sidebar_follow_limit = self.sidebar_follow_limit.min(2);
        self.home_follow_limit = self.home_follow_limit.min(2);
        self.recs_count = self.recs_count.min(2);
        // Valeurs hors liste : comme avant l'option « Perso », elles retombent sur 2 h.
        if self.sleep_timer > 5 {
            self.sleep_timer = 4;
        }
        self.sleep_custom_min = self.sleep_custom_min.clamp(1, 720);
        self.video_sharpen = self.video_sharpen.min(3);
        self.hw_decode = self.hw_decode.min(2);
        self.theme = self.theme.min(crate::ui::theme::THEME_SYSTEM);
        self.scroll_speed = self.scroll_speed.clamp(0.5, 3.0);
        if self.home_sort > 1 {
            self.home_sort = 0; // ancienne valeur « par jeu » : retour au tri par défaut
        }
        self.language = self.language.min(crate::i18n::LANGUAGES.len() as u8 - 1);
        self.chat_width = self.chat_width.clamp(200.0, 800.0);
        self
    }

    /// Réglages d'une installation neuve (aucun `settings.json`) : les valeurs par défaut, sauf la molette de la souris
    /// qui règle le volume (geste attendu d'un lecteur). Les réglages déjà enregistrés ne changent jamais.
    pub fn new_install() -> Self {
        Self {
            wheel_volume: true,
            ..Self::default()
        }
    }

    /// Fichier absent : installation neuve (`new_install`). Fichier illisible : il est mis de côté sous le nom
    /// `settings.illisible-<date>.json` (jamais écrasé par une sauvegarde) et les valeurs par défaut servent.
    /// Une valeur invalide isolée (ex. `"volume": 300`) ne fait perdre que cette valeur.
    pub fn load_from(path: &Path) -> Self {
        let mut s = match fs::read_to_string(path) {
            Ok(text) => match parse_settings(&text) {
                Ok((s, ignored)) => {
                    if !ignored.is_empty() {
                        crate::logging::log(&format!(
                            "settings.json : valeurs invalides ignorées ({})",
                            ignored.join(", ")
                        ));
                        set_aside(path, true);
                    }
                    let mut s = s.sanitize();
                    s.extra = unknown_fields(&text);
                    if s.schema > SETTINGS_SCHEMA {
                        crate::logging::log(&format!(
                            "settings.json écrit par une version plus récente (format {} ; celui-ci : {SETTINGS_SCHEMA}) : champs inconnus gardés",
                            s.schema
                        ));
                        keep_newer_copy(path, s.schema);
                    }
                    s
                }
                Err(e) => {
                    crate::logging::log(&format!("settings.json illisible : {e}"));
                    set_aside(path, false);
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::new_install(),
            Err(e) => {
                crate::logging::log(&format!("settings.json non lu : {e}"));
                set_aside(path, false);
                Self::default()
            }
        };
        s.load_secrets(&secrets_path(path));
        s
    }

    /// Complète les jetons avec `secrets.dat`. Un jeton encore présent dans `settings.json` (ancienne version de
    /// l'application, qui a pu se reconnecter entre-temps) l'emporte : il sera déplacé au prochain enregistrement.
    fn load_secrets(&mut self, path: &Path) {
        let Ok(stored) = fs::read_to_string(path) else {
            return;
        };
        let Some(sec) = commun::dpapi::open_text(&stored)
            .filter(|(_, plain)| !plain)
            .and_then(|(t, _)| serde_json::from_str::<Secrets>(&t).ok())
        else {
            crate::logging::log("secrets.dat illisible (autre session, Trousseau absent ou fichier abîmé) : jetons ignorés");
            return;
        };
        for (field, value) in [
            (&mut self.oauth_token, sec.oauth_token),
            (&mut self.oauth_refresh, sec.oauth_refresh),
        ] {
            if field.is_empty() {
                *field = value;
            }
        }
    }

    fn secrets(&self) -> Secrets {
        Secrets {
            oauth_token: self.oauth_token.clone(),
            oauth_refresh: self.oauth_refresh.clone(),
        }
    }

    pub fn load() -> Self {
        Self::load_from(&settings_path())
    }

    /// Enregistre `settings.json` sans les jetons, et les jetons chiffrés dans `secrets.dat`. Si le chiffrement est
    /// impossible, les jetons ne sont enregistrés nulle part (jamais en clair) : `secrets_unsaved` le signale et la
    /// page Compte affiche l'erreur ; la connexion ne dure alors que jusqu'à la fermeture de l'application.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let mut value = serde_json::to_value(self).map_err(std::io::Error::other)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("schema".into(), self.schema.max(SETTINGS_SCHEMA).into());
            for (k, v) in &self.extra {
                if !obj.contains_key(k) {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        let sec = self.secrets();
        let spath = secrets_path(path);
        let stored = if sec.is_empty() {
            match fs::remove_file(&spath) {
                Ok(()) => true,
                Err(e) => e.kind() == std::io::ErrorKind::NotFound,
            }
        } else {
            let sealed = serde_json::to_string(&sec)
                .ok()
                .and_then(|t| commun::dpapi::seal_text(&t));
            match sealed {
                Some(t) => write_atomic(&spath, t.as_bytes()).is_ok(),
                None => false,
            }
        };
        if !stored {
            crate::logging::log(
                "secrets.dat non enregistré (chiffrement impossible) : jetons non enregistrés",
            );
        }
        SECRETS_UNSAVED.store(!stored, std::sync::atomic::Ordering::Relaxed);
        let json = serde_json::to_string_pretty(&value).map_err(std::io::Error::other)?;
        write_atomic(path, json.as_bytes())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&settings_path())
    }
}

/// Le dernier enregistrement n'a pas pu chiffrer les jetons : ils n'ont pas été écrits sur le disque.
static SECRETS_UNSAVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `true` si les jetons de la connexion Twitch n'ont pas pu être enregistrés de façon chiffrée.
pub fn secrets_unsaved() -> bool {
    SECRETS_UNSAVED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn settings_path() -> PathBuf {
    crate::logging::data_dir().join("settings.json")
}

/// Jetons Twitch, enregistrés à part et chiffrés. Fichier séparé : une ancienne version qui réécrit
/// `settings.json` ne peut pas les effacer.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Secrets {
    oauth_token: String,
    oauth_refresh: String,
}

impl Secrets {
    fn is_empty(&self) -> bool {
        self.oauth_token.is_empty() && self.oauth_refresh.is_empty()
    }
}

fn secrets_path(settings: &Path) -> PathBuf {
    settings.with_file_name("secrets.dat")
}

/// Lit `settings.json`. Si une valeur est invalide, seules les valeurs fautives sont ignorées (leurs noms sont
/// rendus) ; un fichier qui n'est pas un objet JSON (tronqué…) est une erreur.
fn parse_settings(text: &str) -> Result<(Settings, Vec<String>), serde_json::Error> {
    use serde_json::{Map, Value};
    let text = text.trim_start_matches('\u{feff}');
    let first = match serde_json::from_str::<Settings>(text) {
        Ok(s) => return Ok((s, Vec::new())),
        Err(e) => e,
    };
    let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(text) else {
        return Err(first);
    };
    let mut ignored = Vec::new();
    for key in map.keys().cloned().collect::<Vec<_>>() {
        let mut one = Map::new();
        if let Some(v) = map.get(&key) {
            one.insert(key.clone(), v.clone());
        }
        if serde_json::from_value::<Settings>(Value::Object(one)).is_err() {
            map.remove(&key);
            ignored.push(key);
        }
    }
    serde_json::from_value::<Settings>(Value::Object(map)).map(|s| (s, ignored))
}

/// Champs de `settings.json` que `Settings` ne connaît pas. Les jetons, relus mais jamais écrits dans ce fichier, sont
/// connus ; `manual_token` (ancienne version) est abandonné, jamais recopié.
fn unknown_fields(text: &str) -> serde_json::Map<String, serde_json::Value> {
    let Ok(serde_json::Value::Object(mut map)) =
        serde_json::from_str(text.trim_start_matches('\u{feff}'))
    else {
        return serde_json::Map::new();
    };
    let known = match serde_json::to_value(Settings::default()) {
        Ok(serde_json::Value::Object(k)) => k,
        _ => return serde_json::Map::new(),
    };
    map.retain(|k, _| {
        !known.contains_key(k)
            && !matches!(k.as_str(), "oauth_token" | "manual_token" | "oauth_refresh")
    });
    map
}

/// Copie unique du fichier écrit par une version plus récente, sous `settings.schema-<n>.json`, avant toute
/// réécriture par cette version (filet en plus de la conservation des champs inconnus).
fn keep_newer_copy(path: &Path, schema: u32) {
    let dest = path.with_file_name(format!("settings.schema-{schema}.json"));
    if dest.exists() {
        return;
    }
    if let Err(e) = fs::copy(path, &dest) {
        crate::logging::log(&format!(
            "Copie de settings.json (format {schema}) non faite : {e}"
        ));
    }
}

/// Garde le fichier de réglages abîmé sous `settings.illisible-<aaaa-mm-jj>.json` (copie, ou déplacement si le
/// fichier ne sert plus). Seules les 5 dernières copies sont conservées.
fn set_aside(path: &Path, copy: bool) {
    // Le fichier est relu plusieurs fois au démarrage : une copie identique à une sauvegarde existante n'est pas refaite.
    if copy {
        if let (Ok(now), Some(dir)) = (fs::read(path), path.parent()) {
            let already = fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("settings.illisible-")
                    && fs::read(e.path()).is_ok_and(|b| b == now)
            });
            if already {
                return;
            }
        }
    }
    let (y, m, d) = commun::civil_from_days(commun::today_days());
    let stem = format!("settings.illisible-{y}-{m:02}-{d:02}");
    for n in 0..100 {
        let name = if n == 0 {
            format!("{stem}.json")
        } else {
            format!("{stem}-{n}.json")
        };
        let dest = path.with_file_name(name);
        if dest.exists() {
            continue;
        }
        let res = if copy {
            fs::copy(path, &dest).map(|_| ())
        } else {
            fs::rename(path, &dest)
        };
        match res {
            Ok(()) => crate::logging::log(
                "Ancien settings.json gardé sous le nom settings.illisible-….json",
            ),
            Err(e) => crate::logging::log(&format!("settings.json illisible non gardé : {e}")),
        }
        break;
    }
    let Some(dir) = path.parent() else { return };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut old: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("settings.illisible-") && n.ends_with(".json"))
        })
        .collect();
    old.sort();
    let excess = old.len().saturating_sub(5);
    for p in old.into_iter().take(excess) {
        let _ = fs::remove_file(p);
    }
}

/// Écrit dans un fichier temporaire, force l'écriture sur le disque, puis renomme : une coupure de courant
/// laisse l'ancien fichier ou le nouveau, jamais un fichier tronqué.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn valeurs_par_defaut_inchangees() {
        // Au téléchargement, le comportement est celui d'avant les options : rien n'est désactivé.
        let s = Settings::default();
        // Seule exception voulue : actualisation à 1 min (décision Q1 du 24/11/2026), pour ménager l'API Twitch.
        assert_eq!((s.refresh_mode, s.volume_max, s.controls_hide), (1, 130, 0));
        assert_eq!(s.volume, 30, "volume d'une nouvelle installation");
        assert!(
            !s.start_muted
                && !s.fixed_volume_on
                && !s.chat_hide_bots
                && !s.chat_timestamps
                && !s.remember_game
                && s.search_hidden.is_empty()
        );
        assert!(
            s.chat_third_party
                && s.chat_animated
                && s.chat_badges
                && !s.click_pause
                && !s.wheel_volume
        );
        assert!(
            s.video_sharpen == 0
                && !s.video_upscale
                && s.hw_decode == 0
                && s.audio_device.is_empty()
                && s.screenshot_dir.is_empty()
        );
        assert!(s.mini_player && s.show_thumbnails && s.show_follows && s.recommendations);
        assert!(s.check_updates && s.mouse_nav && s.hand_cursor && s.animations && !s.notify_live);
        assert!(
            !s.close_to_tray && !s.tray_hint_shown,
            "la fermeture quitte toujours par défaut"
        );
        assert!(
            !s.reduced_motion && s.motion_level() == 0,
            "animations complètes par défaut"
        );
        assert_eq!(
            (s.sidebar_limit(), s.home_limit(), s.recs_limit()),
            (20, 40, 5)
        );
        assert_eq!(
            s.refresh_interval(),
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(
            s.controls_hide_after(),
            Some(std::time::Duration::from_millis(2500))
        );
        assert_eq!(s.sleep_after(), None);
        assert!(
            s.shortcuts.is_empty()
                && s.card_click == 0
                && (s.chat_font_scale - 1.0).abs() < f32::EPSILON
        );
        assert_eq!(s.multi_quality, 1, "multistream en 720p");
        assert!(!s.multi_mix_sounds, "un seul live a le son par défaut");
        let hors_bornes: Settings = serde_json::from_str(r#"{"multi_quality": 9}"#).unwrap();
        assert_eq!(hors_bornes.sanitize().multi_quality, 4);
    }

    /// le défaut 1 min ne concerne que les nouvelles installations ; un fichier qui enregistre 20 s ou « manuelle »
    /// garde ce choix (une installation existante a toujours `refresh_mode` dans son `settings.json`).
    #[test]
    fn actualisation_existante_inchangee() {
        let vingt: Settings = serde_json::from_str(r#"{"refresh_mode": 0}"#).unwrap();
        assert_eq!(
            vingt.refresh_interval(),
            Some(std::time::Duration::from_secs(20))
        );
        let manuelle: Settings = serde_json::from_str(r#"{"refresh_mode": 2}"#).unwrap();
        assert_eq!(manuelle.refresh_interval(), None);
        let neuve: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            neuve.refresh_interval(),
            Some(std::time::Duration::from_secs(60))
        );
    }

    /// Minuteur « Perso » : durée en minutes, bornée ; les anciennes valeurs (0 à 4) gardent leurs durées.
    #[test]
    fn minuteur_personnalise() {
        let mut s = Settings::default();
        assert_eq!(s.sleep_after(), None);
        assert_eq!(
            s.sleep_custom_min, 45,
            "valeur proposée, sans effet tant que « Perso » n'est pas choisi"
        );
        for (v, min) in [(1u8, 15u64), (2, 30), (3, 60), (4, 120)] {
            s.sleep_timer = v;
            assert_eq!(
                s.sleep_after(),
                Some(std::time::Duration::from_secs(min * 60))
            );
        }
        s.sleep_timer = 5;
        s.sleep_custom_min = 90;
        assert_eq!(
            s.sleep_after(),
            Some(std::time::Duration::from_secs(90 * 60))
        );
        s.sleep_custom_min = 5000;
        assert_eq!(s.clone().sanitize().sleep_custom_min, 720);
        s.sleep_custom_min = 0;
        assert_eq!(s.clone().sanitize().sleep_custom_min, 1);
    }

    /// Un raccourci effacé (« Aucun ») est une valeur vide qui survit à l'écriture et à la relecture du JSON.
    #[test]
    fn raccourci_efface_est_memorise() {
        let mut s = Settings::default();
        s.shortcuts.insert("pause".into(), String::new());
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shortcuts.get("pause").map(String::as_str), Some(""));
    }

    /// Accessibilité : valeurs par défaut (rien ne change à l'installation), activation, persistance, retour à l'état initial.
    #[test]
    fn accessibilite_defauts_persistance_retour() {
        let d = Settings::default();
        assert!(
            (d.ui_zoom - DEFAULT_UI_ZOOM).abs() < f32::EPSILON
                && d.high_contrast == 0
                && !d.focus_strong
        );
        // Par défaut (automatique) : suit Windows, sans le forcer ni l'ignorer.
        assert!(!d.high_contrast_on(false) && d.high_contrast_on(true));
        let p = tmp("access");
        let s = Settings {
            ui_zoom: 1.25,
            high_contrast: 1,
            focus_strong: true,
            ..Settings::default()
        };
        s.save_to(&p).unwrap();
        let l = Settings::load_from(&p);
        assert_eq!(l, s);
        assert!((l.ui_zoom - 1.25).abs() < f32::EPSILON && l.focus_strong);
        // « Activé » et « Désactivé » l'emportent sur Windows (l'utilisateur peut surcharger la préférence système).
        assert!(l.high_contrast_on(false));
        let off = Settings {
            high_contrast: 2,
            ..Settings::default()
        };
        assert!(!off.high_contrast_on(true));
        // Retour à l'état initial.
        let back = Settings {
            ui_zoom: DEFAULT_UI_ZOOM,
            high_contrast: 0,
            focus_strong: false,
            ..l
        };
        assert_eq!(back, Settings::default());
    }

    /// Couleurs : défaut (palette d'origine), changement, persistance, retour aux valeurs par défaut, bornes.
    #[test]
    fn couleurs_defaut_changement_persistance_reinitialisation() {
        let d = Settings::default();
        assert_eq!(
            (d.palette, d.accent_custom),
            (5, crate::ui::theme::ACCENT_NEW)
        );
        assert_eq!(
            d.accent_choice(),
            Some([202, 79, 36]),
            "par défaut : orange #CA4F24"
        );
        assert_eq!(
            Settings {
                palette: 0,
                ..Settings::default()
            }
            .accent_choice(),
            Some(crate::ui::theme::PALETTES[0].1)
        );
        // Changement : palette prédéfinie, puis couleur personnalisée.
        let bleu = Settings {
            palette: 1,
            ..Settings::default()
        };
        assert_eq!(bleu.accent_choice(), Some(crate::ui::theme::PALETTES[1].1));
        let perso = Settings {
            palette: 5,
            accent_custom: [10, 120, 200],
            ..Settings::default()
        };
        assert_eq!(perso.accent_choice(), Some([10, 120, 200]));
        // Persistance.
        let p = tmp("couleurs");
        perso.save_to(&p).unwrap();
        let l = Settings::load_from(&p);
        assert_eq!(l, perso);
        assert_eq!(l.accent_choice(), Some([10, 120, 200]));
        // Réinitialisation : les deux champs reviennent aux valeurs par défaut, le reste est conservé.
        let reset = Settings {
            palette: 5,
            accent_custom: crate::ui::theme::ACCENT_NEW,
            ..l.clone()
        };
        assert_eq!(reset.accent_choice(), Some([202, 79, 36]));
        assert_eq!(reset, Settings::default());
        // Bornes et anciens fichiers.
        let s: Settings = serde_json::from_str(r#"{"palette": 9}"#).unwrap();
        assert_eq!(s.sanitize().palette, 5);
        let s: Settings = serde_json::from_str(r#"{"volume": 80}"#).unwrap();
        assert_eq!(
            (s.palette, s.accent_custom),
            (5, crate::ui::theme::ACCENT_NEW)
        );
    }

    /// Réorganisation des Réglages : tous les réglages déplacés (langue, mises à jour, notifications, démarrage, outils,
    /// compte…) gardent leurs clés et leurs valeurs dans `settings.json`.
    #[test]
    fn reorganisation_valeurs_conservees() {
        let ancien = r#"{"language": 3, "check_updates": false, "notify_live": true, "theme": 2, "streamlink_path": "C:/x/streamlink.exe",
            "libmpv_path": "C:/x/libmpv-2.dll", "account_login": "moi", "oauth_token": "tok", "volume": 55}"#;
        let s: Settings = serde_json::from_str(ancien).unwrap();
        let s = s.sanitize();
        assert_eq!(
            (
                s.language,
                s.check_updates,
                s.notify_live,
                s.theme,
                s.volume
            ),
            (3, false, true, 2, 55)
        );
        assert_eq!(
            (s.streamlink_path.as_str(), s.libmpv_path.as_str()),
            ("C:/x/streamlink.exe", "C:/x/libmpv-2.dll")
        );
        assert_eq!(
            (s.account_login.as_str(), s.oauth_token.as_str()),
            ("moi", "tok")
        );
        // Persistance : l'aller-retour ne perd rien.
        let p = tmp("reorg");
        s.save_to(&p).unwrap();
        // Seul le numéro de format change : fichier ancien (0) enregistré au format courant.
        assert_eq!(
            Settings::load_from(&p),
            Settings {
                schema: SETTINGS_SCHEMA,
                ..s
            }
        );
    }

    /// Taille de fenêtre : conversion du zoom + 20 % une seule fois pour un fichier ancien, jamais pour une installation
    /// neuve, plafonné à l'écran et jamais réduit.
    #[test]
    fn fenetre_agrandie_une_seule_fois() {
        let big = [f32::MAX; 2];
        // Fichier d'une version antérieure : pas de champ `window_native_size`, tailles divisées par le zoom.
        let ancien =
            r#"{"ui_zoom": 1.0, "window_size": [1100.0, 700.0], "window_pos": [100.0, 50.0]}"#;
        let mut s: Settings = serde_json::from_str(ancien).unwrap();
        assert!(!s.window_native_size);
        assert!(s.grow_window_once(big));
        assert_eq!(s.window_size, Some([1320.0, 840.0]));
        assert_eq!(s.window_pos, Some([100.0, 50.0]));
        assert!(!s.grow_window_once(big), "une seule fois");
        assert_eq!(s.window_size, Some([1320.0, 840.0]));
        // Le drapeau est enregistré : relu, la taille n'est plus touchée.
        let mut back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.window_native_size && !back.grow_window_once(big));
        // Zoom de l'interface à 110 % : la taille enregistrée (divisée par 1,1) est d'abord remise en points réels.
        let zoome =
            r#"{"ui_zoom": 1.1, "window_size": [1000.0, 600.0], "window_pos": [100.0, 50.0]}"#;
        let mut z: Settings = serde_json::from_str(zoome).unwrap();
        z.grow_window_once(big);
        assert_eq!(z.window_size, Some([1320.0, 792.0]));
        assert_eq!(z.window_pos, Some([110.0, 55.0]));
        // Installation neuve : rien à convertir.
        let mut neuf = Settings::new_install();
        assert!(
            neuf.window_native_size && neuf.window_size.is_none() && !neuf.grow_window_once(big)
        );
        // Petit écran : plafonné, mais jamais plus petit que la taille d'origine.
        let mut petit: Settings = serde_json::from_str(ancien).unwrap();
        petit.grow_window_once([1200.0, 650.0]);
        assert_eq!(petit.window_size, Some([1200.0, 700.0]));
        const { assert!(DEFAULT_WINDOW[0] >= MIN_WINDOW[0] && DEFAULT_WINDOW[1] >= MIN_WINDOW[1]) };
    }

    /// Les réglages d'accessibilité déplacés gardent leurs valeurs enregistrées (mêmes champs, même fichier).
    #[test]
    fn reglages_deplaces_valeurs_conservees() {
        let ancien = r#"{"ui_zoom": 1.3, "high_contrast": 1, "focus_strong": true, "theme": 2, "animations": false}"#;
        let s: Settings = serde_json::from_str(ancien).unwrap();
        let s = s.sanitize();
        assert!((s.ui_zoom - 1.3).abs() < f32::EPSILON && s.high_contrast == 1 && s.focus_strong);
        assert!(
            s.theme == 2 && !s.animations,
            "Thème et Animations ne bougent pas"
        );
        assert_eq!(
            s.motion_level(),
            2,
            "ancien « Animations désactivées » = niveau désactivé"
        );
        let s: Settings = serde_json::from_str(r#"{"animations": true}"#).unwrap();
        assert_eq!(
            s.motion_level(),
            0,
            "ancien « Animations activées » = niveau complet"
        );
    }

    /// Les trois niveaux d'animations s'enregistrent et se relisent sans se confondre.
    #[test]
    fn niveaux_d_animations() {
        let mut s = Settings::default();
        for level in 0..3u8 {
            s.set_motion_level(level);
            let relu: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            assert_eq!(relu.motion_level(), level);
        }
        let s: Settings =
            serde_json::from_str(r#"{"animations": false, "reduced_motion": true}"#).unwrap();
        assert_eq!(s.motion_level(), 2, "désactivé l'emporte sur réduit");
    }

    /// Contrôle des composants au premier lancement après une mise à jour de l'application : valeur par défaut,
    /// changement, persistance, retour à l'état initial.
    #[test]
    fn controle_des_composants_version_memorisee() {
        let d = Settings::default();
        assert_eq!(
            d.tools_checked_version, "",
            "jamais contrôlé : le premier lancement déclenche le contrôle"
        );
        let p = tmp("outils-version");
        let s = Settings {
            tools_checked_version: "1.4.0".into(),
            ..Settings::default()
        };
        s.save_to(&p).unwrap();
        let l = Settings::load_from(&p);
        assert_eq!(l.tools_checked_version, "1.4.0");
        // Un autre numéro de version de l'application (mise à jour) ne correspond plus : nouveau contrôle.
        assert_ne!(l.tools_checked_version, "1.5.0");
        assert_eq!(
            Settings {
                tools_checked_version: String::new(),
                ..l
            },
            Settings::default()
        );
        // Un ancien fichier sans ce champ reprend la valeur par défaut.
        let s: Settings = serde_json::from_str(r#"{"volume": 80}"#).unwrap();
        assert!(s.tools_checked_version.is_empty());
    }

    #[test]
    fn accessibilite_bornes() {
        let s: Settings = serde_json::from_str(r#"{"ui_zoom": 9.0, "high_contrast": 7}"#).unwrap();
        let s = s.sanitize();
        assert!((s.ui_zoom - 1.5).abs() < f32::EPSILON && s.high_contrast == 2);
        let s: Settings = serde_json::from_str(r#"{"ui_zoom": 0.1}"#).unwrap();
        assert!((s.sanitize().ui_zoom - 0.8).abs() < f32::EPSILON);
        // Un ancien fichier sans ces champs garde les valeurs par défaut.
        let s: Settings = serde_json::from_str(r#"{"volume": 80}"#).unwrap();
        assert!(
            !s.focus_strong
                && s.high_contrast == 0
                && (s.ui_zoom - DEFAULT_UI_ZOOM).abs() < f32::EPSILON
        );
    }

    #[test]
    fn anciens_reglages_et_bornes() {
        // Un fichier sans les nouveaux champs reprend les valeurs par défaut ; les valeurs folles sont ramenées.
        let s: Settings = serde_json::from_str(r#"{"volume": 80, "refresh_mode": 9, "volume_max": 50, "chat_font_scale": 9.0, "sleep_timer": 9}"#).unwrap();
        let s = s.sanitize();
        assert_eq!(s.sleep_after(), Some(std::time::Duration::from_secs(7200)));
        assert_eq!((s.volume, s.refresh_mode, s.volume_max), (80, 2, 100));
        assert!((s.chat_font_scale - 1.5).abs() < f32::EPSILON);
        assert!(s.mini_player && s.show_follows);
    }

    use super::*;

    /// `settings.json` d'un dossier temporaire qui disparaît à la fin du test (Q8 : plus de dossiers oubliés).
    struct Tmp {
        _dir: crate::testutil::TempDir,
        file: PathBuf,
    }

    impl std::ops::Deref for Tmp {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.file
        }
    }

    impl AsRef<Path> for Tmp {
        fn as_ref(&self) -> &Path {
            &self.file
        }
    }

    fn tmp(name: &str) -> Tmp {
        let dir = crate::testutil::TempDir::new(name);
        let file = dir.path().join("settings.json");
        Tmp { _dir: dir, file }
    }

    fn sauvegardes(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("settings.illisible-"))
            })
            .collect()
    }

    /// Une valeur hors limites (`volume` > 255) ne fait pas perdre le reste ; l'original est gardé.
    #[test]
    fn valeur_invalide_isolee_gardee_a_part() {
        let p = tmp("invalide");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"volume": 300, "favorites": ["xqc"], "theme": 1}"#).unwrap();
        let s = Settings::load_from(&p);
        // Relecture du même fichier (l'application le lit deux fois au démarrage) : pas de seconde copie.
        let _ = Settings::load_from(&p);
        assert_eq!(s.favorites, vec!["xqc".to_string()]);
        assert_eq!(s.theme, 1);
        assert_eq!(s.volume, Settings::default().volume);
        let saved = sauvegardes(p.parent().unwrap());
        assert_eq!(saved.len(), 1);
        assert!(fs::read_to_string(&saved[0]).unwrap().contains("300"));
        assert!(p.exists(), "le fichier d'origine reste en place (copie)");
    }

    /// Un fichier tronqué est mis de côté, pas écrasé par les valeurs par défaut.
    #[test]
    fn fichier_tronque_mis_de_cote() {
        let p = tmp("tronque");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"volume": 80, "favorites": ["xq"#).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(s, Settings::default());
        assert!(!p.exists(), "déplacé");
        let saved = sauvegardes(p.parent().unwrap());
        assert_eq!(saved.len(), 1);
        assert!(fs::read_to_string(&saved[0]).unwrap().contains("favorites"));
        // Une sauvegarde suivante ne touche pas à la copie.
        Settings::default().save_to(&p).unwrap();
        assert_eq!(sauvegardes(p.parent().unwrap()).len(), 1);
    }

    #[test]
    fn fichier_absent_sans_copie() {
        let p = tmp("absent");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(s, Settings::new_install());
        assert!(
            s.wheel_volume,
            "installation neuve : la molette règle le volume"
        );
        assert!(
            !Settings::default().wheel_volume,
            "les réglages déjà enregistrés et la réinitialisation gardent le défaut d'origine"
        );
        assert!(sauvegardes(p.parent().unwrap()).is_empty());
    }

    #[test]
    fn ecriture_sans_fichier_temporaire() {
        let p = tmp("atomique");
        write_atomic(&p, b"{}").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "{}");
        assert!(!p.with_extension("tmp").exists());
    }

    #[test]
    fn aller_retour() {
        let p = tmp("rt");
        let s = Settings {
            volume: 80,
            favorites: vec!["xqc".into()],
            ..Settings::default()
        };
        s.save_to(&p).unwrap();
        assert_eq!(Settings::load_from(&p), s);
    }

    /// Les jetons ne sont jamais écrits en clair dans settings.json ; ils reviennent au chargement.
    #[test]
    fn jetons_chiffres_a_part() {
        if commun::dpapi::session_without_keychain() {
            eprintln!("session SSH sur Mac : Trousseau inaccessible, essai sauté");
            return;
        }
        let p = tmp("secrets");
        let s = Settings {
            oauth_token: "jeton-acces-123".into(),
            oauth_refresh: "jeton-renouvellement-456".into(),
            account_login: "moi".into(),
            ..Settings::default()
        };
        s.save_to(&p).unwrap();
        let json = fs::read_to_string(&p).unwrap();
        let sec = fs::read_to_string(secrets_path(&p)).unwrap();
        for t in ["jeton-acces-123", "jeton-renouvellement-456"] {
            assert!(!json.contains(t) && !sec.contains(t), "{t} en clair");
        }
        assert!(!json.contains("oauth_token") && json.contains("\"account_login\": \"moi\""));
        assert!(sec.starts_with(commun::dpapi::PREFIX));
        assert_eq!(Settings::load_from(&p), s);
        // Déconnexion : secrets.dat disparaît.
        let vide = Settings {
            oauth_token: String::new(),
            oauth_refresh: String::new(),
            ..s
        };
        vide.save_to(&p).unwrap();
        assert!(!secrets_path(&p).exists());
        assert_eq!(Settings::load_from(&p).oauth_token, "");
    }

    /// Ancien settings.json avec jetons en clair : relus, puis déplacés dans secrets.dat au premier enregistrement.
    /// Un jeton réécrit par une ancienne version dans settings.json l'emporte sur secrets.dat.
    #[test]
    fn migration_des_jetons() {
        if commun::dpapi::session_without_keychain() {
            eprintln!("session SSH sur Mac : Trousseau inaccessible, essai sauté");
            return;
        }
        let p = tmp("migr");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(
            &p,
            r#"{"oauth_token": "ancien", "oauth_refresh": "r", "volume": 50}"#,
        )
        .unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(
            (s.oauth_token.as_str(), s.oauth_refresh.as_str(), s.volume),
            ("ancien", "r", 50)
        );
        s.save_to(&p).unwrap();
        assert!(!fs::read_to_string(&p).unwrap().contains("ancien"));
        assert_eq!(Settings::load_from(&p).oauth_token, "ancien");
        // Une ancienne version se reconnecte et réécrit settings.json : son jeton est gardé.
        fs::write(&p, r#"{"oauth_token": "nouveau", "volume": 50}"#).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(
            (s.oauth_token.as_str(), s.oauth_refresh.as_str()),
            ("nouveau", "r")
        );
        // secrets.dat abîmé : ignoré, sans plantage.
        fs::write(secrets_path(&p), format!("{}00ff", commun::dpapi::PREFIX)).unwrap();
        fs::write(&p, "{}").unwrap();
        assert_eq!(Settings::load_from(&p).oauth_token, "");
        // secrets.dat en clair (copié à la main) : refusé.
        fs::write(secrets_path(&p), r#"{"oauth_token": "x"}"#).unwrap();
        assert_eq!(Settings::load_from(&p).oauth_token, "");
    }

    #[test]
    fn fichier_corrompu_ou_absent() {
        let p = tmp("bad");
        assert_eq!(Settings::load_from(&p), Settings::new_install());
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "{ pas du json").unwrap();
        assert_eq!(Settings::load_from(&p), Settings::default());
    }

    #[test]
    fn champs_manquants_et_bornes() {
        let p = tmp("partial");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"volume": 250, "quality": 9}"#).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!((s.volume, s.quality), (130, 4));
        assert!(s.low_latency);
        assert_eq!(
            s.hw_decode, 0,
            "ancien settings.json sans la clé : automatique"
        );
        assert!(
            s.audio_device.is_empty(),
            "ancien settings.json sans la clé : périphérique par défaut"
        );
        assert!(
            s.screenshot_dir.is_empty(),
            "ancien settings.json sans la clé : dossier d'origine"
        );
        fs::write(&p, r#"{"hw_decode": 9}"#).unwrap();
        assert_eq!(
            Settings::load_from(&p).hw_decode,
            2,
            "valeur hors bornes ramenée au maximum"
        );
    }

    /// un fichier d'avant le champ `schema` est lu comme format 0, puis enregistré au format courant.
    #[test]
    fn schema_ancien_fichier() {
        let p = tmp("schema-ancien");
        fs::write(&p, r#"{"volume": 50}"#).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!((s.schema, s.volume), (0, 50));
        assert!(s.extra.is_empty());
        s.save_to(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["schema"], SETTINGS_SCHEMA);
        assert_eq!(Settings::new_install().schema, SETTINGS_SCHEMA);
    }

    /// fichier écrit par une version plus récente. Les champs inconnus et le numéro de format sont réécrits
    /// tels quels, une copie `settings.schema-<n>.json` est faite une fois, les valeurs connues sont lues.
    #[test]
    fn schema_plus_recent_rien_de_perdu() {
        let p = tmp("schema-recent");
        let recent = r#"{"schema": 99, "volume": 40, "futur": {"a": [1, 2]}, "autre_futur": "x", "volume_max": 300}"#;
        fs::write(&p, recent).unwrap();
        let mut s = Settings::load_from(&p);
        assert_eq!((s.schema, s.volume), (99, 40));
        assert_eq!(
            s.extra.len(),
            2,
            "seuls les champs inconnus sont gardés à part : {:?}",
            s.extra
        );
        let copie = p.with_file_name("settings.schema-99.json");
        assert_eq!(fs::read_to_string(&copie).unwrap(), recent);
        s.volume = 45;
        s.save_to(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["schema"], 99);
        assert_eq!(v["volume"], 45);
        assert_eq!(v["futur"], serde_json::json!({"a": [1, 2]}));
        assert_eq!(v["autre_futur"], "x");
        // Relu une deuxième fois : la copie n'est pas refaite (le fichier a changé, la copie garde l'original).
        let _ = Settings::load_from(&p);
        assert_eq!(fs::read_to_string(&copie).unwrap(), recent);
        // Un jeton resté dans settings.json (dont l'ancien `manual_token`) n'est jamais pris pour un champ inconnu.
        fs::write(&p, r#"{"schema": 1, "manual_token": "abc"}"#).unwrap();
        assert!(Settings::load_from(&p).extra.is_empty());
    }
}
