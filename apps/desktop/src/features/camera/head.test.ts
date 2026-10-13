import { describe, expect, it } from 'vitest';
import type { HeadCalibration } from '@/api/types';
import { applyH, type Vec2 } from './geometry';
import {
  SCAN_CROP,
  SCAN_MAX_STOPS,
  estimateScanSeconds,
  fitHeadCalibration,
  headCorners,
  headFootprint,
  headPointMm,
  headPxPerMm,
  headTileToBed,
  planScan,
  type HeadObservation,
} from './head';

/** Ruido determinista (-0.5..0.5). */
const wiggle = (k: number) => {
  const v = Math.sin(k * 12.9898) * 43758.5453;
  return v - Math.floor(v) - 0.5;
};

/** Cámara del cabezal verdadera: 1280x720, 8 px/mm, girada 1.7° y con el eje Y de la imagen invertido. */
function truth(): HeadCalibration {
  const s = 1 / 8;
  const th = (1.7 * Math.PI) / 180;
  const c = Math.cos(th) * s;
  const sn = Math.sin(th) * s;
  return {
    image_size: [1280, 720],
    ref_px: [640, 360],
    px_to_mm: [[c, sn], [sn, -c]],
    offset_mm: [23.4, -41.7],
    rms_mm: 0,
    observations: 0,
  };
}

/** Píxel donde se ve el punto `mm` con el láser en `laser` (inversa del modelo). */
function pixelOf(cal: HeadCalibration, laser: Vec2, mm: Vec2): Vec2 {
  const [[a, b], [c, d]] = cal.px_to_mm;
  const rx = mm[0] - laser[0] - cal.offset_mm[0];
  const ry = mm[1] - laser[1] - cal.offset_mm[1];
  const det = a * d - b * c;
  return [cal.ref_px[0] + (d * rx - b * ry) / det, cal.ref_px[1] + (-c * rx + a * ry) / det];
}

describe('calibración del desfase cámara-láser', () => {
  const mark: Vec2 = [150, 100];

  function observe(cal: HeadCalibration, lasers: Vec2[], noisePx: number): HeadObservation[] {
    return lasers.map((laser, i) => {
      const p = pixelOf(cal, laser, mark);
      return { laser, px: [p[0] + noisePx * wiggle(2 * i + 1), p[1] + noisePx * wiggle(2 * i + 2)] };
    });
  }

  // Posiciones del láser que dejan la marca dentro de la imagen (cerca de donde la ve el centro).
  const lasers = (): Vec2[] => {
    const cal = truth();
    const base: Vec2 = [mark[0] - cal.offset_mm[0] + 3, mark[1] - cal.offset_mm[1] - 2];
    return [base, [base[0] + 6, base[1]], [base[0] + 6, base[1] + 6], [base[0], base[1] + 6]];
  };

  it('sin ruido recupera el desfase, la escala y el giro', () => {
    const cal = truth();
    const fit = fitHeadCalibration(observe(cal, lasers(), 0), mark, [1280, 720]);
    expect('calibration' in fit).toBe(true);
    if (!('calibration' in fit)) return;
    const c = fit.calibration;
    expect(c.offset_mm[0]).toBeCloseTo(cal.offset_mm[0], 6);
    expect(c.offset_mm[1]).toBeCloseTo(cal.offset_mm[1], 6);
    for (const i of [0, 1] as const) for (const j of [0, 1] as const) expect(c.px_to_mm[i][j]).toBeCloseTo(cal.px_to_mm[i][j], 8);
    expect(c.rms_mm).toBeLessThan(1e-8);
    expect(headPxPerMm(c)).toBeCloseTo(8, 6);
  });

  it('con 0.4 px de ruido en el clic el desfase queda a menos de 0.1 mm', () => {
    const cal = truth();
    const fit = fitHeadCalibration(observe(cal, lasers(), 0.4), mark, [1280, 720]);
    if (!('calibration' in fit)) throw new Error('no ajustó');
    const c = fit.calibration;
    const err = Math.hypot(c.offset_mm[0] - cal.offset_mm[0], c.offset_mm[1] - cal.offset_mm[1]);
    expect(err).toBeLessThan(0.1);
    expect(c.rms_mm).toBeLessThan(0.1);
    // Predice dónde cae la marca con el láser en otra posición: error pequeño.
    const L: Vec2 = [mark[0] - cal.offset_mm[0] + 1, mark[1] - cal.offset_mm[1] + 2];
    const px = pixelOf(cal, L, mark);
    const est = headPointMm(c, L, px);
    expect(Math.hypot(est[0] - mark[0], est[1] - mark[1])).toBeLessThan(0.15);
  });

  it('con menos de tres observaciones, sin movimiento o con datos inválidos da un error claro', () => {
    const cal = truth();
    const obs = observe(cal, lasers(), 0);
    expect(fitHeadCalibration(obs.slice(0, 2), mark, [1280, 720])).toEqual({ error: 'tooFew' });
    // El cabezal no se movió: la marca cae siempre en el mismo píxel.
    const still = [0, 1, 2, 3].map(() => obs[0]!);
    expect(fitHeadCalibration(still, mark, [1280, 720])).toEqual({ error: 'collinear' });
    // Movimiento solo en una dirección: no se distingue el giro.
    const line = observe(cal, [0, 3, 6, 9].map((d): Vec2 => [lasers()[0]![0] + d, lasers()[0]![1]]), 0);
    expect(fitHeadCalibration(line, mark, [1280, 720])).toEqual({ error: 'collinear' });
    expect(fitHeadCalibration([{ laser: [NaN, 0], px: [0, 0] }, ...obs.slice(1)], mark, [1280, 720])).toEqual({ error: 'invalid' });
  });
});

describe('geometría de la cámara del cabezal', () => {
  it('la homografía de una foto lleva cada píxel al punto que ve', () => {
    const cal = truth();
    const L: Vec2 = [80, 55];
    const h = headTileToBed(cal, L);
    for (const p of [[0, 0], [1280, 720], [640, 360], [123.5, 600.25]] as Vec2[]) {
      const a = applyH(h, p);
      const b = headPointMm(cal, L, p);
      expect(a[0]).toBeCloseTo(b[0], 9);
      expect(a[1]).toBeCloseTo(b[1], 9);
    }
    // El píxel de referencia cae en láser + desfase.
    const c = applyH(h, cal.ref_px);
    expect(c[0]).toBeCloseTo(L[0] + cal.offset_mm[0], 9);
    expect(c[1]).toBeCloseTo(L[1] + cal.offset_mm[1], 9);
    const q = headCorners(cal, L);
    expect(q).toHaveLength(4);
    // La imagen no gira en pantalla más de lo calibrado: el lado superior mide el ancho de la foto.
    expect(Math.hypot(q[1][0] - q[0][0], q[1][1] - q[0][1])).toBeCloseTo(1280 / 8, 6);
  });

  it('la huella de una foto tiene el tamaño en mm y se desplaza con el desfase', () => {
    const cal = truth();
    const fp = headFootprint(cal);
    expect(fp.w).toBeGreaterThan(160);
    expect(fp.w).toBeLessThan(165);
    expect(fp.h).toBeGreaterThan(90);
    expect(fp.h).toBeLessThan(95);
    expect(Math.abs(fp.cx)).toBeLessThan(0.01);
  });
});

describe('recorrido en cuadrícula', () => {
  const cal = (): HeadCalibration => ({ ...truth(), offset_mm: [0, -20] });

  it('cubre toda la cama con traslape y recorre en serpiente', () => {
    const bed: Vec2 = [400, 300];
    const plan = planScan(bed, cal());
    if ('error' in plan) throw new Error(plan.error);
    expect(plan.stops.length).toBe(plan.cols * plan.rows);
    expect(plan.cols).toBeGreaterThanOrEqual(3);
    expect(plan.rows).toBeGreaterThanOrEqual(3);
    // Serpiente: la primera fila va de izquierda a derecha y la segunda al revés.
    const row0 = plan.stops.slice(0, plan.cols);
    const row1 = plan.stops.slice(plan.cols, 2 * plan.cols);
    expect(row0[0]!.center[0]).toBeLessThan(row0.at(-1)!.center[0]);
    expect(row1[0]!.center[0]).toBeGreaterThan(row1.at(-1)!.center[0]);
    // Cada punto de la cama cae dentro de la parte útil de alguna foto, y casi siempre de dos (traslape).
    const [tw, th] = plan.tile;
    let minHits = Infinity;
    for (let y = 0; y <= bed[1]; y += 10) {
      for (let x = 0; x <= bed[0]; x += 10) {
        const hits = plan.stops.filter((s) => Math.abs(x - s.center[0]) <= tw / 2 + 1e-9 && Math.abs(y - s.center[1]) <= th / 2 + 1e-9).length;
        minHits = Math.min(minHits, hits);
      }
    }
    expect(minHits).toBeGreaterThanOrEqual(1);
    expect(plan.partial).toBe(false);
    // El láser nunca sale de la cama y la cámara (con su desfase) ve el centro previsto.
    for (const s of plan.stops) {
      expect(s.laser[0]).toBeGreaterThanOrEqual(0);
      expect(s.laser[1]).toBeGreaterThanOrEqual(0);
      expect(s.laser[0]).toBeLessThanOrEqual(bed[0]);
      expect(s.laser[1]).toBeLessThanOrEqual(bed[1]);
      if (!s.clamped) {
        const seen = headPointMm(cal(), s.laser, cal().ref_px);
        expect(seen[0]).toBeCloseTo(s.center[0], 6);
        expect(seen[1]).toBeCloseTo(s.center[1], 6);
      }
    }
    // Fotos vecinas se traslapan al menos un 15 % de la parte útil.
    const dx = Math.abs(row0[1]!.center[0] - row0[0]!.center[0]);
    expect(1 - dx / tw).toBeGreaterThan(0.15);
    expect(SCAN_CROP).toBeGreaterThan(0);
  });

  it('un desfase que saca al láser de la cama se recorta y se avisa', () => {
    // La cámara ve 60 mm "detrás" del láser: para ver el borde de abajo el láser tendría que salir.
    const c: HeadCalibration = { ...truth(), offset_mm: [0, 60] };
    const plan = planScan([400, 300], c);
    if ('error' in plan) throw new Error(plan.error);
    expect(plan.stops.some((s) => s.clamped)).toBe(true);
    expect(plan.partial).toBe(true);
  });

  it('una cama más chica que la foto necesita una sola posición, y una región acota el recorrido', () => {
    const one = planScan([100, 80], cal());
    if ('error' in one) throw new Error(one.error);
    expect(one.stops).toHaveLength(1);
    const full = planScan([600, 400], cal());
    const part = planScan([600, 400], cal(), [0, 0, 150, 100]);
    if ('error' in full || 'error' in part) throw new Error('plan');
    expect(part.stops.length).toBeLessThan(full.stops.length);
    expect(planScan([400, 300], cal(), [10, 10, 10, 20])).toEqual({ error: 'empty' });
  });

  it('rechaza recorridos con demasiadas fotos y una escala inválida', () => {
    const tiny: HeadCalibration = { ...truth(), px_to_mm: [[0.5, 0], [0, -0.5]], image_size: [64, 36], ref_px: [32, 18] };
    const r = planScan([2000, 2000], tiny);
    expect(r).toEqual({ error: 'tooMany' });
    expect(SCAN_MAX_STOPS).toBe(150);
    expect(planScan([400, 300], { ...truth(), px_to_mm: [[0, 0], [0, 0]] })).toEqual({ error: 'scale' });
  });

  it('estima el tiempo con la distancia y el reposo de cada foto', () => {
    const plan = planScan([400, 300], cal());
    if ('error' in plan) throw new Error(plan.error);
    const fast = estimateScanSeconds(plan, 12000, 0, null);
    const slow = estimateScanSeconds(plan, 3000, 0, null);
    expect(slow).toBeCloseTo(fast * 4, 6);
    expect(estimateScanSeconds(plan, 12000, 500, null)).toBeCloseTo(fast + plan.stops.length * 0.5, 6);
  });
});
