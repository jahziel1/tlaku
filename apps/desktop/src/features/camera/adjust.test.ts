import { describe, expect, it } from 'vitest';
import { adjustMatrix, adjustOf, autoDpm, isAdjusted, NO_ADJUST } from './adjust';
import { applyH, backdropCorners, type Mat3 } from './geometry';

/** Imagen de 1000 x 500 px que ve la cama a 5 px/mm: x = 10 + px/5, y = 200 - py/5. */
const M: Mat3 = [[0.2, 0, 10], [0, -0.2, 200], [0, 0, 1]];
const SPACE: [number, number] = [1000, 500];

describe('ajuste manual del fondo', () => {
  it('mide los puntos por mm de la alineación en el centro', () => {
    expect(autoDpm(M, SPACE)).toBeCloseTo(5, 9);
    // Una matriz degenerada no da resolución.
    expect(autoDpm([[0, 0, 1], [0, 0, 1], [0, 0, 1]], SPACE)).toBe(0);
  });

  it('sin ajuste devuelve la misma matriz', () => {
    expect(adjustMatrix(M, SPACE, NO_ADJUST)).toBe(M);
    expect(isAdjusted(NO_ADJUST)).toBe(false);
  });

  it('el desplazamiento mueve todo el fondo en X e Y', () => {
    const m = adjustMatrix(M, SPACE, { dpm: null, dx: 3, dy: -2 });
    for (const p of [[0, 0], [1000, 500], [250, 100]] as const) {
      const [x0, y0] = applyH(M, [p[0], p[1]]);
      const [x1, y1] = applyH(m, [p[0], p[1]]);
      expect(x1 - x0).toBeCloseTo(3, 9);
      expect(y1 - y0).toBeCloseTo(-2, 9);
    }
  });

  it('fijar la resolución escala alrededor del centro de la imagen', () => {
    const centre = applyH(M, [500, 250]);
    // 10 px/mm en lugar de 5: el fondo mide la mitad y su centro no se mueve.
    const m = adjustMatrix(M, SPACE, { dpm: 10, dx: 0, dy: 0 });
    expect(autoDpm(m, SPACE)).toBeCloseTo(10, 9);
    const c2 = applyH(m, [500, 250]);
    expect(c2[0]).toBeCloseTo(centre[0], 9);
    expect(c2[1]).toBeCloseTo(centre[1], 9);
    const [tl, tr] = backdropCorners(m, SPACE);
    expect(tr[0] - tl[0]).toBeCloseTo(100, 9); // 1000 px a 10 px/mm
    const [, tr0] = [backdropCorners(M, SPACE)[0], backdropCorners(M, SPACE)[1]];
    expect(tr0[0] - backdropCorners(M, SPACE)[0][0]).toBeCloseTo(200, 9);
  });

  it('la misma resolución de la alineación no cambia nada y combina con el desplazamiento', () => {
    const same = adjustMatrix(M, SPACE, { dpm: 5, dx: 0, dy: 0 });
    for (const p of [[0, 0], [700, 90]] as const) {
      const a = applyH(M, [p[0], p[1]]);
      const b = applyH(same, [p[0], p[1]]);
      expect(b[0]).toBeCloseTo(a[0], 9);
      expect(b[1]).toBeCloseTo(a[1], 9);
    }
    const both = adjustMatrix(M, SPACE, { dpm: 10, dx: 1.5, dy: 0.5 });
    const c = applyH(both, [500, 250]);
    const c0 = applyH(M, [500, 250]);
    expect(c[0]).toBeCloseTo(c0[0] + 1.5, 9);
    expect(c[1]).toBeCloseTo(c0[1] + 0.5, 9);
  });

  it('funciona con perspectiva', () => {
    const P: Mat3 = [[0.2, 0, 10], [0, -0.2, 200], [0.0001, 0, 1]];
    const m = adjustMatrix(P, SPACE, { dpm: null, dx: 4, dy: 1 });
    const [x0, y0] = applyH(P, [300, 400]);
    const [x1, y1] = applyH(m, [300, 400]);
    expect(x1 - x0).toBeCloseTo(4, 9);
    expect(y1 - y0).toBeCloseTo(1, 9);
  });

  it('lee las preferencias con los valores que faltan o no sirven', () => {
    expect(adjustOf({ dpm: null, offset_x_mm: undefined, offset_y_mm: undefined })).toEqual(NO_ADJUST);
    expect(adjustOf({ dpm: -3, offset_x_mm: Number.NaN, offset_y_mm: 2 })).toEqual({ dpm: null, dx: 0, dy: 2 });
    expect(adjustOf({ dpm: 8.5, offset_x_mm: 1, offset_y_mm: -1 })).toEqual({ dpm: 8.5, dx: 1, dy: -1 });
  });
});
