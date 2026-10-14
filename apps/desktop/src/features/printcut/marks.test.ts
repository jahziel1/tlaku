import { describe, expect, it } from 'vitest';
import type { Document } from '@/api/types';
import { GUIDE_COLOR } from '@/lib/palette';
import { rectObject } from '@/features/nest/testUtils';
import { buildPairs, computeRegistration, isGoodFit, nextMissing, resolveMode } from './flow';
import { cornerCenters, designBox, marksFromSelection, marksMoved, registrationMarkObjects } from './marks';

const doc = (objects: Document['objects']): Pick<Document, 'objects'> => ({ objects });

describe('marcas de registro', () => {
  const design = doc([rectObject('a', 100, 60, [20, 30])]);
  const box = designBox(design)!;

  it('la caja del diseño no cuenta las guías ni lo oculto', () => {
    expect(box).toEqual([[20, 30], [120, 90]]);
    const withGuide = doc([...design.objects, rectObject('g', 500, 500, [0, 0], { layer: GUIDE_COLOR }), rectObject('h', 500, 500, [0, 0], { hidden: true })]);
    expect(designBox(withGuide)).toEqual(box);
    expect(designBox(doc([]))).toBeNull();
  });

  it('pone 2 marcas en diagonal, 3 en L y 4 en las esquinas, fuera del diseño', () => {
    expect(cornerCenters(box, 2, 8)).toEqual([[12, 22], [128, 98]]);
    expect(cornerCenters(box, 3, 8)).toEqual([[12, 22], [128, 22], [12, 98]]);
    expect(cornerCenters(box, 4, 8)).toHaveLength(4);
  });

  it('crea un objeto por marca en la capa de guías, con ids únicos y el centro correcto', () => {
    const taken = doc([...design.objects, rectObject('printcut-mark-1', 1, 1)]);
    const { objects, marks } = registrationMarkObjects(taken, box, 'cross', 3);
    expect(objects.map((o) => o.id)).toEqual(['printcut-mark-2', 'printcut-mark-3', 'printcut-mark-4']);
    expect(objects.every((o) => o.layer === GUIDE_COLOR)).toBe(true);
    expect(marks.map((m) => m.point)).toEqual(cornerCenters(box, 3));
    // Cruz: dos trazos; círculo: anillo y cruz pequeña.
    const circle = registrationMarkObjects(taken, box, 'circle', 2).objects[0]!;
    expect(circle.kind.type === 'vector' && circle.kind.paths.length).toBe(3);
    const cross = objects[0]!;
    expect(cross.kind.type === 'vector' && cross.kind.paths.length).toBe(2);
  });

  it('toma como marcas los objetos seleccionados, con su centro', () => {
    const d = doc([rectObject('m1', 10, 10, [0, 0]), rectObject('m2', 10, 10, [100, 50]), rectObject('x', 5, 5, [40, 40])]);
    const r = marksFromSelection(d, ['m1', 'm2']);
    expect(r).toEqual({ marks: [{ ids: ['m1'], point: [5, 5] }, { ids: ['m2'], point: [105, 55] }] });
  });

  it('exige entre 2 y 4 marcas separadas', () => {
    const d = doc([rectObject('m1', 2, 2, [0, 0]), rectObject('m2', 2, 2, [5, 0]), rectObject('m3', 2, 2, [200, 0]), rectObject('m4', 2, 2, [0, 200]), rectObject('m5', 2, 2, [200, 200]), rectObject('m6', 2, 2, [100, 100])]);
    expect(marksFromSelection(d, ['m1'])).toEqual({ error: 'count' });
    expect(marksFromSelection(d, ['m1', 'm2', 'm3', 'm4', 'm5'])).toEqual({ error: 'count' });
    expect(marksFromSelection(d, ['m1', 'm2'])).toEqual({ error: 'close' });
    expect('marks' in marksFromSelection(d, ['m1', 'm3', 'm4', 'm5'])).toBe(true);
  });

  it('detecta una marca movida o borrada', () => {
    const d = doc([rectObject('m1', 10, 10, [0, 0]), rectObject('m2', 10, 10, [100, 50])]);
    const marks = marksFromSelection(d, ['m1', 'm2']);
    if (!('marks' in marks)) throw new Error('sin marcas');
    expect(marksMoved(d, marks.marks)).toBe(false);
    expect(marksMoved(doc([rectObject('m1', 10, 10, [0, 0]), rectObject('m2', 10, 10, [101, 50])]), marks.marks)).toBe(true);
    expect(marksMoved(doc([rectObject('m1', 10, 10, [0, 0])]), marks.marks)).toBe(true);
  });
});

describe('flujo del asistente', () => {
  const marks = [
    { ids: ['a'], point: [0, 0] as [number, number] },
    { ids: ['b'], point: [100, 0] as [number, number] },
    { ids: ['c'], point: [0, 80] as [number, number] },
  ];

  it('resuelve el modo según las marcas y cae a similitud si no hay forma de resolver la afín', () => {
    expect(resolveMode('auto', [[0, 0], [10, 0]])).toBe('similarity');
    expect(resolveMode('auto', marks.map((m) => m.point))).toBe('affine');
    expect(resolveMode('affine', [[0, 0], [10, 0]])).toBe('similarity');
    expect(resolveMode('auto', [[0, 0], [50, 0], [100, 0]])).toBe('similarity');
    expect(resolveMode('rigid', marks.map((m) => m.point))).toBe('rigid');
  });

  it('busca la siguiente marca sin medir dando la vuelta', () => {
    expect(nextMissing([[1, 1], null, null], 0)).toBe(1);
    expect(nextMissing([null, [1, 1], null], 2)).toBe(0);
    expect(nextMissing([[1, 1], [2, 2]], 1)).toBe(1);
  });

  it('no calcula hasta que estén todas medidas', () => {
    expect(buildPairs(marks, [[1, 1], null, [2, 2]])).toBeNull();
    expect(computeRegistration(marks, [[1, 1]], 'auto')).toBeNull();
    const r = computeRegistration(marks, [[3, 2], [103, 2], [3, 82]], 'auto');
    expect(r?.ok).toBe(true);
    if (r?.ok) {
      expect(r.fit.transform[4]).toBeCloseTo(3, 9);
      expect(isGoodFit(r.fit.residualMax)).toBe(true);
    }
    expect(isGoodFit(0.5)).toBe(false);
  });
});
