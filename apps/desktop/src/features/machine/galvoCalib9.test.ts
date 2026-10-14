import { describe, expect, it } from 'vitest';
import type { FieldCorrection } from '@/bindings/FieldCorrection';
import { buildGrid9Document, correct, fitNinePoints, forward, grid9Points, IDENTITY_FAMILY, simulateMeasures9, type Fit9Result, type Pt } from './galvoCalib9';

const mark = { power: 15, speedMmS: 500, frequencyKhz: 30, pulseWidthNs: null };

const base = (over: Partial<FieldCorrection> = {}): FieldCorrection => ({
  scale: [1, 1],
  offset: [0, 0],
  rotation_deg: 0,
  invert: [false, false],
  swap_xy: false,
  bulge: [0, 0],
  trapezoid: [0, 0],
  skew: [0, 0],
  cor_file: null,
  cor_transpose: false,
  cor_invert: false,
  ...over,
});

const ok = (r: ReturnType<typeof fitNinePoints>): Fit9Result => {
  if (typeof r === 'string') throw new Error(`falló: ${r}`);
  return r;
};

/** Error máximo (mm) del aterrizaje de `corr` sobre los 9 puntos y una rejilla de 7 x 7, con la lente dada. */
function worstLanding(field: number, corr: FieldCorrection, lens: Parameters<typeof simulateMeasures9>[2]): number {
  const a = field * 0.35;
  let worst = 0;
  for (let i = -3; i <= 3; i++) {
    for (let j = -3; j <= 3; j++) {
      const n: Pt = [(i / 3) * a, (j / 3) * a];
      const q = forward(field, corr, n);
      // El aterrizaje es la lente aplicada al punto del diseño que la corrección dejó en `q`.
      const dsg = corrAxesToDesign(corr, q);
      const land = correct(field, lens, dsg);
      const center = correct(field, lens, corrAxesToDesign(corr, forward(field, corr, [0, 0])));
      worst = Math.max(worst, Math.hypot(land[0] - center[0] - n[0], land[1] - center[1] - n[1]));
    }
  }
  return worst;
}

/** `q` (ejes del galvo) al marco del diseño de la corrección `c`: quita el intercambio y la negación de ejes. */
function corrAxesToDesign(c: FieldCorrection, q: Pt): Pt {
  const a = c.invert[0] ? -q[0] : q[0];
  const b = c.invert[1] ? -q[1] : q[1];
  return c.swap_xy ? [b, a] : [a, b];
}

describe('asistente de nueve puntos', () => {
  it('el modelo copia al del motor: los mismos números que fija calib.rs', () => {
    // Los de `the_ts_mirror_numbers_are_pinned` (crates/tlaku-galvo/src/calib.rs), campo de 110 mm.
    const full = base({
      scale: [1.02, 0.97],
      offset: [0.7, -0.4],
      rotation_deg: 1.3,
      invert: [false, true],
      swap_xy: true,
      bulge: [0.02, -0.015],
      trapezoid: [0.01, -0.02],
      skew: [0.005, 0.008],
    });
    const cases: [Pt, Pt][] = [
      [[0, 0], [-0.4, -0.7]],
      [[38.5, 38.5], [37.048664921, -40.312345662]],
      [[-38.5, 19.25], [17.144920998, 39.518921697]],
      [[10, -30], [-28.923944697, -11.425716223]],
    ];
    for (const [p, want] of cases) {
      const q = forward(110, full, p);
      expect(q[0]).toBeCloseTo(want[0], 7);
      expect(q[1]).toBeCloseTo(want[1], 7);
    }
  });

  it('los nueve puntos van a ±0,35 del campo, fila a fila desde arriba a la izquierda', () => {
    const pts = grid9Points(100);
    expect(pts).toHaveLength(9);
    expect(pts[0]).toEqual([-35, 35]);
    expect(pts[2]).toEqual([35, 35]);
    expect(pts[4]).toEqual([0, 0]);
    expect(pts[6]).toEqual([-35, -35]);
    expect(pts[8]).toEqual([35, -35]);
  });

  it('el documento trae nueve cruces y la marca del lado +X, centradas en la cama', () => {
    const doc = buildGrid9Document([110, 110], 110, mark);
    expect(doc.objects).toHaveLength(10);
    expect(doc.objects.filter((o) => o.id.startsWith('calib9-') && o.id !== 'calib9-x')).toHaveLength(9);
    expect(doc.layers[0]!.mode).toBe('line');
    expect(doc.layers[0]!.power).toBe(15);
    expect(doc.layers[0]!.speed).toBe(500 * 60);
    expect(doc.layers[0]!.galvo.frequency_khz).toBe(30);
  });

  it('una lente perfecta no cambia nada', () => {
    const r = ok(fitNinePoints(110, base(), grid9Points(110), true));
    expect(r.correction.scale[0]).toBeCloseTo(1, 5);
    expect(r.correction.scale[1]).toBeCloseTo(1, 5);
    expect(r.correction.rotation_deg).toBeCloseTo(0, 3);
    expect(r.maxBefore).toBeCloseTo(0, 9);
    expect(r.maxAfter).toBeLessThan(1e-6);
  });

  it('una escala de 2 % en X se corrige con su inversa (cálculo a mano: 1 / 1,02 = 0,98039)', () => {
    // La lente dibuja 2 % más ancho en X: la cruz de la derecha cae en 38,5 x 1,02 = 39,27.
    const m = simulateMeasures9(110, base(), { ...IDENTITY_FAMILY, scale: [1.02, 1] });
    expect(m[5]![0]).toBeCloseTo(39.27, 9);
    const r = ok(fitNinePoints(110, base(), m, true));
    expect(r.correction.scale[0]).toBeCloseTo(0.98039, 4);
    expect(r.correction.scale[1]).toBeCloseTo(1, 4);
    expect(r.maxBefore).toBeCloseTo(0.77, 2);
    expect(r.maxAfter).toBeLessThan(0.005);
  });

  it('con una corrección ya puesta se corrige lo que falta, no todo desde cero', () => {
    // El perfil ya trae escala 1,05 y la lente es perfecta: lo medido sale 5 % grande y lo que falta es volver a 1.
    const current = base({ scale: [1.05, 1.05] });
    const m = simulateMeasures9(110, current, IDENTITY_FAMILY);
    expect(m[5]![0]).toBeCloseTo(38.5 * 1.05, 9);
    const r = ok(fitNinePoints(110, current, m, true));
    expect(r.correction.scale[0]).toBeCloseTo(1, 4);
    expect(r.correction.scale[1]).toBeCloseTo(1, 4);
  });

  it('una lente con escala, giro, barril y trapecio queda en menos de 0,02 mm en toda la cuadrícula', () => {
    const lens = { ...IDENTITY_FAMILY, scale: [1.012, 0.986] as [number, number], rotation_deg: 0.25, bulge: [0.01, -0.008] as [number, number], trapezoid: [0.006, -0.004] as [number, number] };
    const current = base();
    const m = simulateMeasures9(110, current, lens);
    const r = ok(fitNinePoints(110, current, m, true));
    expect(r.maxBefore).toBeGreaterThan(0.5);
    expect(r.maxAfter).toBeLessThan(0.02);
    // Comprobación independiente del modelo interno: la lente real con la corrección nueva.
    expect(worstLanding(110, r.correction, lens)).toBeLessThan(0.02);
    // Y la corrección sigue dentro de lo que admite el motor.
    expect(r.clamped).toBe(false);
    for (const v of [...r.correction.bulge, ...r.correction.trapezoid]) expect(Math.abs(v)).toBeLessThanOrEqual(0.3);
    // El desplazamiento, el sesgo y los ejes no se tocan.
    expect(r.correction.offset).toEqual([0, 0]);
    expect(r.correction.skew).toEqual([0, 0]);
  });

  it('también con ejes intercambiados y negados en el montaje', () => {
    const current = base({ swap_xy: true, invert: [false, true] });
    const lens = { ...IDENTITY_FAMILY, scale: [0.99, 1.015] as [number, number], bulge: [-0.006, 0.004] as [number, number] };
    const m = simulateMeasures9(110, current, lens);
    const r = ok(fitNinePoints(110, current, m, true));
    expect(r.correction.swap_xy).toBe(true);
    expect(r.correction.invert).toEqual([false, true]);
    expect(worstLanding(110, r.correction, lens)).toBeLessThan(0.02);
  });

  it('sin corregir el giro lo deja como está', () => {
    const lens = { ...IDENTITY_FAMILY, scale: [1.01, 1] as [number, number], rotation_deg: 0.3 };
    const current = base({ rotation_deg: 0.1 });
    const m = simulateMeasures9(110, current, lens);
    const r = ok(fitNinePoints(110, current, m, false));
    expect(r.correction.rotation_deg).toBe(0.1);
    const withRot = ok(fitNinePoints(110, current, m, true));
    // Con el giro libre también queda dentro de la tolerancia (la lente de la prueba gira 0,3 grados).
    expect(withRot.maxAfter).toBeLessThan(0.02);
    expect(r.maxBefore).toBeGreaterThan(0.3);
  });

  it('rechaza medidas que no sirven', () => {
    const good = grid9Points(110);
    expect(fitNinePoints(110, base(), good.slice(0, 8), true)).toBe('invalid');
    expect(fitNinePoints(110, base(), good.map((p, i): Pt => (i === 2 ? [Number.NaN, p[1]] : p)), true)).toBe('invalid');
    expect(fitNinePoints(0, base(), good, true)).toBe('invalid');
    // Un punto a 20 mm de donde debería estar (más de un cuarto de los 38,5 mm del lado) es un error de medida.
    expect(fitNinePoints(110, base(), good.map((p, i): Pt => (i === 0 ? [p[0] + 20, p[1]] : p)), true)).toBe('implausible');
  });

  it('la cruz del centro es siempre el origen aunque se mande otra cosa', () => {
    const m = grid9Points(110).map((p, i): Pt => (i === 4 ? [3, 3] : p));
    const r = ok(fitNinePoints(110, base(), m, true));
    expect(r.errorsBefore[4]).toEqual([0, 0]);
  });
});
