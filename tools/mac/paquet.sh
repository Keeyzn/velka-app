#!/bin/bash
# Fabrique Velka.app et son zip à partir de l'exécutable Mac compilé.
# Utilisé par .github/workflows/release.yml, sur un Mac seulement (sips, iconutil, codesign, ditto).
#   tools/mac/paquet.sh <exécutable velka> <dossier de sortie>
# Résultat : <sortie>/Velka.app et <sortie>/Velka-mac.zip. Signature ad hoc (pas de compte Apple Developer) : au
# premier lancement, Réglages Système > Confidentialité et sécurité > « Ouvrir quand même ».
set -euo pipefail

exe="$1"
out="$2"
root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(tr -d '\r' < "$root/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)"
[ -n "$version" ] || { echo "version introuvable dans Cargo.toml" >&2; exit 1; }

mkdir -p "$out"
rm -f "$out/VERSION"
app="$out/Velka.app"
rm -rf "$app" "$out/Velka-mac.zip" "$out/Velka-mac.dmg"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$exe" "$app/Contents/MacOS/velka"
chmod 755 "$app/Contents/MacOS/velka"

# Icône : tailles demandées par macOS, tirées de l'icône 256 px de l'application (512 px agrandie).
iconset="$(mktemp -d)/velka.iconset"
mkdir -p "$iconset"
src="$root/assets/logo/app-icon-256.png"
# Marge transparente de 20 % autour de l'icône, comme les icônes macOS (voir icone.swift).
swift "$root/tools/mac/icone.swift" "$src" "$iconset"
iconutil -c icns "$iconset" -o "$app/Contents/Resources/velka.icns"

# Identifiant app.velka : même préfixe que le LaunchAgent app.velka.login (src/procs_mac.rs).
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Velka</string>
	<key>CFBundleDisplayName</key>
	<string>Velka</string>
	<key>CFBundleIdentifier</key>
	<string>app.velka</string>
	<key>CFBundleExecutable</key>
	<string>velka</string>
	<key>CFBundleIconFile</key>
	<string>velka.icns</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>${version}</string>
	<key>CFBundleVersion</key>
	<string>${version}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.entertainment</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

# Signature ad hoc de tout le paquet (obligatoire sur Apple Silicon ; sans compte Apple, pas de notarisation).
codesign --force --sign - --timestamp=none "$app"
codesign --verify --strict --verbose=2 "$app"

ditto -c -k --keepParent "$app" "$out/Velka-mac.zip"
ls -l "$out/Velka-mac.zip"

# Image disque : Velka.app à côté d'un raccourci « Applications », pour glisser l'appli au lieu de la lancer depuis
# Téléchargements (une appli lancée depuis l'image disque ne peut pas se mettre à jour ni se désinstaller proprement).
rm -f "$out/Velka-mac.dmg"
dmg_src="$(mktemp -d)/Velka"
mkdir -p "$dmg_src"
ditto "$app" "$dmg_src/Velka.app"
ln -s /Applications "$dmg_src/Applications"
hdiutil create -quiet -volname "Velka" -srcfolder "$dmg_src" -ov -format UDZO "$out/Velka-mac.dmg"
rm -rf "$(dirname "$dmg_src")"
ls -l "$out/Velka-mac.dmg"
echo "$version" > "$out/VERSION"
