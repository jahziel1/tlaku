import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { applyAffine, objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { ctxFor, lineObj, rectObj } from '@/features/canvas/testUtil';
import { newVectorObject } from '@/features/canvas/shapes';
import { equalizeDoc } from './equalize';
import { reorderObjects } from './order';
import {
  circleDiameter, isOpenShape, selectableObjects, selectCircles, selectContained, selectInLayer, selectOpen, selectOpenInFill, selectSmaller,
} from './special';
import { flipMatrix, mirrorCopiesDoc, rotateSelectionMatrix, straightLineEnds } from './transformOps';
import { selectionUnits } from './units';

const docOf = (objects: DesignObject[], layers = [makeLayer('#ff0000', 'Rojo')]): Document => ({ version: 2, objects, layers, device_id: null });
const ids = (list: readonly DesignObject[]) => list.map((o) => o.id);

describe('orden de dibujo', () => {
  const abcd = [rectObj('a', 0, 0, 1, 1), rectObj('b', 0, 0, 1, 1), rectObj('c', 0, 0, 1, 1), rectObj('d', 0, 0, 1, 1)];

  it('al frente y al fondo conservan el orden relativo de lo elegido', () => {
    expect(ids(reorderObjects(abcd, new Set(['a', 'c']), 'front'))).toEqual(['b', 'd', 'a', 'c']);
    expect(ids(reorderObjects(abcd, new Set(['b', 'd']), 'back'))).toEqual(['b', 'd', 'a', 'c']);
  });

  it('subir y bajar un nivel mueven el bloque un solo lugar', () => {
    expect(ids(reorderObjects(abcd, new Set(['b']), 'forward'))).toEqual(['a', 'c', 'b', 'd']);
    expect(ids(reorderObjects(abcd, new Set(['b', 'c']), 'forward'))).toEqual(['a', 'd', 'b', 'c']);
    expect(ids(reorderObjects(abcd, new Set(['c']), 'backward'))).toEqual(['a', 'c', 'b', 'd']);
    expect(ids(reorderObjects(abcd, new Set(['b', 'c']), 'backward'))).toEqual(['b', 'c', 'a', 'd']);
  });

  it('si ya están en el extremo, devuelve la misma lista', () => {
    expect(reorderObjects(abcd, new Set(['d']), 'front')).toBe(abcd);
    expect(reorderObjects(abcd, new Set(['d']), 'forward')).toBe(abcd);
    expect(reorderObjects(abcd, new Set(['a']), 'back')).toBe(abcd);
    expect(reorderObjects(abcd, new Set(['a']), 'backward')).toBe(abcd);
    expect(reorderObjects(abcd, new Set(), 'front')).toBe(abcd);
    expect(reorderObjects(abcd, new Set(ids(abcd)), 'front')).toBe(abcd);
  });
});

describe('voltear y girar la selección', () => {
  it('voltear horizontal usa el centro de la caja de toda la selección', () => {
    const objs = [rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 40, 10)];
    const m = flipMatrix(objs, 'h')!;
    expect(applyAffine(m, [0, 5])).toEqual([40, 5]); // centro x = 20
    expect(applyAffine(m, [40, 5])).toEqual([0, 5]);
  });

  it('los bloqueados no cuentan para el centro', () => {
    const objs = [rectObj('a', 0, 0, 10, 10), rectObj('b', 100, 0, 200, 10, { locked: true })];
    expect(applyAffine(flipMatrix(objs, 'h')!, [0, 0])).toEqual([10, 0]);
    expect(flipMatrix([rectObj('l', 0, 0, 1, 1, { locked: true })], 'v')).toBeNull();
  });

  it('girar 90° alrededor del centro deja el centro quieto', () => {
    const m = rotateSelectionMatrix([rectObj('a', 0, 0, 10, 20)], 90)!;
    const [x, y] = applyAffine(m, [5, 10]);
    expect(x).toBeCloseTo(5, 9);
    expect(y).toBeCloseTo(10, 9);
    const b = objectBounds({ ...rectObj('a', 0, 0, 10, 20), transform: m })!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(20, 9);
  });
});

describe('espejo sobre una línea', () => {
  it('detecta una recta de dos puntos y rechaza lo demás', () => {
    expect(straightLineEnds(lineObj('l', [0, 0], [10, 0]))).toEqual([[0, 0], [10, 0]]);
    expect(straightLineEnds({ ...lineObj('l', [0, 0], [10, 0]), transform: [1, 0, 0, 1, 5, 5] })).toEqual([[5, 5], [15, 5]]);
    expect(straightLineEnds(rectObj('r', 0, 0, 1, 1))).toBeNull();
    expect(straightLineEnds(newVectorObject('p', '#ff0000', [polylineBez([[0, 0], [1, 0], [2, 1]], false)]))).toBeNull();
    expect(straightLineEnds(lineObj('z', [3, 3], [3, 3]))).toBeNull();
  });

  it('crea copias reflejadas con ids nuevos y deja los originales intactos', () => {
    const d = docOf([rectObj('a', 10, 0, 20, 10)]);
    const { doc, created } = mirrorCopiesDoc(d, ['a'], [-1, 0, 0, 1, 0, 0]); // sobre el eje Y
    expect(doc.objects).toHaveLength(2);
    expect(doc.objects[0]).toBe(d.objects[0]);
    expect(created).toHaveLength(1);
    expect(created[0]).not.toBe('a');
    const b = objectBounds(doc.objects[1]!)!;
    expect(b[0][0]).toBeCloseTo(-20, 9);
    expect(b[1][0]).toBeCloseTo(-10, 9);
  });

  it('ignora bloqueados y no hace nada sin objetos', () => {
    const d = docOf([rectObj('a', 0, 0, 1, 1, { locked: true })]);
    expect(mirrorCopiesDoc(d, ['a'], [-1, 0, 0, 1, 0, 0]).doc).toBe(d);
  });
});

describe('igualar ancho y alto', () => {
  const d = docOf([rectObj('a', 0, 0, 10, 20), rectObj('b', 50, 0, 80, 10), rectObj('c', 100, 0, 105, 5)]);
  const units = selectionUnits(d, ['a', 'b', 'c']);

  it('iguala el ancho al de la primera, alrededor del centro de cada una', () => {
    const out = equalizeDoc(d, units, 'width', false);
    const b = objectBounds(out.objects[1]!)!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(10, 9);
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(65, 9); // mismo centro
    expect(b[1][1] - b[0][1]).toBeCloseTo(10, 9); // el alto no cambia
    expect(out.objects[0]).toBe(d.objects[0]);
  });

  it('con proporción escala los dos lados igual', () => {
    const out = equalizeDoc(d, units, 'height', true);
    const b = objectBounds(out.objects[1]!)!;
    expect(b[1][1] - b[0][1]).toBeCloseTo(20, 9);
    expect(b[1][0] - b[0][0]).toBeCloseTo(60, 9);
  });

  it('necesita dos unidades y no toca a las bloqueadas', () => {
    expect(equalizeDoc(d, units.slice(0, 1), 'width', false)).toBe(d);
    const locked = docOf([rectObj('a', 0, 0, 10, 20), rectObj('b', 50, 0, 80, 10, { locked: true })]);
    expect(equalizeDoc(locked, selectionUnits(locked, ['a', 'b']), 'width', false)).toBe(locked);
  });
});

describe('selecciones especiales', () => {
  const circle = (id: string, r: number, x = 0, layer = '#ff0000'): DesignObject => ({
    ...newVectorObject(id, layer, [ellipseBez(x, 0, r, r)]),
  });
  const layers = [makeLayer('#ff0000', 'Rojo'), makeLayer('#00ff00', 'Verde', { mode: 'fill' })];

  it('formas abiertas, y abiertas en capas de relleno', () => {
    const objs = [rectObj('closed', 0, 0, 5, 5), lineObj('open', [0, 0], [5, 5]), lineObj('openFill', [0, 0], [5, 5], { layer: '#00ff00' })];
    expect(isOpenShape(objs[0]!)).toBe(false);
    expect(isOpenShape(objs[1]!)).toBe(true);
    expect(selectOpen(objs)).toEqual(['open', 'openFill']);
    expect(selectOpenInFill(objs, layers)).toEqual(['openFill']);
  });

  it('todo en una capa', () => {
    const objs = [rectObj('a', 0, 0, 1, 1), rectObj('b', 0, 0, 1, 1, { layer: '#00ff00' })];
    expect(selectInLayer(objs, '#00FF00')).toEqual(['b']);
  });

  it('solo se consideran objetos seleccionables', () => {
    const objs = [rectObj('a', 0, 0, 1, 1), rectObj('b', 0, 0, 1, 1, { locked: true }), rectObj('c', 0, 0, 1, 1, { hidden: true })];
    expect(ids(selectableObjects(objs, layers))).toEqual(['a']);
  });

  it('formas contenidas: la contenedora se queda y se ignoran las agrupadas', () => {
    const big = rectObj('big', 0, 0, 100, 100);
    const inside = rectObj('in', 10, 10, 20, 20);
    const grouped = rectObj('g', 30, 30, 40, 40, { group: 'x' });
    const outside = rectObj('out', 90, 90, 120, 120);
    expect(selectContained([big, inside, grouped, outside], [big]).sort()).toEqual(['big', 'in']);
  });

  it('formas más chicas que un tamaño dado', () => {
    const a = rectObj('a', 0, 0, 10, 10);
    const b = rectObj('b', 0, 0, 5, 5);
    const c = rectObj('c', 0, 0, 20, 20);
    expect(selectSmaller([a, b, c], 100, new Set(['a']))).toEqual(['b']);
  });

  it('círculos por diámetro exacto o rango', () => {
    const objs = [circle('m3', 1.5), circle('m4', 2), circle('m5', 2.5), rectObj('sq', 0, 0, 4, 4), lineObj('l', [0, 0], [4, 0])];
    expect(circleDiameter(objs[0]!)).toBeCloseTo(3, 9);
    expect(circleDiameter(objs[3]!)).toBeNull(); // un cuadrado no es círculo
    expect(circleDiameter(objs[4]!)).toBeNull();
    expect(selectCircles(objs, 4, 4)).toEqual(['m4']);
    expect(selectCircles(objs, 3, 5)).toEqual(['m3', 'm4', 'm5']);
    expect(selectCircles(objs, 5, 3)).toEqual(['m3', 'm4', 'm5']); // rango al revés
    expect(selectCircles(objs, 10, 20)).toEqual([]);
    // Un círculo escalado por la matriz del objeto también cuenta.
    expect(circleDiameter({ ...circle('s', 1), transform: [3, 0, 0, 3, 0, 0] })).toBeCloseTo(6, 9);
    // Una elipse no.
    expect(circleDiameter({ ...circle('e', 1), transform: [3, 0, 0, 1, 0, 0] })).toBeNull();
  });

  it('el contexto de selección usa la capa visible', () => {
    expect(ctxFor([], layers).layer('#00ff00')?.mode).toBe('fill');
  });
});
