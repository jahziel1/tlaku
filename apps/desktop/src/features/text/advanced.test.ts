import { afterEach, describe, expect, it } from 'vitest';
import { mockFontCapabilities } from '@/api/mock/text';
import type { FontAxisInfo, TextObject } from '@/api/types';
import { setLanguage } from '@/i18n';
import {
  axisLabel,
  axisStep,
  axisValue,
  changedFeatureCount,
  featureGroup,
  featureLabel,
  featureOn,
  groupFeatures,
  matchingInstance,
  storedFeature,
  variationsOfInstance,
  withAxis,
  withFeature,
} from './advanced';
import { capabilitiesKey } from './capabilities';
import { previewBox, previewPathData } from './FeaturePreview';
import { paramsKey } from './signature';

const kind = (patch: Partial<TextObject> = {}): TextObject => ({
  text: 'Hola', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [], ...patch,
});

const WGHT: FontAxisInfo = { tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 };
const SLNT: FontAxisInfo = { tag: 'slnt', name: null, min: -12, default: 0, max: 0 };

afterEach(() => setLanguage('es'));

describe('funciones OpenType del objeto', () => {
  it('lo que queda como la fuente no deja rastro', () => {
    // kern viene encendido: dejarlo así no se guarda; apagarlo sí.
    expect(withFeature(undefined, 'kern', true, true)).toBeUndefined();
    expect(withFeature(undefined, 'kern', false, true)).toEqual([{ tag: 'kern', value: 0 }]);
    expect(withFeature(undefined, 'ss01', true, false)).toEqual([{ tag: 'ss01', value: 1 }]);
    // Volver al valor de la fuente borra la entrada.
    const on = withFeature(undefined, 'ss01', true, false);
    expect(withFeature(on, 'ss01', false, false)).toBeUndefined();
  });

  it('lee el estado efectivo con y sin valor guardado', () => {
    const f = [{ tag: 'kern', value: 0 }, { tag: 'smcp', value: 1 }];
    expect(featureOn(f, 'kern', true)).toBe(false);
    expect(featureOn(f, 'smcp', false)).toBe(true);
    expect(featureOn(f, 'liga', true)).toBe(true);
    expect(featureOn(undefined, 'liga', false)).toBe(false);
    expect(storedFeature(f, 'kern')).toBe(0);
    expect(storedFeature(f, 'zero')).toBeUndefined();
    expect(changedFeatureCount(f)).toBe(2);
  });

  it('encender una de un par excluyente quita la otra', () => {
    const lnum = withFeature(undefined, 'lnum', true, false);
    const onum = withFeature(lnum, 'onum', true, false);
    expect(onum).toEqual([{ tag: 'onum', value: 1 }]);
    expect(withFeature(onum, 'tnum', true, false)).toEqual([
      { tag: 'onum', value: 1 },
      { tag: 'tnum', value: 1 },
    ]);
  });

  it('las funciones numeradas guardan la alternativa elegida', () => {
    expect(withFeature(undefined, 'salt', true, false, 3)).toEqual([{ tag: 'salt', value: 3 }]);
    expect(withFeature([{ tag: 'salt', value: 3 }], 'salt', true, false, 1)).toEqual([{ tag: 'salt', value: 1 }]);
  });

  it('agrupa solo lo que la fuente declara y en un orden estable', () => {
    const caps = mockFontCapabilities({ ...kind(), font_family: 'Helvetica Neue' });
    const groups = groupFeatures(caps.features);
    expect(groups.map((g) => g.group)).toEqual(['ligatures', 'alternates', 'figures', 'position']);
    expect(groups[0]!.items.map((i) => i.tag)).toEqual(['dlig', 'kern', 'liga']);
    expect(featureGroup('ss07')).toBe('alternates');
    expect(featureGroup('cv12')).toBe('alternates');
    expect(featureGroup('smcp')).toBe('caps');
    expect(groupFeatures([])).toEqual([]);
  });

  it('da nombre legible en es y en, distinto entre sí', () => {
    expect(featureLabel('smcp')).toBe('Versalitas');
    expect(featureLabel('ss03')).toBe('Conjunto estilístico 3');
    expect(featureLabel('zzzz')).toBe('zzzz');
    setLanguage('en');
    expect(featureLabel('smcp')).toBe('Small caps');
    expect(featureLabel('ss03')).toBe('Stylistic set 3');
    expect(featureLabel('cv05')).toBe('Character variant 5');
  });
});

describe('ejes de fuente variable', () => {
  it('guarda solo los ejes que se alejan del valor por omisión y acota al rango', () => {
    expect(withAxis(undefined, WGHT, 400)).toBeUndefined();
    expect(withAxis(undefined, WGHT, 700)).toEqual([{ tag: 'wght', value: 700 }]);
    expect(withAxis(undefined, WGHT, 5000)).toEqual([{ tag: 'wght', value: 900 }]);
    const both = withAxis(withAxis(undefined, WGHT, 700), SLNT, -8);
    expect(both).toEqual([{ tag: 'wght', value: 700 }, { tag: 'slnt', value: -8 }]);
    expect(withAxis(both, WGHT, 400)).toEqual([{ tag: 'slnt', value: -8 }]);
    expect(axisValue(both, WGHT)).toBe(700);
    expect(axisValue(undefined, SLNT)).toBe(0);
  });

  it('elige el paso según el rango', () => {
    expect(axisStep(WGHT)).toBe(1);
    expect(axisStep(SLNT)).toBe(0.1);
  });

  it('las instancias con nombre se aplican y se reconocen', () => {
    const caps = mockFontCapabilities({ ...kind(), font_family: 'Helvetica Neue' });
    const bold = caps.instances[1]!;
    const v = variationsOfInstance(bold, caps.axes);
    expect(v).toEqual([{ tag: 'wght', value: 700 }, { tag: 'wdth', value: 80 }]);
    expect(matchingInstance(v, caps.instances, caps.axes)?.name).toBe('Bold Condensed');
    expect(matchingInstance(undefined, caps.instances, caps.axes)?.name).toBe('Regular');
    expect(matchingInstance([{ tag: 'wght', value: 555 }], caps.instances, caps.axes)).toBeUndefined();
    expect(variationsOfInstance(caps.instances[0]!, caps.axes)).toBeUndefined();
  });

  it('usa nombres propios para los ejes comunes y el de la fuente para el resto', () => {
    expect(axisLabel({ tag: 'wght', name: 'Weight' })).toBe('Peso');
    expect(axisLabel({ tag: 'TLKX', name: 'Tlaku X' })).toBe('Tlaku X');
    expect(axisLabel({ tag: 'XXXX', name: null })).toBe('XXXX');
    setLanguage('en');
    expect(axisLabel({ tag: 'wght', name: 'Weight' })).toBe('Weight');
    expect(axisLabel({ tag: 'slnt', name: null })).toBe('Slant');
  });
});

describe('firma y clave', () => {
  it('las opciones avanzadas cambian la firma y su ausencia equivale al valor por omisión', () => {
    const base = paramsKey(kind());
    expect(paramsKey(kind({ features: [], variations: [], uppercase: false, valign: 'baseline', word_spacing_mm: 0 }))).toBe(base);
    const variants: Partial<TextObject>[] = [
      { features: [{ tag: 'liga', value: 0 }] },
      { variations: [{ tag: 'wght', value: 700 }] },
      { uppercase: true },
      { valign: 'top' },
      { word_spacing_mm: 1 },
    ];
    for (const v of variants) expect(paramsKey(kind(v)), JSON.stringify(v)).not.toBe(base);
    expect(paramsKey(kind({ features: [{ tag: 'liga', value: 0 }] }))).not.toBe(paramsKey(kind({ features: [{ tag: 'liga', value: 1 }] })));
  });

  it('la clave de las capacidades depende de la cara y los ejes, no del contenido', () => {
    const a = capabilitiesKey(kind({ text: 'uno' }), 'Arial');
    expect(capabilitiesKey(kind({ text: 'otro', features: [{ tag: 'liga', value: 0 }] }), 'ARIAL ')).toBe(a);
    expect(capabilitiesKey(kind({ bold: true }), 'Arial')).not.toBe(a);
    expect(capabilitiesKey(kind({ variations: [{ tag: 'wght', value: 700 }] }), 'Arial')).not.toBe(a);
    expect(capabilitiesKey(kind(), 'Georgia')).not.toBe(a);
  });
});

describe('vista previa', () => {
  it('convierte las curvas a un trayecto SVG con Y hacia abajo', () => {
    const caps = mockFontCapabilities({ ...kind(), font_family: 'Georgia' });
    const f = caps.features.find((x) => x.tag === 'smcp')!;
    const d = previewPathData(f.on);
    expect(d).toMatch(/^M0 0L/);
    expect(d).toContain('Z');
    expect(previewPathData([])).toBe('');
    const box = previewBox(f.on, f.off);
    expect(box.w).toBeGreaterThan(0);
    // Con la función encendida el mock sube la altura: la caja común la abarca.
    expect(box.y).toBeLessThan(-8);
  });
});
