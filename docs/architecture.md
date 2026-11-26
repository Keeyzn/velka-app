# Architecture de Velka

## Intention
Lecteur Twitch **intégré, une seule fenêtre** (vidéo et chat), léger, pour Windows 10/11 x64, macOS (Apple Silicon) et
Linux x86_64. Interface en français, traduite en 24 autres langues. Streamlink et libmpv sont téléchargés à la demande
et restent invisibles. Priorités : 1) une vraie application intégrée, 2) légère (exe < 20 Mo), 3) facile à installer.

## Pile
Rust stable, édition 2021. `eframe`/`egui` 0.37 (`glow`, `default_fonts`, `clipboard`, `links` : ne pas retirer),
`libloading` (libmpv chargée dynamiquement), `ureq` 3, `tungstenite`, `serde`, `zip`, `sevenz-rust2`, `image`,
`windows` 0.62, `winresource`. Police Inter embarquée. Release : `lto`, `panic="abort"`, `strip`, `debug = false`.
`.cargo/config.toml` : `crt-static` sous Windows (ne pas retirer). Espace de travail Cargo : application `velka`,
`crates/commun` (signatures de publication, composants tiers, secrets chiffrés) et `tools/sign-release` (clé de
publication et manifeste de mise à jour signé, sur le PC du mainteneur).

## Modules `src/`
| Module | Rôle |
|---|---|
| `main.rs`, `cli.rs` | Entrée, instance unique, géométrie de fenêtre, options (`velka <chaîne> [--pip] [--no-chat] …`, valables pour la session). |
| `app/mod.rs` + `app/*.rs` | `VelkaApp`, `View`, `Msg`, boucle `ui`. Un fichier par thème : `maj`, `nav`, `statuts`, `lecteur`, `compte`, `outils`, `messages`, `fenetre`. |
| `app/home.rs`, `sidebar.rs`, `watch.rs`, `mini.rs`, `multi.rs` | Accueil, barre latérale, page lecteur, mini-lecteur flottant, multistream (4 lives). |
| `app/settings.rs` + `settings/cartes.rs` | Réglages en overlay ; une méthode par carte ; recherche `SEARCH_INDEX`. |
| `chat/` | Client IRC WebSocket, analyse des messages, emotes (Twitch, 7TV, BTTV, FFZ), badges. |
| `ui/` | `theme.rs` (charte, composants), `chat.rs`, `first_run.rs`, `fonts.rs` (polices CJK du système à la demande), `logo.rs`. |
| `mpv.rs` | Bindings libmpv + rendu OpenGL dans egui (FBO puis blit dans le `PaintCallback`) ; options du lecteur `MPV_OPTIONS`. |
| `stream.rs` | Streamlink en processus caché, serveur HTTP local ; arguments `STREAMLINK_ARGS`. |
| `procs*.rs` | Fonctions système par plateforme : Job Object (Windows), notifications, démarrage avec le système, instance unique. |
| `tools.rs` | Streamlink/libmpv : détection, téléchargement, versions validées (`commun::outils`), extraction. |
| `twitch.rs`, `follows.rs`, `auth.rs` | GraphQL non officiel (seul endroit), Helix officiel, connexion Device Code (sans secret). |
| `avatars.rs`, `thumbs.rs`, `cache.rs` | Photos de profil (cache 7 j), aperçus des lives, purge du cache d'images. |
| `update.rs`, `publication.rs` | Mise à jour depuis les Releases GitHub (`GITHUB_REPO`), manifeste `velka-maj.json` signé Ed25519, retour arrière automatique. |
| `config.rs`, `history.rs`, `uninstall.rs`, `i18n.rs`, `logging.rs`, `net.rs`, `crash.rs`, `diag.rs`, `smoke.rs` | Réglages, historique, désinstallation, traductions, journal, agent HTTP unique, rapport de plantage local, diagnostic, essai de démarrage. |

## Lecture
Streamlink tourne en fond (sans fenêtre, serveur HTTP local `127.0.0.1:<port libre>`), libmpv lit ce port. Le lecteur
mpv est créé à la demande (contexte OpenGL courant). Sous Windows, Streamlink est dans un Job Object tué à la fermeture.
Le jeton de connexion Twitch ne sert **qu'au chat et aux chaînes suivies** : il n'est jamais donné à Streamlink.

## Mises à jour
Le workflow `.github/workflows/release.yml` compile les trois systèmes et crée une Release brouillon (fichiers +
`SHA256SUMS.txt`). Sur le PC du mainteneur, `sign-release sign` vérifie les empreintes et ajoute `velka-maj.json`
(version, puis pour chaque plateforme : fichier, taille, SHA-256, signature Ed25519 de la clé de publication) ; la
Release est ensuite publiée à la main (`docs/RELEASE.md`). L'app ne lit que la dernière Release publiée (ni brouillon
ni pré-version) par l'API GitHub, vérifie la signature avec `assets/publication.pub`, télécharge le fichier depuis
la Release du même dépôt, vérifie taille et empreinte, puis remplace l'application. Une version qui ne démarre pas est
remplacée par la précédente au lancement suivant.

## Données
`%LOCALAPPDATA%\Velka\` (Windows), `~/Library/Application Support/Velka` (macOS), `~/.local/share/Velka` (Linux) :
`settings.json` (lecture tolérante, écriture atomique, numéro de format `schema`), `secrets.dat` (jetons chiffrés :
DPAPI, Trousseau ou trousseau du bureau ; jamais écrits en clair), `history.txt`, `log.txt`, `crash.txt` (local,
jamais envoyé), `mpv.log` (seulement avec `--diag`), `tools/`, `cache/`.

## Règles de code
- Clippy `-D warnings` (pas de `#![allow(dead_code)]`), aucun `unwrap()`/`expect()` hors tests.
- Tout texte visible : `crate::i18n::tr("…")` + anglais dans `i18n.rs` + une ligne dans chaque `src/lang/*.tsv` (test `chaque_langue_est_complete`). Ne jamais réordonner `i18n::LANGUAGES`.
- Nouveau réglage : champ + défaut (= comportement d'origine) + bornes dans `config.rs`, ligne `t::switch_row`/`t::control_row`, aide dans `help.rs`, entrée dans `SEARCH_INDEX`.
- Couleurs par `t::TEXT()` etc. ; pas d'emoji ni de glyphe spécial (Inter ne les a pas) ; icônes dessinées ; infobulles `t::show_tip` ; `t::focus_ring` sur tout composant dessiné à la main. Conventions : `DESIGN.md`.
- Ne jamais coder d'URL de téléchargement d'outil (elles viennent de l'API GitHub) ; ne jamais journaliser un jeton.

## Pièges
- egui 0.37 : `App::ui`, `Panel::…show(ui)`, `Id::unique` ; `ui.horizontal_centered`/`with_layout` remplissent la hauteur ; `ui.horizontal` dans un parent `right_to_left` inverse l'ordre.
- Twitch : GraphQL ≤ 10 alias par requête (400 sinon) ; le `Client-ID` GraphQL public n'est pas celui de l'application (Helix, connexion).
- Windows : l'instance unique empêche de lancer une 2e copie pendant un essai ; libmpv verrouille sa DLL.
- Distribution : les exécutables ne sont pas signés (Authenticode) ; SmartScreen propose « Exécuter quand même ».
