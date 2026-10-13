import { beforeAll, describe, expect, it } from 'vitest';
import type { DesignObject, Point } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { fitBounds } from '@/lib/viewport';
import { boxesIntersect, buildGeom, getGeom, visibleBox } from './geom';
import { pickObject, selectByBox } from './hit';
import { ctxFor } from './testUtil';
import { newVectorObject } from './shapes';

/**
 * Pruebas de rendimiento de la parte de JavaScript del lienzo (geometría, descarte, hit-test y
 * el recorrido del renderer con un contexto simulado). El trazado real lo hace el navegador;
 * aquí se vigila que nada de lo nuestro escale mal con 50 000 segmentos. Para no fallar en CI
 * cargado: cada medición toma el mejor de varios lotes (el ruido del sistema solo empeora un lote,
 * nunca todos), los límites son holgados (decenas de veces lo medido) y lo esencial se verifica
 * con comparaciones relativas dentro de la misma corrida (caché frente a construir, descartar
 * frente a recorrer, lejos frente a cerca).
 */

const SEGMENTS = 50_000;

/** Una polilínea de 50 000 segmentos que recorre la cama de 400 x 400 en zigzag. */
function denseObject(): DesignObject {
  const pts: Point[] = [];
  for (let i = 0; i <= SEGMENTS; i++) {
    const row = Math.floor(i / 250);
    const col = i % 250;
    const x = row % 2 === 0 ? col * 1.6 : 400 - col * 1.6;
    pts.push([x, 2 + row * 2 + (col % 2)]);
  }
  return newVectorObject('dense', '#ff0000', [polylineBez(pts, false)]);
}

/** 5 000 objetos de 10 segmentos cada uno (50 000 en total) repartidos por la cama. */
function manyObjects(): DesignObject[] {
  return Array.from({ length: 5000 }, (_, k) => {
    const ox = (k % 100) * 4;
    const oy = Math.floor(k / 100) * 7;
    const pts: Point[] = Array.from({ length: 11 }, (_, i) => [ox + i * 0.3, oy + (i % 2) * 2]);
    return newVectorObject(`o${k}`, '#ff0000', [polylineBez(pts, false)]);
  });
}

/** Milisegundos por llamada: el mejor de `BATCHES` lotes de `n` llamadas (tras calentar). */
const BATCHES = 5;
function timeIt(n: number, fn: () => void): number {
  fn(); // calentar
  let best = Infinity;
  for (let b = 0; b < BATCHES; b++) {
    const t0 = performance.now();
    for (let i = 0; i < n; i++) fn();
    best = Math.min(best, (performance.now() - t0) / n);
  }
  return best;
}

describe('rendimiento con 50 000 segmentos', () => {
  let dense: DesignObject;
  /** Lo que cuesta construir la geometría (mejor de tres: la primera puede pagar el JIT o una pausa del GC). */
  let buildMs = Infinity;
  beforeAll(() => {
    dense = denseObject();
    for (let i = 0; i < 3; i++) {
      const t0 = performance.now();
      buildGeom(dense);
      buildMs = Math.min(buildMs, performance.now() - t0);
    }
  });

  it('la geometría se construye una vez y se reutiliza (caché por objeto)', () => {
    const g = buildGeom(dense);
    expect(g.segments).toBe(SEGMENTS);
    expect(g.chunks.length).toBeGreaterThan(40); // trozos para poder descartar
    expect(getGeom(dense)).toBe(getGeom(dense));
    const cached = timeIt(200, () => getGeom(dense));
    console.warn(`[perf] construir geometría 50k seg: ${buildMs.toFixed(1)} ms; consultar caché: ${(cached * 1000).toFixed(2)} us`);
    expect(buildMs).toBeLessThan(500);
    expect(cached).toBeLessThan(buildMs / 20); // consultar la caché es mucho más barato que construir
  });

  it('un objeto nuevo (editado) no reutiliza la geometría vieja', () => {
    const moved = { ...dense, transform: [1, 0, 0, 1, 10, 0] as DesignObject['transform'] };
    expect(getGeom(moved)).not.toBe(getGeom(dense));
    expect(getGeom(moved).box![0]).toBeCloseTo(getGeom(dense).box![0] + 10, 9);
  });

  it('con zoom, el descarte deja una fracción pequeña de los trozos', () => {
    const g = getGeom(dense);
    const fit = fitBounds([[0, 0], [400, 400]], 1200, 800, 20);
    const vbFit = visibleBox(fit, 1200, 800);
    const all = g.chunks.filter((c) => boxesIntersect(c.box, vbFit)).length;
    // Zoom 8x sobre una esquina.
    const zoomed = { zoom: fit.zoom * 8, panX: -50, panY: 700 };
    const vbZoom = visibleBox(zoomed, 1200, 800, 3);
    const some = g.chunks.filter((c) => boxesIntersect(c.box, vbZoom)).length;
    console.warn(`[perf] trozos visibles: ajustado ${all}/${g.chunks.length}, zoom 8x ${some}/${g.chunks.length}`);
    expect(all).toBeGreaterThan(some);
    expect(some).toBeLessThan(g.chunks.length / 2);
  });

  it('el cálculo de lo visible por cuadro es despreciable', () => {
    const g = getGeom(dense);
    const view = { zoom: 3, panX: -100, panY: 900 };
    const ms = timeIt(500, () => {
      const vb = visibleBox(view, 1200, 800, 3);
      let n = 0;
      for (const c of g.chunks) if (boxesIntersect(c.box, vb)) n++;
      return n;
    });
    console.warn(`[perf] descarte de ${g.chunks.length} trozos: ${(ms * 1000).toFixed(1)} us por cuadro`);
    expect(ms).toBeLessThan(1);
    expect(ms * 20).toBeLessThan(buildMs); // y más barato que reconstruir la geometría
  });

  it('el hit-test sobre 50 000 segmentos tarda lo que un cuadro de hover', () => {
    const layers = [makeLayer('#ff0000', 'Rojo')];
    const ctx = ctxFor([dense], layers);
    const hit = timeIt(100, () => pickObject(ctx, [123.4, 77.7], 1));
    const miss = timeIt(100, () => pickObject(ctx, [123.4, 1000], 1));
    console.warn(`[perf] hit-test 50k seg: ${hit.toFixed(3)} ms (cerca), ${miss.toFixed(3)} ms (lejos)`);
    expect(hit).toBeLessThan(5);
    expect(miss).toBeLessThan(0.2);
  });

  it('la selección por caja sobre 50 000 segmentos es inmediata', () => {
    const ctx = ctxFor([dense]);
    const ms = timeIt(50, () => selectByBox(ctx, [100, 100, 140, 120], 'crossing'));
    console.warn(`[perf] caja "toca" 50k seg: ${ms.toFixed(3)} ms`);
    expect(ms).toBeLessThan(10);
  });
});

describe('rendimiento con muchos objetos', () => {
  const objs = manyObjects();
  const layers = [makeLayer('#ff0000', 'Rojo')];

  it('hit-test y caja con 5 000 objetos', () => {
    const ctx = ctxFor(objs, layers);
    objs.forEach(getGeom); // la primera vez construye; después solo consulta
    const hit = timeIt(50, () => pickObject(ctx, [200, 150], 1));
    const box = timeIt(50, () => selectByBox(ctx, [0, 0, 400, 350], 'window'));
    console.warn(`[perf] 5000 objetos: hit-test ${hit.toFixed(2)} ms, caja completa ${box.toFixed(2)} ms`);
    expect(hit).toBeLessThan(10);
    expect(box).toBeLessThan(30);
  });
});
