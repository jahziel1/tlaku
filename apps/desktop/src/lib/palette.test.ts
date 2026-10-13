import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { normalizeCombo } from '@/commands/shortcuts';
import {
  GUIDE_COLOR,
  GUIDE_ENTRY,
  INK_DARK,
  INK_LIGHT,
  LAYER_PALETTE,
  colorName,
  contrastRatio,
  isGuideColor,
  paletteEntry,
  paletteLabel,
  readableOn,
  relativeLuminance,
} from './palette';

describe('paleta de capas', () => {
  it('colores, ids y nombres no se repiten, y todos los nombres existen en el diccionario', () => {
    const colors = LAYER_PALETTE.map((p) => p.color);
    expect(new Set(colors).size).toBe(colors.length);
    expect(new Set(LAYER_PALETTE.map((p) => p.id)).size).toBe(LAYER_PALETTE.length);
    const names = LAYER_PALETTE.map(paletteLabel);
    expect(new Set(names).size).toBe(names.length);
    for (const p of LAYER_PALETTE) expect(es, p.id).toHaveProperty(p.nameKey);
  });

  it('las muestras se llaman por nombre, nunca por código numérico', () => {
    for (const p of LAYER_PALETTE) expect(paletteLabel(p)).toMatch(/^\p{L}+$/u);
    expect(colorName('#FF3B4A')).toBe('Rojo');
    expect(colorName('#123456')).toBe('#123456');
  });

  it('hay una sola capa de guías, al final, y se reconoce sin importar mayúsculas', () => {
    expect(LAYER_PALETTE.filter((p) => p.guide)).toEqual([GUIDE_ENTRY]);
    expect(LAYER_PALETTE[LAYER_PALETTE.length - 1]).toBe(GUIDE_ENTRY);
    expect(isGuideColor(GUIDE_COLOR.toUpperCase())).toBe(true);
    expect(isGuideColor('#ff3b4a')).toBe(false);
    expect(paletteEntry('#7E8AA0')?.guide).toBe(true);
  });

  it('los atajos numéricos son únicos, canónicos y usan Alt (no chocan con Mod+1..3 de los paneles)', () => {
    const keys = LAYER_PALETTE.map((p) => p.shortcut).filter((s): s is string => !!s);
    expect(new Set(keys).size).toBe(keys.length);
    for (const k of keys) {
      expect(normalizeCombo(k)).toBe(k);
      expect(k.startsWith('Alt+')).toBe(true);
    }
    expect(keys.length).toBe(11);
  });
});

describe('contraste', () => {
  it('relaciones conocidas de WCAG', () => {
    expect(relativeLuminance('#000000')).toBe(0);
    expect(relativeLuminance('#ffffff')).toBeCloseTo(1, 6);
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 4);
    expect(contrastRatio('#777777', '#ffffff')).toBeCloseTo(4.48, 1);
    expect(contrastRatio('#ff0000', '#ff0000')).toBe(1);
  });

  it('la tinta elegida sobre cada color de la paleta llega a 4,5:1', () => {
    for (const p of LAYER_PALETTE) {
      const ink = readableOn(p.color);
      expect([INK_DARK, INK_LIGHT]).toContain(ink);
      expect(contrastRatio(p.color, ink), `${p.id} ${p.color}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('cualquier fondo, aun los grises medios, alcanza 4,5:1 con la mejor de las dos tintas', () => {
    for (let v = 0; v <= 255; v += 5) {
      const hex = `#${v.toString(16).padStart(2, '0').repeat(3)}`;
      expect(contrastRatio(hex, readableOn(hex)), hex).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('negro sobre colores claros y blanco sobre oscuros', () => {
    expect(readableOn('#e6e600')).toBe(INK_DARK);
    expect(readableOn('#f2f2f2')).toBe(INK_DARK);
    expect(readableOn('#1d3d8f')).toBe(INK_LIGHT);
  });
});
