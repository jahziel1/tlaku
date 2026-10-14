import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { boxesIntersect, getGeom, type Box } from './geom';
import { HIT_PX, isFilled, isSelectable, hitObject, pickObject, selectByBox } from './hit';
import { diffObjects } from './objectDiff';
import { INDEX_MIN_OBJECTS, SpatialIndex, resetSpatialIndex, spatialIndexFor } from './spatialIndex';
import { newVectorObject, rectPoints } from './shapes';
import { ctxFor, lineObj, rectObj } from './testUtil';

beforeEach(() => resetSpatialIndex());

/** Generador determinista. */
function rng(seed: number) {
  let s = seed;
  return () => (s = (s * 1664525 + 1013904223) % 4294967296) / 4294967296;
}

/** `n` objetos repartidos por una cama de 400 mm: cuadrados, líneas y algunos grandes. */
function scatter(n: number, seed = 1): DesignObject[] {
  const r = rng(seed);
  return Array.from({ length: n }, (_, i) => {
    const x = r() * 380;
    const y = r() * 380;
    if (i % 50 === 0) return rectObj(`big${i}`, x, y, x + 120 + r() * 100, y + 90 + r() * 100); // grandes
    if (i % 3 === 0) return lineObj(`l${i}`, [x, y], [x + r() * 6, y + r() * 6]);
    return rectObj(`r${i}`, x, y, x + 1 + r() * 4, y + 1 + r() * 4);
  });
}

/** El recorrido lineal de antes. */
function linearPick(objects: DesignObject[], p: [number, number], tol: number, layers = [makeLayer('#ff0000', 'Rojo')]): DesignObject | null {
  const ctx = ctxFor(objects, layers);
  for (let i = objects.length - 1; i >= 0; i--) {
    const obj = objects[i]!;
    const layer = ctx.layer(obj.layer);
    if (!isSelectable(obj, layer)) continue;
    if (hitObject(getGeom(obj), p[0], p[1], tol, isFilled(obj, layer))) return obj;
  }
  return null;
}

describe('rejilla de cajas', () => {
  it('atPoint devuelve de arriba hacia abajo exactamente los objetos cuya caja ensanchada contiene el punto', () => {
    const objects = scatter(2000);
    const idx = new SpatialIndex(objects);
    const r = rng(9);
    for (let k = 0; k < 200; k++) {
      const p: [number, number] = [r() * 400, r() * 400];
      const tol = r() * 5;
      const got = idx.atPoint(p[0], p[1], tol);
      const want: number[] = [];
      for (let i = objects.length - 1; i >= 0; i--) {
        const b = getGeom(objects[i]!).box!;
        if (b[0] - tol <= p[0] && p[0] <= b[2] + tol && b[1] - tol <= p[1] && p[1] <= b[3] + tol) want.push(i);
      }
      expect(got).toEqual(want);
    }
  });

  it('inBox de menor a mayor coincide con el recorrido completo, también con cajas enormes (recorre todo)', () => {
    const objects = scatter(1500, 3);
    const idx = new SpatialIndex(objects);
    for (const r of [[10, 10, 30, 30], [0, 0, 400, 400], [-50, -50, 5, 5], [200, 200, 201, 201], [0, 0, 1e6, 1e6]] as Box[]) {
      const want = objects.map((o, i) => [o, i] as const).filter(([o]) => boxesIntersect(getGeom(o).box!, r)).map(([, i]) => i);
      expect(idx.inBox(r)).toEqual(want);
      expect(idx.inBox(r, true)).toEqual([...want].reverse());
    }
  });

  it('los objetos del tamaño de la cama van a la lista de grandes y siempre se encuentran', () => {
    const objects = [...scatter(300), rectObj('cama', 0, 0, 400, 400)];
    const idx = new SpatialIndex(objects);
    expect(idx.bigCount).toBeGreaterThan(0);
    expect(idx.atPoint(399, 1, 0)[0]).toBe(300); // el último documento es el de más arriba
  });

  it('un trayecto con millones de coordenadas fuera de rango o sin geometría no rompe nada', () => {
    const vacio = newVectorObject('v', '#ff0000', []);
    const lejos = rectObj('lejos', 1e9, 1e9, 1e9 + 5, 1e9 + 5);
    const idx = new SpatialIndex([vacio, lejos, rectObj('a', 0, 0, 5, 5)]);
    expect(idx.atPoint(2, 2, 0)).toEqual([2]);
    expect(idx.atPoint(1e9 + 1, 1e9 + 1, 0)).toEqual([1]);
  });
});

describe('actualización incremental', () => {
  it('tras mover objetos, añadir al final o deshacer, el índice da lo mismo que uno nuevo', () => {
    const base = scatter(2500, 5);
    const first = spatialIndexFor(base);
    const r = rng(11);
    let cur = base;
    for (let round = 0; round < 15; round++) {
      const next = cur.slice();
      for (let k = 0; k < 5; k++) {
        const i = Math.floor(r() * next.length);
        const o = next[i]!;
        next[i] = { ...o, transform: [1, 0, 0, 1, o.transform[4] + (r() - 0.5) * 80, o.transform[5] + (r() - 0.5) * 80] };
      }
      if (round % 4 === 0) next.push(rectObj(`nuevo${round}`, r() * 300, r() * 300, 350, 350));
      const idx = spatialIndexFor(next);
      expect(idx).toBe(first); // se actualizó el mismo, no se reconstruyó
      const fresh = new SpatialIndex(next);
      for (let q = 0; q < 40; q++) {
        const p: [number, number] = [r() * 400, r() * 400];
        expect(idx.atPoint(p[0], p[1], 2)).toEqual(fresh.atPoint(p[0], p[1], 2));
      }
      cur = next;
    }
    // volver al arreglo original (deshacer varios pasos) cambia más de lo que se parcha: se reconstruye y sigue siendo exacto
    const back = spatialIndexFor(base);
    const fresh = new SpatialIndex(base);
    for (let q = 0; q < 40; q++) {
      const p: [number, number] = [r() * 400, r() * 400];
      expect(back.atPoint(p[0], p[1], 2)).toEqual(fresh.atPoint(p[0], p[1], 2));
    }
  });

  it('diffObjects: cambios por posición, añadidos al final, y null si faltan objetos o cambia demasiado', () => {
    const a = scatter(100);
    const b = a.slice();
    b[3] = { ...b[3]! };
    b[40] = { ...b[40]! };
    expect(diffObjects(a, b, 10)).toEqual([3, 40]);
    expect(diffObjects(a, [...b, rectObj('x', 0, 0, 1, 1)], 10)).toEqual([3, 40, 100]);
    expect(diffObjects(a, b.slice(0, 99), 10)).toBeNull();
    expect(diffObjects(a, b, 1)).toBeNull();
    expect(diffObjects(a, a.map((o) => ({ ...o })), 50)).toBeNull();
    expect(diffObjects(a, a, 0)).toEqual([]);
  });
});

describe('hit-test con índice', () => {
  const layers = [makeLayer('#ff0000', 'Rojo')];

  it('pickObject con índice elige lo mismo que el recorrido lineal (muchos objetos, ocultos, bloqueados y de relleno)', () => {
    const objects = scatter(INDEX_MIN_OBJECTS + 1500, 21).map((o, i) => (i % 17 === 0 ? { ...o, hidden: true } : i % 19 === 0 ? { ...o, locked: true } : o));
    const ctx = ctxFor(objects, layers);
    const r = rng(4);
    let hits = 0;
    for (let k = 0; k < 400; k++) {
      const o = objects[Math.floor(r() * objects.length)]!;
      const b = getGeom(o).box!;
      const p: [number, number] = k % 2 ? [b[0] + 0.01, b[1] + (b[3] - b[1]) / 2] : [r() * 400, r() * 400];
      const tol = HIT_PX / (0.5 + r() * 4);
      const got = pickObject(ctx, p, tol);
      const want = linearPick(objects, p, tol, layers);
      expect(got?.id).toBe(want?.id);
      if (got) hits++;
    }
    expect(hits).toBeGreaterThan(50);
  });

  it('una capa de relleno se agarra por dentro y una invisible no se puede elegir', () => {
    const fill = { ...makeLayer('#ff0000', 'Rojo'), mode: 'fill' as const };
    const objects = [...scatter(INDEX_MIN_OBJECTS + 10, 8).filter((o) => o.id.startsWith('r')), rectObj('lleno', 300, 300, 330, 330)];
    const ctxFill = ctxFor(objects, [fill]);
    expect(pickObject(ctxFill, [315, 315], 1)?.id).toBe('lleno');
    const hidden = { ...makeLayer('#ff0000', 'Rojo'), visible: false };
    expect(pickObject(ctxFor(objects, [hidden]), [315, 315], 1)).toBeNull();
  });

  it('selectByBox con índice da los mismos ids y en el mismo orden que el recorrido completo, en ventana y en cruce', () => {
    const objects = scatter(INDEX_MIN_OBJECTS + 700, 33);
    const ctx = ctxFor(objects, layers);
    const small = ctxFor(objects.slice(0, 10), layers);
    expect(small.objects.length).toBeLessThan(INDEX_MIN_OBJECTS);
    const r = rng(5);
    for (let k = 0; k < 30; k++) {
      const x = r() * 350;
      const y = r() * 350;
      const box: Box = [x, y, x + r() * 60, y + r() * 60];
      for (const mode of ['window', 'crossing'] as const) {
        const got = selectByBox(ctx, box, mode);
        // referencia: el mismo cálculo con una lista corta (sin índice) repartida en trozos del mismo orden
        const want: string[] = [];
        for (let i = 0; i < objects.length; i += 100) want.push(...selectByBox(ctxFor(objects.slice(i, i + 100), layers), box, mode));
        expect(got).toEqual(want);
      }
    }
  });

  it('con 100 000 objetos un clic cuesta una fracción del recorrido completo', { timeout: 30_000 }, () => {
    const n = 100_000;
    const cols = 320;
    const objects = Array.from({ length: n }, (_, k) => {
      const x = 5 + (k % cols) * 1.2;
      const y = 5 + Math.floor(k / cols) * 1.2;
      return newVectorObject(`o${k}`, '#ff0000', [polylineBez(rectPoints(x, y, x + 0.8, y + 0.8), true)]);
    });
    const ctx = ctxFor(objects, layers);
    for (const o of objects) getGeom(o); // la geometría ya está (la pintó el renderer)
    pickObject(ctx, [100, 100], 1); // construye el índice una vez
    const time = (reps: number, fn: () => void) => {
      let best = Infinity;
      for (let i = 0; i < 3; i++) {
        const t0 = performance.now();
        for (let j = 0; j < reps; j++) fn();
        best = Math.min(best, (performance.now() - t0) / reps);
      }
      return best;
    };
    const indexed = time(50, () => pickObject(ctx, [100.1, 100.1], 2));
    const linear = time(2, () => linearPick(objects, [100.1, 100.1], 2, layers));
    expect(indexed).toBeLessThan(linear / 5);
    expect(indexed).toBeLessThan(5);
  });
});
