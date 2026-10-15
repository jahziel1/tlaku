#!/usr/bin/env python3
"""Genera las imágenes para redes del sitio (Open Graph y Twitter Card, 1200x630):
site/og-es.png y site/og-en.png. Solo se vuelve a correr cuando cambie el diseño o el texto;
el resultado se guarda en el repositorio (no se genera en la CI).

    python3 scripts/site-og.py

Usa solo cosas propias del proyecto: el ícono (site/favicon.svg) y una captura de la propia
aplicación (apps/desktop/docs/screenshots/). Necesita Pillow y ImageMagick (`magick`) para
rasterizar el ícono SVG, y una fuente del sistema (Helvetica en macOS, DejaVu en Linux).
"""
import os
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHOTS = os.path.join(ROOT, "apps", "desktop", "docs", "screenshots")
W, H = 1200, 630

# Mismos colores que el sitio (site/style.css).
BG = (14, 17, 21)
CHROME = (21, 25, 32)
BORDER = (105, 120, 140)
TEXT = (233, 237, 243)
MUTED = (161, 172, 187)
ACCENT = (135, 147, 255)

PAGES = {
    "es": {
        "out": "og-es.png",
        "shot": "01-principal-oscuro.png",
        "title": "Alternativa libre y gratis a LightBurn",
        "sub": "Software de código abierto (MIT) para grabadoras y cortadoras láser",
        "chips": ["Beta", "MIT", "GRBL", "Falcon2"],
    },
    "en": {
        "out": "og-en.png",
        "shot": "38-english.png",
        "title": "Free open-source LightBurn alternative",
        "sub": "Open source (MIT) software for laser engravers and cutters",
        "chips": ["Beta", "MIT", "GRBL", "Falcon2"],
    },
}

BOLD_FONTS = [
    ("/System/Library/Fonts/HelveticaNeue.ttc", 1),
    ("/System/Library/Fonts/Helvetica.ttc", 1),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 0),
]
REGULAR_FONTS = [
    ("/System/Library/Fonts/HelveticaNeue.ttc", 0),
    ("/System/Library/Fonts/Helvetica.ttc", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
]


def font(size, bold):
    for path, index in BOLD_FONTS if bold else REGULAR_FONTS:
        if os.path.exists(path):
            return ImageFont.truetype(path, size, index=index)
    sys.exit("no encontré una fuente del sistema")


def wrap(draw, text, fnt, width):
    lines, line = [], ""
    for word in text.split():
        trial = f"{line} {word}".strip()
        if draw.textlength(trial, font=fnt) <= width:
            line = trial
        else:
            lines.append(line)
            line = word
    lines.append(line)
    return lines


def logo(size):
    with tempfile.TemporaryDirectory() as tmp:
        png = os.path.join(tmp, "logo.png")
        subprocess.run(
            ["magick", "-background", "none", "-density", "384", os.path.join(ROOT, "site", "favicon.svg"),
             "-resize", f"{size}x{size}", png],
            check=True,
        )
        return Image.open(png).convert("RGBA")


def rounded_shot(path, width):
    shot = Image.open(os.path.join(SHOTS, path)).convert("RGB")
    height = round(shot.height * width / shot.width)
    shot = shot.resize((width, height), Image.LANCZOS)
    mask = Image.new("L", shot.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, width - 1, height - 1), radius=14, fill=255)
    out = Image.new("RGBA", shot.size, (0, 0, 0, 0))
    out.paste(shot, (0, 0), mask)
    return out


def make(lang, spec):
    img = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(img)

    # Franja de color a la izquierda, como el acento del sitio.
    draw.rectangle((0, 0, 8, H), fill=ACCENT)

    # Captura de la app a la derecha, con borde, saliendo por el borde inferior.
    shot = rounded_shot(spec["shot"], 640)
    sx, sy = W - 640 + 70, 250
    frame = Image.new("RGBA", (shot.width + 4, shot.height + 4), (0, 0, 0, 0))
    ImageDraw.Draw(frame).rounded_rectangle((0, 0, frame.width - 1, frame.height - 1), radius=16, fill=BORDER + (255,))
    img.paste(frame, (sx - 2, sy - 2), frame)
    img.paste(shot, (sx, sy), shot)

    # Texto a la izquierda (se ve por encima de la captura si se cruzan).
    mark = logo(76)
    img.paste(mark, (64, 56), mark)
    draw.text((152, 70), "Tlaku", font=font(48, True), fill=TEXT)

    title_font = font(64, True)
    lines = wrap(draw, spec["title"], title_font, 560)
    y = 170
    for line in lines:
        draw.text((64, y), line, font=title_font, fill=TEXT)
        y += 76

    sub_font = font(28, False)
    y += 14
    for line in wrap(draw, spec["sub"], sub_font, 520):
        draw.text((64, y), line, font=sub_font, fill=MUTED)
        y += 38

    # Etiquetas en la parte baja.
    chip_font = font(22, True)
    x, y = 64, H - 70
    for chip in spec["chips"]:
        w = draw.textlength(chip, font=chip_font) + 28
        draw.rounded_rectangle((x, y, x + w, y + 38), radius=19, outline=BORDER, width=2, fill=CHROME)
        draw.text((x + 14, y + 6), chip, font=chip_font, fill=TEXT)
        x += w + 12

    out = os.path.join(ROOT, "site", spec["out"])
    img.save(out, optimize=True)
    print(f"{lang}: {out} ({os.path.getsize(out) // 1024} KB)")


if __name__ == "__main__":
    for lang, spec in PAGES.items():
        make(lang, spec)
