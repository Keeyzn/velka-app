#!/bin/bash
# Compilation et vérifications Linux depuis Windows, dans WSL2 (développement seulement : les versions publiques sont
# compilées uniquement par GitHub Actions).
#   bash tools/linux/wsl.sh verifier   clippy + tests sous Linux (Ubuntu 24.04)
#   bash tools/linux/wsl.sh build      build release Linux (~/velka-target/release/velka)
#   bash tools/linux/wsl.sh tout       verifier + build
#   bash tools/linux/wsl.sh paquet     build release et AppImage d'essai dans Ubuntu 22.04 (glibc 2.35), copiée dans
#                                      ../velka-paquet-linux/Velka-x86_64.AppImage côté Windows
# Ubuntu reçoit une copie du dossier de travail (fichiers suivis et nouveaux, sans les fichiers ignorés), dans ~/velka,
# remplacée à chaque envoi. Les compilations restent dans ~/velka-target (incrémentales). Prérequis dans la
# distribution : Rust (rustup), et les paquets de développement listés dans docs/build.md.
set -euo pipefail

action="${1:-verifier}"
distro="Ubuntu-24.04"
[ "$action" = paquet ] && distro="Ubuntu-22.04"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

echo "Envoi du dossier de travail vers $distro (WSL)…"
git ls-files -co --exclude-standard -z | tar --null -T - -czf - |
  wsl.exe -d "$distro" -e bash -c 'rm -rf ~/velka.new && mkdir ~/velka.new && tar xzf - -C ~/velka.new && rm -rf ~/velka && mv ~/velka.new ~/velka && sed -i "s/\r$//" ~/velka/Cargo.toml'

# Chemins de compilation remplacés par des noms neutres dans l'exécutable (voir docs/build.md).
remote='source ~/.cargo/env && cd ~/velka && export CARGO_TARGET_DIR=~/velka-target && export RUSTFLAGS="--remap-path-prefix=$HOME/velka=/build --remap-path-prefix=$HOME/.cargo=/cargo --remap-path-prefix=$HOME/.rustup=/rustup"'
# Clé d'essai à part (VELKA_TROUSSEAU, versions de développement) : les essais ne touchent jamais au trousseau du bureau.
verifier='export VELKA_TROUSSEAU=~/velka-target/essai-secrets.key && cargo clippy --workspace --all-targets -- -D warnings && { cargo test --workspace > ~/velka-tests.log 2>&1 && ok=1 || ok=0; grep -E "^test result|FAILED|panicked" ~/velka-tests.log; [ $ok = 1 ]; }'
build='cargo build --release -p velka && ls -l ~/velka-target/release/velka'
paquet="$build"' && bash tools/linux/paquet.sh ~/velka-target/release/velka ~/velka-paquet'

case "$action" in
  verifier) cmd="$verifier" ;;
  build) cmd="$build" ;;
  tout) cmd="$verifier && $build" ;;
  paquet) cmd="$paquet" ;;
  *) echo "Usage : bash tools/linux/wsl.sh verifier|build|tout|paquet" >&2; exit 2 ;;
esac
wsl.exe -d "$distro" -e bash -c "$remote && $cmd"

if [ "$action" = paquet ]; then
  dest="$root/../velka-paquet-linux"
  mkdir -p "$dest"
  wsl.exe -d "$distro" -e bash -c 'cat ~/velka-paquet/Velka-x86_64.AppImage' > "$dest/Velka-x86_64.AppImage"
  echo "AppImage copiée : $dest/Velka-x86_64.AppImage ($(wc -c < "$dest/Velka-x86_64.AppImage") octets)"
fi
