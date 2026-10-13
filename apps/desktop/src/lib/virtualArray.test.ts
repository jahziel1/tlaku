import { describe, expect, it } from 'vitest';
import type { DesignObject, VirtualArray } from '@/api/types';
import { rectObj } from '@/features/canvas/testUtil';
import { applyAffine, objectBounds } from './geometry';
import { MAX_VIRTUAL_COPIES, objectBoundsWithCopies, virtualArrayIssue, virtualCopies } from './virtualArray';

const withArray = (array: VirtualArray | null, patch: Partial<DesignObject> = {}): DesignObject => ({ ...rectObj('a', 5, 5, 15, 15), array, ...patch });
const near = (a: readonly number[], b: readonly number[]) => a.forEach((v, i) => expect(v).toBeCloseTo(b[i]!, 9));

// Los mismos casos que las pruebas de `virtual_copies` en crates/tlaku-doc/src/tests.rs: el lienzo y el
// trabajo enviado deben coincidir.
describe('virtualCopies', () => {
  it('sin arreglo no hay copias', () => {
    expect(virtualCopies(withArray(null))).toEqual([]);
  });

  it('rejilla: pasos de borde a borde y sin contar la original', () => {
    const m = virtualCopies(withArray({ type: 'grid', cols: 3, rows: 2, dx: 2, dy: 4 }));
    expect(m).toHaveLength(5);
    near(applyAffine(m[0]!, [0, 0]), [12, 0]);
    near(applyAffine(m[1]!, [0, 0]), [24, 0]);
    near(applyAffine(m[2]!, [0, 0]), [0, 14]);
    near(applyAffine(m[4]!, [0, 0]), [24, 14]);
  });

  it('circular de vuelta completa no repite la original', () => {
    const o = { ...withArray({ type: 'circular', count: 4, center: [0, 0], sweep_deg: 360, rotate: true }) };
    const m = virtualCopies(o);
    expect(m).toHaveLength(3);
    near(applyAffine(m[0]!, [10, 0]), [0, 10]);
    near(applyAffine(m[1]!, [10, 0]), [-10, 0]);
    near(applyAffine(m[2]!, [10, 0]), [0, -10]);
  });

  it('circular en arco incluye ambos extremos y puede conservar la orientación', () => {
    const o = withArray({ type: 'circular', count: 3, center: [0, 0], sweep_deg: 180, rotate: false });
    const m = virtualCopies(o);
    expect(m).toHaveLength(2);
    // La caja del objeto está en 5..15: su centro (10, 10) recorre media vuelta.
    near(applyAffine(m[0]!, [10, 10]), [-10, 10]);
    for (const t of m) expect(t.slice(0, 4)).toEqual([1, 0, 0, 1]);
    const [x, y] = applyAffine(m[1]!, [10, 10]);
    expect(Math.hypot(x, y)).toBeCloseTo(Math.hypot(10, 10), 9);
    near([x, y], [-10, -10]);
  });

  it('las copias siguen el transform del objeto', () => {
    const o = withArray({ type: 'grid', cols: 2, rows: 1, dx: 0, dy: 0 }, { transform: [1, 0, 0, 1, 50, 0] });
    const m = virtualCopies(o);
    near(applyAffine(m[0]!, [50, 5]), [60, 5]);
  });

  it('arreglos inválidos no generan copias', () => {
    expect(virtualCopies(withArray({ type: 'grid', cols: 0, rows: 2, dx: 0, dy: 0 }))).toEqual([]);
    expect(virtualCopies(withArray({ type: 'grid', cols: 100, rows: 100, dx: 0, dy: 0 }))).toEqual([]);
    expect(virtualCopies(withArray({ type: 'circular', count: 3, center: [Number.NaN, 0], sweep_deg: 360, rotate: true }))).toEqual([]);
  });

  it('virtualArrayIssue nombra el problema', () => {
    expect(virtualArrayIssue({ type: 'grid', cols: 0, rows: 1, dx: 0, dy: 0 })).toBe('counts');
    expect(virtualArrayIssue({ type: 'grid', cols: MAX_VIRTUAL_COPIES, rows: 2, dx: 0, dy: 0 })).toBe('tooMany');
    expect(virtualArrayIssue({ type: 'circular', count: 2, center: [0, Infinity], sweep_deg: 90, rotate: false })).toBe('numbers');
    expect(virtualArrayIssue({ type: 'circular', count: 6, center: [0, 0], sweep_deg: 360, rotate: true })).toBeNull();
  });
});

// Mismos casos que `bounds_include_the_virtual_copies` de crates/tlaku-doc/src/tests.rs.
describe('objectBoundsWithCopies', () => {
  it('sin arreglo es la caja del objeto', () => {
    const o = rectObj('a', 5, 5, 15, 15);
    expect(objectBoundsWithCopies(o)).toEqual(objectBounds(o));
  });

  it('la rejilla suma las copias a la caja', () => {
    const o = withArray({ type: 'grid', cols: 3, rows: 2, dx: 2, dy: 4 });
    // Caja de 10 x 10: paso de 12 en X y de 14 en Y.
    expect(objectBoundsWithCopies(o)).toEqual([[5, 5], [5 + 10 + 24, 5 + 10 + 14]]);
    expect(objectBounds(o)).toEqual([[5, 5], [15, 15]]);
  });

  it('el arreglo circular con giro usa la caja exacta de cada copia', () => {
    const o = withArray({ type: 'circular', count: 4, center: [0, 0], sweep_deg: 360, rotate: true });
    const [[x0, y0], [x1, y1]] = objectBoundsWithCopies(o)!;
    near([x0, y0, x1, y1], [-15, -15, 15, 15]);
  });

  it('un arreglo inválido no cambia la caja', () => {
    const o = withArray({ type: 'grid', cols: 0, rows: 2, dx: 0, dy: 0 });
    expect(objectBoundsWithCopies(o)).toEqual(objectBounds(o));
  });
});
