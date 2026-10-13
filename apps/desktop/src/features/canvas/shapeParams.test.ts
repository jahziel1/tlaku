import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { objectBounds } from '@/lib/geometry';
import { clampShape, editShape, effectiveShape, newShapeObject, registerShape, shapeOf, shapePaths } from './shapeParams';
import { newVectorObject } from './shapes';

const size = (o: DesignObject) => {
  const b = objectBounds(o)!;
  return [b[1][0] - b[0][0], b[1][1] - b[0][1]];
};

describe('formas paramétricas', () => {
  it('un polígono nuevo recuerda sus parámetros y está centrado en el punto dado', () => {
    const o = newShapeObject('p', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [50, 40]);
    expect(shapeOf(o)?.params).toMatchObject({ type: 'polygon', sides: 6 });
    const b = objectBounds(o)!;
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(50, 6);
    expect(size(o)[1]).toBeCloseTo(2 * 10 * Math.cos(0), 0); // hexágono con vértice arriba: alto = 2R
  });

  it('mover, escalar y copiar conservan los parámetros; cambiar el contorno los pierde', () => {
    const o = newShapeObject('p', '#ff0000', { type: 'star', points: 5, outer: 10, ratio: 0.5, rounding: 0 }, [0, 0]);
    expect(shapeOf({ ...o, transform: [2, 0, 0, 2, 5, 5] })).not.toBeNull();
    expect(shapeOf({ ...o, id: 'copia' })).not.toBeNull();
    expect(shapeOf({ ...o, kind: { type: 'vector', paths: shapePaths({ type: 'ellipse', w: 1, h: 1 }) } })).toBeNull();
    expect(shapeOf(newVectorObject('x', '#ff0000', []))).toBeNull();
  });

  it('los parámetros efectivos incluyen la escala del objeto', () => {
    const o = newShapeObject('p', '#ff0000', { type: 'polygon', sides: 5, radius: 10, rounding: 1 }, [0, 0]);
    const scaled: DesignObject = { ...o, transform: [2, 0, 0, 2, 0, 0] };
    expect(effectiveShape(scaled)).toEqual({ type: 'polygon', sides: 5, radius: 20, rounding: 2 });
    expect(effectiveShape({ ...o, transform: [1, 0, 0.5, 1, 0, 0] })).toBeNull(); // sesgada
  });

  it('editar un parámetro regenera la forma, hornea la escala y conserva centro y giro', () => {
    const o0 = newShapeObject('p', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [30, 20], Math.PI / 6);
    const o: DesignObject = { ...o0, transform: [o0.transform[0] * 2, o0.transform[1] * 2, o0.transform[2] * 2, o0.transform[3] * 2, 30, 20] };
    const edited = editShape(o, { sides: 8 })!;
    expect(edited).not.toBeNull();
    const eff = effectiveShape(edited)!;
    expect(eff).toMatchObject({ type: 'polygon', sides: 8 });
    expect((eff as { radius: number }).radius).toBeCloseTo(20, 9); // la escala quedó en el radio
    expect(edited.transform[0] ** 2 + edited.transform[1] ** 2).toBeCloseTo(1, 9); // ya solo giro
    expect(edited.transform[4]).toBeCloseTo(30, 9);
    expect(edited.transform[5]).toBeCloseTo(20, 9);
    expect(Math.atan2(edited.transform[1], edited.transform[0])).toBeCloseTo(Math.PI / 6, 9);
    expect(edited.kind).not.toBe(o.kind);
  });

  it('un rectángulo dibujado en coordenadas de documento se edita alrededor de su centro', () => {
    const rect = newVectorObject('r', '#ff0000', shapePaths({ type: 'rect', w: 40, h: 20, radius: 0 }, [100, 50]));
    registerShape(rect, { params: { type: 'rect', w: 40, h: 20, radius: 0 }, center: [100, 50] });
    const edited = editShape(rect, { w: 60, radius: 5 })!;
    const b = objectBounds(edited)!;
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(100, 6);
    expect((b[0][1] + b[1][1]) / 2).toBeCloseTo(50, 6);
    expect(size(edited)[0]).toBeCloseTo(60, 6);
    expect(size(edited)[1]).toBeCloseTo(20, 6);
  });

  it('lo que no es paramétrico no se edita', () => {
    expect(editShape(newVectorObject('x', '#ff0000', []), { w: 3 } as never)).toBeNull();
  });

  it('los parámetros se limitan a valores válidos', () => {
    expect(clampShape({ type: 'polygon', sides: 1, radius: -5, rounding: -1 })).toEqual({ type: 'polygon', sides: 3, radius: 0.01, rounding: 0 });
    expect(clampShape({ type: 'polygon', sides: 500, radius: 1, rounding: 0 })).toMatchObject({ sides: 100 });
    expect(clampShape({ type: 'star', points: 5, outer: 10, ratio: 2, rounding: 0 })).toMatchObject({ ratio: 0.95 });
    expect(clampShape({ type: 'rect', w: 10, h: 4, radius: 99 })).toMatchObject({ radius: 2 });
  });

  it('la estrella con redondeo sigue siendo un solo contorno cerrado', () => {
    const paths = shapePaths({ type: 'star', points: 5, outer: 10, ratio: 0.5, rounding: 1 });
    expect(paths).toHaveLength(1);
    expect(paths[0]!.at(-1)).toEqual({ type: 'close' });
    expect(paths[0]!.some((s) => s.type === 'cubic_to')).toBe(true);
  });
});
