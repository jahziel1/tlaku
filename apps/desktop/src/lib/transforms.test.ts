import { describe, expect, it } from 'vitest';
import type { Point } from '@/api/types';
import { regularPolygonPoints, roundedPolygonBez, starPoints, flattenBez, bezBounds } from './bezpath';
import { applyAffine, flipHorizontalAbout, flipVerticalAbout, reflectAcrossLine, transformKind, twoPointSimilarity } from './geometry';

const near = (a: Point, b: Point, d = 8) => {
  expect(a[0]).toBeCloseTo(b[0], d);
  expect(a[1]).toBeCloseTo(b[1], d);
};

describe('voltear y reflejar', () => {
  it('voltear horizontal refleja sobre la vertical y deja los puntos de la línea en su sitio', () => {
    const m = flipHorizontalAbout(10);
    near(applyAffine(m, [4, 7]), [16, 7]);
    near(applyAffine(m, [10, 3]), [10, 3]);
  });

  it('voltear vertical refleja sobre la horizontal', () => {
    near(applyAffine(flipVerticalAbout(5), [4, 7]), [4, 3]);
  });

  it('reflejar sobre una recta oblicua: la recta es fija y la imagen del origen es simétrica', () => {
    const m = reflectAcrossLine([0, 0], [1, 1])!;
    near(applyAffine(m, [3, 3]), [3, 3]); // sobre la recta
    near(applyAffine(m, [2, 0]), [0, 2]); // la diagonal intercambia x e y
    // Recta que no pasa por el origen: y = 2 (horizontal).
    const h = reflectAcrossLine([-5, 2], [9, 2])!;
    near(applyAffine(h, [3, 5]), [3, -1]);
    // Reflejar dos veces es la identidad.
    near(applyAffine(h, applyAffine(h, [3, 5])), [3, 5]);
  });

  it('una recta de longitud cero no define reflejo', () => {
    expect(reflectAcrossLine([1, 1], [1, 1])).toBeNull();
  });
});

describe('girar y escalar por dos puntos', () => {
  it('lleva el punto de referencia hacia la dirección del destino (solo giro)', () => {
    const m = twoPointSimilarity([0, 0], [10, 0], [0, 5], false)!;
    near(applyAffine(m, [10, 0]), [0, 10]); // gira 90° sin escalar
  });

  it('con escala el punto de referencia cae exactamente en el destino', () => {
    const m = twoPointSimilarity([5, 5], [15, 5], [5, 25], true)!;
    near(applyAffine(m, [15, 5]), [5, 25]);
    near(applyAffine(m, [5, 5]), [5, 5]); // el pivote no se mueve
  });

  it('sin distancia al pivote no hay transformación', () => {
    expect(twoPointSimilarity([1, 1], [1, 1], [4, 4], true)).toBeNull();
    expect(twoPointSimilarity([1, 1], [4, 4], [1, 1], true)).toBeNull();
  });
});

describe('clasificar una transformación', () => {
  it('distingue mover, girar y escalar (el reflejo cuenta como escalar)', () => {
    expect(transformKind([1, 0, 0, 1, 5, -3])).toBe('move');
    expect(transformKind([0, 1, -1, 0, 5, 5])).toBe('rotate');
    expect(transformKind([2, 0, 0, 2, 0, 0])).toBe('scale');
    expect(transformKind(flipHorizontalAbout(3))).toBe('scale');
    expect(transformKind([1, 0, 0, 1, 0, 0])).toBe('move');
  });
});

describe('polígonos y estrellas', () => {
  it('un polígono regular tiene sus vértices en la circunferencia, con el primero arriba', () => {
    const pts = regularPolygonPoints(10, 20, 5, 6);
    expect(pts).toHaveLength(6);
    near(pts[0]!, [10, 25]);
    for (const p of pts) expect(Math.hypot(p[0] - 10, p[1] - 20)).toBeCloseTo(5, 9);
  });

  it('una estrella alterna radio externo e interno', () => {
    const pts = starPoints(0, 0, 10, 4, 5);
    expect(pts).toHaveLength(10);
    pts.forEach((p, i) => expect(Math.hypot(p[0], p[1])).toBeCloseTo(i % 2 === 0 ? 10 : 4, 9));
  });

  it('redondear esquinas deja curvas y la forma sigue dentro de la original', () => {
    const square: Point[] = [[0, 0], [10, 0], [10, 10], [0, 10]];
    const rounded = roundedPolygonBez(square, 2);
    expect(rounded.filter((s) => s.type === 'cubic_to')).toHaveLength(4);
    const box = bezBounds([rounded])!;
    expect(box[0][0]).toBeCloseTo(0, 6);
    expect(box[1][1]).toBeCloseTo(10, 6);
    // Sin radio son solo rectas.
    expect(roundedPolygonBez(square, 0).some((s) => s.type === 'cubic_to')).toBe(false);
    // El arco de una esquina de 90° con radio r pasa a r(1 - 1/√2) de la esquina.
    const flat = flattenBez(rounded, 0.001)[0]!.points;
    const nearCorner = Math.min(...flat.map(([x, y]) => Math.hypot(x - 0, y - 0)));
    expect(nearCorner).toBeCloseTo(2 * (Math.SQRT2 - 1), 1);
  });

  it('un radio demasiado grande se limita a la mitad del lado', () => {
    const square: Point[] = [[0, 0], [10, 0], [10, 10], [0, 10]];
    const box = bezBounds([roundedPolygonBez(square, 50)])!;
    expect(box[1][0] - box[0][0]).toBeCloseTo(10, 6); // se vuelve un círculo, sin salirse
  });
});
