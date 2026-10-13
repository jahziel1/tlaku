import { describe, expect, it } from 'vitest';
import { convexHull } from './hull';

describe('convexHull', () => {
  it('descarta interiores, alineados, repetidos y valores que no son números', () => {
    const h = convexHull([[10, 10], [20, 10], [30, 10], [30, 30], [10, 30], [20, 20], [20, 20], [Number.NaN, 5]]);
    expect(h).toEqual([[10, 10], [30, 10], [30, 30], [10, 30]]);
  });

  it('va en sentido antihorario y empieza por el punto más bajo', () => {
    const h = convexHull([[0, 5], [10, 0], [5, 12], [4, 5]]);
    expect(h[0]).toEqual([10, 0]);
    const area = h.reduce((sum, a, i) => {
      const b = h[(i + 1) % h.length]!;
      return sum + a[0] * b[1] - b[0] * a[1];
    }, 0);
    expect(area).toBeGreaterThan(0);
    expect(h).not.toContainEqual([4, 5]);
  });

  it('casos degenerados', () => {
    expect(convexHull([])).toEqual([]);
    expect(convexHull([[1, 2], [1, 2]])).toEqual([[1, 2]]);
    expect(convexHull([[0, 0], [5, 5], [10, 10], [2, 2]])).toEqual([[0, 0], [10, 10]]);
  });
});
