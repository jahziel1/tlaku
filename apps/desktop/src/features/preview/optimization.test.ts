import { describe, expect, it } from 'vitest';
import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { DEFAULT_OPTIMIZATION, OPTIMIZATION_GROUPS, sameOptimization, sanitizeOptimization } from './optimization';

describe('ajustes de optimización', () => {
  it('los valores de fábrica son los del motor', () => {
    expect(DEFAULT_JOB_OPTIONS.optimization).toEqual(DEFAULT_OPTIMIZATION);
    expect(DEFAULT_OPTIMIZATION).toEqual({
      by_layer: true,
      by_group: false,
      by_priority: true,
      inner_first: true,
      best_start: true,
      choose_direction: true,
      remove_overlaps: false,
      overlap_tolerance: 0.01,
      hide_backlash: false,
      reduce_travel: true,
    });
  });

  it('el diálogo ofrece todas las opciones sí/no, cada una una sola vez', () => {
    const keys = OPTIMIZATION_GROUPS.flatMap((g) => g.flags.map((f) => f.key)).sort();
    expect(keys).toEqual(Object.keys(DEFAULT_OPTIMIZATION).filter((k) => k !== 'overlap_tolerance').sort());
  });

  it('sanea: tipos incorrectos al valor de fábrica y tolerancia acotada', () => {
    expect(sanitizeOptimization(null)).toEqual(DEFAULT_OPTIMIZATION);
    expect(sanitizeOptimization({ by_layer: 'no', hide_backlash: true, overlap_tolerance: 0 })).toEqual({
      ...DEFAULT_OPTIMIZATION,
      hide_backlash: true,
      overlap_tolerance: 0.001,
    });
    expect(sanitizeOptimization({ overlap_tolerance: Number.NaN }).overlap_tolerance).toBe(0.01);
    expect(sanitizeOptimization({ overlap_tolerance: 7 }).overlap_tolerance).toBe(1);
  });

  it('compara por contenido', () => {
    expect(sameOptimization(DEFAULT_OPTIMIZATION, { ...DEFAULT_OPTIMIZATION })).toBe(true);
    expect(sameOptimization(DEFAULT_OPTIMIZATION, { ...DEFAULT_OPTIMIZATION, by_group: true })).toBe(false);
  });
});
