import { describe, expect, it } from 'vitest';
import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { buildCalibDocument, calibJobOptions, calibSizes, fitCorrection, simulateMeasures, type FitResult } from './galvoCalib';

const IDENTITY = { scale: [1, 1] as [number, number], bulge: [0, 0] as [number, number] };
const fit = (r: ReturnType<typeof fitCorrection>): FitResult => {
  if (typeof r === 'string') throw new Error(`falló: ${r}`);
  return r;
};

describe('asistente de calibración del campo', () => {
  it('los lados de la rejilla son fracciones del campo', () => {
    expect(calibSizes(100)).toEqual({ outer: 70, inner: 35 });
  });

  it('una lente perfecta no cambia nada', () => {
    const { outer, inner } = calibSizes(110);
    const r = fit(fitCorrection(110, IDENTITY, { outer: [outer, outer], inner: [inner, inner] }));
    expect(r.scale).toEqual([1, 1]);
    expect(r.bulge[0]).toBeCloseTo(0, 5);
    expect(r.bulge[1]).toBeCloseTo(0, 5);
    expect(r.clamped).toBe(false);
  });

  it('una escala distinta en cada eje se corrige con su inversa', () => {
    // La lente dibuja 1 % más grande en X y 2 % más chico en Y.
    const m = simulateMeasures(110, { c: [1.01, 0.98], k: [0, 0] });
    const r = fit(fitCorrection(110, IDENTITY, m));
    expect(r.scale[0]).toBeCloseTo(1 / 1.01, 4);
    expect(r.scale[1]).toBeCloseTo(1 / 0.98, 4);
    expect(r.bulge).toEqual([0, 0]);
    // El error medido es el que se ve con la regla.
    expect(r.errorOuter[0]).toBeCloseTo(77 * 0.01, 4);
    expect(r.errorOuter[1]).toBeCloseTo(-77 * 0.02, 4);
  });

  it('el barril se recupera con dos tamaños y el signo contrario al de la lente', () => {
    const m = simulateMeasures(150, { c: [1, 1], k: [0.04, -0.02] });
    const r = fit(fitCorrection(150, IDENTITY, m));
    expect(r.bulge[0]).toBeCloseTo(-0.04, 4);
    expect(r.bulge[1]).toBeCloseTo(0.02, 4);
    expect(r.scale[0]).toBeCloseTo(1, 4);
  });

  it('suma al barril y a la escala que ya tenía el perfil (se puede repetir)', () => {
    const m = simulateMeasures(110, { c: [1.02, 1], k: [0.03, 0] });
    const r = fit(fitCorrection(110, { scale: [0.99, 1], bulge: [0.01, 0] }, m));
    expect(r.scale[0]).toBeCloseTo(0.99 / 1.02, 4);
    expect(r.bulge[0]).toBeCloseTo(0.01 - 0.03, 4);
  });

  it('aplicar la corrección calculada a la lente deja un error casi nulo', () => {
    // Primera pasada: lente con escala y barril. La corrección resultante, aplicada a la misma lente,
    // lleva el tamaño medido a (casi) el nominal.
    const lens = { c: [1.015, 0.99] as [number, number], k: [0.02, -0.01] as [number, number] };
    const r = fit(fitCorrection(110, IDENTITY, simulateMeasures(110, lens)));
    const h = 55;
    const { outer } = calibSizes(110);
    const x = outer / 2;
    // Corrección de la app (escala y luego barril) seguida de la lente.
    const corrected = (axis: 0 | 1) => {
      const y = x * r.scale[axis];
      const yb = y * (1 + r.bulge[axis] * (y / h) ** 2);
      return yb * lens.c[axis] * (1 + lens.k[axis] * (yb / h) ** 2);
    };
    expect(Math.abs(corrected(0) - x)).toBeLessThan(0.02);
    expect(Math.abs(corrected(1) - x)).toBeLessThan(0.02);
  });

  it('medidas inválidas o absurdas se rechazan', () => {
    const { outer, inner } = calibSizes(110);
    expect(fitCorrection(110, IDENTITY, { outer: [0, outer], inner: [inner, inner] })).toBe('invalid');
    expect(fitCorrection(110, IDENTITY, { outer: [outer, Number.NaN], inner: [inner, inner] })).toBe('invalid');
    expect(fitCorrection(110, IDENTITY, { outer: [-5, outer], inner: [inner, inner] })).toBe('invalid');
    expect(fitCorrection(0, IDENTITY, { outer: [outer, outer], inner: [inner, inner] })).toBe('invalid');
    // Se midió en otra unidad (cm en vez de mm): la escala sale absurda.
    expect(fitCorrection(110, IDENTITY, { outer: [outer / 10, outer / 10], inner: [inner / 10, inner / 10] })).toBe('implausible');
  });

  it('los valores que el motor no admite se recortan y se avisa', () => {
    // Un barril enorme en la lente: el perfil solo admite hasta ±0,3.
    const m = simulateMeasures(110, { c: [1, 1], k: [0.9, 0] });
    const r = fit(fitCorrection(110, IDENTITY, m));
    expect(r.bulge[0]).toBe(-0.3);
    expect(r.clamped).toBe(true);
  });

  it('la rejilla: dos cuadrados concéntricos y la marca del lado +X, dentro del campo y con la capa pedida', () => {
    const doc = buildCalibDocument([110, 110], 110, { power: 15, speedMmS: 500, frequencyKhz: 30, pulseWidthNs: null });
    expect(doc.objects.map((o) => o.id)).toEqual(['calib-outer', 'calib-inner', 'calib-x']);
    const layer = doc.layers[0]!;
    expect(layer.speed).toBe(30000);
    expect(layer.power).toBe(15);
    expect(layer.galvo.frequency_khz).toBe(30);
    expect(layer.mode).toBe('line');
    // Todos los puntos caen dentro del campo (la cama de 110 mm).
    const xs: number[] = [];
    const ys: number[] = [];
    for (const o of doc.objects) {
      if (o.kind.type !== 'vector') throw new Error('se esperaba un vector');
      for (const seg of o.kind.paths.flat()) {
        if ('to' in seg) {
          xs.push(seg.to[0]);
          ys.push(seg.to[1]);
        }
      }
    }
    expect(Math.min(...xs)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...xs)).toBeLessThanOrEqual(110);
    expect(Math.min(...ys)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...ys)).toBeLessThanOrEqual(110);
    // El cuadrado grande mide 77 mm y está centrado.
    const outer = doc.objects[0]!;
    if (outer.kind.type !== 'vector') throw new Error('vector');
    const pts = outer.kind.paths[0]!.flatMap((s) => ('to' in s ? [s.to] : []));
    expect(Math.max(...pts.map((p) => p[0])) - Math.min(...pts.map((p) => p[0]))).toBeCloseTo(77, 6);
    expect((Math.max(...pts.map((p) => p[0])) + Math.min(...pts.map((p) => p[0]))) / 2).toBeCloseTo(55, 6);
  });

  it('con una cama más grande que el campo, la rejilla queda en el centro de la cama', () => {
    const doc = buildCalibDocument([300, 200], 110, { power: 10, speedMmS: 300, frequencyKhz: 40, pulseWidthNs: 9 });
    const o = doc.objects[0]!;
    if (o.kind.type !== 'vector') throw new Error('vector');
    const pts = o.kind.paths[0]!.flatMap((s) => ('to' in s ? [s.to] : []));
    expect((Math.max(...pts.map((p) => p[1])) + Math.min(...pts.map((p) => p[1]))) / 2).toBeCloseTo(100, 6);
    expect(doc.layers[0]!.galvo.pulse_width_ns).toBe(9);
  });

  it('el trabajo de la rejilla va absoluto, sin selección ni tramos', () => {
    const o = calibJobOptions({ ...DEFAULT_JOB_OPTIONS, start_from: 'current_position', user_origin: [5, 5], selection: ['a'], use_selection_origin: true });
    expect(o.start_from).toBe('absolute');
    expect(o.user_origin).toBeNull();
    expect(o.selection).toBeNull();
    expect(o.use_selection_origin).toBe(false);
    expect(o.tile ?? null).toBeNull();
  });
});
