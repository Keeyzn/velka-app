"""Génère les images du logo à partir de sa géométrie (voir logo.svg).

Usage (nécessite Pillow) :  python assets/logo/generer.py
Sorties : assets/velka.ico, assets/logo/mark-white.png (logo blanc, fond transparent),
          assets/logo/app-icon-256.png (icône d'application).
"""
from pathlib import Path
from PIL import Image, ImageDraw

RACINE = Path(__file__).resolve().parent.parent
SS = 8  # suréchantillonnage pour des bords lisses
TRIANGLES = [[(28, 18), (73.5, 44), (28, 44)], [(20, 56), (65.5, 56), (20, 82)]]
DECALAGE_X = 3
EPAISSEUR = 8


def marque(taille, echelle, centre, couleur=(255, 255, 255, 255)):
    """Logo seul sur fond transparent. `echelle` = pixels par unité du viewBox 100x100."""
    g = taille * SS
    img = Image.new("RGBA", (g, g), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    k = echelle * SS

    def px(p):
        return ((p[0] + DECALAGE_X - 50) * k + centre[0] * SS, (p[1] - 50) * k + centre[1] * SS)

    r = EPAISSEUR / 2 * k
    for tri in TRIANGLES:
        pts = [px(p) for p in tri]
        d.polygon(pts, fill=couleur)
        for i, a in enumerate(pts):
            b = pts[(i + 1) % 3]
            d.line([a, b], fill=couleur, width=int(round(2 * r)))
            d.ellipse([a[0] - r, a[1] - r, a[0] + r, a[1] + r], fill=couleur)
    return img.resize((taille, taille), Image.LANCZOS)


def icone(taille):
    """Carré noir arrondi (rx 230/1024) avec le logo blanc (échelle 7,5/1024)."""
    g = taille * SS
    fond = Image.new("RGBA", (g, g), (0, 0, 0, 0))
    ImageDraw.Draw(fond).rounded_rectangle([0, 0, g - 1, g - 1], radius=int(g * 230 / 1024), fill=(11, 11, 15, 255))
    fond = fond.resize((taille, taille), Image.LANCZOS)
    m = marque(taille, taille * 7.5 / 1024, (taille / 2, taille / 2))
    fond.alpha_composite(m)
    return fond


def main():
    marque(256, 2.56, (128, 128)).save(RACINE / "logo" / "mark-white.png", optimize=True)
    icone(256).save(RACINE / "logo" / "app-icon-256.png", optimize=True)
    tailles = [16, 24, 32, 48, 64, 128, 256]
    images = [icone(s) for s in tailles]
    images[-1].save(RACINE / "velka.ico", format="ICO", sizes=[(s, s) for s in tailles], append_images=images[:-1])


main()
