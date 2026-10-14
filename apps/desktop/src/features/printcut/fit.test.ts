import { describe, expect, it } from 'vitest';
import type { Affine, Point } from '@/api/types';
import { applyAffine, multiplyAffine } from '@/lib/geometry';
import { areCollinear, autoMode, fitRegistration, SCALE_WARN, type MarkPair } from './fit';

const rotate = (r: number): Affine => [Math.cos(r), Math.sin(r), -Math.sin(r), Math.cos(r), 0, 0];
const translate = (x: number, y: number): Affine => [1, 0, 0, 1, x, y];
const scale = (sx: number, sy: number): Affine => [sx, 0, 0, sy, 0, 0];

const pairsOf = (t: Affine, pts: Point[]): MarkPair[] => pts.map((d) => ({ design: d, physical: applyAffine(t, d) }));

/** Generador pseudoaleatorio determinista en [-1, 1]. */
function rng(seed: number) {
  let s = seed;
  return () => {
    s = (s * 1664525 + 1013904223) % 4294967296;
    return (s / 4294967296) * 2 - 1;
  };
}

function expectClose(a: readonly number[], b: readonly number[], tol: number) {
  a.forEach((v, i) => expect(Math.abs(v - b[i]!), `índice ${i}: ${v} vs ${b[i]}`).toBeLessThanOrEqual(tol));
}

function fit(pairs: MarkPair[], mode: Parameters<typeof fitRegistration>[1]) {
  const r = fitRegistration(pairs, mode);
  if (!r.ok) throw new Error(`ajuste falló: ${r.error.kind}`);
  return r.fit;
}

describe('ajuste con 2 marcas', () => {
  it('recupera una traslación pura', () => {
    const t = translate(3, -2);
    const f = fit(pairsOf(t, [[0, 0], [100, 0]]), 'similarity');
    expectClose(f.transform, t, 1e-9);
    expect(f.rotationDeg).toBeCloseTo(0, 9);
    expect(f.scale).toBeCloseTo(1, 12);
    expect(f.residualMax).toBeLessThan(1e-9);
    expect(f.warnings).toEqual([]);
  });

  it('recupera giro, escala uniforme y traslación', () => {
    const t = multiplyAffine(translate(10, 5), multiplyAffine(rotate(0.02), scale(1.003, 1.003)));
    const f = fit(pairsOf(t, [[5, 5], [155, 105]]), 'similarity');
    expectClose(f.transform, t, 1e-9);
    expect(f.rotationDeg).toBeCloseTo((0.02 * 180) / Math.PI, 9);
    expect(f.scale).toBeCloseTo(1.003, 9);
    expect(f.residualRms).toBeLessThan(1e-9);
    // 0,3 % no llega al umbral del 0,5 %.
    expect(f.warnings).toEqual([]);
  });

  it('sin escala reparte la diferencia de distancias entre las dos marcas', () => {
    const f = fit(
      [
        { design: [0, 0], physical: [0, 0] },
        { design: [100, 0], physical: [100.4, 0] },
      ],
      'rigid',
    );
    expect(f.scale).toBeCloseTo(1.004, 9);
    expect(Math.hypot(f.transform[0], f.transform[1])).toBeCloseTo(1, 12);
    expect(f.residuals[0]).toBeCloseTo(0.2, 9);
    expect(f.residuals[1]).toBeCloseTo(0.2, 9);
  });

  it('avisa si la escala se aleja de 1 más del 0,5 %', () => {
    const pts: Point[] = [[0, 0], [200, 0]];
    expect(fit(pairsOf(scale(1 + SCALE_WARN * 0.8, 1 + SCALE_WARN * 0.8), pts), 'similarity').warnings).toEqual([]);
    const big = fit(pairsOf(scale(1.01, 1.01), pts), 'similarity');
    expect(big.warnings).toHaveLength(1);
    expect(big.warnings[0]).toMatchObject({ kind: 'scale' });
    expect(fit(pairsOf(scale(0.99, 0.99), pts), 'similarity').warnings[0]).toMatchObject({ kind: 'scale' });
    // También en modo rígido: avisa con la escala que habría hecho falta.
    expect(fit(pairsOf(scale(1.01, 1.01), pts), 'rigid').warnings[0]).toMatchObject({ kind: 'scale' });
  });
});

describe('ajuste con 3 o más marcas', () => {
  it('la afín recupera sesgo y escala distinta en cada eje', () => {
    const t: Affine = [1.002, 0.01, -0.004, 0.998, 7, -3];
    const f = fit(pairsOf(t, [[0, 0], [200, 0], [0, 150]]), 'affine');
    expectClose(f.transform, t, 1e-9);
    expect(f.residualMax).toBeLessThan(1e-9);
  });

  it('avisa de una impresión estirada en un solo eje', () => {
    const f = fit(pairsOf(scale(1.012, 1), [[0, 0], [200, 0], [0, 150]]), 'affine');
    expect(f.warnings.some((w) => w.kind === 'anisotropy')).toBe(true);
  });

  it('avisa de una impresión espejada', () => {
    const f = fit(pairsOf([-1, 0, 0, 1, 200, 0], [[0, 0], [200, 0], [0, 150]]), 'affine');
    expect(f.warnings.some((w) => w.kind === 'mirrored')).toBe(true);
  });

  it('con ruido de ±0,05 mm el error residual lo refleja y la transformación sigue cerca', () => {
    const truth = multiplyAffine(translate(12, -4), rotate(-0.015));
    const noise = rng(42);
    const pts: Point[] = [[0, 0], [300, 0], [300, 200], [0, 200]];
    const pairs: MarkPair[] = pts.map((d) => {
      const [x, y] = applyAffine(truth, d);
      return { design: d, physical: [x + 0.05 * noise(), y + 0.05 * noise()] };
    });
    for (const mode of ['rigid', 'similarity', 'affine'] as const) {
      const f = fit(pairs, mode);
      expect(f.residualRms, mode).toBeGreaterThan(0);
      expect(f.residualRms, mode).toBeLessThan(0.08);
      expect(f.residualMax, mode).toBeLessThan(0.12);
      expect(f.residuals).toHaveLength(4);
      for (const p of pts) {
        const got = applyAffine(f.transform, p);
        const want = applyAffine(truth, p);
        expect(Math.hypot(got[0] - want[0], got[1] - want[1]), mode).toBeLessThan(0.1);
      }
      expect(f.rotationDeg).toBeCloseTo((-0.015 * 180) / Math.PI, 1);
    }
  });
});

describe('validación', () => {
  it('rechaza pocas marcas, marcas pegadas y números inválidos', () => {
    expect(fitRegistration([{ design: [0, 0], physical: [1, 1] }], 'similarity')).toEqual({ ok: false, error: { kind: 'few', need: 2, have: 1 } });
    expect(fitRegistration(pairsOf(translate(0, 0), [[0, 0], [50, 0]]), 'affine')).toEqual({ ok: false, error: { kind: 'few', need: 3, have: 2 } });
    expect(fitRegistration([{ design: [0, 0], physical: [0, 0] }, { design: [0.2, 0], physical: [50, 0] }], 'similarity')).toEqual({ ok: false, error: { kind: 'close', a: 1, b: 2 } });
    expect(fitRegistration([{ design: [0, 0], physical: [0, 0] }, { design: [50, 0], physical: [Number.NaN, 0] }], 'similarity')).toEqual({ ok: false, error: { kind: 'nan' } });
  });

  it('tres marcas en línea no sirven para la afín pero sí para la similitud', () => {
    const line = pairsOf(translate(1, 1), [[0, 0], [50, 0], [100, 0]]);
    expect(fitRegistration(line, 'affine')).toEqual({ ok: false, error: { kind: 'collinear' } });
    expect(fitRegistration(line, 'similarity').ok).toBe(true);
    expect(areCollinear([[0, 0], [50, 0], [100, 0]])).toBe(true);
    expect(areCollinear([[0, 0], [50, 0], [0, 100]])).toBe(false);
    expect(areCollinear([[0, 0], [50, 0]])).toBe(false);
  });

  it('el modo automático depende de cuántas marcas hay', () => {
    expect(autoMode(2)).toBe('similarity');
    expect(autoMode(3)).toBe('affine');
    expect(autoMode(4)).toBe('affine');
  });
});
