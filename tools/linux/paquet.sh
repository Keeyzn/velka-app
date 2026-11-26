#!/bin/bash
# Fabrique l'AppImage de Velka (Linux x86_64) à partir d'un exécutable déjà compilé. Lancé dans Ubuntu 22.04 par
# `tools/linux/wsl.sh paquet` (glibc 2.35 : l'AppImage marche sur les distributions de 2022 et plus récentes).
#   bash tools/linux/paquet.sh <exécutable velka> <dossier de sortie>
# Résultat : <dossier>/Velka-x86_64.AppImage. Une AppImage = le « runtime » officiel d'AppImage (AppImage/type2-runtime,
# étiquette et empreinte figées ci-dessous ; il n'a pas besoin de libfuse2 sur la machine) suivi d'une image squashfs
# du dossier de l'application (AppDir). Streamlink et libmpv n'y sont pas : l'appli les installe elle-même.
# Outils demandés : mksquashfs (paquet squashfs-tools), curl, sha256sum.
set -euo pipefail

exe="$(realpath "$1")"
out="$(realpath -m "$2")"
root="$(cd "$(dirname "$0")/../.." && pwd)"
runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
runtime_sha="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/velka-paquet"
runtime="$cache/runtime-x86_64-20251108"

mkdir -p "$cache" "$out"
version="$(tr -d '\r' < "$root/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)"
rm -f "$out/VERSION"
if ! echo "$runtime_sha  $runtime" | sha256sum -c - > /dev/null 2>&1; then
  curl -sSfL -o "$runtime.part" "$runtime_url"
  echo "$runtime_sha  $runtime.part" | sha256sum -c - > /dev/null || { echo "runtime AppImage : empreinte inattendue" >&2; exit 1; }
  mv "$runtime.part" "$runtime"
fi

work="$(mktemp -d)"
app="$work/Velka.AppDir"
mkdir -p "$app/usr/bin" "$app/usr/share/applications" "$app/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$exe" "$app/usr/bin/velka"
install -m 644 "$root/assets/logo/app-icon-256.png" "$app/usr/share/icons/hicolor/256x256/apps/velka.png"
cp "$app/usr/share/icons/hicolor/256x256/apps/velka.png" "$app/velka.png"
ln -s velka.png "$app/.DirIcon"
# Lanceur interne de l'AppImage ; celui du menu des applications est écrit par Velka au premier lancement
# (`procs::ensure_start_menu_shortcut`, il vise le fichier AppImage). `velka` : identifiant de fenêtre.
cat > "$app/velka.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Velka
Exec=velka
Icon=velka
Terminal=false
Categories=AudioVideo;Video;Player;
StartupWMClass=velka
DESKTOP
cp "$app/velka.desktop" "$app/usr/share/applications/velka.desktop"
cat > "$app/AppRun" <<'SH'
#!/bin/sh
# Point d'entrée de l'AppImage : lance Velka depuis l'image montée (APPIMAGE est posé par le runtime).
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/velka" "$@"
SH
chmod 755 "$app/AppRun"
if command -v desktop-file-validate > /dev/null; then desktop-file-validate "$app/velka.desktop"; fi

image="$work/velka.squashfs"
mksquashfs "$app" "$image" -root-owned -noappend -comp zstd -quiet
result="$out/Velka-x86_64.AppImage"
cat "$runtime" "$image" > "$result.part"
chmod 755 "$result.part"
mv "$result.part" "$result"
rm -rf "$work"
ls -l "$result"
sha256sum "$result"
echo "$version" > "$out/VERSION"
