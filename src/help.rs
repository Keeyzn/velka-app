//! Infobulles des réglages : un « ? » apparaît à côté d'un libellé qui a une explication ici.
//! Les clés sont les libellés **en français** ; la recherche passe par la traduction courante.
use crate::i18n::tr;

/// (libellé français, explication française).
const HELP: &[(&str, &str)] = &[
    (
        "Qualité par défaut",
        "Qualité utilisée à l'ouverture d'un live. « Source » est la qualité maximale de la chaîne ; « Audio » ne joue que le son.",
    ),
    (
        "Faible latence",
        "Réduit le retard entre le streamer et vous. Désactivez-le si la vidéo saccade ou se coupe.",
    ),
    ("Démarrer en muet", "Le live démarre sans le son ; vous le réactivez quand vous voulez."),
    (
        "Cumuler le son des lives en Multistream",
        "Sur la page Multistream, un clic sur un live fait entendre son son en plus de ceux déjà écoutés, jusqu'aux quatre lives. Le bouton du son de chaque live le coupe.",
    ),
    ("Taille du texte", "Agrandit ou réduit le texte des messages du chat."),
    ("Horodatage des messages", "Affiche l'heure devant chaque message du chat."),
    ("Chat à gauche de la vidéo", "Place le chat à gauche de la vidéo au lieu de la droite."),
    ("Toujours au premier plan", "Garde la fenêtre devant toutes les autres applications."),
    ("Fenêtre PiP au format de la vidéo", "En mini-fenêtre PiP, la fenêtre prend le format du live pour supprimer les bandes noires."),
    ("Lecteur au format de la vidéo", "Dans la fenêtre principale, l'image prend exactement le format du live (coins arrondis, sans bandes autour) et reste centrée."),
    (
        "Chat en surimpression en plein écran",
        "Affiche le chat par-dessus la vidéo quand vous êtes en plein écran.",
    ),
    (
        "Molette de la souris sur la vidéo = volume",
        "Faire rouler la molette au-dessus de la vidéo monte ou baisse le volume.",
    ),
    ("Clic sur la vidéo = pause", "Un clic sur la vidéo met en pause ou relance la lecture."),
    (
        "Volume stable",
        "Garde un niveau sonore régulier (normalisation du son) : relève les passages faibles et atténue les passages forts, d'une chaîne à l'autre et au sein d'un même live.",
    ),
    (
        "Périphérique de sortie",
        "Choisit le casque ou les enceintes pour le son des lives, sans changer le son de Windows. « Périphérique par défaut du système » suit le choix de Windows. Le changement s'applique tout de suite. Si le périphérique choisi disparaît (casque débranché), l'application revient au choix par défaut.",
    ),
    (
        "Dossier d'enregistrement",
        "Dossier où la touche S enregistre les captures d'écran. Par défaut : le dossier « Velka » de vos Images. Le dossier choisi doit accepter l'écriture ; s'il disparaît (disque débranché, dossier supprimé), l'application revient au dossier par défaut. Le changement s'applique à la prochaine capture.",
    ),
    ("Vitesse de défilement", "Vitesse de la molette dans les listes et dans le chat. À 1,0 ×, un cran de molette fait défiler 3 lignes de 75 px, de façon fluide."),
    (
        "Volume fixe au lancement",
        "À chaque lancement, l'application démarre avec le volume choisi ci-dessous, au lieu du dernier volume réglé.",
    ),
    (
        "Volume maximum",
        "100 % = volume normal. 130 % permet d'amplifier une chaîne trop silencieuse, mais le son peut saturer.",
    ),
    (
        "Masquage des contrôles",
        "Temps sans bouger la souris avant que les boutons posés sur la vidéo disparaissent.",
    ),
    ("Minuteur de veille", "Arrête le live automatiquement après la durée choisie, à partir du moment où vous changez ce réglage. Une minute avant, un compte à rebours s'affiche sur la vidéo. Le minuteur revient sur Désactivé à chaque nouveau live et à chaque lancement de l'application."),
    ("Émotes 7TV, BTTV et FFZ", "Affiche les émotes des extensions 7TV, BetterTTV et FrankerFaceZ (globales et propres à la chaîne). Elles sont chargées à l'ouverture du live."),
    ("Émotes animées", "Anime les émotes qui le sont (Twitch et extensions). Utilise un peu plus le processeur quand le chat en affiche."),
    ("Badges du chat", "Affiche les badges (abonné, modérateur, VIP…) devant les pseudos."),
    ("Netteté de l'image", "Renforce les contours de l'image (calculé par la carte graphique). Utile sur un flux flou ; trop fort, l'image paraît granuleuse."),
    ("Agrandissement de haute qualité", "Utilise un filtre d'agrandissement plus fin quand la vidéo est plus petite que la fenêtre (480p, 720p). Demande davantage à la carte graphique."),
    (
        "Décodage vidéo matériel (GPU)",
        "Utile si la vidéo saccade ou si le processeur chauffe. « Automatique » : réglage d'origine de l'application. « Activé » : la carte graphique décode la vidéo, avec retour automatique au décodage normal si elle n'y arrive pas. « Désactivé » : décodage normal par le processeur. Le changement s'applique tout de suite : le live en cours est relancé.",
    ),
    (
        "Mini-lecteur",
        "Quand vous quittez la page d'un live avec « Retour », il continue dans une petite fenêtre en bas à droite. Désactivé : il s'arrête.",
    ),
    (
        "Actualisation",
        "Fréquence à laquelle l'accueil vérifie quelles chaînes sont en direct. Plus c'est rapide, plus cela utilise internet. « Manuelle » : seulement avec le bouton Actualiser.",
    ),
    (
        "Aperçus des lives",
        "Télécharge une image du direct pour chaque carte. Désactivez pour économiser des données.",
    ),
    (
        "Chaînes suivies sur Twitch",
        "Les chaînes que vous suivez sur votre compte Twitch (il faut être connecté dans la carte « Compte Twitch »).",
    ),
    (
        "Chaînes dans la barre latérale",
        "Nombre maximum de chaînes Twitch en direct listées dans la barre de gauche (ce n'est pas un délai).",
    ),
    (
        "Chaînes sur l'accueil",
        "Nombre maximum de chaînes Twitch en direct affichées sur la page d'accueil (ce n'est pas un délai).",
    ),
    (
        "Se souvenir du jeu choisi",
        "Garde le jeu sélectionné dans la barre « Jeux en direct » au prochain lancement.",
    ),
    (
        "Clic sur une carte",
        "Ouvrir : le live s'affiche en grand. PiP : il s'ouvre directement dans une petite fenêtre flottante.",
    ),
    (
        "Mots surlignés",
        "Les messages qui contiennent un de ces mots (séparés par des virgules) sont surlignés en jaune dans le chat.",
    ),
    (
        "Recommandations de chaînes",
        "Quand une chaîne est hors ligne ou que son live se termine, propose d'autres lives : vos chaînes suivies du même jeu d'abord.",
    ),
    (
        "Nombre de recommandations",
        "Nombre de chaînes proposées quand un live est hors ligne ou terminé.",
    ),
    (
        "Vérification des mises à jour",
        "Vérifie au démarrage, puis toutes les 6 heures, si une nouvelle version est disponible. Rien n'est installé sans votre clic.",
    ),
    (
        "Boutons Précédent / Suivant de la souris",
        "Les boutons latéraux de la souris changent de page, comme dans un navigateur. Alt + flèche gauche ou droite fait pareil.",
    ),
    ("Main sur les éléments cliquables", "Le pointeur devient une main au-dessus des boutons et des cartes."),
    (
        "Animations",
        "Complet : fondus, glissements, cascades et compteurs animés. Réduit : fondus et changements de couleur seulement, sans déplacement. Désactivé : aucune animation (utile sur un ordinateur lent). Le réglage d'animations de Windows n'est pas pris en compte.",
    ),
    (
        "Rappel pour soutenir le projet",
        "Affiche de temps en temps sur l'accueil, au plus une fois par mois, un message avec un lien pour soutenir le projet.",
    ),
    (
        crate::i18n::os::NOTIFY,
        crate::i18n::os::NOTIFY_HELP,
    ),
    (
        "Chaîne ouverte au démarrage",
        "Au lancement, si cette chaîne est en direct, son live s'ouvre directement dans le lecteur. Hors ligne ou introuvable, l'accueil s'affiche normalement.",
    ),
    (
        crate::i18n::os::STARTUP,
        crate::i18n::os::STARTUP_HELP,
    ),
    (
        "Réduire dans la zone de notification à la fermeture",
        "La croix de fermeture cache la fenêtre dans la zone de notification (près de l'horloge) au lieu de quitter ; le live en cours est arrêté. Clic gauche sur l'icône : rouvrir. Clic droit : Ouvrir ou Quitter. Pour être prévenu quand un favori passe en direct, activez aussi « Notifications Windows ».",
    ),
    (
        "Composants externes",
        "État et version de Streamlink et de libmpv, mise à jour, et chemins personnalisés (facultatifs) vers streamlink.exe et libmpv-2.dll.",
    ),
    (
        "Journaux et diagnostic",
        "Ouvre le dossier du journal, copie le diagnostic (version, Windows, carte graphique, composants) ou le rapport de plantage local pour le support. Rien n'est envoyé automatiquement.",
    ),
    (
        "Cache des images",
        "Photos de profil et émotes déjà téléchargées. Les vider libère de la place ; elles se retéléchargent au besoin.",
    ),
];

/// Explication du réglage dont le libellé (déjà traduit) est `label`, si elle existe.
pub fn lookup(label: &str) -> Option<&'static str> {
    HELP.iter()
        .find(|(fr, _)| tr(fr) == label)
        .map(|(_, help)| tr(help))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recherche_par_libelle() {
        assert!(lookup("Faible latence").is_some());
        assert!(lookup("Libellé inconnu").is_none());
    }
}
