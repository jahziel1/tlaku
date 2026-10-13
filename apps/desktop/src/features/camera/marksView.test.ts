import { describe, expect, it } from 'vitest';
import type { MarkResult, MarksReport } from '@/api/types';
import { applyH } from './geometry';
import { markTone, missingMarkPositions, rateMm, reportError, reportGrade, rescaleReport } from './marksView';

const mark = (over: Partial<MarkResult> = {}): MarkResult => ({
  index: 0,
  bed: [20, 10],
  image: [100, 200],
  confidence: 0.9,
  used: true,
  residual_mm: 0.1,
  loo_mm: null,
  ...over,
});

const report = (over: Partial<MarksReport> = {}): MarksReport => ({
  marks: [],
  candidates: [],
  to_bed: [[0.5, 0, 0], [0, -0.5, 300], [0, 0, 1]],
  expected: 9,
  found: 9,
  used: 9,
  rms_mm: 0.1,
  max_residual_mm: 0.2,
  loo_rms_mm: 0.25,
  loo_max_mm: 0.55,
  scale_px_per_mm: 2,
  used_prior: false,
  warnings: [],
  ...over,
});

describe('calificación del error', () => {
  it('umbrales de 0,3, 0,6 y 1 mm', () => {
    expect([0.05, 0.29, 0.3, 0.59, 0.6, 0.99, 1, 4].map(rateMm)).toEqual(['excellent', 'excellent', 'good', 'good', 'fair', 'fair', 'poor', 'poor']);
  });

  it('califica con la validación cruzada si existe y con el residuo si no', () => {
    expect(reportError(report())).toBe(0.55);
    expect(reportGrade(report())).toBe('good');
    const four = report({ loo_rms_mm: null, loo_max_mm: null, max_residual_mm: 0.05 });
    expect(reportError(four)).toBe(0.05);
    expect(reportGrade(four)).toBe('excellent');
  });
});

describe('tono de cada marca', () => {
  it('no encontrada, descartada, con error alto y buena', () => {
    expect(markTone(mark({ image: null, used: false, residual_mm: null }))).toBe('missing');
    expect(markTone(mark({ used: false }))).toBe('bad');
    expect(markTone(mark({ residual_mm: 0.2, loo_mm: 0.9 }))).toBe('warn');
    expect(markTone(mark({ residual_mm: 0.2, loo_mm: 0.3 }))).toBe('ok');
  });

  it('calcula dónde debería estar una marca que falta', () => {
    // to_bed lleva (x, y) px a (0.5 x, 300 - 0.5 y) mm: la marca en la cama (20, 10) cae en (40, 580).
    const pos = missingMarkPositions(report({ marks: [mark(), mark({ index: 3, image: null, used: false })] }));
    expect(pos).toHaveLength(1);
    expect(pos[0]!.index).toBe(3);
    expect(pos[0]!.at[0]).toBeCloseTo(40, 6);
    expect(pos[0]!.at[1]).toBeCloseTo(580, 6);
  });
});

describe('rescaleReport', () => {
  it('lleva las posiciones y la homografía al espacio de la alineación sin cambiar los mm', () => {
    const r = report({ marks: [mark({ image: [100, 50] })], candidates: [[10, 20]] });
    const big = rescaleReport(r, 1.5);
    expect(big.marks[0]!.image).toEqual([150, 75]);
    expect(big.candidates).toEqual([[15, 30]]);
    expect(big.scale_px_per_mm).toBe(3);
    // El mismo punto de la cama sale de ambos píxeles.
    const a = applyH(r.to_bed, [100, 50]);
    const b = applyH(big.to_bed, [150, 75]);
    expect(Math.hypot(a[0] - b[0], a[1] - b[1])).toBeLessThan(1e-9);
    expect(big.marks[0]!.residual_mm).toBe(r.marks[0]?.residual_mm ?? 0.1);
    expect(rescaleReport(r, 1)).toBe(r);
  });
});
