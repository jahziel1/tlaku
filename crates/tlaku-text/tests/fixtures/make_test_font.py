#!/usr/bin/env python3
"""Genera TlakuTest-Var.ttf: fuente sintética de prueba (formas geométricas) para las pruebas de
funciones OpenType y de fuentes variables de tlaku-text.

Es obra propia del proyecto (MIT, ver README.txt): no deriva de ninguna fuente existente.
Uso: python3 make_test_font.py   (necesita fonttools; reescribe TlakuTest-Var.ttf)

Contenido:
- Ejes: wght 400..900, wdth 100..150, slnt -12..0, opsz 8..72, TLKX 0..100 (personalizado).
- GSUB: liga (f i), dlig (a b), salt (a), ss01 (a), ss02 (b), swsh (b), smcp (a b),
  lnum/onum/tnum/pnum (1 2 3), frac (1/2, 1/3, 2/3). GPOS: kern (a b).
"""
import io
import math
import os

from fontTools.designspaceLib import (AxisDescriptor, DesignSpaceDocument, InstanceDescriptor,
                                      SourceDescriptor)
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.varLib import build as var_build

UPM = 1000


def rect(x0, y0, x1, y1):
    return [(x0, y0), (x0, y1), (x1, y1), (x1, y0)]  # sentido horario (exterior en TrueType)


def hole(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]  # antihorario (hueco)


# nombre: (ancho, [contornos]); el segundo contorno de 'b' y 'B' es un hueco.
GLYPHS = {
    ".notdef": (500, [rect(50, 0, 450, 700)]),
    "space": (300, []),
    "a": (500, [rect(60, 0, 440, 500)]),
    "b": (500, [rect(60, 0, 440, 700), hole(160, 120, 340, 580)]),
    "f": (350, [rect(60, 0, 290, 720)]),
    "i": (260, [rect(70, 0, 190, 650)]),
    "A": (620, [rect(60, 0, 560, 700)]),
    "B": (620, [rect(60, 0, 560, 700), hole(180, 140, 440, 560)]),
    "one": (300, [rect(80, 0, 220, 700)]),
    "two": (520, [rect(60, 0, 460, 700)]),
    "three": (520, [rect(60, 0, 460, 650)]),
    "slash": (360, [rect(60, 0, 300, 700)]),
    # variantes
    "f_i": (560, [rect(60, 0, 500, 720)]),
    "a_b": (900, [rect(60, 0, 840, 640)]),
    "a.ss01": (500, [rect(60, 0, 440, 560)]),
    "a.salt2": (500, [rect(60, 0, 440, 440)]),
    "b.ss02": (500, [rect(60, 0, 440, 760)]),
    "b.swsh": (560, [rect(40, -80, 520, 780)]),
    "a.sc": (480, [rect(60, 0, 420, 400)]),
    "b.sc": (480, [rect(60, 0, 420, 420)]),
    "one.lf": (520, [rect(80, 0, 440, 700)]),
    "two.lf": (520, [rect(60, 0, 460, 710)]),
    "three.lf": (520, [rect(60, 0, 460, 720)]),
    "one.osf": (520, [rect(80, 0, 440, 450)]),
    "two.osf": (520, [rect(60, 0, 460, 460)]),
    "three.osf": (520, [rect(60, 0, 460, 470)]),
    "one.tf": (560, [rect(80, 0, 480, 700)]),
    "two.tf": (560, [rect(60, 0, 500, 700)]),
    "three.tf": (560, [rect(60, 0, 500, 650)]),
    "one.numr": (300, [rect(60, 340, 240, 700)]),
    "two.numr": (300, [rect(60, 340, 240, 710)]),
    "three.numr": (300, [rect(60, 340, 240, 720)]),
    "one.dnom": (300, [rect(60, -40, 240, 300)]),
    "two.dnom": (300, [rect(60, -40, 240, 310)]),
    "three.dnom": (300, [rect(60, -40, 240, 320)]),
    "slash.frac": (200, [rect(30, 0, 170, 700)]),
}
ORDER = list(GLYPHS)
CMAP = {
    0x20: "space", 0x61: "a", 0x62: "b", 0x66: "f", 0x69: "i", 0x41: "A", 0x42: "B",
    0x31: "one", 0x32: "two", 0x33: "three", 0x2F: "slash",
}

FEA = """
languagesystem DFLT dflt;
languagesystem latn dflt;
@DIG = [one two three];
feature liga { sub f i by f_i; } liga;
feature dlig { sub a b by a_b; } dlig;
feature salt { sub a from [a.ss01 a.salt2]; } salt;
feature ss01 { sub a by a.ss01; } ss01;
feature ss02 { sub b by b.ss02; } ss02;
feature swsh { sub b by b.swsh; } swsh;
feature smcp { sub a by a.sc; sub b by b.sc; } smcp;
feature lnum { sub one by one.lf; sub two by two.lf; sub three by three.lf; } lnum;
feature onum { sub one by one.osf; sub two by two.osf; sub three by three.osf; } onum;
feature tnum { sub one by one.tf; sub two by two.tf; sub three by three.tf; } tnum;
feature pnum { sub one.tf by one; sub two.tf by two; sub three.tf by three; } pnum;
lookup NUMR { sub one by one.numr; sub two by two.numr; sub three by three.numr; } NUMR;
lookup DNOM { sub one by one.dnom; sub two by two.dnom; sub three by three.dnom; } DNOM;
lookup SLASH { sub slash by slash.frac; } SLASH;
feature frac {
    sub @DIG' lookup NUMR slash' lookup SLASH @DIG' lookup DNOM;
} frac;
feature kern { pos a b -120; pos A B -80; } kern;
"""


def transform(pt, params):
    x, y = pt
    w = params.get("wght", 0.0)   # 0..1 hacia 900
    wd = params.get("wdth", 0.0)  # 0..1 hacia 150
    sl = params.get("slnt", 0.0)  # 0..1 hacia -12
    op = params.get("opsz", 0.0)  # 0..1 hacia 72
    cu = params.get("TLKX", 0.0)  # 0..1 hacia 100
    x = x * (1 + 0.25 * w) * (1 + 0.5 * wd)
    y = y * (1 - 0.15 * op) * (1 + 0.2 * cu)
    x = x + y * math.tan(math.radians(12)) * sl
    return (round(x), round(y))


def advance(adv, params):
    return round(adv * (1 + 0.25 * params.get("wght", 0.0)) * (1 + 0.5 * params.get("wdth", 0.0)))


def make_master(params):
    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(ORDER)
    fb.setupCharacterMap(CMAP)
    glyphs = {}
    metrics = {}
    for name, (adv, contours) in GLYPHS.items():
        pen = TTGlyphPen(None)
        for c in contours:
            pts = [transform(p, params) for p in c]
            pen.moveTo(pts[0])
            for p in pts[1:]:
                pen.lineTo(p)
            pen.closePath()
        glyphs[name] = pen.glyph()
        metrics[name] = (advance(adv, params), 0)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({
        "familyName": "Tlaku Test", "styleName": "Regular", "uniqueFontIdentifier": "TlakuTest-Var-1.0",
        "fullName": "Tlaku Test Regular", "psName": "TlakuTest-Regular", "version": "Version 1.0",
        "copyright": "Tlaku project, MIT license. Synthetic test font.",
    })
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=900, usWinDescent=250,
                sCapHeight=700, sxHeight=500)
    fb.setupPost()
    addOpenTypeFeaturesFromString(fb.font, FEA)
    return fb.font


# (etiqueta, mínimo, por omisión, máximo, nombre)
AXES = [
    ("wght", 400, 400, 900, "Weight"),
    ("wdth", 100, 100, 150, "Width"),
    ("slnt", -12, 0, 0, "Slant"),
    ("opsz", 8, 8, 72, "Optical size"),
    ("TLKX", 0, 0, 100, "Tlaku X"),
]


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ds = DesignSpaceDocument()
    for tag, mn, df, mx, name in AXES:
        a = AxisDescriptor()
        a.tag, a.name, a.minimum, a.default, a.maximum = tag, name, mn, df, mx
        ds.addAxis(a)

    def location(**kw):
        loc = {name: df for tag, mn, df, mx, name in AXES}
        for tag, mn, df, mx, name in AXES:
            if tag in kw:
                loc[name] = kw[tag]
        return loc

    masters = [({}, location())]
    for tag, mn, df, mx, name in AXES:
        # Cada eje tiene un solo extremo distinto del valor por omisión.
        far = mn if mn != df else mx
        masters.append(({tag: 1.0}, location(**{tag: far})))
    for params, loc in masters:
        font = make_master(params)
        buf = io.BytesIO()
        font.save(buf)
        buf.seek(0)
        src = SourceDescriptor()
        src.font = TTFont(buf)
        src.location = loc
        src.name = "m" + str(len(ds.sources))
        ds.addSource(src)
    # Instancias con nombre (tabla fvar).
    for style, kw in (("Regular", {}), ("Bold Wide", {"wght": 900, "wdth": 150})):
        inst = InstanceDescriptor()
        inst.familyName, inst.styleName = "Tlaku Test", style
        inst.name = "Tlaku Test " + style
        inst.location = location(**kw)
        ds.addInstance(inst)
    vf, _, _ = var_build(ds)
    out = os.path.join(here, "TlakuTest-Var.ttf")
    vf.save(out)
    print(out, os.path.getsize(out), "bytes")


if __name__ == "__main__":
    main()
