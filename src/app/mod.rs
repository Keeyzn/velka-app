//! État global et navigation entre les vues.
mod compte;
mod fenetre;
mod home;
mod lecteur;
mod maj;
mod messages;
mod mini;
mod multi;
mod nav;
mod onboarding;
mod outils;
mod settings;
mod sidebar;
mod soutien;
mod statuts;
mod watch;

use crate::auth;
use crate::chat::client::{AuthState, ChatClient, ChatEvent};
use crate::chat::emotes::Emotes;
use crate::chat::parse::{ChatMessage, Segment};
use crate::chat::ChatState;
use crate::config::Settings;
use crate::history::History;
use crate::mpv::{GetProc, Player};
use crate::stream::{self, Stream, StreamError, StreamEvent};
use crate::tools::{self, Tool};
use crate::twitch::{self, LiveStatus};
use crate::ui::first_run::{self, ToolRow, ToolStatus};
use crate::ui::theme;
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// Délai entre la saisie et la vérification du statut.
/// Chaînes suivies en direct : toutes les minutes. Liste complète des noms : toutes les 10 minutes.
const FOLLOWS_NAMES_REFRESH: Duration = Duration::from_secs(600);
/// Délai avant de réessayer le renouvellement du jeton Twitch après une coupure réseau.
const TOKEN_RETRY: Duration = Duration::from_secs(60);
const SUGGEST_DEBOUNCE: Duration = Duration::from_millis(250);
const DEBOUNCE: Duration = Duration::from_millis(450);
/// Durée de validité d'un statut en cache.
const CACHE_TTL: Duration = Duration::from_secs(25);
/// Période du rafraîchissement automatique.
/// Le nombre de spectateurs affiché ne change qu'une fois par minute, pour que l'écran ne bouge pas sans cesse.
const VIEWERS_REFRESH: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq)]
enum FollowsState {
    Idle,
    Loading,
    Ready,
    /// Le jeton n'a pas le droit de lire les chaînes suivies (connexion faite avant cette fonction).
    NeedsReconnect,
    Failed,
}

struct RecsData {
    login: String,
    game: Option<String>,
    same_game: Vec<twitch::StreamRec>,
    top: Vec<twitch::StreamRec>,
}

/// Largeur de fenêtre (points, zoom compris) sous laquelle la barre latérale se réduit seule.
const SIDEBAR_AUTO_CLOSE_W: f32 = 840.0;
/// Largeur à partir de laquelle elle se rouvre seule (plus grande que la précédente : pas de va-et-vient au bord).
const SIDEBAR_AUTO_OPEN_W: f32 = 900.0;

/// Aperçu de l'écran de premier lancement (version de développement seulement, variable `VELKA_APERCU_OUTILS`) :
/// les composants sont vus comme absents et « Installer » simule une progression sans rien télécharger ni écrire.
pub(crate) fn apercu_outils() -> bool {
    cfg!(debug_assertions) && std::env::var_os("VELKA_APERCU_OUTILS").is_some()
}

/// Aperçu en thème Clair (`VELKA_APERCU_OUTILS=clair`), sans toucher au réglage enregistré.
pub(crate) fn apercu_clair() -> bool {
    apercu_outils() && std::env::var("VELKA_APERCU_OUTILS").is_ok_and(|v| v == "clair")
}

/// Actions du lecteur qui ont un raccourci clavier : identifiant, libellé et touche par défaut.
pub(crate) const SHORTCUTS: [(&str, &str, egui::Key); 9] = [
    ("pause", "Pause / lecture", egui::Key::Space),
    ("mute", "Couper le son", egui::Key::M),
    ("volume_up", "Volume +", egui::Key::ArrowUp),
    ("volume_down", "Volume -", egui::Key::ArrowDown),
    ("fullscreen", "Plein écran", egui::Key::F),
    ("chat", "Afficher / masquer le chat", egui::Key::C),
    ("pip", "Image dans l'image", egui::Key::P),
    ("screenshot", "Capture d'écran", egui::Key::S),
    ("tech_info", "Infos techniques", egui::Key::I),
];

/// Point de l'historique de navigation (boutons Précédent / Suivant de la souris).
#[derive(Clone, PartialEq)]
enum NavPoint {
    Home,
    Multi,
    Settings,
    Watch(String),
}

/// Au plus cette longueur d'historique est gardée.
const NAV_MAX: usize = 50;
/// Sondes de connexion négatives de suite (une toutes les 3 s) avant d'afficher « La connexion Internet s'est coupée. ».
const NET_PROBE_MAX_FAILS: u8 = 3;

enum Msg {
    Progress(Tool, f32, String),
    Done(Tool, Result<PathBuf, String>),
    Finished,
    Status(u64, LiveStatus),
    Statuses(HashMap<String, LiveStatus>),
    Suggest(String, Vec<twitch::Suggestion>),
    Follows(Result<crate::follows::Snapshot, crate::follows::FollowsError>),
    Recs(
        String,
        Option<String>,
        Vec<twitch::StreamRec>,
        Vec<twitch::StreamRec>,
    ),
    Stream(u64, StreamEvent),
    /// Résultat de la sonde de connexion Internet pendant un live (génération du live, Internet joignable).
    NetProbe(u64, bool),
    Chat(u64, ChatEvent),
    /// Emotes externes et badges chargés pour le chat d'un live (époque du chat).
    ChatExtras(u64, crate::chat::extra::ChatExtras),
    Auth(auth::AuthEvent),
    /// Renouvellement du jeton Twitch en arrière-plan : jeton de renouvellement envoyé, résultat.
    TokenRefreshed(String, Result<auth::Tokens, auth::RefreshError>),
    /// Réponse de la recherche de mise à jour (une erreur réseau est ignorée).
    UpdateInfo(Result<Option<crate::update::Info>, String>),
    UpdateProgress(f32),
    UpdateReady(Result<PathBuf, String>),
    /// Taille du cache d'images et démarrage avec Windows, lus en arrière-plan à l'ouverture des Réglages.
    SettingsInfo(u64, bool),
    /// Démarrage avec Windows écrit en arrière-plan : valeur demandée, réussite.
    StartupSet(bool, bool),
    /// Cache d'images vidé en arrière-plan.
    CacheCleared,
    /// Résultat de la vérification des composants (Streamlink, libmpv).
    ToolsChecked(Result<Vec<tools::ToolReport>, String>),
    /// Diagnostic pour le support, composé en arrière-plan : à copier dans le presse-papiers.
    Diagnostic(String),
    /// Multistream : statut d'une chaîne ajoutée (numéro de la vignette).
    MultiStatus(u64, LiveStatus),
    /// Multistream : événement du Streamlink d'une vignette.
    MultiStream(u64, StreamEvent),
}

/// Action destructive qui attend la confirmation de l'utilisateur.
#[derive(PartialEq, Clone, Copy)]
enum Confirm {
    Reset,
    Uninstall,
}

#[derive(PartialEq, Clone, Copy)]
enum View {
    FirstRun,
    Home,
    /// Page « Multistream » : jusqu'à 4 lives ensemble (`multi.rs`).
    Multi,
    Watch,
    Settings,
}

#[derive(PartialEq, Clone)]
enum WatchState {
    Checking,
    Starting,
    /// Changement de qualité en cours : le flux est relancé.
    Switching,
    /// Actualisation demandée par l'utilisateur : même relance, autre libellé.
    Refreshing,
    Playing,
    Error(String),
}

struct Watch {
    login: String,
    state: WatchState,
    status: Option<LiveStatus>,
    /// Streamlink signale une coupure du flux : pastille « Reprise dans un instant… ».
    on_break: bool,
}

impl Watch {
    /// Le live est arrêté faute de connexion Internet (l'un des deux messages de coupure) : reprise automatique au retour.
    fn net_lost(&self) -> bool {
        matches!(&self.state, WatchState::Error(m)
            if m.as_str() == crate::i18n::tr("La connexion Internet s'est coupée.") || m.as_str() == crate::i18n::tr("Pas de connexion Internet."))
    }
}

pub struct VelkaApp {
    settings: Settings,
    saved_settings: Settings,
    history: History,
    view: View,
    prev_view: View,
    rows: Vec<ToolRow>,
    busy: bool,
    /// Redémarrage demandé à cet instant : voile sombre et barre de chargement en boucle jusqu'à la fermeture.
    restarting: Option<Instant>,
    /// Dernière vérification des versions des composants, dernières versions connues (et quand) et erreurs de la
    /// dernière mise à jour (composant, message).
    tools_check: tools::ToolsCheck,
    tools_latest: Vec<(Tool, String)>,
    tools_latest_at: Option<Instant>,
    tool_errors: Vec<(Tool, String)>,
    /// La section « Avancé et Système » était affichée à l'image précédente (pour contrôler les composants à chaque ouverture).
    advanced_visible_prev: bool,
    /// Images restantes pendant lesquelles les Réglages restent collés en bas (zone de couleur personnalisée qui s'ouvre).
    advanced_scroll: u8,
    /// Dernière position de défilement des Réglages (en mémoire seulement : repartie de zéro à chaque lancement).
    settings_scroll_mem: f32,
    /// Images restantes pendant lesquelles on remet cette position à la réouverture des Réglages.
    settings_restore_scroll: u8,
    /// Le contrôle du premier lancement après mise à jour de l'application a été examiné.
    tools_boot_checked: bool,
    /// La chaîne « ouverte au démarrage » a déjà été vérifiée pour cette session.
    startup_ch_done: bool,
    /// Le champ « chaîne au démarrage » des Réglages est en cours de saisie (l'autocomplétion lui est réservée), et le
    /// pointeur est sur sa liste de suggestions.
    startup_editing: bool,
    startup_hover: bool,
    /// Interrupteur « chaîne au démarrage » activé mais aucune chaîne encore choisie ; texte en cours de saisie ;
    /// focus à donner au champ ; dernier retour visuel (moment, positif ?) : contour vert (ajout) ou rouge (suppression) du champ.
    startup_armed: bool,
    startup_draft: String,
    startup_focus: bool,
    startup_feedback: Option<(Instant, bool)>,
    /// Redémarrage demandé pour mettre à jour libmpv : une fois les versions contrôlées, tout ce qui est à mettre à
    /// jour l'est automatiquement.
    tools_update_after_check: bool,
    // Accueil
    input: String,
    last_input: String,
    input_changed: Instant,
    status_cache: HashMap<String, (Instant, LiveStatus)>,
    /// Dernière mise à jour du nombre de spectateurs affiché, par chaîne.
    viewer_stamp: HashMap<String, Instant>,
    status_inflight: bool,
    /// Chaînes suivies sur le compte Twitch connecté.
    followed: Vec<String>,
    /// Jeu choisi dans la barre de l'accueil : ses lives passent en premier.
    home_game: Option<String>,
    /// Recommandations quand un live est hors ligne ou terminé : chaîne, jeu, lives du même jeu, lives les plus regardés.
    recs: Option<RecsData>,
    recs_inflight: bool,
    /// Demande de défilement vers le bas dans la liste de la barre latérale.
    sidebar_scroll_down: bool,
    /// Demande de défilement vers le bas dans la page d'accueil (flèche en bas des cartes).
    home_scroll_down: bool,
    /// Liste des chaînes de la barre latérale : une seule ligne est un arrêt de Tab (la dernière utilisée), les flèches
    /// haut et bas passent d'une ligne à l'autre. `side_focus_to` : ligne à mettre au premier plan à l'image suivante.
    side_rove: usize,
    /// Fenêtre trop étroite pour la barre latérale ouverte (repli automatique, hors réglage de l'utilisateur).
    side_auto: bool,
    /// L'utilisateur a ouvert la barre à la main malgré le manque de place : elle reste ouverte jusqu'à ce que la place revienne.
    side_force: bool,
    side_focus_to: Option<usize>,
    /// Groupes de la barre latérale repliés d'un clic sur leur titre : chaînes suivies sur Twitch, favoris (le temps de la
    /// session ; tout est déplié au lancement).
    side_collapsed: [bool; 2],
    /// Largeur actuelle de la barre latérale (animée vers 236 px ou 76 px).
    sidebar_w: f32,
    /// Placer le curseur dans la recherche dès que le champ existe (la barre réduite s'agrandit en animation).
    focus_search: bool,
    /// Liste des langues affichée pendant cette image (il faut alors les polices chinoise et coréenne).
    lang_list_shown: bool,
    /// Page affichée sous l'overlay des Réglages (celle d'où on l'a ouvert : accueil ou live).
    overlay_base: View,
    /// Coins arrondis de Windows 11 déjà demandés pour la fenêtre sans bordure.
    corners_done: bool,
    /// Raccourci en cours de réaffectation dans les réglages.
    capture_shortcut: Option<&'static str>,
    /// Fermeture voulue (menu « Quitter », mise à jour, redémarrage, désinstallation) : jamais interceptée par l'option
    /// « Réduire dans la zone de notification à la fermeture ».
    quitting: bool,
    /// Icône permanente de la zone de notification en place, et langue de ses libellés.
    tray_on: bool,
    tray_lang: u8,
    /// Fenêtre cachée dans la zone de notification (par la croix de fermeture), depuis quand.
    hidden_in_tray: Option<Instant>,
    /// Onglet ouvert dans les Réglages (pour la session).
    settings_tab: u8,
    /// Texte de la recherche des Réglages.
    settings_query: String,
    /// Taille du cache d'images et état du démarrage avec Windows, lus à l'ouverture des réglages.
    cache_size: Option<u64>,
    /// Dernier zoom appliqué à la fenêtre (pour détecter Ctrl + / Ctrl − et appliquer le curseur au relâchement).
    zoom_applied: f32,
    /// Couleur d'accent en cours d'édition dans Réglages > Apparence (pas encore appliquée si trop peu contrastée),
    /// et son code `#RRGGBB` affiché.
    accent_draft: Option<[u8; 3]>,
    accent_hsv: Option<egui::ecolor::Hsva>,
    /// Blocs repliés des Réglages : « Chemins personnalisés (facultatif) » (section Avancé et Système) et liste des langues.
    paths_open: bool,
    lang_open: bool,
    colors_open: bool,
    /// Changement de thème en cours (fondu par capture de l'ancien affichage, voir `theme_fade_ui`).
    theme_fade: Option<settings::ThemeShot>,
    /// Étape courante du didacticiel de bienvenue.
    onb_step: usize,
    /// Le didacticiel se ferme (fondu en cours) : il est marqué comme vu à la fin du fondu.
    onb_closing: bool,
    /// Images restantes pendant lesquelles on place le focus sur la recherche des Réglages (après leur ouverture).
    settings_focus_frames: u8,
    /// Réglages en fenêtre étroite : le menu des sections (réduit en barre d'icônes) est déplié par le burger.
    settings_nav_open: bool,
    settings_info_inflight: bool,
    startup_on: Option<bool>,
    /// Écriture du démarrage avec Windows et vidage du cache en cours (fils de fond).
    startup_busy: bool,
    cache_clearing: bool,
    nav_cur: NavPoint,
    nav_back: Vec<NavPoint>,
    nav_fwd: Vec<NavPoint>,
    follows_state: FollowsState,
    follows_at: Option<Instant>,
    follows_names_at: Option<Instant>,
    /// Autocomplétion de la recherche : fragment voulu, depuis quand, dernier demandé, résultats.
    suggest_want: String,
    suggest_want_at: Instant,
    suggest_for: String,
    suggest_inflight: bool,
    suggestions: Vec<twitch::Suggestion>,
    last_status_req: Option<Instant>,
    /// Rapport du dernier plantage (`crash.txt`), lu au démarrage ; `crash_copied` : copié dans le presse-papiers.
    crash_report: Option<String>,
    crash_copied: bool,
    /// Diagnostic (version, Windows, carte graphique, composants) copié dans le presse-papiers.
    diag_copied: bool,
    /// Première actualisation des statuts au démarrage : décalée de 0 à 4 s au hasard, pour que des milliers de
    /// clients lancés en même temps (après une coupure, au démarrage de Windows) ne tapent pas Twitch ensemble.
    status_start: Instant,
    /// Part aléatoire (0 à 10 % de l'intervalle) ajoutée à la prochaine actualisation périodique : les clients ne se
    /// resynchronisent pas.
    poll_jitter: Duration,
    /// Dernière action de l'utilisateur (souris, clavier) : intervalle plus long quand l'application est au repos.
    last_activity: Instant,
    /// Dernière validation horaire du jeton Twitch (indépendante de la vue, exigence Twitch).
    identity_at: Option<Instant>,
    // Lecture
    get_proc: Option<GetProc>,
    player: Option<Player>,
    stream: Option<Stream>,
    watch: Option<Watch>,
    paused: bool,
    muted: bool,
    controls_seen: Instant,
    /// Connexion Twitch en cours : code à confirmer, annulation, dernière erreur.
    auth_code: Option<auth::DeviceCode>,
    auth_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    auth_error: Option<String>,
    /// Instant où l'utilisateur a lui-même annulé la connexion (message « Connexion annulée », quelques secondes).
    auth_cancelled_at: Option<Instant>,
    /// Confirmation en attente dans les réglages (réinitialiser / désinstaller).
    confirm: Option<Confirm>,
    /// « Se déconnecter » armé (second clic pour confirmer, 5 s).
    logout_armed: Option<Instant>,
    /// Même confirmation pour le bouton « Se déconnecter » de l'onglet Compte, indépendante de celle du menu.
    logout_armed_account: Option<Instant>,
    /// Renouvellement du jeton Twitch en cours, et dernier échec réseau (pour ne pas réessayer en boucle).
    token_refresh_inflight: bool,
    token_retry_at: Option<Instant>,
    /// Mise à jour : version proposée, avancement du téléchargement, dernière erreur, dernière recherche.
    update_info: Option<crate::update::Info>,
    update_progress: Option<f32>,
    update_error: Option<String>,
    update_checked: Option<Instant>,
    /// Une recherche de mise à jour est en cours ; la dernière a été demandée par le bouton ; son résultat.
    update_inflight: bool,
    update_manual: bool,
    update_check: maj::UpdateCheck,
    /// Bandeau de soutien affiché pendant cette session (décidé au démarrage, `soutien.rs`).
    support_visible: bool,
    uninstalling: bool,
    pending_channel: Option<String>,
    pending_pip: bool,
    /// Réglages avant / après les options de la ligne de commande (valables pour cette session seulement).
    cli_guard: Option<(Settings, Settings)>,
    /// Dernier changement de volume (pour l'indication affichée sur la vidéo).
    volume_toast: Option<Instant>,
    /// Message bref affiché sur la vidéo (capture d'écran…).
    info_toast: Option<(Instant, String)>,
    /// Instant où l'historique a été effacé depuis les Réglages (message « Historique effacé. » quelques secondes).
    history_cleared: Option<Instant>,
    /// Capture d'écran en cours d'écriture (fichier attendu, heure de la demande).
    shot_check: Option<(PathBuf, Instant)>,
    /// Fenêtre de choix du dossier des captures ouverte (réponse attendue).
    shot_pick: Option<Receiver<Option<PathBuf>>>,
    /// Dernier contrôle du dossier des captures choisi (Réglages).
    shot_dir_check: Instant,
    /// Le dossier des captures choisi a disparu : message dans les Réglages jusqu'au prochain choix.
    shot_dir_lost: bool,
    /// Le dossier choisi dans la fenêtre n'accepte pas l'écriture : refusé, message dans les Réglages.
    shot_dir_err: bool,
    /// Panneau d'infos techniques du lecteur (touche I), pour la session seulement.
    tech_info: bool,
    /// Valeurs du panneau d'infos techniques, relues deux fois par seconde.
    tech_cache: Option<(Instant, Vec<(&'static str, String)>)>,
    /// Fin du minuteur de veille pour le live en cours.
    sleep_deadline: Option<Instant>,
    /// Filtre d'agrandissement d'origine de mpv (lu au premier réglage de l'image).
    scale_default: Option<String>,
    /// Netteté ou agrandissement déjà posés sur le lecteur (sinon mpv n'est pas touché).
    video_opts_set: bool,
    /// Valeur d'origine de `hwdec` (celle de `mpv::MPV_OPTIONS`), relue avant la première modification par le réglage.
    hwdec_default: Option<String>,
    /// Vrai une fois `hwdec` modifié : permet de restaurer l'origine en revenant sur « Automatique ».
    hwdec_set: bool,
    /// Périphériques de sortie audio connus de mpv (nom, description), relus régulièrement (voir `tick_audio_device`).
    audio_devices: Vec<(String, String)>,
    /// Dernière lecture de la liste des périphériques.
    audio_dev_check: Instant,
    /// Vrai une fois `audio-device` modifié : permet de revenir à `auto` en retrouvant le choix par défaut.
    audio_dev_set: bool,
    /// Le périphérique choisi a disparu : message discret dans les Réglages jusqu'au prochain choix.
    audio_dev_lost: bool,
    /// Le lecteur a déjà été demandé depuis les Réglages pour lister les périphériques (un seul essai).
    audio_dev_tried: bool,
    /// Emotes externes et badges du live en cours.
    chat_extras: Option<std::sync::Arc<crate::chat::extra::ChatExtras>>,
    /// Dernière demande d'emotes externes/badges : (époque du chat, emotes, badges).
    extras_req: Option<(u64, bool, bool)>,
    volume_acc: f32,
    pending_click: Option<Instant>,
    /// Ligne choisie au clavier dans les suggestions de recherche.
    palette_sel: Option<usize>,
    palette_hover: bool,
    anim: Option<mini::ViewAnim>,
    /// Mini-lecteur tenu à la souris ou en train de revenir vers un bord (position libre, vitesse).
    mini_motion: Option<mini::MiniMotion>,
    /// Dernier rectangle du mini-lecteur (départ de l'animation vers le lecteur normal).
    mini_last: Option<egui::Rect>,
    /// Redimensionnement du mini-lecteur en cours (partie saisie, prise, coin fixe) et largeur animée vers la taille d'origine.
    mini_resize: Option<mini::MiniResize>,
    mini_w_anim: Option<f32>,
    /// Le mini-lecteur a été déplacé depuis le dernier appui (seuil de 4 px dépassé) : le relâchement n'est pas un clic.
    mini_dragged: bool,
    /// Rectangle cible de la vidéo (sa place finale, sans l'animation en cours).
    last_video_rect: egui::Rect,
    /// Rectangle réellement dessiné cette image (celui de l'animation en cours) : point de départ si on rebascule.
    shown_video_rect: egui::Rect,
    cur_anim: Option<(egui::Rect, f32)>,
    fullscreen: bool,
    pip: bool,
    pip_restore: Option<(egui::Vec2, Option<egui::Pos2>)>,
    /// Fenêtre agrandie avant le PiP : elle l'est de nouveau à la sortie.
    pip_was_maximized: bool,
    /// PiP gardé au premier plan (vrai à chaque entrée en PiP, comme avant ; l'icône « punaise » de la barre du haut du
    /// PiP le coupe ou le remet pour la durée de ce PiP).
    pip_on_top: bool,
    /// macOS : PiP demandé depuis le plein écran, en attente de la fin de sa sortie (début de l'attente, dernière
    /// taille vue, moment où elle a changé). Voir `VelkaApp::tick_pip_wait`.
    pip_wait: Option<(Instant, egui::Vec2, Instant)>,
    /// Dernière vérification du format de la fenêtre PiP.
    pip_fit_at: Instant,
    geometry_hold: Instant,
    last_save: Instant,
    // Chat
    chat: ChatState,
    chat_client: Option<ChatClient>,
    emotes: Emotes,
    avatars: crate::avatars::Avatars,
    thumbs: crate::thumbs::Thumbs,
    chat_jump: bool,
    chat_input: String,
    chat_epoch: u64,
    generation: u64,
    /// Sonde de connexion pendant un live : dernier envoi, sonde en cours, échecs de suite.
    net_probe_at: Option<Instant>,
    net_probe_busy: bool,
    net_probe_fails: u8,
    // Multistream
    /// Lives de la page « Multistream » (au plus `multi::MULTI_MAX`), dans l'ordre d'ajout.
    multi: Vec<multi::Slot>,
    /// Dernier numéro de vignette donné (jamais réutilisé).
    multi_next: u64,
    /// Vignettes qui ont le son (une, ou plusieurs avec le réglage « Cumuler le son des lives en Multistream »), la plus récente en
    /// dernier ; et l'heure du dernier changement (contour d'accent affiché quelques secondes).
    multi_audio: Vec<u64>,
    multi_audio_at: Option<Instant>,
    /// Lives qui avaient le son avant « Couper tous les sons » (rétablis par « Réactiver le son »).
    multi_muted: Vec<u64>,
    /// Live tenu à la souris (glisser-déposer) et point saisi (depuis le coin de sa vignette) ; heure du dernier
    /// échange de places (les vignettes glissent vers leur nouvelle place).
    multi_drag: Option<(u64, egui::Vec2)>,
    /// Bouton maintenu sur un live (numéro, point d'appui) : il s'attrape dès que la souris bouge (`multi::DRAG_SLOP`).
    multi_press: Option<(u64, egui::Pos2)>,
    multi_swap_at: Option<Instant>,
    /// Lecteurs des lives retirés, détruits un par image (`multi_free_one`) pour ne pas figer l'interface.
    multi_dying: Vec<Player>,
    /// Saisie du champ « Ajouter une chaîne ».
    multi_input: String,
    /// Message bref sous le champ (texte français, traduit à l'affichage) et son heure.
    multi_msg: Option<(Instant, &'static str)>,
    /// Lives mis de côté quand un live est ouvert seul (ou la fenêtre cachée), et celui qui avait le son : relancés au
    /// retour sur la page (en mémoire seulement).
    multi_saved: Vec<String>,
    multi_saved_audio: Vec<String>,
    /// Champ « Ajouter une chaîne » en cours de saisie (l'autocomplétion lui est alors réservée), pointeur sur sa liste,
    /// ligne choisie au clavier dans la liste.
    multi_editing: bool,
    multi_hover: bool,
    multi_sel: Option<usize>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl VelkaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut settings = Settings::load();
        // Le minuteur de veille ne se garde pas d'un lancement à l'autre : il repart sur « Désactivé ».
        settings.sleep_timer = 0;
        if apercu_clair() {
            settings.theme = 2; // Clair, en mémoire seulement (l'aperçu n'écrit aucun réglage)
        }
        let (tx, rx) = channel();
        // Un clic sur une notification de live réveille l'interface, qui ouvre alors la chaîne (voir `ui`).
        crate::procs::set_notify_wake({
            let ctx = cc.egui_ctx.clone();
            move || ctx.request_repaint()
        });
        let rows: Vec<ToolRow> = Tool::ALL
            .iter()
            .map(|&tool| {
                let status = match tools::detect(tool, custom_path(&settings, tool))
                    .filter(|_| !apercu_outils())
                {
                    Some(p) => ToolStatus::Ready(p.display().to_string()),
                    None => ToolStatus::Missing,
                };
                ToolRow { tool, status }
            })
            .collect();
        let all_ready = rows
            .iter()
            .all(|r| matches!(r.status, ToolStatus::Ready(_)));
        crate::i18n::set(settings.language);
        apply_scroll_speed(&cc.egui_ctx, settings.scroll_speed);
        apply_access(&cc.egui_ctx, &settings);
        let zoom0 = settings.ui_zoom;
        crate::ui::theme::install_fonts(&cc.egui_ctx);
        crate::ui::fonts::ensure(&cc.egui_ctx, false);
        apply_animations(&cc.egui_ctx, &settings);
        apply_on_top(&cc.egui_ctx, settings.on_top);
        // Diagnostic pour le support : carte graphique lue maintenant (le contexte OpenGL est actif), puis
        // version, Windows, carte graphique et composants écrits dans le journal, en arrière-plan.
        if let Some(gl) = cc.gl.as_ref() {
            crate::diag::set_gpu(gl);
        }
        // OpenGL avant 3.0 : la fenêtre s'ouvre mais la vidéo risque de rester noire ; prévenu une fois par lancement,
        // sans bloquer le démarrage.
        if crate::diag::gl_too_old() {
            crate::logging::log("OpenGL antérieur à 3.0 : la vidéo peut rester noire");
            if !crate::smoke::enabled() {
                let text = crate::i18n::tr("La carte graphique n'annonce pas OpenGL 3.0 : la vidéo peut rester noire. Mettez à jour le pilote de la carte graphique. Sur un bureau à distance ou une machine virtuelle, l'affichage peut ne pas être pris en charge.");
                std::thread::spawn(move || crate::procs::message_box(text, true));
            }
        }
        {
            let (streamlink, libmpv) = (
                settings.streamlink_path.clone(),
                settings.libmpv_path.clone(),
            );
            std::thread::spawn(move || {
                for line in crate::diag::collect(&streamlink, &libmpv).lines() {
                    crate::logging::log(&format!("Diagnostic : {line}"));
                }
            });
        }
        let now = Instant::now();
        if settings.fixed_volume_on {
            settings.volume = settings.fixed_volume.min(settings.volume_max);
        }
        let start_muted = settings.start_muted;
        let home_game0 = (settings.remember_game && !settings.home_game.is_empty())
            .then(|| settings.home_game.clone());
        let sidebar_w0 = if settings.sidebar_collapsed {
            76.0
        } else {
            236.0
        };
        let mut app = Self {
            saved_settings: settings.clone(),
            settings,
            history: History::load(),
            view: if all_ready {
                View::Home
            } else {
                View::FirstRun
            },
            prev_view: View::Home,
            rows,
            busy: false,
            restarting: None,
            tools_check: tools::ToolsCheck::Idle,
            tools_latest: Vec::new(),
            tools_latest_at: None,
            tool_errors: Vec::new(),
            advanced_visible_prev: false,
            advanced_scroll: 0,
            settings_scroll_mem: 0.0,
            settings_restore_scroll: 0,
            tools_boot_checked: false,
            startup_ch_done: false,
            startup_editing: false,
            startup_hover: false,
            startup_armed: false,
            startup_draft: String::new(),
            startup_focus: false,
            startup_feedback: None,
            tools_update_after_check: tools::take_update_pending(),
            input: String::new(),
            last_input: String::new(),
            input_changed: now,
            status_cache: HashMap::new(),
            viewer_stamp: HashMap::new(),
            status_inflight: false,
            followed: Vec::new(),
            home_game: home_game0,
            recs: None,
            recs_inflight: false,
            sidebar_scroll_down: false,
            home_scroll_down: false,
            side_rove: 0,
            side_collapsed: [false, false],
            side_auto: false,
            side_force: false,
            side_focus_to: None,
            sidebar_w: sidebar_w0,
            focus_search: false,
            lang_list_shown: false,
            overlay_base: View::Home,
            corners_done: false,
            capture_shortcut: None,
            quitting: false,
            tray_on: false,
            tray_lang: 0,
            hidden_in_tray: None,
            settings_tab: 0,
            settings_query: String::new(),
            cache_size: None,
            zoom_applied: zoom0,
            accent_draft: None,
            accent_hsv: None,
            paths_open: false,
            lang_open: false,
            colors_open: false,
            theme_fade: None,
            onb_step: 0,
            onb_closing: false,
            settings_focus_frames: 0,
            settings_nav_open: false,
            settings_info_inflight: false,
            startup_on: None,
            startup_busy: false,
            cache_clearing: false,
            nav_cur: NavPoint::Home,
            nav_back: Vec::new(),
            nav_fwd: Vec::new(),
            follows_state: FollowsState::Idle,
            follows_at: None,
            follows_names_at: None,
            suggest_want: String::new(),
            suggest_want_at: now,
            suggest_for: String::new(),
            suggest_inflight: false,
            suggestions: Vec::new(),
            last_status_req: None,
            crash_report: crate::crash::read(),
            crash_copied: false,
            diag_copied: false,
            status_start: now + crate::net::random_delay(Duration::from_secs(4)),
            poll_jitter: Duration::ZERO,
            last_activity: now,
            identity_at: None,
            get_proc: cc.get_proc_address.clone(),
            player: None,
            stream: None,
            watch: None,
            paused: false,
            muted: start_muted,
            controls_seen: now,
            auth_code: None,
            auth_cancel: None,
            auth_error: None,
            auth_cancelled_at: None,
            confirm: None,
            logout_armed: None,
            logout_armed_account: None,
            token_refresh_inflight: false,
            token_retry_at: None,
            update_info: None,
            update_progress: None,
            update_error: None,
            update_checked: None,
            update_inflight: false,
            update_manual: false,
            update_check: maj::UpdateCheck::Idle,
            support_visible: false,
            uninstalling: false,
            pending_channel: None,
            pending_pip: false,
            cli_guard: None,
            volume_toast: None,
            info_toast: None,
            history_cleared: None,
            shot_check: None,
            shot_pick: None,
            shot_dir_check: Instant::now(),
            shot_dir_lost: false,
            shot_dir_err: false,
            tech_info: false,
            tech_cache: None,
            sleep_deadline: None,
            scale_default: None,
            video_opts_set: false,
            hwdec_default: None,
            hwdec_set: false,
            audio_devices: Vec::new(),
            audio_dev_check: Instant::now(),
            audio_dev_set: false,
            audio_dev_lost: false,
            audio_dev_tried: false,
            chat_extras: None,
            extras_req: None,
            volume_acc: 0.0,
            pending_click: None,
            palette_sel: None,
            palette_hover: false,
            anim: None,
            mini_motion: None,
            mini_last: None,
            mini_resize: None,
            mini_w_anim: None,
            mini_dragged: false,
            last_video_rect: egui::Rect::NOTHING,
            shown_video_rect: egui::Rect::NOTHING,
            cur_anim: None,
            fullscreen: false,
            pip: false,
            pip_restore: None,
            pip_was_maximized: false,
            pip_on_top: true,
            pip_wait: None,
            pip_fit_at: now,
            geometry_hold: now,
            last_save: now,
            chat: ChatState::new(),
            chat_client: None,
            emotes: Emotes::new(&cc.egui_ctx),
            avatars: crate::avatars::Avatars::new(&cc.egui_ctx),
            thumbs: crate::thumbs::Thumbs::new(&cc.egui_ctx),
            chat_jump: false,
            chat_input: String::new(),
            chat_epoch: 0,
            generation: 0,
            net_probe_at: None,
            net_probe_busy: false,
            net_probe_fails: 0,
            multi: Vec::new(),
            multi_next: 0,
            multi_audio: Vec::new(),
            multi_audio_at: None,
            multi_muted: Vec::new(),
            multi_drag: None,
            multi_press: None,
            multi_swap_at: None,
            multi_dying: Vec::new(),
            multi_input: String::new(),
            multi_msg: None,
            multi_saved: Vec::new(),
            multi_saved_audio: Vec::new(),
            multi_editing: false,
            multi_hover: false,
            multi_sel: None,
            tx,
            rx,
        };
        app.apply_cli(&cc.egui_ctx, crate::cli::current());
        app.init_support_reminder();
        app
    }

    /// Remet réglages, favoris et historique à zéro (les outils restent installés).
    fn reset_all(&mut self, ctx: &egui::Context) {
        if self.watch.is_some() {
            self.stop_watch(ctx);
        }
        self.settings = Settings::default();
        self.cli_guard = None;
        self.history = History::default();
        if let Err(e) = self.history.save() {
            crate::logging::log(&format!("Historique non enregistré : {e}"));
        }
        self.status_cache.clear();
        crate::i18n::set(self.settings.language);
        apply_access(ctx, &self.settings);
        apply_on_top(ctx, self.settings.on_top);
        if let Err(e) = self.settings.save() {
            crate::logging::log(&format!("Réglages non enregistrés : {e}"));
        }
        self.saved_settings = self.settings.clone();
        self.redetect_tools();
        self.view = View::Settings;
    }

    /// Ferme proprement l'application puis la relance. Le live est arrêté d'abord (flux et lecteur libérés) ; la
    /// fermeture normale enregistre les réglages et libère l'instance unique à la fin du processus. Un petit processus
    /// caché attend cette fin avant de relancer (voir `update::relaunch`). Refusé pendant une installation d'outil.
    fn restart_app(&mut self, ctx: &egui::Context) {
        if self.busy || self.restarting.is_some() {
            return;
        }
        if let Err(e) = crate::update::relaunch() {
            crate::logging::log(&format!("Redémarrage impossible : {e}"));
            return;
        }
        if self.watch.is_some() {
            self.stop_watch(ctx);
        }
        self.multi_stop_all();
        self.player = None;
        crate::logging::log("Redémarrage de l'application demandé");
        // La fenêtre se ferme après `RESTART_SHOW` (voir `restart_overlay`) : le temps de voir la barre de chargement.
        self.restarting = Some(Instant::now());
        ctx.request_repaint();
    }

    /// Redémarrage en cours : voile noir sur toute la fenêtre (clics et clavier bloqués) et barre de chargement orange
    /// qui tourne en boucle ; la fenêtre se ferme quand `RESTART_SHOW` est écoulé et le processus caché relance l'appli.
    fn restart_overlay(&mut self, ctx: &egui::Context) {
        const RESTART_SHOW: Duration = Duration::from_millis(1400);
        let Some(start) = self.restarting else {
            return;
        };
        let screen = ctx.content_rect();
        let id = egui::Id::unique("restart_overlay");
        ctx.memory_mut(|m| m.set_modal_layer(egui::LayerId::new(egui::Order::Tooltip, id)));
        let k = (start.elapsed().as_secs_f32() / 0.18).clamp(0.0, 1.0);
        egui::Area::new(id)
            .order(egui::Order::Tooltip)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
                let p = ui.painter();
                p.rect_filled(
                    r,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_black_alpha((225.0 * k) as u8),
                );
                // Barre indéterminée : un segment orange (couleur d'accent) traverse la piste en boucle, sans fin.
                let track = egui::Rect::from_center_size(r.center(), egui::vec2(240.0, 6.0));
                p.rect_filled(
                    track,
                    egui::CornerRadius::same(3),
                    egui::Color32::from_white_alpha((36.0 * k) as u8),
                );
                let phase = ((ctx.input(|i| i.time) * 0.9) % 1.0) as f32;
                let seg = track.width() * 0.35;
                let x0 = track.left() - seg + phase * (track.width() + seg);
                let part = egui::Rect::from_min_max(
                    egui::pos2(x0.max(track.left()), track.top()),
                    egui::pos2((x0 + seg).min(track.right()), track.bottom()),
                );
                if part.width() > 0.0 {
                    p.rect_filled(
                        part,
                        egui::CornerRadius::same(3),
                        theme::ACCENT().gamma_multiply(k),
                    );
                }
            });
        ctx.request_repaint();
        if start.elapsed() >= RESTART_SHOW {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Supprime toutes les données de l'application, puis la ferme.
    fn uninstall(&mut self, ctx: &egui::Context) {
        if self.watch.is_some() {
            self.stop_watch(ctx);
        }
        self.multi_stop_all();
        self.player = None;
        self.uninstalling = true;
        if crate::uninstall::spawn_cleanup(&crate::logging::data_dir()) {
            crate::logging::log("Désinstallation demandée");
            crate::procs::forget_start_menu_shortcut();
            // Plus de démarrage avec Windows : sinon Windows chercherait encore l'application après la suppression.
            crate::procs::set_startup(false);
        } else {
            crate::logging::log("Désinstallation refusée : dossier non sûr");
            self.uninstalling = false;
            return;
        }
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Applique les options de la ligne de commande (au démarrage ou depuis une seconde instance).
    fn apply_cli(&mut self, ctx: &egui::Context, cli: &crate::cli::Cli) {
        let before = self
            .cli_guard
            .as_ref()
            .map_or_else(|| self.settings.clone(), |(b, _)| b.clone());
        cli.apply(&mut self.settings);
        self.cli_guard = Some((before, self.settings.clone()));
        if cli.on_top {
            apply_on_top(ctx, true);
        }
        if cli.channel.is_some() {
            self.pending_channel.clone_from(&cli.channel);
            self.pending_pip = cli.pip;
        } else if cli.pip && self.watch.is_some() {
            self.pending_pip = true;
        }
    }

    /// Ouvre la chaîne demandée dès que l'application est prête (outils installés).
    fn run_pending(&mut self, ctx: &egui::Context) {
        // Rafraîchissement lent : lit les demandes d'une seconde instance même en veille.
        ctx.request_repaint_after(Duration::from_secs(2));
        if let Some(cli) = crate::cli::take_inbox() {
            for w in &cli.warnings {
                crate::logging::log(&format!("Ligne de commande : {w}"));
            }
            self.apply_cli(ctx, &cli);
        }
        if self.view != View::FirstRun {
            if let Some(login) = self.pending_channel.take() {
                self.start_watch(ctx, login);
            }
        }
        if self.pending_pip && self.view == View::Watch && !self.pip {
            self.pending_pip = false;
            self.enter_pip(ctx);
        }
        self.tick_pip_wait(ctx);
    }

    // ---------- Activation ----------

    // ---------- Outils ----------

    /// Photos de profil des favoris, des récents et de la chaîne en cours (chacune une seule fois).
    fn request_avatars(&mut self) {
        let logins = self
            .settings
            .favorites
            .iter()
            .chain(self.history.items.iter());
        for l in logins {
            self.avatars.request(l);
        }
        if let Some(w) = &self.watch {
            self.avatars.request(&w.login);
        }
        for l in self.live_followed(20) {
            self.avatars.request(&l);
        }
    }

    // ---------- Navigation (Précédent / Suivant) ----------

    /// Bouton « Retour » : page précédente de l'historique, ou l'accueil s'il n'y en a pas.
    pub(super) fn go_back(&mut self, ctx: &egui::Context) {
        match self.nav_back.pop() {
            Some(target) => {
                self.nav_fwd.push(self.nav_cur.clone());
                self.nav_goto(ctx, target);
            }
            None => self.nav_goto(ctx, NavPoint::Home),
        }
    }

    // ---------- Recommandations ----------

    /// Chaînes à proposer quand `login` est hors ligne ou vient de se terminer (voir `twitch::rank_recommendations`).
    /// Lance au besoin la recherche des lives du même jeu et des lives les plus regardés.
    pub(super) fn recommendations(
        &mut self,
        ctx: &egui::Context,
        login: &str,
        known_game: Option<&str>,
    ) -> Vec<twitch::StreamRec> {
        let have = self.recs.as_ref().is_some_and(|r| r.login == login);
        if !have && !self.recs_inflight {
            self.recs_inflight = true;
            let (tx, ctx, login, game) = (
                self.tx.clone(),
                ctx.clone(),
                login.to_string(),
                known_game.map(str::to_string),
            );
            std::thread::spawn(move || {
                let game = game.or_else(|| twitch::fetch_channel_game(&login));
                let same = game
                    .as_deref()
                    .map(|g| twitch::fetch_streams_list(Some(g)))
                    .unwrap_or_default();
                let top = twitch::fetch_streams_list(None);
                let _ = tx.send(Msg::Recs(login, game, same, top));
                ctx.request_repaint();
            });
        }
        let (game, same, top) = match self.recs.as_ref().filter(|r| r.login == login) {
            Some(r) => (
                r.game.clone().or_else(|| known_game.map(str::to_string)),
                r.same_game.clone(),
                r.top.clone(),
            ),
            None => (known_game.map(str::to_string), Vec::new(), Vec::new()),
        };
        let mut seen: Vec<&String> = Vec::new();
        let mut followed_live: Vec<twitch::StreamRec> = Vec::new();
        for l in self.settings.favorites.iter().chain(self.followed.iter()) {
            if seen.contains(&l) {
                continue;
            }
            seen.push(l);
            if let Some((_, LiveStatus::Live { viewers, game, .. })) = self.status_cache.get(l) {
                followed_live.push(twitch::StreamRec {
                    login: l.clone(),
                    viewers: *viewers,
                    game: game.clone(),
                });
            }
        }
        twitch::rank_recommendations(
            game.as_deref(),
            &followed_live,
            &same,
            &top,
            login,
            self.settings.recs_limit(),
        )
    }

    // ---------- Autocomplétion ----------

    // ---------- Statut live ----------

    // ---------- Lecture ----------

    // ---------- Plein écran, PiP, raccourcis ----------

    /// Raccourcis : Espace, M, flèches, F, C, P, Échap (sans interférer avec la saisie).
    /// Touche d'une action : celle choisie dans les réglages, sinon la touche par défaut ; `None` si désactivée.
    pub(super) fn shortcut_key(&self, action: &str) -> Option<egui::Key> {
        match self.settings.shortcuts.get(action) {
            Some(name) if name.is_empty() => None,
            Some(name) => egui::Key::from_name(name),
            None => {
                let key = SHORTCUTS
                    .iter()
                    .find(|(id, _, _)| *id == action)
                    .map(|(_, _, k)| *k)?;
                // Une touche par défaut déjà choisie pour une autre action (ex. S réaffectée avant l'ajout de la
                // capture d'écran) est laissée à cette action : une touche ne sert qu'à une seule action.
                let taken = self.settings.shortcuts.iter().any(|(other, name)| {
                    other != action && egui::Key::from_name(name) == Some(key)
                });
                (!taken).then_some(key)
            }
        }
    }
}

fn custom_path(s: &Settings, tool: Tool) -> &str {
    match tool {
        Tool::Streamlink => &s.streamlink_path,
        Tool::Libmpv => &s.libmpv_path,
    }
}

/// Filtre audio mpv : égalisation dynamique du volume, ou aucun filtre.
fn audio_filter(on: bool) -> &'static str {
    if on {
        "lavfi=[dynaudnorm=f=250:g=31:p=0.9]"
    } else {
        ""
    }
}

/// Vitesse de défilement des listes et du chat (1.0 = vitesse normale).
/// Applique l'accessibilité et le thème : contraste élevé (réglage, ou thème de contraste de Windows en automatique),
/// focus renforcé, taille de l'interface.
pub(crate) fn apply_access(ctx: &egui::Context, s: &Settings) {
    use crate::ui::theme;
    theme::set_high_contrast(s.high_contrast_on(crate::procs::system_high_contrast()));
    theme::set_focus_strong(s.focus_strong);
    // Couleur libre : corrigée seulement si l'utilisateur l'a demandé, ou en contraste élevé. Les palettes de base le
    // sont toujours (déjà conformes).
    let hc = s.high_contrast_on(crate::procs::system_high_contrast());
    theme::set_accent(s.accent_choice(), s.palette < 5 || s.accent_boost || hc);
    theme::apply(ctx, s.theme);
    ctx.set_zoom_factor(s.ui_zoom);
}

/// Applique le niveau d'animations choisi dans les Réglages (complet, réduit, désactivé). Le réglage d'animations de
/// Windows n'est volontairement jamais lu : l'application reste en mode complet tant que l'utilisateur ne change rien.
pub(crate) fn apply_animations(ctx: &egui::Context, s: &Settings) {
    crate::ui::theme::set_reduced_motion(s.reduced_motion);
    let secs = if s.animations { 1.0 / 12.0 } else { 0.0 };
    ctx.global_style_mut(|s| {
        s.animation_time = secs;
        // Infobulles un peu plus rapides qu'à l'origine (0,5 s), sans apparaître au moindre passage du pointeur.
        s.interaction.tooltip_delay = 0.3;
        // Passer d'un bouton à l'autre : la 2e infobulle s'affiche aussitôt si la 1re vient de disparaître (egui : 0,2 s).
        s.interaction.tooltip_grace_time = 0.5;
    });
}

/// 75 px par ligne de molette à 1,0 × (egui lisse lui-même le mouvement : rapide mais fluide).
fn apply_scroll_speed(ctx: &egui::Context, speed: f32) {
    ctx.options_mut(|o| o.input_options.line_scroll_speed = 75.0 * speed.clamp(0.5, 3.0));
}

fn apply_on_top(ctx: &egui::Context, on_top: bool) {
    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on_top {
        egui::WindowLevel::AlwaysOnTop
    } else {
        egui::WindowLevel::Normal
    }));
}

impl eframe::App for VelkaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Zone vidéo OpenGL d'un lecteur détruit (fenêtre cachée, redémarrage) : supprimée ici, contexte courant (S5).
        if let Some(gl) = frame.gl() {
            crate::mpv::free_dropped(gl);
        }
        // Lecteurs du multistream retirés : un détruit par image (contexte OpenGL courant).
        self.multi_free_one(&ctx);
        // Retour arrière des mises à jour (S7) : démarrage confirmé après quelques secondes de fonctionnement
        // (en tête de fonction : plein écran et mini-lecteur sortent de `ui` plus bas).
        if let Some(left) = crate::update::trial_tick() {
            ctx.request_repaint_after(left);
        }
        if ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving()) {
            self.last_activity = Instant::now();
        }
        theme::track_focus_modality(&ctx);
        theme::track_focus_change(&ctx);
        if self.theme_fade.is_none() {
            theme::sync_system_theme(&ctx, self.settings.theme);
        }
        // Zoom au clavier géré ici (et non par egui, qui l'applique en fin d'image : aux bornes 80 % / 150 %, une image
        // à 160 % ou 70 % s'affichait avant le retour à la borne). Aux bornes, la touche ne fait donc plus rien.
        // Ctrl 0 : taille par défaut (`DEFAULT_UI_ZOOM`). Sur un clavier AZERTY, la touche 0 donne « à » sans Maj : egui ne reçoit pas « 0 », on lit
        // donc aussi la touche physique.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (zoom_in, zoom_out, reset_zoom) = ctx.input_mut(|i| {
            use egui::{Key, KeyboardShortcut as Sc, Modifiers as M};
            let zin = i.consume_shortcut(&Sc::new(M::COMMAND, Key::Plus)) | i.consume_shortcut(&Sc::new(M::COMMAND, Key::Equals));
            let zout = i.consume_shortcut(&Sc::new(M::COMMAND, Key::Minus));
            let reset = i.consume_shortcut(&Sc::new(M::COMMAND, Key::Num0))
                | i.events.iter().any(|e| {
                    matches!(e, egui::Event::Key { physical_key: Some(Key::Num0), pressed: true, modifiers, .. } if modifiers.command)
                });
            (zin, zout, reset)
        });
        let current = ctx.zoom_factor();
        let target = if reset_zoom {
            crate::config::DEFAULT_UI_ZOOM
        } else if zoom_in {
            ((current + 0.1).clamp(0.8, 1.5) * 10.0).round() / 10.0
        } else if zoom_out {
            ((current - 0.1).clamp(0.8, 1.5) * 10.0).round() / 10.0
        } else {
            current
        };
        if (target - current).abs() > 0.001 {
            ctx.set_zoom_factor(target);
        }
        // Ctrl + / Ctrl − changent le zoom : on le mémorise dans les réglages (borné à 80–150 %).
        let zoom = ctx.zoom_factor();
        if (zoom - self.zoom_applied).abs() > 0.001 {
            let z = zoom.clamp(0.8, 1.5);
            if (z - zoom).abs() > 0.001 {
                ctx.set_zoom_factor(z);
            }
            self.settings.ui_zoom = z;
            self.zoom_applied = z;
        }
        self.lang_list_shown = false;
        if self.view != View::Settings {
            self.overlay_base = self.view;
        }
        if !std::mem::replace(&mut self.corners_done, true) {
            crate::procs::round_window_corners();
        }
        crate::smoke::tick(&ctx, self.get_proc.as_ref());
        self.poll(&ctx);
        self.tick_tray(&ctx);
        if self.view != View::Settings {
            self.advanced_visible_prev = false;
            ctx.data_mut(|d| d.remove::<bool>(settings::overlay_open_id()));
        }
        // Premier lancement après une mise à jour de l'application : on contrôle l'état des composants.
        // Chaîne choisie pour le démarrage : ouverte systématiquement, une seule fois, dès que l'accueil est affiché (ni
        // activation, ni didacticiel). Hors ligne, la page du lecteur affiche son message habituel avec « Réessayer ».
        if !self.startup_ch_done && self.view == View::Home && !self.onboarding_active() {
            self.startup_ch_done = true;
            if let Some(login) = crate::login::normalize(&self.settings.startup_channel) {
                if self.watch.is_none() && self.pending_channel.is_none() {
                    self.start_watch(&ctx, login);
                }
            }
        }
        // Autocomplétion du Multistream : seulement sur sa page (dessinée aussi sous l'overlay des Réglages).
        if self.view != View::Multi
            && !(self.view == View::Settings && self.overlay_base == View::Multi)
        {
            self.multi_editing = false;
            self.multi_hover = false;
            // Page quittée pendant un glisser-déposer : il est abandonné (aucun échange au retour).
            self.multi_drag = None;
            self.multi_press = None;
        }
        if self.view != View::Settings {
            self.startup_editing = false;
            // Réglages fermés avec l'option « chaîne au démarrage » activée mais sans chaîne validée : elle se désactive.
            if self.startup_armed && self.settings.startup_channel.is_empty() {
                self.startup_armed = false;
                self.startup_draft.clear();
                self.startup_focus = false;
            }
        }
        if !std::mem::replace(&mut self.tools_boot_checked, true) {
            let current = env!("CARGO_PKG_VERSION");
            let version_changed = self.settings.tools_checked_version != current;
            if version_changed {
                self.settings.tools_checked_version = current.to_string();
            }
            if version_changed || self.tools_update_after_check {
                self.start_tools_check(&ctx, true);
            }
        }
        {
            // Clic sur la notification d'un live : la fenêtre revient au premier plan et la chaîne s'ouvre.
            if let Some(login) = crate::procs::take_notification_click() {
                self.restore_from_tray(&ctx);
                self.start_watch(&ctx, login);
            }
            if self.view == View::Home {
                self.tick_status(&ctx);
            } else {
                self.tick_notify_poll(&ctx);
            }
            self.emotes.set_animate(self.settings.chat_animated);
            self.emotes.poll(&ctx);
            self.tick_chat_extras(&ctx);
            self.avatars.poll(&ctx);
            self.thumbs.poll(&ctx);
            self.request_avatars();
            self.tick_suggest(&ctx);
            self.tick_follows(&ctx);
            self.tick_identity();
            self.tick_update(&ctx);
            self.run_pending(&ctx);
            self.tick_nav(&ctx);
            self.handle_shortcuts(&ctx);
            self.tick_sleep(&ctx);
            self.tick_connection(&ctx);
            self.tick_audio_device(&ctx);
            self.tick_screenshot(&ctx);
            self.tick_screenshot_dir(&ctx);
        }
        self.track_window(&ctx);
        // Plein écran et PiP : la vidéo occupe toute la fenêtre, sans décor.
        if self.view == View::Watch && (self.fullscreen || self.pip) {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(egui::Color32::BLACK))
                .show(ui, |ui| self.watch_view(ui, &ctx));
            self.update_cursor(&ctx);
            self.save_settings_if_changed(&ctx);
            return;
        }
        // La fenêtre n'a pas de bordure Windows : le contenu va jusqu'aux bords, avec une barre de titre intégrée.
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG())
                    .inner_margin(theme::maximized_inset(&ctx)),
            )
            .show(ui, |ui| {
                egui::Frame::NONE.show(ui, |ui| {
                    ui.set_min_size(ui.available_size().max(egui::Vec2::ZERO));
                    // Les Réglages sont un overlay : en dessous, la page d'où on vient continue de s'afficher.
                    let base = if self.view == View::Settings {
                        self.overlay_base
                    } else {
                        self.view
                    };
                    if !matches!(base, View::Home | View::Multi | View::Settings) {
                        self.title_bar_panel(ui, &ctx);
                    }
                    match base {
                        View::FirstRun => {
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE)
                                .show(ui, |ui| {
                                    if first_run::show(ui, &self.rows, self.busy) {
                                        self.start_install(&ctx);
                                    }
                                });
                        }
                        View::Home | View::Multi | View::Settings => {
                            // Repli automatique : sous une largeur utile (en points, donc zoom compris), la barre se réduit
                            // seule ; elle revient quand la place revient (hystérésis). Le réglage de l'utilisateur n'est pas
                            // modifié. Ouvrir la barre à la main malgré le manque de place la garde ouverte (`side_force`).
                            let width = ctx.content_rect().width();
                            if self.side_auto {
                                if width >= SIDEBAR_AUTO_OPEN_W {
                                    self.side_auto = false;
                                    self.side_force = false;
                                }
                            } else if width < SIDEBAR_AUTO_CLOSE_W {
                                self.side_auto = true;
                            }
                            let collapsed = self.settings.sidebar_collapsed
                                || (self.side_auto && !self.side_force);
                            // Largeur animée : 236 px, ou 76 px quand la barre est réduite.
                            let target_w = if collapsed { 76.0 } else { 236.0 };
                            // Approche exponentielle : départ rapide, arrivée douce (pas de saccade).
                            if !crate::ui::theme::moves(&ctx) {
                                self.sidebar_w = target_w;
                            } else if (self.sidebar_w - target_w).abs() > 0.5 {
                                let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
                                self.sidebar_w +=
                                    (target_w - self.sidebar_w) * (1.0 - (-dt * 16.0).exp());
                                ctx.request_repaint();
                            } else {
                                self.sidebar_w = target_w;
                            }
                            let side_w = self.sidebar_w;
                            // Le contenu s'efface autour du changement de présentation (150 px) pour qu'il passe inaperçu.
                            let fade = ((side_w - 150.0).abs() / 36.0).clamp(0.0, 1.0);
                            // macOS : les boutons ronds du système sont en haut à gauche, sur la barre latérale ; elle
                            // commence donc sous une bande de la hauteur de la barre de titre, qui sert à déplacer la fenêtre.
                            let side_top = if theme::NATIVE_FRAME {
                                theme::TITLE_BAR_H as i8
                            } else {
                                16
                            };
                            let side = egui::Panel::left("sidebar")
                                .exact_size(side_w)
                                .resizable(false)
                                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                    left: 16,
                                    right: 16,
                                    top: side_top,
                                    bottom: 16,
                                }))
                                .show(ui, |ui| {
                                    ui.set_opacity(fade);
                                    if side_w < 150.0 {
                                        self.sidebar_collapsed_ui(ui, &ctx);
                                    } else {
                                        self.sidebar_ui(ui, &ctx);
                                    }
                                });
                            if theme::NATIVE_FRAME {
                                let r = side.response.rect;
                                let strip = egui::Rect::from_min_size(
                                    r.min,
                                    egui::vec2(r.width(), theme::TITLE_BAR_H),
                                );
                                theme::window_drag_zone(ui, &ctx, strip, "sidebar");
                            }
                            self.title_bar_panel(ui, &ctx);
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                    left: 24,
                                    right: 24,
                                    top: 4,
                                    bottom: 24,
                                }))
                                .show(ui, |ui| {
                                    self.update_banner(ui, &ctx);
                                    self.support_banner(ui, &ctx);
                                    if base == View::Multi {
                                        self.multi_main(ui, &ctx);
                                    } else {
                                        self.home_main(ui, &ctx);
                                    }
                                });
                        }
                        View::Watch => self.watch_view(ui, &ctx),
                    }
                });
            });
        if self.mini_visible() {
            self.mini_ui(&ctx);
        }
        // Vidéo non dessinée à cette image (Réglages sur l'accueil…) : mpv continue d'avancer, pour rester au direct.
        if self.watch.is_some() {
            if let Some(p) = &self.player {
                p.keep_live(&ctx);
            }
        }
        self.multi_keep_live(&ctx);
        if self.view == View::Settings {
            self.settings_overlay(&ctx);
        } else {
            settings::overlay_fade_out(&ctx);
        }
        if self.onboarding_active() {
            self.onboarding_ui(&ctx);
        }
        self.theme_fade_ui(&ctx);
        self.update_cursor(&ctx);
        // Tab repart de l'élément cliqué (sauf sur la page du lecteur : Espace et flèches y sont des raccourcis).
        theme::focus_follows_click(&ctx, self.view != View::Watch);
        if self.settings_focus_frames > 0 {
            self.settings_focus_frames -= 1;
            ctx.memory_mut(|m| m.request_focus(egui::Id::unique("settings_search")));
        }
        self.window_edges(&ctx);
        // Taille choisie avec le curseur des Réglages : appliquée au relâchement (au clavier : tout de suite).
        if (self.settings.ui_zoom - self.zoom_applied).abs() > 0.001
            && !ctx.input(|i| i.pointer.any_down())
        {
            ctx.set_zoom_factor(self.settings.ui_zoom);
            self.zoom_applied = self.settings.ui_zoom;
        }
        crate::ui::fonts::ensure(&ctx, self.lang_list_shown);
        self.restart_overlay(&ctx);
        self.save_settings_if_changed(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        crate::update::confirm(); // fermée normalement : la version démarre, l'ancienne n'est plus utile
        crate::procs::remove_tray_icon();
        // Aperçu : aucun réglage écrit. Désinstallation : rien à enregistrer, le dossier va être supprimé.
        if !apercu_outils() && !self.uninstalling {
            if let Err(e) = self.persistable().save() {
                crate::logging::log(&format!("Réglages non enregistrés : {e}"));
            }
        }
        self.fast_exit();
    }
}

impl VelkaApp {
    /// Fermeture après un live (seul ou multistream) : une fois les réglages enregistrés, les Streamlink sont arrêtés
    /// puis le processus se termine aussitôt, sans attendre la destruction des lecteurs libmpv (plusieurs centaines de
    /// millisecondes chacun, fenêtre figée pendant ce temps ; et sortie parfois bloquée sous Windows). Rien d'autre n'attend la fin normale : réglages, historique et journal sont déjà
    /// écrits sur le disque. Sans lecteur créé, la fermeture reste la fermeture normale.
    fn fast_exit(&mut self) {
        let had_video =
            self.player.is_some() || !self.multi.is_empty() || !self.multi_dying.is_empty();
        // `Drop` de `Stream` tue Streamlink (sur Mac et Linux, rien d'autre ne le ferait après un arrêt immédiat).
        self.stream = None;
        self.multi_kill_streams();
        if had_video && !crate::smoke::enabled() {
            crate::logging::log("Fermeture rapide après un live");
            crate::procs::terminate_now();
        }
    }
}
