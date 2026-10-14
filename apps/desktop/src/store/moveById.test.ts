import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { bezPointCount, polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { translateAffine } from '@/lib/geometry';
import { createAppStore } from './index';
import { entryCost, translateById } from './document';
import { objectIndex, objectsByIds, shareIndex } from './objectIndex';

function vector(id: string, x: number, patch: Partial<DesignObject> = {}): DesignObject {
  return {
    id,
    layer: '#ff0000',
    transform: [1, 0, 0, 1, x, 0],
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [1, 0], [1, 1]], true)] },
    locked: false,
    hidden: false,
    group: null,
    ...patch,
  };
}

const docOf = (objects: DesignObject[]): Document => ({ version: 2, objects, layers: [makeLayer('#ff0000', 'Rojo')], device_id: null });

/** La implementación de antes (recorrer todo con `map`): el resultado debe ser idéntico. */
function reference(doc: Document, ids: string[], dx: number, dy: number): Document {
  const set = new Set(ids);
  const r6 = (n: number) => Math.round(n * 1e6) / 1e6;
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      if (!set.has(o.id) || o.locked) return o;
      const t = translateAffine(o.transform, dx, dy);
      return { ...o, transform: [t[0], t[1], t[2], t[3], r6(t[4]), r6(t[5])] };
    }),
  };
}

describe('índice de objetos por id', () => {
  it('da la posición de cada id y avisa si hay ids repetidos', () => {
    const objects = [vector('a', 0), vector('b', 1), vector('c', 2)];
    const idx = objectIndex(objects);
    expect(idx.unique).toBe(true);
    expect([...idx.at]).toEqual([['a', 0], ['b', 1], ['c', 2]]);
    expect(objectIndex(objects)).toBe(idx); // se calcula una vez por arreglo
    expect(objectIndex([vector('a', 0), vector('a', 1)]).unique).toBe(false);
  });

  it('shareIndex pasa el índice a un arreglo con los mismos ids en el mismo orden', () => {
    const a = [vector('a', 0), vector('b', 1)];
    const idx = objectIndex(a);
    const b = [a[0]!, { ...a[1]!, transform: [1, 0, 0, 1, 9, 0] as DesignObject['transform'] }];
    shareIndex(a, b);
    expect(objectIndex(b)).toBe(idx);
    const c = [a[0]!];
    shareIndex(a, c); // otra longitud: no se comparte
    expect(objectIndex(c)).not.toBe(idx);
  });

  it('objectsByIds devuelve en orden del documento, sin repetidos ni ajenos, igual con pocos y con muchos ids', () => {
    const objects = Array.from({ length: 100 }, (_, i) => vector(`o${i}`, i));
    expect(objectsByIds(objects, ['o50', 'zz', 'o7', 'o50']).map((o) => o.id)).toEqual(['o7', 'o50']);
    const many = objects.map((o) => o.id).reverse();
    expect(objectsByIds(objects, many)).toEqual(objects);
    const ids = ['o3'];
    expect(objectsByIds(objects, ids)).toBe(objectsByIds(objects, ids)); // se comparte entre quien lo pide
  });
});

describe('translateById', () => {
  it('solo toca los objetos pedidos: los demás conservan su referencia y la geometría se comparte', () => {
    const objects = Array.from({ length: 50 }, (_, i) => vector(`o${i}`, i));
    const doc = docOf(objects);
    const next = translateById(doc, ['o3', 'o10'], 2.5, -1);
    expect(next.objects).toHaveLength(50);
    next.objects.forEach((o, i) => {
      if (i === 3 || i === 10) {
        expect(o).not.toBe(objects[i]);
        expect(o.kind).toBe(objects[i]!.kind);
        expect(o.transform).toEqual([1, 0, 0, 1, i + 2.5, -1]);
      } else expect(o).toBe(objects[i]);
    });
    expect(next.layers).toBe(doc.layers);
  });

  it('no mueve los bloqueados, ignora ids ajenos y repetidos, y sin cambios devuelve el mismo documento', () => {
    const doc = docOf([vector('a', 0), vector('b', 1, { locked: true }), vector('c', 2)]);
    const next = translateById(doc, ['a', 'b', 'a', 'nada'], 1, 1);
    expect(next.objects[0]!.transform).toEqual([1, 0, 0, 1, 1, 1]); // 'a' una sola vez
    expect(next.objects[1]).toBe(doc.objects[1]);
    expect(next.objects[2]).toBe(doc.objects[2]);
    expect(translateById(doc, ['b'], 1, 1)).toBe(doc);
    expect(translateById(doc, [], 1, 1)).toBe(doc);
  });

  it('da lo mismo que recorrer todo con map (incluido el redondeo) y el arreglo hereda el índice', () => {
    let seed = 7;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) % 4294967296) / 4294967296;
    const objects = Array.from({ length: 300 }, (_, i) => vector(`o${i}`, rnd() * 100, { locked: rnd() < 0.1 }));
    const doc = docOf(objects);
    for (let round = 0; round < 20; round++) {
      const ids = objects.filter(() => rnd() < 0.3).map((o) => o.id);
      const dx = Math.round(rnd() * 1e4) / 1e3 - 5;
      const dy = rnd() * 3.3333333;
      expect(translateById(doc, ids, dx, dy)).toEqual(reference(doc, ids, dx, dy));
    }
    const moved = translateById(doc, ['o1', 'o2'], 1, 0);
    expect(objectIndex(moved.objects)).toBe(objectIndex(doc.objects)); // sin recorrer de nuevo
  });

  it('con ids repetidos en el documento cae al recorrido completo y mueve todos los que coinciden', () => {
    const doc = docOf([vector('a', 0), vector('a', 5), vector('b', 9)]);
    const next = translateById(doc, ['a'], 1, 0);
    expect(next.objects.map((o) => o.transform[4])).toEqual([1, 6, 9]);
  });

  it('con 100 000 objetos mover pocos no recorre el documento con map (mucho más rápido que la referencia)', () => {
    const objects = Array.from({ length: 100_000 }, (_, i) => vector(`o${i}`, i % 400));
    const doc = docOf(objects);
    objectIndex(doc.objects); // el índice ya existe en una sesión real (lo hereda cada arreglo nuevo)
    const ids = ['o99990', 'o99991', 'o5'];
    const time = (fn: () => void) => {
      let best = Infinity;
      for (let k = 0; k < 5; k++) {
        const t0 = performance.now();
        fn();
        best = Math.min(best, performance.now() - t0);
      }
      return best;
    };
    const fast = time(() => translateById(doc, ids, 1, 1));
    const slow = time(() => reference(doc, ids, 1, 1));
    expect(fast).toBeLessThan(slow);
    expect(fast).toBeLessThan(25);
  });
});

describe('mover con el store', () => {
  let store: ReturnType<typeof createAppStore>;
  const s = () => store.getState();
  beforeEach(() => {
    store = createAppStore();
    s().applyImport({ objects: Array.from({ length: 20 }, (_, i) => vector(`o${i}`, i)), new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  });

  it('confirmar un movimiento es un solo paso y deshacer devuelve exactamente el documento anterior', () => {
    const before = s().doc;
    const steps = s().past.length;
    s().moveObjects(['o1', 'o2', 'o3'], 4, 2, 'drag');
    expect(s().past.length).toBe(steps + 1);
    expect(s().doc.objects[1]!.transform).toEqual([1, 0, 0, 1, 5, 2]);
    expect(s().doc.objects[0]).toBe(before.objects[0]);
    s().moveObjects(['o1', 'o2', 'o3'], 1, 0, 'drag'); // misma clave y poco tiempo: se funde
    expect(s().past.length).toBe(steps + 1);
    s().undo();
    expect(s().doc).toBe(before);
  });

  it('el costo del paso no cuenta geometría (los objetos movidos conservan su kind)', () => {
    s().moveObjects(['o1'], 1, 1, 'k');
    const entry = s().past[s().past.length - 1]!;
    expect(entry.cost).toBe(20 / 8);
  });
});

describe('entryCost con la misma cantidad de objetos', () => {
  function reference(before: Document, after: Document): number {
    const afterSet = new Set(after.objects);
    const beforeSet = new Set(before.objects);
    const keptKinds = new Set<object>();
    for (const o of after.objects) if (!beforeSet.has(o)) keptKinds.add(o.kind);
    let points = 0;
    for (const o of before.objects) {
      if (afterSet.has(o) || keptKinds.has(o.kind)) continue;
      points += o.kind.type === 'vector' ? o.kind.paths.reduce((n, p) => n + bezPointCount(p), 0) : 0;
    }
    return points + before.objects.length / 8;
  }

  it('coincide con el cálculo por conjuntos completos en mover, reemplazar geometría y reordenar', () => {
    const objects = Array.from({ length: 40 }, (_, i) => vector(`o${i}`, i));
    const before = docOf(objects);
    const moved = translateById(before, ['o1', 'o2'], 1, 0);
    expect(entryCost(before, moved)).toBe(reference(before, moved));
    const replaced = docOf(objects.map((o, i) => (i % 5 === 0 ? { ...o, kind: { type: 'vector' as const, paths: [polylineBez([[0, 0], [2, 2]], false)] } } : o)));
    expect(entryCost(before, replaced)).toBe(reference(before, replaced));
    expect(entryCost(before, replaced)).toBeGreaterThan(before.objects.length / 8);
    const reordered = docOf([...objects].reverse());
    expect(entryCost(before, reordered)).toBe(reference(before, reordered));
    expect(entryCost(before, before)).toBe(40 / 8);
  });
});
