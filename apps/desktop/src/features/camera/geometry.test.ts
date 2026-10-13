import { describe, expect, it } from 'vitest';
import {
  applyH,
  backdropCorners,
  homographyFrom4,
  invertH,
  isConvexQuad,
  pointInPolygon,
  rectCorners,
  type Quad,
} from './geometry';
import { fitAlignment } from './setup';

const near = (a: [number, number], b: [number, number], eps = 1e-6) => {
  expect(a[0]).toBeCloseTo(b[0], -Math.log10(eps));
  expect(a[1]).toBeCloseTo(b[1], -Math.log10(eps));
};

describe('homografía de 4 puntos', () => {
  // Hoja carta vista con perspectiva en 640x480 -> mm de la cama.
  const px: Quad = [[100, 400], [540, 410], [530, 60], [110, 70]];
  const mm: Quad = rectCorners(10, 10, 279.4, 215.9);

  it('lleva cada punto marcado a su coordenada en mm', () => {
    const h = homographyFrom4(px, mm)!;
    expect(h).not.toBeNull();
    px.forEach((p, i) => near(applyH(h, p), mm[i]!));
  });

  it('proyecta puntos intermedios de forma proyectiva (no afín)', () => {
    // Plano real: x_mm = 2 * X, y_mm = 3 * Y con perspectiva H conocida; se recupera con 4 puntos.
    const truth: [[number, number, number], [number, number, number], [number, number, number]] = [
      [0.9, 0.1, 5],
      [-0.05, 1.1, 8],
      [0.0004, 0.0002, 1],
    ];
    const src: Quad = [[0, 0], [600, 0], [600, 400], [0, 400]];
    const dst = src.map((p) => applyH(truth, p)) as Quad;
    const h = homographyFrom4(src, dst)!;
    for (const p of [[300, 200], [123, 321], [599, 1]] as [number, number][]) near(applyH(h, p), applyH(truth, p), 1e-5);
  });

  it('la inversa deshace la transformación', () => {
    const h = homographyFrom4(px, mm)!;
    const inv = invertH(h)!;
    near(applyH(inv, applyH(h, [250, 220])), [250, 220]);
    mm.forEach((p, i) => near(applyH(inv, p), px[i]!, 1e-5));
  });

  it('acepta una cámara en espejo (orientación invertida)', () => {
    const mirrored: Quad = px.map(([x, y]) => [640 - x, y]) as Quad;
    const h = homographyFrom4(mirrored, mm)!;
    mirrored.forEach((p, i) => near(applyH(h, p), mm[i]!));
  });

  it('rechaza puntos degenerados (tres alineados)', () => {
    const bad: Quad = [[0, 0], [10, 10], [20, 20], [5, 40]];
    expect(homographyFrom4(bad, mm)).toBeNull();
  });

  it('las esquinas del fondo salen de proyectar los bordes de la imagen', () => {
    // Sin perspectiva: 1 px = 0,5 mm, imagen de 640x480 con origen en (10, 20).
    const h = homographyFrom4([[0, 0], [640, 0], [640, 480], [0, 480]], [[10, 20], [330, 20], [330, 260], [10, 260]])!;
    const c = backdropCorners(h, [640, 480]);
    near(c[0], [10, 20]);
    near(c[1], [330, 20]);
    near(c[2], [330, 260]);
    near(c[3], [10, 260]);
  });

  it('el ajuste devuelve la homografía y un error casi nulo', () => {
    const fit = fitAlignment([640, 480], px, mm)!;
    expect(fit.rms_mm).toBeLessThan(1e-6);
    near(applyH(fit.to_bed, px[2]!), mm[2]!);
    expect(fitAlignment([640, 480], [[0, 0], [10, 10], [20, 20], [30, 30]], mm)).toBeNull();
  });
});

describe('cuadriláteros', () => {
  it('detecta convexos y cruzados', () => {
    expect(isConvexQuad([[0, 0], [10, 0], [10, 10], [0, 10]])).toBe(true);
    expect(isConvexQuad([[0, 0], [10, 10], [10, 0], [0, 10]])).toBe(false); // lazo
    expect(isConvexQuad([[0, 0], [10, 0], [10, 10], [9, 1]])).toBe(false); // cóncavo
    expect(isConvexQuad([[0, 0], [1, 0], [1, 1], [0, 1]], 5)).toBe(false); // muy pequeño
  });

  it('punto en polígono', () => {
    const sq: [number, number][] = [[0, 0], [10, 0], [10, 10], [0, 10]];
    expect(pointInPolygon([5, 5], sq)).toBe(true);
    expect(pointInPolygon([15, 5], sq)).toBe(false);
  });
});
