# Compiler Velka

## Règle de publication
Les versions publiques (Releases GitHub) sont compilées **uniquement par GitHub Actions**
(`.github/workflows/release.yml`, déclenché par une étiquette `v*`), **jamais sur un PC personnel**. Les compilations
locales servent au développement et aux essais.

## Prérequis
- Rust stable (`rustup`), avec `clippy`.
- Windows : outils de compilation Visual Studio (MSVC). Installateur : Inno Setup 6 (facultatif).
- Linux (Ubuntu 22.04 ou plus récent) :
  `sudo apt-get install pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libdbus-1-dev`
  (AppImage : en plus `squashfs-tools zip file desktop-file-utils`).
- macOS (Apple Silicon) : Xcode Command Line Tools.

Streamlink et libmpv ne sont pas nécessaires pour compiler : l'application les télécharge au premier lancement
(« Tout installer »), ou utilise les chemins personnalisés des Réglages.

## Commandes
- Lancer : `cargo run`
- Instance isolée (essais, captures d'écran ; versions de développement seulement) : définir `VELKA_DATA_DIR` sur un
  dossier vide, puis `cargo run`. Réglages, favoris et composants y sont séparés des données habituelles.
- Vérifier : `cargo clippy --workspace --all-targets -- -D warnings` puis `cargo test --workspace`
- Licences et failles des dépendances : `cargo deny check` (outil `cargo-deny`)
- Compiler : `cargo build --release -p velka`
- Notices des composants tiers (après un changement de `Cargo.lock`) : `python tools/notices/generer.py`
- Paquets : `ISCC.exe installer\velka.iss` (Windows), `bash tools/mac/paquet.sh <exe> <dossier>` (macOS),
  `bash tools/linux/paquet.sh <exe> <dossier>` (Linux) ; sous Windows, `bash tools/linux/wsl.sh paquet` passe par WSL.

## Chemins de compilation neutres
Rust inscrit dans l'exécutable l'emplacement des sources (messages d'erreur internes), donc le dossier du projet et
celui de Cargo (qui contient le nom du compte). Pour un exécutable que l'on partage, remplacez-les par des noms neutres
avec `--remap-path-prefix`. La variable `RUSTFLAGS` remplace les options de `.cargo/config.toml` : sous Windows, redonnez
aussi `+crt-static`.

Windows (PowerShell, depuis la racine du projet) :

```powershell
$env:RUSTFLAGS = "-C target-feature=+crt-static --remap-path-prefix=$PWD=/build --remap-path-prefix=$env:USERPROFILE\.cargo=/cargo --remap-path-prefix=$env:USERPROFILE\.rustup=/rustup"
cargo build --release -p velka
```

macOS et Linux :

```sh
export RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=${RUSTUP_HOME:-$HOME/.rustup}=/rustup"
cargo build --release -p velka
```

Le profil `release` de `Cargo.toml` retire déjà les symboles (`strip = true`, `debug = false`).

## Publication (mainteneur)
Voir `docs/RELEASE.md`. Le workflow compile Windows, macOS et Linux et crée une Release **brouillon** avec
`velka.exe`, `velka-setup.exe`, `Velka-mac.zip`, `Velka-mac.dmg`, `Velka-x86_64.AppImage` et `SHA256SUMS.txt`. Il ne
signe rien : `velka-maj.json` est signé sur le PC du mainteneur avec `tools/sign-release`, puis la Release est publiée à
la main. Signature de code et avertissements du système : `docs/DISTRIBUTION.md`.
