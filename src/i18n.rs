//! Langues de l'interface. Chaque texte est écrit **en français** dans le code et passe par `tr` :
//! - français : le texte d'origine ;
//! - anglais : la table `english()` ci-dessous ;
//! - autres langues : un fichier `src/lang/<code>.tsv` par langue (« texte français », tabulation, traduction),
//!   embarqué dans l'exécutable et lu à la première utilisation. Repli : anglais, puis français.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Police à charger depuis Windows pour les écritures chinoise, japonaise et coréenne (absentes d'Inter).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cjk {
    ChineseSimplified,
    ChineseTraditional,
    Japanese,
    Korean,
}

pub struct Language {
    /// Code de langue (BCP 47).
    pub code: &'static str,
    /// Nom dans la langue elle-même (affiché dans le carrousel).
    pub native: &'static str,
    /// Nom en français (texte secondaire du carrousel, traduit par `tr`).
    pub name: &'static str,
    /// Virgule comme séparateur décimal (sinon point).
    pub comma: bool,
    pub cjk: Option<Cjk>,
    table: Option<&'static str>,
}

const fn lang(
    code: &'static str,
    native: &'static str,
    name: &'static str,
    comma: bool,
    table: &'static str,
) -> Language {
    Language {
        code,
        native,
        name,
        comma,
        cjk: None,
        table: Some(table),
    }
}

const fn cjk(
    code: &'static str,
    native: &'static str,
    name: &'static str,
    cjk: Cjk,
    table: &'static str,
) -> Language {
    Language {
        code,
        native,
        name,
        comma: false,
        cjk: Some(cjk),
        table: Some(table),
    }
}

/// Langues proposées. L'indice est enregistré dans `settings.json` (`language`) : ne jamais réordonner,
/// seulement ajouter à la fin.
pub static LANGUAGES: [Language; 25] = [
    Language {
        code: "fr",
        native: "Français",
        name: "Français",
        comma: true,
        cjk: None,
        table: None,
    },
    Language {
        code: "en",
        native: "English",
        name: "Anglais",
        comma: false,
        cjk: None,
        table: None,
    },
    lang(
        "es",
        "Español",
        "Espagnol",
        true,
        include_str!("lang/es.tsv"),
    ),
    lang(
        "pt-BR",
        "Português",
        "Portugais",
        true,
        include_str!("lang/pt.tsv"),
    ),
    lang(
        "de",
        "Deutsch",
        "Allemand",
        true,
        include_str!("lang/de.tsv"),
    ),
    lang(
        "it",
        "Italiano",
        "Italien",
        true,
        include_str!("lang/it.tsv"),
    ),
    lang(
        "nl",
        "Nederlands",
        "Néerlandais",
        true,
        include_str!("lang/nl.tsv"),
    ),
    lang(
        "pl",
        "Polski",
        "Polonais",
        true,
        include_str!("lang/pl.tsv"),
    ),
    lang("ru", "Русский", "Russe", true, include_str!("lang/ru.tsv")),
    lang(
        "uk",
        "Українська",
        "Ukrainien",
        true,
        include_str!("lang/uk.tsv"),
    ),
    lang("tr", "Türkçe", "Turc", true, include_str!("lang/tr.tsv")),
    lang(
        "vi",
        "Tiếng Việt",
        "Vietnamien",
        true,
        include_str!("lang/vi.tsv"),
    ),
    lang(
        "id",
        "Bahasa Indonesia",
        "Indonésien",
        true,
        include_str!("lang/id.tsv"),
    ),
    cjk(
        "zh-CN",
        "简体中文",
        "Chinois simplifié",
        Cjk::ChineseSimplified,
        include_str!("lang/zh-CN.tsv"),
    ),
    cjk(
        "zh-TW",
        "繁體中文",
        "Chinois traditionnel",
        Cjk::ChineseTraditional,
        include_str!("lang/zh-TW.tsv"),
    ),
    cjk(
        "ja",
        "日本語",
        "Japonais",
        Cjk::Japanese,
        include_str!("lang/ja.tsv"),
    ),
    cjk(
        "ko",
        "한국어",
        "Coréen",
        Cjk::Korean,
        include_str!("lang/ko.tsv"),
    ),
    lang(
        "cs",
        "Čeština",
        "Tchèque",
        true,
        include_str!("lang/cs.tsv"),
    ),
    lang("ro", "Română", "Roumain", true, include_str!("lang/ro.tsv")),
    lang(
        "hu",
        "Magyar",
        "Hongrois",
        true,
        include_str!("lang/hu.tsv"),
    ),
    lang(
        "sv",
        "Svenska",
        "Suédois",
        true,
        include_str!("lang/sv.tsv"),
    ),
    lang("da", "Dansk", "Danois", true, include_str!("lang/da.tsv")),
    lang(
        "nb",
        "Norsk",
        "Norvégien",
        true,
        include_str!("lang/nb.tsv"),
    ),
    lang("fi", "Suomi", "Finnois", true, include_str!("lang/fi.tsv")),
    lang("el", "Ελληνικά", "Grec", true, include_str!("lang/el.tsv")),
];

/// Indice de la langue courante dans `LANGUAGES`.
static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: u8) {
    LANG.store(lang.min(LANGUAGES.len() as u8 - 1), Ordering::Relaxed);
}

pub fn current() -> &'static Language {
    &LANGUAGES[LANG.load(Ordering::Relaxed) as usize]
}

/// Indice d'une langue d'après un code système (« es-ES », « pt-BR », « zh-Hant-TW »…), s'il est proposé.
pub fn index_for_locale(locale: &str) -> Option<u8> {
    let l = locale.to_ascii_lowercase().replace('_', "-");
    let primary = l.split('-').next().unwrap_or("");
    let code = match primary {
        "zh" if l.contains("hant")
            || l.ends_with("-tw")
            || l.ends_with("-hk")
            || l.ends_with("-mo") =>
        {
            "zh-TW"
        }
        "zh" => "zh-CN",
        "pt" => "pt-BR",
        "no" | "nn" | "nb" => "nb",
        p => p,
    };
    LANGUAGES
        .iter()
        .position(|x| x.code == code)
        .map(|i| i as u8)
}

/// Langue proposée pour une nouvelle installation : celle de Windows si elle existe, sinon l'anglais.
pub fn default_index() -> u8 {
    crate::procs::user_locale()
        .and_then(|l| index_for_locale(&l))
        .unwrap_or(1)
}

/// Textes qui nomment le système d'exploitation (pistes Mac et Linux) : clé française « Windows », « macOS » ou « Linux », traduites comme
/// les autres. Utilisées partout à la place du texte en dur (réglages, recherche, aide).
pub mod os {
    const MAC: bool = cfg!(target_os = "macos");
    const LINUX: bool = cfg!(target_os = "linux");
    /// Texte pour ce système : Windows, macOS ou Linux.
    const fn pick(windows: &'static str, mac: &'static str, linux: &'static str) -> &'static str {
        if MAC {
            mac
        } else if LINUX {
            linux
        } else {
            windows
        }
    }
    pub const STARTUP: &str = pick(
        "Démarrer avec Windows",
        "Démarrer avec macOS",
        "Démarrer avec Linux",
    );
    pub const STARTUP_HELP: &str = pick(
        "Lance Velka automatiquement quand vous ouvrez votre session Windows.",
        "Lance Velka automatiquement quand vous ouvrez votre session macOS.",
        "Lance Velka automatiquement quand vous ouvrez votre session Linux.",
    );
    pub const NOTIFY: &str = pick(
        "Notifications Windows",
        "Notifications macOS",
        "Notifications Linux",
    );
    pub const NOTIFY_HELP: &str = pick(
        "Affiche une notification Windows quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite).",
        "Affiche une notification macOS quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite).",
        "Affiche une notification Linux quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite).",
    );
}

/// Virgule décimale dans la langue courante ?
pub fn uses_comma() -> bool {
    current().comma
}

/// Nombre à `digits` décimales avec le séparateur de la langue courante (virgule en français).
pub fn decimal(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    if uses_comma() {
        s.replace('.', ",")
    } else {
        s
    }
}

fn parse_table(data: &'static str) -> HashMap<&'static str, &'static str> {
    data.lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

/// Table d'une langue (lue une seule fois).
fn table(index: usize) -> Option<&'static HashMap<&'static str, &'static str>> {
    static TABLES: OnceLock<Vec<OnceLock<HashMap<&'static str, &'static str>>>> = OnceLock::new();
    let tables = TABLES.get_or_init(|| LANGUAGES.iter().map(|_| OnceLock::new()).collect());
    let data = LANGUAGES.get(index)?.table?;
    Some(tables.get(index)?.get_or_init(|| parse_table(data)))
}

/// Traduit un texte de l'interface dans la langue courante.
pub fn tr(fr: &'static str) -> &'static str {
    match LANG.load(Ordering::Relaxed) as usize {
        0 => fr,
        1 => english(fr).unwrap_or(fr),
        i => table(i)
            .and_then(|t| t.get(fr).copied())
            .or_else(|| english(fr))
            .unwrap_or(fr),
    }
}

pub(crate) fn english(fr: &str) -> Option<&'static str> {
    Some(match fr {
        // Accessibilité
        "Aide" => "Help",
        // Fenêtre (barre de titre intégrée)
        "Réduire" => "Minimize",
        "Agrandir" => "Maximize",
        "Restaurer" => "Restore",
        "Fermer" => "Close",
        // Overlay des Réglages et noms des langues
        "Échap" => "Esc",
        "Français" => "French",
        "Anglais" => "English",
        "Espagnol" => "Spanish",
        "Portugais" => "Portuguese",
        "Allemand" => "German",
        "Italien" => "Italian",
        "Néerlandais" => "Dutch",
        "Polonais" => "Polish",
        "Russe" => "Russian",
        "Ukrainien" => "Ukrainian",
        "Turc" => "Turkish",
        "Vietnamien" => "Vietnamese",
        "Indonésien" => "Indonesian",
        "Chinois simplifié" => "Simplified Chinese",
        "Chinois traditionnel" => "Traditional Chinese",
        "Japonais" => "Japanese",
        "Coréen" => "Korean",
        "Tchèque" => "Czech",
        "Roumain" => "Romanian",
        "Hongrois" => "Hungarian",
        "Suédois" => "Swedish",
        "Danois" => "Danish",
        "Norvégien" => "Norwegian",
        "Finnois" => "Finnish",
        "Grec" => "Greek",
        // Connexion
        "Annuler" => "Cancel",
        "Échec de la connexion." => "Sign-in failed.",
        "Connexion annulée" => "Sign-in cancelled",
        // Accueil
        "spectateurs" => "viewers",
        "Hors ligne" => "Offline",
        "Cette chaîne n'existe pas." => "This channel doesn't exist.",
        "Statut indisponible, vous pouvez quand même essayer." => "Status unavailable, you can still try.",
        "Accueil" => "Home",
        "En direct" => "Live",
        // Multistream
        "Multistream" => "Multistream",
        "Regardez jusqu'à 4 lives en même temps." => "Watch up to 4 streams at once.",
        "Ajouter une chaîne" => "Add a channel",
        "Ajouter" => "Add",
        "Tout arrêter" => "Stop all",
        "Cette chaîne est déjà affichée." => "This channel is already shown.",
        "4 lives au maximum." => "4 streams at most.",
        "Ajoutez une chaîne pour commencer." => "Add a channel to start.",
        "Retirer ce live" => "Remove this stream",
        "Écouter ce live" => "Listen to this stream",
        "Couper tous les sons" => "Mute all",
        "Cumuler le son des lives en Multistream" => "Combine stream sound in Multistream",
        "Un clic sur un live ajoute son son au lieu de le remplacer" => "Clicking a stream adds its sound instead of replacing it",
        "Sur la page Multistream, un clic sur un live fait entendre son son en plus de ceux déjà écoutés, jusqu'aux quatre lives. Le bouton du son de chaque live le coupe." => "On the Multistream page, clicking a stream plays its sound in addition to those already playing, up to all four streams. Each stream's sound button mutes it.",
        "chaînes" => "channels",
        "Favoris" => "Favorites",
        "Spectateurs" => "Viewers",
        "au total" => "total",
        "Résultat" => "Result",
        "Vérification…" => "Checking…",
        "Regarder" => "Watch",
        "spectateur" => "viewer",
        "Nom de chaîne invalide." => "Invalid channel name.",
        "En direct parmi vos favoris" => "Live among your favorites",
        "Aucun favori en direct pour le moment." => "No favorites live right now.",
        "Récents" => "Recent",
        "Retirer de l'historique" => "Remove from history",
        // Barre latérale
        "Rechercher" => "Search",
        "Plateforme" => "Platform",
        "Réglages" => "Settings",
        "Aucun favori pour le moment. Cherchez une chaîne puis choisissez « Ajouter aux favoris »." => "No favorites yet. Search for a channel, then choose Add to favorites.",
        // Lecteur
        "Quitter le PiP" => "Exit PiP",
        "Reprise dans un instant…" => "Resuming in a moment…",
        "Retour" => "Back",
        "Arrêter" => "Stop",
        "Quitter le plein écran" => "Exit fullscreen",
        "Plein écran" => "Fullscreen",
        "Lecture" => "Play",
        "Réactiver le son" => "Unmute",
        "Couper le son" => "Mute",
        "Chat" => "Chat",
        "Changement de qualité…" => "Switching quality…",
        "Actualisation en cours" => "Refreshing…",
        "Qualité" => "Quality",
        "Enregistrée" => "Saved",
        "Désactivée" => "Off",
        "Chaîne ouverte au démarrage" => "Channel opened at startup",
        "Démarrage et fenêtre" => "Startup and window",
        "Son live s'ouvre au lancement si elle est en direct. Sinon, l'accueil s'affiche normalement." => "Its live opens at launch when it is online. Otherwise the home page appears as usual.",
        "Nom de la chaîne" => "Channel name",
        "Au lancement, si cette chaîne est en direct, son live s'ouvre directement dans le lecteur. Hors ligne ou introuvable, l'accueil s'affiche normalement." => "At launch, if this channel is live, its stream opens straight in the player. If it is offline or not found, the home page appears normally.",
        "Taille et place d'origine" => "Original size and position",
        "Lecteur au format de la vidéo" => "Player matches the video shape",
        "Dans la fenêtre principale, l'image prend exactement le format du live (coins arrondis, sans bandes autour) et reste centrée." => "In the main window, the picture takes exactly the live's aspect ratio (rounded corners, no bars around it) and stays centered.",
        "Connexion en cours" => "Connecting",
        "Audio du live uniquement" => "Live audio only",
        "Retirer des favoris" => "Remove from favorites",
        "Orange (par défaut)" => "Orange (default)",
        "Favori" => "Favorite",
        "Le live démarre sans le son ; vous le réactivez quand vous voulez." => "The live starts without sound; turn it back on whenever you like.",
        "Agrandit ou réduit le texte des messages du chat." => "Makes the chat message text larger or smaller.",
        "Affiche l'heure devant chaque message du chat." => "Shows the time before each chat message.",
        "Place le chat à gauche de la vidéo au lieu de la droite." => "Puts the chat on the left of the video instead of the right.",
        "Ultra légère et optimisée, parfaite pour jouer en même temps." => "Ultra-light and optimized, perfect for playing at the same time.",
        "Revoir le didacticiel" => "Replay the tutorial",
        "Passer" => "Skip",
        "Suivant" => "Next",
        "C'est parti" => "Let's go",
        "Chat avec émotes" => "Chat with emotes",
        "Mini-lecteur et PiP" => "Mini player and PiP",
        "Choisissez votre style" => "Choose your style",
        "Couleur d'accent" => "Accent color",
        "Réglez la lecture" => "Set up playback",
        "Restez informé" => "Stay informed",
        "Tout se règle aussi plus tard dans les Réglages." => "You can change everything later in Settings.",
        "Thème et couleur de l'application." => "Theme and color of the app.",
        "Les réglages les plus utiles, en un coup d'œil." => "The most useful settings at a glance.",
        "Ne ratez plus vos streamers préférés." => "Never miss your favorite streamers again.",
        "Redémarrer l'application" => "Restart the app",
        "Réessayer" => "Retry",
        "depuis" => "for",
        "connexion…" => "connecting…",
        "changement de qualité…" => "switching quality…",
        "interrompu" => "interrupted",
        // Réglages
        "Lecture vidéo" => "Playback",
        "Qualité par défaut" => "Default quality",
        "Faible latence" => "Low latency",
        "Désactiver pour un flux plus stable" => "Turn off for a more stable stream",
        "Réduire la barre latérale" => "Collapse sidebar",
        "Agrandir la barre latérale" => "Expand sidebar",
        "Chat à gauche de la vidéo" => "Chat on the left of the video",
        "Toujours au premier plan" => "Always on top",
        "Fenêtre PiP au format de la vidéo" => "PiP window matches the video shape",
        "En mini-fenêtre PiP, la fenêtre prend le format du live pour supprimer les bandes noires." => "In the PiP mini window, the window takes the live's aspect ratio to remove black bars.",
        "Chat en surimpression en plein écran" => "Chat overlay in fullscreen",
        "Apparence" => "Appearance",
        "Général" => "General",
        "Thème" => "Theme",
        "Langue" => "Language",
        "Sombre" => "Slate",
        "Dark" => "Dark",
        "Clair" => "Light",
        "Compte Twitch" => "Twitch account",
        "Connecté en tant que" => "Signed in as",
        "Vous pouvez écrire dans le chat." => "You can write in chat.",
        "chaînes suivies sur Twitch chargées." => "followed Twitch channels loaded.",
        "Pour charger vos chaînes suivies, déconnectez-vous puis reconnectez-vous." => {
            "To load your followed channels, log out and log back in."
        }
        "Impossible de charger vos chaînes suivies pour le moment." => "Unable to load your followed channels right now.",
        "Chargement de vos chaînes suivies…" => "Loading your followed channels…",
        "Reconnectez-vous dans Réglages pour afficher vos chaînes suivies sur Twitch." => {
            "Log in again in Settings to show your followed Twitch channels."
        }
        "En direct parmi vos chaînes suivies sur Twitch" => "Live among your followed Twitch channels",
        "Suivies sur Twitch" => "Followed on Twitch",
        "Prévient quand un favori passe en direct" => "Notifies you when a favorite goes live",
        "Activé : vos favoris sont vérifiés toutes les minutes en arrière-plan, même pendant un live. Cela utilise un peu plus d'internet et de mémoire." => "On: your favorites are checked every minute in the background, even during a live. This uses a bit more internet and memory.",
        "Chaînes dans la barre latérale" => "Channels in the sidebar",
        "Chaînes sur l'accueil" => "Channels on Home",
        "Qualité utilisée à l'ouverture d'un live. « Source » est la qualité maximale de la chaîne ; « Audio » ne joue que le son." => "Quality used when a live opens. \"Source\" is the channel's maximum quality; \"Audio\" plays sound only.",
        "Réduit le retard entre le streamer et vous. Désactivez-le si la vidéo saccade ou se coupe." => "Reduces the delay behind the streamer. Turn it off if the video stutters or cuts out.",
        "Garde la fenêtre devant toutes les autres applications." => "Keeps the window in front of all other applications.",
        "Affiche le chat par-dessus la vidéo quand vous êtes en plein écran." => "Shows the chat over the video when in full screen.",
        "Faire rouler la molette au-dessus de la vidéo monte ou baisse le volume." => "Scrolling the wheel over the video raises or lowers the volume.",
        "Un clic sur la vidéo met en pause ou relance la lecture." => "A click on the video pauses or resumes playback.",
        "Garde un niveau sonore régulier (normalisation du son) : relève les passages faibles et atténue les passages forts, d'une chaîne à l'autre et au sein d'un même live." => "Keeps the sound level steady (audio normalization): raises quiet parts and softens loud ones, from one channel to another and within a single live.",
        "Vitesse de la molette dans les listes et dans le chat. À 1,0 ×, un cran de molette fait défiler 3 lignes de 75 px, de façon fluide." => "Mouse wheel speed in lists and in the chat. At 1.0 ×, one wheel notch scrolls 3 lines of 75 px, smoothly.",
        "À chaque lancement, l'application démarre avec le volume choisi ci-dessous, au lieu du dernier volume réglé." => "At every launch, the app starts with the volume chosen below instead of the last volume set.",
        "100 % = volume normal. 130 % permet d'amplifier une chaîne trop silencieuse, mais le son peut saturer." => "100% = normal volume. 130% can boost a channel that is too quiet, but the sound may distort.",
        "Temps sans bouger la souris avant que les boutons posés sur la vidéo disparaissent." => "Time without moving the mouse before the buttons over the video disappear.",
        "Quand vous quittez la page d'un live avec « Retour », il continue dans une petite fenêtre en bas à droite. Désactivé : il s'arrête." => "When you leave a live's page with \"Back\", it keeps playing in a small window at the bottom right. Off: it stops.",
        "Fréquence à laquelle l'accueil vérifie quelles chaînes sont en direct. Plus c'est rapide, plus cela utilise internet. « Manuelle » : seulement avec le bouton Actualiser." => "How often Home checks which channels are live. The faster, the more internet it uses. \"Manual\": only with the Refresh button.",
        "Télécharge une image du direct pour chaque carte. Désactivez pour économiser des données." => "Downloads a live image for each card. Turn off to save data.",
        "Les chaînes que vous suivez sur votre compte Twitch (il faut être connecté dans la carte « Compte Twitch »)." => "The channels you follow on your Twitch account (you must be signed in in the \"Twitch account\" card).",
        "Nombre maximum de chaînes Twitch en direct listées dans la barre de gauche (ce n'est pas un délai)." => "Maximum number of live Twitch channels listed in the left bar (not a delay).",
        "Nombre maximum de chaînes Twitch en direct affichées sur la page d'accueil (ce n'est pas un délai)." => "Maximum number of live Twitch channels shown on the Home page (not a delay).",
        "Garde le jeu sélectionné dans la barre « Jeux en direct » au prochain lancement." => "Keeps the game selected in the \"Live games\" bar at the next launch.",
        "Ouvrir : le live s'affiche en grand. PiP : il s'ouvre directement dans une petite fenêtre flottante." => "Open: the live shows full size. PiP: it opens directly in a small floating window.",
        "Les messages qui contiennent un de ces mots (séparés par des virgules) sont surlignés en jaune dans le chat." => "Messages containing one of these words (comma-separated) are highlighted in yellow in the chat.",
        "Quand une chaîne est hors ligne ou que son live se termine, propose d'autres lives : vos chaînes suivies du même jeu d'abord." => "When a channel is offline or its live ends, suggests other lives: your followed channels in the same game first.",
        "Nombre de chaînes proposées quand un live est hors ligne ou terminé." => "Number of channels suggested when a live is offline or has ended.",
        "Les boutons latéraux de la souris changent de page, comme dans un navigateur. Alt + flèche gauche ou droite fait pareil." => "The mouse side buttons change page, like in a browser. Alt + left or right arrow does the same.",
        "Le pointeur devient une main au-dessus des boutons et des cartes." => "The pointer becomes a hand over buttons and cards.",
        "Complet : fondus, glissements, cascades et compteurs animés. Réduit : fondus et changements de couleur seulement, sans déplacement. Désactivé : aucune animation (utile sur un ordinateur lent). Le réglage d'animations de Windows n'est pas pris en compte." => "Full: fades, slides, cascades and animated counters. Reduced: only fades and colour changes, no movement. Off: no animation (useful on a slow computer). The Windows animation setting is ignored.",
        "Complet" => "Full",
        "Réduit" => "Reduced",
        "Affiche une notification Windows quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite)." => "Shows a Windows notification when one of your favorites goes live. The app must stay open (it can be minimized).",
        "Affiche une notification macOS quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite)." => "Shows a macOS notification when one of your favorites goes live. The app must stay open (it can be minimized).",
        "Affiche une notification Linux quand l'un de vos favoris passe en direct. L'application doit rester ouverte (elle peut être réduite)." => "Shows a Linux notification when one of your favorites goes live. The app must stay open (it can be minimized).",
        "Lance Velka automatiquement quand vous ouvrez votre session Windows." => "Starts Velka automatically when you sign in to Windows.",
        "Lance Velka automatiquement quand vous ouvrez votre session macOS." => "Starts Velka automatically when you sign in to macOS.",
        "Lance Velka automatiquement quand vous ouvrez votre session Linux." => "Starts Velka automatically when you sign in to Linux.",
        "Photos de profil et émotes déjà téléchargées. Les vider libère de la place ; elles se retéléchargent au besoin." => "Profile pictures and emotes already downloaded. Clearing them frees space; they are downloaded again when needed.",
        "Notifications Windows" => "Windows notifications",
        "Notifications macOS" => "macOS notifications",
        "Notifications Linux" => "Linux notifications",
        "Prévient quand un favori passe en direct. Vérifie vos favoris toutes les minutes, même pendant un live : utilise un peu plus d'internet et de mémoire." => "Notifies you when a favorite goes live. Checks your favorites every minute, even during a live: uses a bit more internet and memory.",
        "Envoyer une notification de test" => "Send a test notification",
        "est en direct" => "is live",
        "Notification de test : tout fonctionne." => "Test notification: everything works.",
        "Volume et lecteur" => "Volume and player",
        "Démarrer en muet" => "Start muted",
        "Volume fixe au lancement" => "Fixed volume at launch",
        "Sinon : le dernier volume utilisé" => "Otherwise: the last volume used",
        "Volume au lancement" => "Launch volume",
        "Volume maximum" => "Maximum volume",
        "Masquage des contrôles" => "Hide controls after",
        "Jamais" => "Never",
        // Lecteur : capture, infos techniques, minuteur de veille
        "Capture d'écran" => "Screenshot",
        "Infos techniques" => "Technical info",
        "Capture enregistrée :" => "Screenshot saved:",
        "Capture impossible." => "Screenshot failed.",
        "Minuteur de veille" => "Sleep timer",
        "Désactivé" => "Off",
        "À jour" => "Up to date",
        "Mise à jour disponible" => "Update available",
        "Chemin personnalisé" => "Custom path",
        "Version inconnue" => "Unknown version",
        "Installée :" => "Installed:",
        "Disponible :" => "Available:",
        "Mise à jour en cours…" => "Updating…",
        "Redémarrage nécessaire" => "Restart required",
        "Impossible de vérifier les mises à jour :" => "Couldn't check for updates:",
        "Échec de la mise à jour, version précédente conservée :" => "Update failed, previous version kept:",
        "Mises à jour" => "Updates",
        "À propos" => "About",
        "Version de l'application" => "App version",
        "Couleurs" => "Colors",
        "Palette de couleurs" => "Color palette",
        "Turquoise" => "Turquoise",
        "Bleu" => "Blue",
        "Vert" => "Green",
        "Orange foncé" => "Dark orange",
        "Éclaircit ou assombrit la couleur choisie pour que le texte reste lisible." => {
            "Lightens or darkens the chosen color so text stays readable."
        }
        "État et version de Streamlink et de libmpv, mise à jour, et chemins personnalisés (facultatifs) vers streamlink.exe et libmpv-2.dll." => {
            "Status and version of Streamlink and libmpv, update, and optional custom paths to streamlink.exe and libmpv-2.dll."
        }
        "Ouvre le dossier du journal, copie le diagnostic (version, Windows, carte graphique, composants) ou le rapport de plantage local pour le support. Rien n'est envoyé automatiquement." => {
            "Opens the log folder, copies the diagnostics (version, Windows, graphics card, components) or the local crash report for support. Nothing is sent automatically."
        }
        "Rose" => "Pink",
        "Personnalisée" => "Custom",
        "Couleur d'accent personnalisée" => "Custom accent color",
        "Code couleur" => "Color code",
        "Rouge" => "Red",
        "Exemple" => "Example",
        "Contraste du texte sur l'accent" => "Text contrast on the accent",
        "Conforme" => "Passes",
        "Insuffisant, minimum" => "Insufficient, minimum",
        "Couleur non appliquée : le contraste est trop faible." => "Color not applied: the contrast is too low.",
        "Couleur conforme la plus proche :" => "Closest passing color:",
        "Utiliser cette couleur" => "Use this color",
        "Rétablir les couleurs par défaut" => "Reset colors to default",
        "Accessibilité" => "Accessibility",
        "Taille de l'interface" => "Interface size",
        "Mise à l'échelle" => "Scaling",
        "Contraste élevé" => "High contrast",
        "Automatique" => "Automatic",
        "Activé" => "On",
        "Focus clavier renforcé" => "Enhanced keyboard focus",
        "Arrêt du live dans" => "Live stops in",
        "logiciel" => "software",
        "Résolution" => "Resolution",
        "Images par seconde" => "Frames per second",
        "Débit vidéo" => "Video bitrate",
        "Débit audio" => "Audio bitrate",
        "Images perdues" => "Dropped frames",
        "Images perdues au décodage" => "Frames dropped by the decoder",
        "Mémoire tampon" => "Buffer",
        "Format vidéo" => "Video format",
        "Décodage" => "Decoding",
        "Arrête le live automatiquement après la durée choisie, à partir du moment où vous changez ce réglage. Une minute avant, un compte à rebours s'affiche sur la vidéo. Le minuteur revient sur Désactivé à chaque nouveau live et à chaque lancement de l'application." => "Stops the live automatically after the chosen duration, counted from the moment you change this setting. A minute before, a countdown appears over the video. The timer goes back to Off for every new live and every time the app starts.",
        // Chat : emotes externes, animées, badges
        "Émotes 7TV, BTTV et FFZ" => "7TV, BTTV and FFZ emotes",
        "Émotes animées" => "Animated emotes",
        "Badges du chat" => "Chat badges",
        "Affiche les émotes des extensions 7TV, BetterTTV et FrankerFaceZ (globales et propres à la chaîne). Elles sont chargées à l'ouverture du live." => "Shows emotes from the 7TV, BetterTTV and FrankerFaceZ extensions (global and channel-specific). They are loaded when the live opens.",
        "Anime les émotes qui le sont (Twitch et extensions). Utilise un peu plus le processeur quand le chat en affiche." => "Animates emotes that are animated (Twitch and extensions). Uses a little more CPU while the chat shows them.",
        "Affiche les badges (abonné, modérateur, VIP…) devant les pseudos." => "Shows badges (subscriber, moderator, VIP…) before user names.",
        // Image : netteté, agrandissement
        "Netteté de l'image" => "Picture sharpness",
        "Agrandissement de haute qualité" => "High-quality upscaling",
        "Légère" => "Light",
        "Moyenne" => "Medium",
        "Forte" => "Strong",
        "Image plus fine en 480p et 720p" => "Finer picture at 480p and 720p",
        "Renforce les contours de l'image (calculé par la carte graphique). Utile sur un flux flou ; trop fort, l'image paraît granuleuse." => "Sharpens the edges of the picture (done by the graphics card). Useful on a blurry stream; too strong, the picture looks grainy.",
        "Utilise un filtre d'agrandissement plus fin quand la vidéo est plus petite que la fenêtre (480p, 720p). Demande davantage à la carte graphique." => "Uses a finer upscaling filter when the video is smaller than the window (480p, 720p). Asks more of the graphics card.",
        // Réglages : onglets et cartes
        "Compte" => "Account",
        "Qualité et flux" => "Quality and stream",
        "Audio" => "Audio",
        "Lecteur et comportement" => "Player and behavior",
        "Affichage et lisibilité" => "Display and readability",
        "Filtrage et alertes" => "Filtering and alerts",
        "Notifications" => "Notifications",
        "Avancé et Système" => "Advanced and System",
        "Stockage et configuration" => "Storage and configuration",
        "Émotes et badges" => "Emotes and badges",
        "Les changements s'appliquent tout de suite." => "Changes apply immediately.",
        "2,5 s" => "2.5 s",
        "5 s" => "5 s",
        "10 s" => "10 s",
        "Mo" => "MB",
        "Ko" => "KB",
        "Rechercher un réglage" => "Search settings",
        "Résultats de la recherche" => "Search results",
        "Aucun réglage trouvé." => "No setting found.",
        "Mini-lecteur" => "Mini player",
        "Garder le live dans un coin quand on quitte la page" => "Keep the live in a corner when leaving the page",
        "Accueil et barre latérale" => "Home and sidebar",
        "Actualisation" => "Refresh rate",
        "Manuelle" => "Manual",
        "Actualiser" => "Refresh",
        "Aperçus des lives" => "Live previews",
        "Images du direct sur les cartes" => "Live images on the cards",
        "Chaînes suivies sur Twitch" => "Followed Twitch channels",
        "Dans la barre latérale, sur l'accueil et dans les recommandations" => "In the sidebar, on Home and in recommendations",
        "Dans la barre latérale" => "In the sidebar",
        "Sur l'accueil" => "On Home",
        "Se souvenir du jeu choisi" => "Remember the chosen game",
        "Clic sur une carte" => "Click on a card",
        "Ouvrir" => "Open",
        "Taille du texte" => "Text size",
        "Masquer les bots" => "Hide bots",
        "Horodatage des messages" => "Message timestamps",
        "Mots surlignés" => "Highlighted words",
        "mot, pseudo… (séparés par des virgules)" => "word, name… (comma-separated)",
        "Recommandations" => "Recommendations",
        "Recommandations de chaînes" => "Channel recommendations",
        "Proposer des chaînes quand un live est hors ligne ou terminé" => "Suggest channels when a live is offline or has ended",
        "Nombre de recommandations" => "Number of recommendations",
        "Captures d'écran" => "Screenshots",
        "Dossier d'enregistrement" => "Save folder",
        "Parcourir…" => "Browse…",
        "Ouvrir le dossier" => "Open folder",
        "Dossier où la touche S enregistre les captures d'écran. Par défaut : le dossier « Velka » de vos Images. Le dossier choisi doit accepter l'écriture ; s'il disparaît (disque débranché, dossier supprimé), l'application revient au dossier par défaut. Le changement s'applique à la prochaine capture." => "Folder where the S key saves screenshots. Default: the “Velka” folder in your Pictures. The chosen folder must accept writing; if it disappears (drive unplugged, folder deleted), the application goes back to the default folder. The change applies to the next screenshot.",
        "Le dossier des captures n'est plus disponible : retour au dossier par défaut." => "The screenshot folder is no longer available: back to the default folder.",
        "Ce dossier n'accepte pas l'écriture : choisissez-en un autre." => "This folder does not accept writing: choose another one.",
        "Périphérique de sortie" => "Output device",
        "Périphérique par défaut du système" => "System default device",
        "Choisit le casque ou les enceintes pour le son des lives, sans changer le son de Windows. « Périphérique par défaut du système » suit le choix de Windows. Le changement s'applique tout de suite. Si le périphérique choisi disparaît (casque débranché), l'application revient au choix par défaut." => "Picks the headphones or speakers for live sound, without changing Windows sound. “System default device” follows the Windows choice. The change applies immediately. If the chosen device disappears (headphones unplugged), the application goes back to the default choice.",
        "Le périphérique audio choisi n'est plus disponible : retour au périphérique par défaut." => "The chosen audio device is no longer available: back to the default device.",
        "Décodage vidéo matériel (GPU)" => "Hardware video decoding (GPU)",
        "Utile si la vidéo saccade ou si le processeur chauffe. « Automatique » : réglage d'origine de l'application. « Activé » : la carte graphique décode la vidéo, avec retour automatique au décodage normal si elle n'y arrive pas. « Désactivé » : décodage normal par le processeur. Le changement s'applique tout de suite : le live en cours est relancé." => "Helps if the video stutters or the processor runs hot. “Automatic”: the application's original setting. “On”: the graphics card decodes the video, with automatic fallback to normal decoding if it fails. “Off”: normal decoding by the processor. The change applies immediately: the current live is restarted.",
        "Mouvement et lisibilité" => "Motion and readability",
        "Navigation et souris" => "Navigation and mouse",
        "Vérification des mises à jour" => "Update check",
        "Nouvelle version disponible :" => "New version available:",
        "Mettre à jour" => "Update",
        "Téléchargement de la mise à jour…" => "Downloading the update…",
        "Mise à jour impossible :" => "Update failed:",
        "Boutons Précédent / Suivant de la souris" => "Mouse Back / Forward buttons",
        "Main sur les éléments cliquables" => "Hand cursor on clickable items",
        "Animations" => "Animations",
        "Raccourcis clavier" => "Keyboard shortcuts",
        "Appuyez sur une touche…" => "Press a key…",
        "Rétablir les raccourcis" => "Reset shortcuts",
        "Aucun" => "None",
        "Perso" => "Custom",
        "Durée personnalisée" => "Custom duration",
        "Ajuster uniquement la taille du texte du chat" => "Adjust only the chat text size",
        "Ajuster la taille globale de l'interface" => "Adjust the overall interface size",
        "Options du lecteur" => "Player options",
        "Ouvrir le dossier de configuration" => "Open the settings folder",
        "Historique des chaînes" => "Channel history",
        "Effacer l'historique" => "Clear history",
        "Historique effacé." => "History cleared.",
        "Connectez-vous à Twitch pour activer" => "Sign in to Twitch to enable",
        "Rechercher des mises à jour" => "Check for updates",
        "Vous utilisez la dernière version." => "You are using the latest version.",
        "Mise à jour {version} disponible" => "Update {version} available",
        "Impossible de se connecter. Vérifiez votre connexion internet." => "Could not connect. Check your internet connection.",
        "La recherche de mise à jour a échoué." => "The update check failed.",
        "Réduire dans la zone de notification à la fermeture" => "Minimize to the notification area on close",
        "L'application reste active en fond pour les notifications" => "The app keeps running in the background for notifications",
        "La croix de fermeture cache la fenêtre dans la zone de notification (près de l'horloge) au lieu de quitter ; le live en cours est arrêté. Clic gauche sur l'icône : rouvrir. Clic droit : Ouvrir ou Quitter. Pour être prévenu quand un favori passe en direct, activez aussi « Notifications Windows »." => "The close button hides the window in the notification area (next to the clock) instead of quitting; the current live is stopped. Left-click the icon to reopen it, right-click for Open or Quit. To be told when a favorite goes live, also turn on \"Windows notifications\".",
        "Ouvrir Velka" => "Open Velka",
        "Quitter" => "Quit",
        "Velka reste actif dans la zone de notification. Clic droit sur l'icône pour quitter." => "Velka is still running in the notification area. Right-click the icon to quit.",
        "Effacer un raccourci" => "Clear shortcut",
        "Raccourcis non modifiables" => "Fixed shortcuts",
        "Quitter le plein écran, le PiP ou le live" => "Leave full screen, PiP or the live",
        "Rechercher une chaîne" => "Search for a channel",
        "Zoom de l'interface" => "Interface zoom",
        "Page précédente / suivante" => "Previous / next page",
        "Pause / lecture" => "Pause / play",
        "Volume +" => "Volume +",
        "Volume -" => "Volume -",
        "Afficher / masquer le chat" => "Show / hide chat",
        "Image dans l'image" => "Picture in picture",
        "Système" => "System",
        "Démarrer avec Windows" => "Start with Windows",
        "Démarrer avec macOS" => "Start with macOS",
        "Démarrer avec Linux" => "Start with Linux",
        "Ouvre l'application à l'ouverture de session" => "Opens the app when you sign in",
        "Cache des images" => "Image cache",
        "Vider" => "Clear",
        "Espace" => "Space",
        "Flèche haut" => "Up arrow",
        "Flèche bas" => "Down arrow",
        "Flèche gauche" => "Left arrow",
        "Flèche droite" => "Right arrow",
        "Entrée" => "Enter",
        "Jeux en direct" => "Live games",
        "Aucun live pour ce jeu en ce moment." => "No live for this game right now.",
        "Chaînes recommandées" => "Recommended channels",
        "Replier le chat" => "Collapse chat",
        "Afficher le chat" => "Show chat",
        "Qualité maximale proposée par la chaîne, sans réduction." => "Highest quality the channel offers, with no downscaling.",
        "Ajouter aux favoris" => "Add to favorites",
        "Se déconnecter" => "Sign out",
        "Se connecter" => "Sign in",
        "Autres langues" => "Other languages",
        "Personnaliser" => "Customize",
        "Améliorer le contraste" => "Improve contrast",
        "Une page Twitch s'est ouverte. Vérifiez ce code, puis cliquez sur « Autoriser » :" => {
            "A Twitch page has opened. Check this code, then click “Authorize”:"
        }
        "En attente de votre accord sur Twitch…" => "Waiting for your approval on Twitch…",
        "Rouvrir la page Twitch" => "Reopen the Twitch page",
        "Connexion à Twitch…" => "Connecting to Twitch…",
        "Connectez-vous pour écrire dans le chat." => "Sign in to write in chat.",
        "Se connecter avec Twitch" => "Sign in with Twitch",
        "La connexion Twitch n'est pas disponible dans cette version." => "Twitch sign-in is not available in this version.",
        "Connexion refusée sur Twitch." => "Sign-in refused on Twitch.",
        "Le code a expiré, recommencez." => "The code expired, please try again.",
        "Twitch a refusé la connexion." => "Twitch refused the sign-in.",
        "Impossible de joindre Twitch. Vérifiez votre connexion." => "Can't reach Twitch. Check your connection.",
        "Twitch a refusé la demande de connexion." => "Twitch refused the sign-in request.",
        "Composants externes" => "External components",
        "Non installé" => "Not installed",
        "Prêt" => "Ready",
        "Installer" => "Install",
        "Utilisée en ce moment : redémarrez l'application pour la mettre à jour." => {
            "In use right now: restart the app to update it."
        }
        "Chemins personnalisés (facultatif)" => "Custom paths (optional)",
        "Journaux et diagnostic" => "Logs and diagnostics",
        "Volume" => "Volume",
        "Molette de la souris sur la vidéo = volume" => "Mouse wheel on the video = volume",
        "Clic sur la vidéo = pause" => "Click on the video = pause",
        "Volume stable" => "Stable volume",
        "Afficher le menu" => "Show menu",
        "Masquer le menu" => "Hide menu",
        "Limite les écarts de volume entre les chaînes" => "Limits volume differences between channels",
        "Vitesse de défilement" => "Scroll speed",
        "Résultats" => "Results",
        "Zone de réinitialisation" => "Reset zone",
        "Réinitialiser" => "Reset",
        "Désinstaller" => "Uninstall",
        "Oui, réinitialiser" => "Yes, reset",
        "Oui, désinstaller" => "Yes, uninstall",
        "Réinitialiser remet les réglages, les favoris et l'historique à zéro. Les outils restent installés." => {
            "Reset puts settings, favorites and history back to zero. The tools stay installed."
        }
        "Désinstaller supprime les outils, les réglages, l'historique et le cache, puis ferme l'application." => {
            "Uninstall deletes the tools, settings, history and cache, then closes the app."
        }
        "Tout remettre à zéro ? Réglages, favoris et historique seront perdus." => {
            "Reset everything? Settings, favorites and history will be lost."
        }
        "Tout supprimer ? Outils, réglages, historique et cache seront effacés définitivement." => {
            "Delete everything? Tools, settings, history and cache will be permanently erased."
        }
        "Ouvrir le dossier du journal" => "Open the log folder",
        "Rapport de la dernière fermeture inattendue, sans donnée personnelle. Rien n'est envoyé automatiquement." => "Report from the last unexpected shutdown, with no personal data. Nothing is sent automatically.",
        "Copier le rapport de plantage" => "Copy the crash report",
        "Rapport copié." => "Report copied.",
        "Copier le diagnostic" => "Copy diagnostics",
        "Diagnostic copié." => "Diagnostics copied.",
        "Velka n'a pas pu ouvrir sa fenêtre : la carte graphique ou son pilote ne prend pas en charge OpenGL 3.0. Mettez à jour le pilote de la carte graphique, puis relancez Velka. Sur un bureau à distance ou une machine virtuelle, l'affichage peut ne pas être pris en charge." => "Velka could not open its window: the graphics card or its driver does not support OpenGL 3.0. Update the graphics card driver, then restart Velka. On a remote desktop or a virtual machine, display may not be supported.",
        "La carte graphique n'annonce pas OpenGL 3.0 : la vidéo peut rester noire. Mettez à jour le pilote de la carte graphique. Sur un bureau à distance ou une machine virtuelle, l'affichage peut ne pas être pris en charge." => "The graphics card does not report OpenGL 3.0: video may stay black. Update the graphics card driver. On a remote desktop or a virtual machine, display may not be supported.",
        // Chat
        "Connexion au chat…" => "Connecting to chat…",
        "Chat connecté" => "Chat connected",
        "Reconnexion du chat" => "Reconnecting chat",
        "Envoyer un message" => "Send a message",
        "Chat réservé aux followers" => "Followers-only chat",
        "Jeton requis pour écrire (Réglages)" => "Token required to write (Settings)",
        "Revenir en bas" => "Back to bottom",
        "En attente de messages…" => "Waiting for messages…",
        "Affiche d'abord les lives qui ont le plus de spectateurs." => "Shows the lives with the most viewers first.",
        "Affiche d'abord les lives qui ont le moins de spectateurs." => "Shows the lives with the fewest viewers first.",
        "Spectateurs : décroissant" => "Viewers: descending",
        "Spectateurs : croissant" => "Viewers: ascending",
        "Confirmer la déconnexion" => "Confirm sign out",
        "Ce jeton n'a pas le droit d'écrire dans le chat (chat:edit)." => {
            "This token isn't allowed to write in chat (chat:edit)."
        }
        "Réponse de Twitch inattendue pour ce jeton." => "Unexpected response from Twitch for this token.",
        "Jeton refusé par Twitch (expiré ou invalide)." => "Token rejected by Twitch (expired or invalid).",
        "Connexion refusée avec ce jeton." => "Connection refused with this token.",
        // Premier lancement
        "Bienvenue dans Velka" => "Welcome to Velka",
        "Ils sont installés dans votre dossier utilisateur, sans droits administrateur." => {
            "They are installed in your user folder, without administrator rights."
        }
        "Tout installer" => "Install all",
        "Installation en cours…" => "Installing…",
        // Flux et erreurs
        "La chaîne est hors ligne." => "The channel is offline.",
        "Pas de connexion à Twitch. Vérifiez votre réseau." => "Can't reach Twitch. Check your network.",
        "Pas de connexion Internet." => "No Internet connection.",
        "La connexion Internet s'est coupée." => "The Internet connection was lost.",
        "Licences des composants" => "Component licenses",
        "Le lecteur vidéo utilise libmpv (mpv) et FFmpeg, sous licence LGPL version 2.1 ou ultérieure. Vous pouvez remplacer libmpv par votre propre version dans {section}." => "The video player uses libmpv (mpv) and FFmpeg, licensed under the LGPL version 2.1 or later. You can replace libmpv with your own build in {section}.",
        "Code source et compilation" => "Source code and build scripts",
        "Le flux met trop de temps à démarrer." => "The stream is taking too long to start.",
        "Le flux s'est interrompu :" => "The stream was interrupted:",
        "arrêt inattendu" => "unexpected stop",
        "Impossible de lancer Streamlink :" => "Couldn't start Streamlink:",
        "Préparation…" => "Preparing…",
        "libmpv est introuvable. Réinstallez les outils depuis les réglages." => {
            "libmpv not found. Reinstall the tools from Settings."
        }
        "Le rendu OpenGL n'est pas disponible." => "OpenGL rendering is not available.",
        "Streamlink est introuvable. Réinstallez les outils depuis les réglages." => {
            "Streamlink not found. Reinstall the tools from Settings."
        }
        "Aucun port local disponible." => "No local port available.",
        // Outils
        "Impossible de joindre GitHub :" => "Couldn't reach GitHub:",
        "Réponse GitHub illisible :" => "Unreadable GitHub response:",
        "Versions des composants non validées par l'éditeur." => "Component versions not approved by the publisher.",
        "Archive validée introuvable sur GitHub." => "Approved archive not found on GitHub.",
        "Adresse de téléchargement inattendue." => "Unexpected download address.",
        "Empreinte du fichier téléchargé incorrecte." => "Downloaded file checksum doesn't match.",
        "Aucune archive compatible dans la dernière version." => "No compatible archive in the latest release.",
        "Téléchargement impossible :" => "Download failed:",
        "Écriture impossible :" => "Write failed:",
        // Erreurs de mise à jour de l'application et du lecteur
        "Téléchargement refusé par le serveur." => "Download refused by the server.",
        "Fichier reçu invalide (taille ou empreinte différente)." => "Received file is invalid (size or checksum doesn't match).",
        "Serveur injoignable :" => "Server unreachable:",
        "Remplacement impossible :" => "Replacement failed:",
        "Relance impossible :" => "Restart failed:",
        "Impossible de charger libmpv :" => "Couldn't load libmpv:",
        "Fonction libmpv manquante :" => "Missing libmpv function:",
        "Le lecteur n'a pas pu démarrer." => "The player couldn't start.",
        "Initialisation du lecteur impossible :" => "Player initialization failed:",
        "Rendu vidéo impossible :" => "Video rendering failed:",
        "Option du lecteur refusée :" => "Player option refused:",
        "Propriété du lecteur refusée :" => "Player property refused:",
        "Commande du lecteur refusée :" => "Player command refused:",
        "Téléchargement interrompu :" => "Download interrupted:",
        "Le fichier téléchargé est vide." => "The downloaded file is empty.",
        "Recherche de la dernière version…" => "Looking for the latest version…",
        "Dossier inaccessible :" => "Folder not accessible:",
        "Téléchargement…" => "Downloading…",
        "Extraction…" => "Extracting…",
        "Extraction impossible :" => "Extraction failed:",
        "Archive invalide :" => "Invalid archive:",
        "streamlink.exe introuvable dans l'archive." => "streamlink.exe not found in the archive.",
        "Structure d'archive inattendue." => "Unexpected archive layout.",
        "libmpv-2.dll introuvable dans l'archive." => "libmpv-2.dll not found in the archive.",
        "Installation impossible :" => "Installation failed:",
        "Fichier installé introuvable ou vide." => "Installed file missing or empty.",
        "Installé" => "Installed",
        // Projet libre (À propos, accueil, enregistrement des jetons)
        "Lecteur Twitch léger avec multistream, chat et notifications." => "Lightweight Twitch player with multistream, chat and notifications.",
        "Quelques composants sont nécessaires pour lire les lives Twitch." => "A few components are needed to play Twitch streams.",
        "Logiciel libre sous licence GPL-3.0 ou ultérieure." => "Free software licensed under GPL-3.0 or later.",
        "Application non affiliée à Twitch." => "Not affiliated with Twitch.",
        "Code source sur GitHub" => "Source code on GitHub",
        "Soutenir le projet" => "Support the project",
        "La connexion Twitch n'a pas pu être enregistrée de façon sécurisée (trousseau du système indisponible) : elle sera perdue à la fermeture de Velka." => "Your Twitch sign-in could not be saved securely (system keychain unavailable): it will be lost when Velka closes.",
        // Rappel de soutien
        "Velka est gratuit et le restera. Si l'application vous plaît, un petit don aide à la faire vivre." => "Velka is free and will stay free. If you like the app, a small donation helps keep it going.",
        "Plus tard" => "Later",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduction_et_repli() {
        // On teste la table directement : la langue courante est partagée entre les tests.
        assert_eq!(english("Accueil"), Some("Home"));
        assert_eq!(english("Texte sans traduction"), None);
        assert_eq!(LANGUAGES[0].code, "fr");
        assert_eq!(LANGUAGES[1].code, "en");
    }

    /// Textes passés à `tr` qui n'ont volontairement pas de traduction (identiques en anglais).
    const SANS_TRADUCTION: &[&str] = &[];

    /// Lit une chaîne Rust littérale qui commence juste après le guillemet ouvrant.
    fn literal(s: &str) -> Option<String> {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '\n' => {
                        // Continuation de ligne : les espaces du début de la ligne suivante sont ignorés.
                        let rest = chars.as_str().trim_start();
                        chars = rest.chars();
                    }
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
        None
    }

    /// Tous les `tr("…")` littéraux d'un fichier source.
    fn tr_calls(src: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = src;
        while let Some(i) = rest.find("tr(") {
            let before = rest[..i].chars().next_back();
            let after = &rest[i + 3..];
            rest = after;
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if let Some(body) = after.trim_start().strip_prefix('"') {
                if let Some(text) = literal(body) {
                    found.push(text);
                }
            }
        }
        found
    }

    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    #[test]
    fn extraction_des_textes() {
        let src = "a(tr(\"Un\")); attr(\"Non\"); crate::i18n::tr(\n  \"Deux \\\"x\\\"\");";
        assert_eq!(
            tr_calls(src),
            vec!["Un".to_string(), "Deux \"x\"".to_string()]
        );
    }

    /// Clés françaises de la table anglaise (lues dans ce fichier source).
    fn english_keys() -> Vec<String> {
        let src = include_str!("i18n.rs");
        let body = src
            .split("pub(crate) fn english")
            .nth(1)
            .unwrap()
            .split("_ => return None")
            .next()
            .unwrap();
        body.lines()
            // Entrées sur une ligne (`"fr" => "en",`) et entrées dont l'anglais est sur la ligne suivante (`"fr" => {`).
            .filter(|l| l.contains("\" => \"") || l.trim_end().ends_with("\" => {"))
            .filter_map(|l| literal(l.trim_start().strip_prefix('"')?))
            .collect()
    }

    /// Chaque fichier de langue traduit tous les textes de la table anglaise, sans clé inconnue ni traduction vide.
    #[test]
    fn chaque_langue_est_complete() {
        let keys = english_keys();
        assert!(keys.len() > 300, "{} clés", keys.len());
        let mut errors = Vec::new();
        for (i, l) in LANGUAGES.iter().enumerate().skip(2) {
            let t = table(i).unwrap();
            for k in &keys {
                if !t.contains_key(k.as_str()) {
                    errors.push(format!("{} : manque « {k} »", l.code));
                }
            }
            for k in t.keys() {
                if english(k).is_none() {
                    errors.push(format!("{} : clé inconnue « {k} »", l.code));
                }
            }
            let lines = l.table.unwrap().lines().filter(|x| !x.is_empty()).count();
            if lines != t.len() {
                errors.push(format!(
                    "{} : {} lignes pour {} traductions (doublon ou ligne vide)",
                    l.code,
                    lines,
                    t.len()
                ));
            }
        }
        assert!(
            errors.is_empty(),
            "{} problème(s) :
{}",
            errors.len(),
            errors.iter().take(40).cloned().collect::<Vec<_>>().join(
                "
"
            )
        );
    }

    #[test]
    fn langue_du_systeme() {
        assert_eq!(index_for_locale("es-ES"), Some(2));
        assert_eq!(index_for_locale("pt-PT"), Some(3));
        assert_eq!(index_for_locale("zh-Hant-TW"), Some(14));
        assert_eq!(index_for_locale("zh-CN"), Some(13));
        assert_eq!(index_for_locale("nn-NO"), Some(22));
        assert_eq!(index_for_locale("ar-SA"), None);
        assert_eq!(LANGUAGES.iter().filter(|l| l.cjk.is_some()).count(), 4);
    }

    #[test]
    fn chaque_texte_a_sa_traduction() {
        let mut files = Vec::new();
        rust_files(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut missing = Vec::new();
        for f in files.iter().filter(|f| !f.ends_with("i18n.rs")) {
            let src = std::fs::read_to_string(f).unwrap();
            for text in tr_calls(&src) {
                if english(&text).is_none() && !SANS_TRADUCTION.contains(&text.as_str()) {
                    missing.push(format!("{} : {text}", f.display()));
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "Textes sans traduction anglaise ({}) :\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
}
