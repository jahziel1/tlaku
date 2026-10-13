import { describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { polylineLength } from '@/lib/geometry';
import { bezBounds, ellipseBez, flattenBez, roundedRectBez, type BezPath } from '@/lib/bezpath';
import {
  boxShapePaths, constrainAngle, constrainDelta, dedupePoints, dragBox, newVectorObject, pickDrawLayer, polylinePaths, rectPoints,
} from './shapes';

describe('creación de formas', () => {
  const kinds = (p: BezPath) => p.map((s) => s.type);

  it('rectángulo: cuatro esquinas antihorarias', () => {
    expect(rectPoints(10, 20, 30, 50)).toEqual([[10, 20], [30, 20], [30, 50], [10, 50]]);
    expect(rectPoints(30, 50, 10, 20)).toEqual(rectPoints(10, 20, 30, 50)); // esquinas en cualquier orden
  });

  it('rectángulo con radio: mismas dimensiones, esquinas como cúbicas reales', () => {
    const path = roundedRectBez(0, 0, 40, 20, 5);
    expect(bezBounds([path])).toEqual([[0, 0], [40, 20]]);
    expect(kinds(path).filter((k) => k === 'cubic_to')).toHaveLength(4);
    expect(kinds(path).filter((k) => k === 'line_to')).toHaveLength(4);
    expect(kinds(path).at(-1)).toBe('close');
    // El vértice de la esquina ya no existe: el trazo pasa por (5, 0) y (0, 5).
    const starts = flattenBez(path)[0].points;
    expect(starts.some(([x, y]) => x === 0 && y === 0)).toBe(false);
  });

  it('sin radio, el rectángulo son cuatro rectas cerradas', () => {
    expect(kinds(roundedRectBez(0, 0, 10, 10))).toEqual(['move_to', 'line_to', 'line_to', 'line_to', 'close']);
  });

  it('el radio se limita a la mitad del lado menor', () => {
    expect(bezBounds([roundedRectBez(0, 0, 40, 20, 999)])).toEqual([[0, 0], [40, 20]]);
    for (const s of roundedRectBez(0, 0, 40, 20, 999)) {
      if (s.type === 'cubic_to') for (const p of [...s.ctrl1, ...s.ctrl2, ...s.to]) expect(Number.isFinite(p)).toBe(true);
    }
  });

  it('elipse: cuatro cúbicas con la caja exacta', () => {
    const path = ellipseBez(50, 50, 30, 20);
    expect(kinds(path)).toEqual(['move_to', 'cubic_to', 'cubic_to', 'cubic_to', 'cubic_to', 'close']);
    const b = bezBounds([path])!;
    expect(b[0][0]).toBeCloseTo(20, 9);
    expect(b[1][0]).toBeCloseTo(80, 9);
    expect(b[0][1]).toBeCloseTo(30, 9);
    expect(b[1][1]).toBeCloseTo(70, 9);
  });

  it('el aplanado de una elipse respeta la tolerancia', () => {
    const pts = flattenBez(ellipseBez(0, 0, 25, 25), 0.01)[0].points;
    for (const [x, y] of pts) expect(Math.abs(Math.hypot(x, y) - 25)).toBeLessThan(0.02);
    expect(polylineLength(pts, true)).toBeCloseTo(2 * Math.PI * 25, 1);
  });

  it('Mayús: cuadrado y círculo, hacia el lado en que se arrastra', () => {
    const sq = dragBox([10, 10], [40, 25], { square: true, fromCenter: false });
    expect(sq).toEqual([[10, 10], [40, 40]]);
    const sqNeg = dragBox([10, 10], [-20, 15], { square: true, fromCenter: false });
    expect(sqNeg).toEqual([[-20, 10], [10, 40]]);
    const free = dragBox([10, 10], [40, 25], { square: false, fromCenter: false });
    expect(free).toEqual([[10, 10], [40, 25]]);
    const centered = dragBox([50, 50], [60, 55], { square: false, fromCenter: true });
    expect(centered).toEqual([[40, 45], [60, 55]]);
    const circle = boxShapePaths('ellipse', sq);
    const b = bezBounds(circle)!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(b[1][1] - b[0][1], 9);
  });

  it('boxShapePaths aplica el radio solo a rectángulos', () => {
    expect(boxShapePaths('rect', [[0, 0], [10, 10]])[0].filter((s) => s.type === 'line_to')).toHaveLength(3);
    expect(boxShapePaths('rect', [[0, 0], [10, 10]], 2)[0].some((s) => s.type === 'cubic_to')).toBe(true);
    expect(boxShapePaths('rect', [[0, 0], [10, 10]])[0].at(-1)).toEqual({ type: 'close' });
    expect(boxShapePaths('ellipse', [[0, 0], [10, 10]], 2)[0].some((s) => s.type === 'cubic_to')).toBe(true);
  });

  it('línea y polilínea: sin puntos repetidos, cerrada solo con 3 o más', () => {
    expect(dedupePoints([[0, 0], [0, 0], [5, 0], [5, 0], [5, 5]])).toEqual([[0, 0], [5, 0], [5, 5]]);
    expect(polylinePaths([[0, 0]], false)).toBeNull();
    expect(polylinePaths([[0, 0], [0, 0]], false)).toBeNull();
    expect(polylinePaths([[0, 0], [10, 0]], false)).toEqual([
      [{ type: 'move_to', to: [0, 0] }, { type: 'line_to', to: [10, 0] }],
    ]);
    expect(polylinePaths([[0, 0], [10, 0]], true)![0].at(-1)!.type).toBe('line_to'); // una línea no se cierra
    expect(polylinePaths([[0, 0], [10, 0], [10, 10]], true)![0].at(-1)!.type).toBe('close');
    expect(polylinePaths([[0, 0], [10, 0], [10, 10], [10, 10]], false)![0]).toHaveLength(3); // doble clic
  });

  it('Mayús fija ángulos de 45° al dibujar', () => {
    const p = constrainAngle([0, 0], [10, 1]);
    expect(p[0]).toBeCloseTo(Math.hypot(10, 1), 9);
    expect(p[1]).toBeCloseTo(0, 9);
    const d = constrainAngle([0, 0], [10, 9]);
    expect(d[0]).toBeCloseTo(d[1], 9);
    expect(constrainAngle([3, 3], [3, 3])).toEqual([3, 3]);
  });

  it('Mayús al mover deja solo el eje dominante', () => {
    expect(constrainDelta(10, 3)).toEqual([10, 0]);
    expect(constrainDelta(-2, 7)).toEqual([0, 7]);
  });

  it('el objeto nuevo es un vector visible y desbloqueado con transform identidad', () => {
    const o = newVectorObject('rect-1', '#ff0000', boxShapePaths('rect', [[0, 0], [10, 10]]));
    expect(o).toMatchObject({ id: 'rect-1', layer: '#ff0000', locked: false, hidden: false, group: null, transform: [1, 0, 0, 1, 0, 0] });
    expect(o.kind.type).toBe('vector');
  });
});

describe('capa donde se dibuja', () => {
  const lines = makeLayer('#ff0000', 'Rojo');
  const image = makeLayer('#808080', 'Foto', { mode: 'image' });
  it('usa la capa activa si existe y admite vectores', () => {
    expect(pickDrawLayer([lines, makeLayer('#0000ff', 'Azul')], '#0000ff')).toBe('#0000ff');
    expect(pickDrawLayer([lines, makeLayer('#0000ff', 'Azul')], '#0000FF')).toBe('#0000ff');
  });
  it('si la activa es de imagen, usa la primera que no lo sea', () => {
    expect(pickDrawLayer([image, lines], '#808080')).toBe('#ff0000');
  });
  it('sin activa, la primera capa', () => {
    expect(pickDrawLayer([image, lines], null)).toBe('#ff0000');
  });
  it('una activa que aún no existe en el documento se respeta (se creará)', () => {
    expect(pickDrawLayer([lines], '#00ff00')).toBe('#00ff00');
  });
  it('sin capas: un color libre de la paleta', () => {
    expect(pickDrawLayer([], null)).toMatch(/^#[0-9a-f]{6}$/i);
  });
});
