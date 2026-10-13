import { describe, expect, it } from 'vitest';
import { bezBounds } from '@/lib/bezpath';
import { cornerNode, dedupeNodes, handleLines, hasCurves, penDraft, penPath, smoothNode } from './pen';

describe('dibujo de curvas', () => {
  it('con solo clics sale una polilínea de rectas', () => {
    const path = penPath([cornerNode([0, 0]), cornerNode([10, 0]), cornerNode([10, 10])], false)!;
    expect(path.map((s) => s.type)).toEqual(['move_to', 'line_to', 'line_to']);
    expect(hasCurves(path)).toBe(false);
  });

  it('arrastrar al poner un nodo saca manijas simétricas y el tramo siguiente es curvo', () => {
    const a = smoothNode([0, 0], [5, 5]);
    expect(a.hout).toEqual([5, 5]);
    expect(a.hin).toEqual([-5, -5]); // reflejo
    const path = penPath([a, cornerNode([10, 0])], false)!;
    expect(path[1]).toEqual({ type: 'cubic_to', ctrl1: [5, 5], ctrl2: [10, 0], to: [10, 0] });
    expect(hasCurves(path)).toBe(true);
  });

  it('el nodo de llegada con manija entrante curva el tramo por su lado', () => {
    const b = smoothNode([10, 0], [15, -5]); // hin = (5, 5)
    const path = penPath([cornerNode([0, 0]), b], false)!;
    expect(path[1]).toEqual({ type: 'cubic_to', ctrl1: [0, 0], ctrl2: [5, 5], to: [10, 0] });
  });

  it('sin distancia entre puntero y nodo no hay manijas', () => {
    expect(smoothNode([1, 1], [1, 1])).toEqual(cornerNode([1, 1]));
  });

  it('cerrar une el último con el primero; con manijas lo hace con una curva', () => {
    const square = penPath([cornerNode([0, 0]), cornerNode([4, 0]), cornerNode([4, 4])], true)!;
    expect(square.at(-1)).toEqual({ type: 'close' });
    expect(square.map((s) => s.type)).toEqual(['move_to', 'line_to', 'line_to', 'close']);
    const curved = penPath([smoothNode([0, 0], [-3, 3]), cornerNode([4, 0]), cornerNode([4, 4])], true)!;
    expect(curved.map((s) => s.type)).toEqual(['move_to', 'cubic_to', 'line_to', 'cubic_to', 'close']);
  });

  it('dos nodos no se cierran (no hay figura)', () => {
    expect(penPath([cornerNode([0, 0]), cornerNode([4, 0])], true)!.some((s) => s.type === 'close')).toBe(false);
    expect(penPath([cornerNode([0, 0])], false)).toBeNull();
    expect(penPath([], false)).toBeNull();
  });

  it('los nodos repetidos (doble clic) se funden', () => {
    expect(dedupeNodes([cornerNode([0, 0]), cornerNode([0, 0]), cornerNode([1, 0])])).toHaveLength(2);
    expect(penPath([cornerNode([0, 0]), cornerNode([1, 0]), cornerNode([1, 0])], false)!).toHaveLength(2);
  });

  it('la vista previa agrega el tramo elástico hasta el cursor, curvo si el último nodo tiene manija', () => {
    const straight = penDraft([cornerNode([0, 0])], [5, 5]);
    expect(straight.map((s) => s.type)).toEqual(['move_to', 'line_to']);
    const curvy = penDraft([smoothNode([0, 0], [2, 4])], [10, 0]);
    expect(curvy[1]).toMatchObject({ type: 'cubic_to', ctrl1: [2, 4], to: [10, 0] });
    expect(penDraft([], [1, 1])).toEqual([]);
    expect(penDraft([cornerNode([0, 0]), cornerNode([3, 0])], null)).toHaveLength(2);
  });

  it('muestra las dos manijas del nodo', () => {
    expect(handleLines(smoothNode([0, 0], [1, 1]))).toEqual([[[0, 0], [-1, -1]], [[0, 0], [1, 1]]]);
    expect(handleLines(undefined)).toEqual([]);
    expect(handleLines(cornerNode([0, 0]))).toEqual([]);
  });

  it('la curva queda dentro de la caja de sus puntos de control', () => {
    const path = penPath([smoothNode([0, 0], [0, 10]), cornerNode([10, 0])], false)!;
    const box = bezBounds([path])!;
    expect(box[1][1]).toBeGreaterThan(0);
    expect(box[1][1]).toBeLessThanOrEqual(10);
  });
});
