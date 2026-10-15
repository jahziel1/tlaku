import { describe, expect, it } from 'vitest';
import {
  curveFor,
  CUSTOM_TONE_MATERIAL,
  fitCurve,
  hexOf,
  presetCurve,
  rgbOfHex,
  TONE_MATERIALS,
  toneDarkness,
  toneDose,
  toneMix,
  withCalibration,
  type ToneCalibration,
  type ToneCurve,
} from './curve';

const wood = presetCurve('light_wood');

describe('materiales de partida', () => {
  it('trae los materiales pedidos y el personalizado, todos válidos', () => {
    for (const id of ['light_wood', 'mdf', 'plywood', 'leather', 'acrylic', 'cardboard', 'paper', 'anodized', CUSTOM_TONE_MATERIAL]) {
      expect(TONE_MATERIALS).toContain(id);
    }
    for (const id of TONE_MATERIALS) {
      const c = presetCurve(id);
      expect(c.dose_char).toBeGreaterThan(0);
      expect(c.gamma).toBeGreaterThanOrEqual(0.2);
      expect(c.gamma).toBeLessThanOrEqual(4);
      expect(c.base).toHaveLength(3);
    }
    // Un material desconocido cae en el personalizado.
    expect(presetCurve('no-existe')).toEqual(presetCurve(CUSTOM_TONE_MATERIAL));
  });
});

describe('curva de tono', () => {
  it('es monótona, acotada y nunca NaN', () => {
    let prev = 0;
    for (let i = 0; i <= 2000; i++) {
      const d = toneDarkness(wood, i * 0.005);
      expect(d).toBeGreaterThanOrEqual(prev);
      expect(d).toBeLessThanOrEqual(1);
      prev = d;
    }
    expect(toneDarkness(wood, 0)).toBe(0);
    expect(toneDarkness(wood, -2)).toBe(0);
    expect(toneDarkness(wood, NaN)).toBe(0);
    expect(toneDarkness(wood, Infinity)).toBe(1);
    expect(toneDarkness(wood, 1e6)).toBeGreaterThan(0.999);
  });

  it('repite los vectores de la prueba de Rust', () => {
    // Misma entrada que `color_goes_from_base_to_burn` en crates/tlaku-doc/src/tonal.rs.
    expect(toneDarkness(wood, 0.3)).toBeCloseTo(1 - Math.exp(-1), 12);
    expect(toneMix(wood, 0.5)).toEqual([148, 114, 79]);
    expect(toneMix(wood, 0)).toEqual(wood.base);
    expect(toneMix(wood, 1)).toEqual(wood.burn);
  });

  it('más potencia, menos velocidad o más pasadas oscurecen; más intervalo aclara', () => {
    const d = (p: number, v: number, n: number, w: number) => toneDarkness(wood, toneDose(p, v, n, w));
    const base = d(40, 3000, 1, 0.1);
    expect(d(60, 3000, 1, 0.1)).toBeGreaterThan(base);
    expect(d(40, 1500, 1, 0.1)).toBeGreaterThan(base);
    expect(d(40, 3000, 2, 0.1)).toBeGreaterThan(base);
    expect(d(40, 3000, 1, 0.2)).toBeLessThan(base);
    expect(toneDose(0, 3000, 1, 0.1)).toBe(0);
    expect(toneDose(40, 0, 1, 0.1)).toBe(0);
  });

  it('las pasadas acumulan: dos pasadas valen una pasada al doble de potencia', () => {
    expect(toneDose(30, 1200, 2, 0.1)).toBeCloseTo(toneDose(60, 1200, 1, 0.1), 12);
  });

  it('convierte colores a hexadecimal y de vuelta', () => {
    expect(hexOf([226, 190, 140])).toBe('#e2be8c');
    expect(rgbOfHex('#e2be8c')).toEqual([226, 190, 140]);
    expect(rgbOfHex('rojo')).toBeNull();
  });
});

describe('ajuste con observaciones', () => {
  const truth: ToneCurve = { ...wood, dose_char: 0.5, gamma: 1.8 };
  const samplesOf = (doses: number[]) => doses.map((dose) => ({ dose, darkness: toneDarkness(truth, dose) }));

  it('recupera la dosis característica y la pendiente de una curva conocida', () => {
    const fit = fitCurve(wood, samplesOf([0.15, 0.25, 0.35, 0.5, 0.7]));
    expect(fit.dose_char).toBeCloseTo(0.5, 2);
    expect(fit.gamma).toBeCloseTo(1.8, 2);
    expect(fit.base).toEqual(wood.base);
    expect(fit.burn).toEqual(wood.burn);
  });

  it('con una sola observación mueve solo la dosis y conserva la pendiente', () => {
    const fit = fitCurve(wood, [{ dose: 0.5, darkness: 0.6321205588 }]);
    expect(fit.gamma).toBe(wood.gamma);
    // Con darkness = 1 - e^-1 la dosis característica es la dosis observada.
    expect(fit.dose_char).toBeCloseTo(0.5, 3);
    // Una celda que se ve más oscura de lo previsto baja la dosis característica.
    expect(fitCurve(wood, [{ dose: 0.2, darkness: 0.9 }]).dose_char).toBeLessThan(wood.dose_char);
    expect(fitCurve(wood, [{ dose: 0.2, darkness: 0.05 }]).dose_char).toBeGreaterThan(wood.dose_char);
  });

  it('sin observaciones válidas devuelve la misma curva y acota los extremos', () => {
    expect(fitCurve(wood, [])).toBe(wood);
    expect(fitCurve(wood, [{ dose: 0, darkness: 0.5 }, { dose: NaN, darkness: 0.5 }])).toBe(wood);
    const extreme = fitCurve(wood, [{ dose: 0.1, darkness: 1 }, { dose: 0.1001, darkness: 0 }, { dose: 5, darkness: 0 }]);
    expect(Number.isFinite(extreme.dose_char) && Number.isFinite(extreme.gamma)).toBe(true);
    expect(extreme.gamma).toBeGreaterThanOrEqual(0.5);
    expect(extreme.gamma).toBeLessThanOrEqual(3);
  });
});

describe('calibraciones guardadas', () => {
  const cal = (material: string, dose_char: number): ToneCalibration => ({ material, curve: { ...presetCurve(material), dose_char }, samples: [] });

  it('curveFor prefiere la calibración y cae en la de partida', () => {
    expect(curveFor('mdf', undefined)).toEqual(presetCurve('mdf'));
    expect(curveFor('mdf', [cal('mdf', 0.9)]).dose_char).toBe(0.9);
    expect(curveFor('leather', [cal('mdf', 0.9)])).toEqual(presetCurve('leather'));
  });

  it('withCalibration reemplaza, agrega y quita sin tocar las demás', () => {
    const a = [cal('mdf', 0.9), cal('paper', 0.1)];
    expect(withCalibration(a, 'mdf', cal('mdf', 0.5)).map((c) => [c.material, c.curve.dose_char])).toEqual([['paper', 0.1], ['mdf', 0.5]]);
    expect(withCalibration(a, 'mdf', null).map((c) => c.material)).toEqual(['paper']);
    expect(withCalibration(undefined, 'leather', cal('leather', 0.3))).toHaveLength(1);
  });
});
