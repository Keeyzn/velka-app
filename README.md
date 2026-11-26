<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/logo-white.svg">
    <img src="assets/logo/logo.svg" alt="Velka" width="96">
  </picture>
</p>

# Velka

Lecteur Twitch léger pour Windows, macOS et Linux : la vidéo et le chat dans une seule fenêtre, avec multistream,
mini-lecteur et notifications. Logiciel libre et gratuit, sous licence GPL-3.0-or-later.

> Velka n'est pas affilié à Twitch. « Twitch » est une marque de Twitch Interactive, Inc.

*English summary: Velka is a free and open-source lightweight Twitch player for Windows, macOS and Linux (video and
chat in one window, multistream, mini-player, notifications). The interface is available in 25 languages.*

## Fonctionnalités

- **Une seule fenêtre** : vidéo et chat côte à côte, chat à gauche ou à droite, repliable.
- **Multistream** : jusqu'à 4 lives en même temps.
- **Mini-lecteur et PiP** : la vidéo continue dans une petite fenêtre, toujours au premier plan si vous le souhaitez.
- **Chat complet** : émotes Twitch, 7TV, BTTV et FFZ, badges, écriture dans le chat une fois connecté.
- **Favoris et notifications** : vos chaînes favorites sur l'accueil, une notification quand l'une d'elles passe en
  direct ; avec la connexion Twitch, vos chaînes suivies aussi.
- **Lecture** : qualité au choix, faible latence, décodage matériel, captures d'écran, minuteur de veille.
- **Confort** : thèmes clair et sombres, couleur d'accent, raccourcis clavier modifiables, taille de l'interface,
  options d'accessibilité (contraste élevé, focus renforcé, animations réduites), 25 langues.
- **Léger et discret** : aucune publicité ajoutée, aucune télémétrie, aucune statistique d'usage. Les jetons de
  connexion sont chiffrés sur votre ordinateur.

## Installation

Téléchargez la dernière version dans les [Releases](https://github.com/Keeyzn/velka-app/releases/latest).

| Système | Fichier |
|---|---|
| Windows 10 / 11 (x64) | `velka-setup.exe` (installateur, sans droits administrateur) ou `velka.exe` (portable) |
| macOS 11 ou plus récent (Apple Silicon) | `Velka-mac.dmg` |
| Linux x86_64 (X11 ou Wayland) | `Velka-x86_64.AppImage` |

- **Windows** : si « Windows a protégé votre ordinateur » s'affiche, cliquez sur « Informations complémentaires »,
  puis « Exécuter quand même ».
- **macOS** : glissez Velka dans Applications. Au premier lancement, faites un clic droit sur Velka, puis « Ouvrir ».
- **Linux** : rendez le fichier exécutable (`chmod +x Velka-x86_64.AppImage`), puis ouvrez-le. Sans FUSE :
  `./Velka-x86_64.AppImage --appimage-extract-and-run`.

Les exécutables ne sont pas signés par un certificat payant : voir [docs/DISTRIBUTION.md](docs/DISTRIBUTION.md) pour
les avertissements du système et la vérification des fichiers (`SHA256SUMS.txt`).

Au premier lancement, cliquez sur **« Tout installer »** : Velka télécharge ses deux composants,
[Streamlink](https://streamlink.github.io) et libmpv, depuis leurs dépôts officiels, après vérification de leur
empreinte. Tapez ensuite le nom d'une chaîne dans la recherche, ou ouvrez un live depuis l'accueil.

**Mises à jour** : Velka les propose tout seul (Réglages > À propos). Chaque mise à jour est vérifiée par une
signature Ed25519 avant d'être installée.

**Désinstallation** : Réglages > Avancé et Système > « Désinstaller » (supprime l'application et ses données), ou par
les outils habituels du système.

## Ligne de commande

```
velka <chaîne> [--pip] [--no-chat] [--screen 2] [--volume 80] [--on-top] [--chat-left] [--stable] [--quality 720p] [--diag]
```

## Compiler et contribuer

- Compiler depuis les sources : [docs/build.md](docs/build.md)
- Architecture : [docs/architecture.md](docs/architecture.md) · Interface : [DESIGN.md](DESIGN.md)
- Contribuer : [CONTRIBUTING.md](CONTRIBUTING.md) · Code de conduite : [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Signaler une faille de sécurité : [SECURITY.md](SECURITY.md)

## Licence

Copyright (C) 2026 Keeyzn. Velka est distribué sous licence [GNU GPL version 3 ou ultérieure](LICENSE). Les bibliothèques et composants tiers
(dont libmpv sous LGPL, Streamlink sous BSD-2-Clause et la police Inter sous OFL) sont listés dans
[installer/THIRD-PARTY-NOTICES.txt](installer/THIRD-PARTY-NOTICES.txt).
