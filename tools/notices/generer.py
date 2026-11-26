#!/usr/bin/env python3
"""Génère installer/THIRD-PARTY-NOTICES.txt : licences des bibliothèques Rust embarquées dans velka.exe.

Aucun outil à installer : `cargo metadata` (déjà présent) donne les dépendances de l'exe pour Windows ; les textes de
licence sont lus dans les sources des crates (`~/.cargo/registry`). Les textes identiques sont écrits une seule fois.

À relancer quand `Cargo.lock` change (nouvelle dépendance ou nouvelle version) :
    python tools/notices/generer.py
Le script échoue (code 1) si une dépendance n'a aucun fichier de licence et n'est pas dans EXCEPTIONS.
"""
import hashlib
import json
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[2]
SORTIE = RACINE / "installer" / "THIRD-PARTY-NOTICES.txt"
CIBLE = "x86_64-pc-windows-msvc"
RACINE_PAQUET = "velka"
LOCALES = {"velka", "licfmt", "sign-release"}  # crates du dépôt : pas des tiers
PREFIXES = ("LICEN", "COPYING", "NOTICE", "UNLICENSE", "COPYRIGHT")
# Crates sans fichier de licence dans leurs sources : (licence, justification). Vide tant que tout est couvert.
EXCEPTIONS = {}

# Composants téléchargés par l'application (pas inclus dans l'installateur ni dans velka.exe).
COMPOSANTS = """\
COMPOSANTS TÉLÉCHARGÉS PAR L'APPLICATION / DOWNLOADED COMPONENTS
================================================================
Velka ne contient pas ces programmes : il les télécharge sur l'ordinateur de l'utilisateur depuis leurs dépôts
officiels, après vérification d'une empreinte signée. Ce sont des programmes séparés (Streamlink est lancé comme un
processus à part ; libmpv est une bibliothèque dynamique, remplaçable par l'utilisateur dans Réglages > Avancé et
Système > Chemins personnalisés).
Velka does not contain these programs: it downloads them to the user's computer from their official repositories,
after checking a signed checksum. They are separate programs (Streamlink runs as a separate process; libmpv is a
dynamic library that the user can replace in Settings > Advanced and System > Custom paths).

* libmpv (mpv), build LGPL (compilé avec -Dgpl=false) - licence GNU LGPL version 2.1 ou ultérieure.
  Sources de mpv : https://github.com/mpv-player/mpv
  Compilation et références exactes des sources : https://github.com/zhongfly/mpv-winbuild
* FFmpeg (bibliothèques intégrées à libmpv) - licence GNU LGPL version 2.1 ou ultérieure (build sans --enable-gpl).
  Sources : https://ffmpeg.org/download.html
* Streamlink - licence BSD 2-Clause (Copyright (c) 2011-2016 Christopher Rosell, (c) 2016-2026 Streamlink Team).
  Sources : https://github.com/streamlink/streamlink
* FFmpeg fourni dans l'archive de Streamlink - licence GNU GPL version 3 (le texte de licence est dans l'archive,
  fichier ffmpeg\\LICENSE.txt). Programme séparé, lancé par Streamlink ; Velka ne le charge pas.
  Sources : https://ffmpeg.org/download.html
* Python (fourni dans l'archive de Streamlink) - licence PSF (texte dans l'archive, Python\\LICENSE.txt).
* macOS et Linux : Streamlink et libmpv (LGPL : mpv compilé avec -Dgpl=false, FFmpeg sans --enable-gpl) viennent
  d'archives fabriquées pour Velka, avec leurs scripts de fabrication et les références exactes des sources :
  https://github.com/Keeyzn/velka-composants-mac et https://github.com/Keeyzn/velka-composants-linux
  (Python de https://github.com/astral-sh/python-build-standalone, licence PSF).

Police Inter (SIL Open Font License 1.1), incluse dans l'application : Copyright (c) The Inter Project Authors
(https://github.com/rsms/inter). Texte complet plus bas, avec ceux des polices par défaut d'egui (crate
epaint_default_fonts : Hack, Ubuntu, Noto Emoji, emoji-icon-font).
"""


def metadata():
    sortie = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", CIBLE, "--locked", "--offline"],
        cwd=RACINE, check=True, capture_output=True, text=True, encoding="utf-8",
    ).stdout
    return json.loads(sortie)


def dependances_normales(meta):
    paquets = {p["id"]: p for p in meta["packages"]}
    noeuds = {n["id"]: n for n in meta["resolve"]["nodes"]}
    racine = next(p["id"] for p in meta["packages"] if p["name"] == RACINE_PAQUET)
    vus, pile = set(), [racine]
    while pile:
        i = pile.pop()
        if i in vus:
            continue
        vus.add(i)
        for d in noeuds[i]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):
                pile.append(d["pkg"])
    return [paquets[i] for i in sorted(vus, key=lambda x: (paquets[x]["name"], paquets[x]["version"]))
            if paquets[i]["name"] not in LOCALES]


def fichiers_de_licence(paquet):
    dossier = pathlib.Path(paquet["manifest_path"]).parent
    trouves = []
    for f in sorted(dossier.iterdir()):
        if f.is_file() and f.name.upper().startswith(PREFIXES):
            trouves.append(f)
    # Les polices par défaut d'egui rangent leurs licences (Hack, OFL, Ubuntu, emoji) dans un sous-dossier.
    if paquet["name"] == "epaint_default_fonts":
        trouves.extend(sorted((dossier / "fonts").glob("*.txt")))
    return trouves


BSL = """Boost Software License - Version 1.0 - August 17th, 2003

Permission is hereby granted, free of charge, to any person or organization
obtaining a copy of the software and accompanying documentation covered by
this license (the "Software") to use, reproduce, display, distribute,
execute, and transmit the Software, and to prepare derivative works of the
Software, and to permit third-parties to whom the Software is furnished to
do so, all subject to the following:

The copyright notices in the Software and this entire statement, including
the above license grant, this restriction and the following disclaimer,
must be included in all copies of the Software, in whole or in part, and
all derivative works of the Software, unless such copies or derivative
works are solely in the form of machine-executable object code generated by
a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE."""

MIT = """MIT License

Copyright (c) {auteurs}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""


def auteurs_sans_courriel(paquet):
    noms = [a.split("<")[0].strip() for a in (paquet.get("authors") or [])]
    return ", ".join(n for n in noms if n) or paquet["name"]


def textes_standard(paquet, textes):
    """Textes de licence standard pour une crate qui n'en embarque aucun (les crates d'egui et d'AccessKit publient leur
    licence dans le dépôt, pas dans le paquet). Liste vide si la licence n'est pas reconnue."""
    licence = paquet.get("license") or ""
    trouves = []
    if "BSL-1.0" in licence:
        trouves.append(BSL)
    if "MIT" in licence:
        trouves.append(MIT.format(auteurs=auteurs_sans_courriel(paquet)))
    if "Apache-2.0" in licence:
        apache = next((t for t, _ in textes.values() if "Apache License" in t and "Version 2.0, January 2004" in t), None)
        if apache:
            trouves.append(apache)
    return trouves


def main():
    meta = metadata()
    paquets = dependances_normales(meta)
    textes = {}      # empreinte -> (texte, [crates])
    sans_licence = []
    en_attente = []
    lignes_crates = []
    for p in paquets:
        etiquette = f'{p["name"]} {p["version"]}'
        lignes_crates.append(f'{etiquette} - {p.get("license") or "licence non indiquée"}'
                             + (f' - {p["repository"]}' if p.get("repository") else ""))
        fichiers = fichiers_de_licence(p)
        for f in fichiers:
            texte = f.read_bytes().decode("utf-8", errors="replace").replace("\r\n", "\n").strip()
            if not texte:
                continue
            h = hashlib.sha256(texte.encode("utf-8")).hexdigest()
            textes.setdefault(h, (texte, []))[1].append(etiquette)
        if not fichiers:
            en_attente.append((p, etiquette))
    # Crates sans fichier de licence : textes standard (après la lecture des autres, pour réutiliser le texte Apache).
    for p, etiquette in en_attente:
        standard = textes_standard(p, textes)
        if not standard and p["name"] not in EXCEPTIONS:
            sans_licence.append(etiquette)
        for texte in standard:
            h = hashlib.sha256(texte.encode("utf-8")).hexdigest()
            textes.setdefault(h, (texte, []))[1].append(etiquette)
    # Police Inter (fichier du dépôt, pas une crate).
    inter = (RACINE / "assets" / "fonts" / "Inter-LICENSE.txt").read_bytes().decode("utf-8").replace("\r\n", "\n").strip()
    textes.setdefault(hashlib.sha256(inter.encode("utf-8")).hexdigest(), (inter, []))[1].append("police Inter (incluse dans l'application)")
    if sans_licence:
        print("Aucun fichier de licence pour :", ", ".join(sans_licence), file=sys.stderr)
        sys.exit(1)

    sortie = []
    sortie.append("Velka - NOTICES DES COMPOSANTS TIERS / THIRD-PARTY NOTICES")
    sortie.append("Velka est un logiciel libre sous licence GNU GPL version 3 ou ultérieure (fichier LICENSE).")
    sortie.append("Velka is free software licensed under the GNU GPL version 3 or later (see LICENSE).")
    sortie.append("=" * 62)
    sortie.append("Velka est construit avec les bibliothèques libres ci-dessous. Leurs licences l'exigent : voici la liste")
    sortie.append("et les textes complets. / Velka is built with the open-source libraries listed below; their licenses")
    sortie.append("require this notice and the full license texts.")
    sortie.append("")
    sortie.append(COMPOSANTS)
    sortie.append("BIBLIOTHÈQUES RUST INCLUSES DANS L'APPLICATION / RUST LIBRARIES INCLUDED IN THE APPLICATION")
    sortie.append("=" * 90)
    sortie.extend(lignes_crates)
    sortie.append("")
    sortie.append("TEXTES DES LICENCES / LICENSE TEXTS")
    sortie.append("=" * 35)
    for n, (texte, crates) in enumerate(sorted(textes.values(), key=lambda t: (-len(t[1]), t[1][0])), 1):
        sortie.append("")
        sortie.append("-" * 78)
        sortie.append(f"Texte {n} - s'applique à : " + ", ".join(crates))
        sortie.append("-" * 78)
        sortie.append(texte)
    SORTIE.write_bytes(("\r\n".join("\n".join(sortie).split("\n")) + "\r\n").encode("utf-8"))
    print(f"{len(paquets)} bibliothèques, {len(textes)} textes de licence, {SORTIE.stat().st_size // 1024} Ko -> {SORTIE}")


if __name__ == "__main__":
    main()
