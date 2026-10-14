import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Affine, DesignObject, Point } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { Canvas2DRenderer, HEAVY_SEGMENTS, MOVING_BITMAP_SEGMENTS } from './canvas2d';
import { resetSpatialIndex } from './spatialIndex';
import type { CanvasPalette, Scene } from './renderer';
import { newVectorObject } from './shapes';

/** Contextos 2D que anotan `canvas:método` y no dibujan nada: cuántos trazos, borrados y composiciones hace cada cuadro. */
let calls: string[] = [];
let canvasCount = 0;
function fakeCanvas() {
  const id = canvasCount++;
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (t, k: string) => {
      if (k === 'canvas') return canvas;
      if (!(k in t)) t[k] = () => void calls.push(`${id}:${k}`);
      return t[k];
    },
    set: (t, k: string, v) => {
      t[k] = v;
      return true;
    },
  });
  const canvas = { width: 0, height: 0, getContext: () => ctx } as unknown as HTMLCanvasElement;
  return canvas;
}

const PALETTE: CanvasPalette = {
  workspace: '#000', bed: '#111', bedEdge: '#222', gridMinor: '#333', gridMajor: '#444', origin: '#555', selection: '#666',
  selectionFill: '#777', travel: '#888', cut: '#999', engrave: '#aaa', overscan: '#bbb', head: '#ccc', ink: '#ddd', guide: '#eee', crossing: '#fff',
};

const W = 800;
const H = 600;
const COLS = 60;

/** `n` polilíneas de 10 segmentos en una rejilla de 6 mm que cabe en la vista a 3 px/mm. */
function grid(n: number): DesignObject[] {
  return Array.from({ length: n }, (_, k) => {
    const ox = 5 + (k % COLS) * 4;
    const oy = 5 + Math.floor(k / COLS) * 5;
    const pts: Point[] = Array.from({ length: 11 }, (_, i) => [ox + i * 0.3, oy + (i % 2) * 2]);
    return newVectorObject(`o${k}`, '#ff0000', [polylineBez(pts, false)]);
  });
}

const layers = new Map([['#ff0000', makeLayer('#ff0000', 'Rojo')]]);

function scene(objects: DesignObject[], patch: Partial<Scene> = {}, live: Affine | null = null): Scene {
  return {
    width: W, height: H, dpr: 1, view: { zoom: 3, panX: 10, panY: 590 },
    bed: { width: 400, height: 200, origin: 'bottom_left' }, objects, layers, selectedIds: new Set<string>(), wireframe: false,
    preview: null, palette: PALETTE,
    overlay: { hoverId: null, live, rotating: false, rotateMode: false, canRotate: true, marquee: null, guides: [], draft: null, pivot: null, nodes: null },
    head: null, backdrop: null, ...patch,
  };
}

/** Segmentos de trazo `stroke` por cuadro (solo cuenta los de los bitmaps y los directos). */
const strokes = () => calls.filter((c) => c.endsWith(':stroke')).length;
const count = (name: string) => calls.filter((c) => c.endsWith(`:${name}`)).length;

beforeEach(() => {
  calls = [];
  canvasCount = 0;
  resetSpatialIndex();
  vi.stubGlobal('Path2D', class { moveTo() {} lineTo() {} bezierCurveTo() {} quadraticCurveTo() {} closePath() {} });
  vi.stubGlobal('document', { createElement: () => fakeCanvas() });
  vi.stubGlobal('window', { setTimeout: (f: () => void, ms: number) => setTimeout(f, ms), clearTimeout: (t: number) => clearTimeout(t) });
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function renderer() {
  const r = new Canvas2DRenderer();
  r.attach(fakeCanvas());
  r.resize(W, H, 1);
  return r;
}

/** Mide qué hace un cuadro: llamadas de dibujo desde el último `reset`. */
function frame(r: Canvas2DRenderer, s: Scene) {
  calls = [];
  r.render(s);
  return { strokes: strokes(), clears: count('clearRect'), draws: count('drawImage'), clips: count('clip') };
}

describe('lienzo con un diseño denso: gesto y cambios pequeños', () => {
  it('el diseño de prueba es denso y la selección pequeña no necesita bitmap propio', () => {
    expect(grid(3000).length * 10).toBeGreaterThan(HEAVY_SEGMENTS);
    expect(50).toBeLessThan(MOVING_BITMAP_SEGMENTS);
  });

  it('el primer cuadro traza todo en el bitmap y los siguientes solo lo componen', () => {
    const r = renderer();
    const objs = grid(3000);
    const first = frame(r, scene(objs));
    expect(first.strokes).toBeGreaterThanOrEqual(3000);
    const again = frame(r, scene(objs));
    expect(again.strokes).toBeLessThan(10); // cama, cuadrícula y origen
    expect(again.draws).toBeGreaterThanOrEqual(1);
  });

  it('con pocos objetos seleccionados, el arrastre no repinta el diseño: saca la selección del bitmap y la traza directo', () => {
    const r = renderer();
    const objs = grid(3000);
    const sel = new Set(['o10', 'o11', 'o12']);
    frame(r, scene(objs, { selectedIds: sel }));
    const live1 = frame(r, scene(objs, { selectedIds: sel }, [1, 0, 0, 1, 5, 2]));
    // 3 objetos borrados del bitmap (zonas pequeñas) + 3 trazos directos; nada cercano a los 3000 del diseño
    expect(live1.strokes).toBeLessThan(150);
    expect(live1.clips).toBeGreaterThanOrEqual(1);
    const live2 = frame(r, scene(objs, { selectedIds: sel }, [1, 0, 0, 1, 9, 4]));
    expect(live2.strokes).toBeLessThan(12); // solo los seleccionados (y su contorno)
    expect(live2.clears).toBeLessThanOrEqual(live1.clears);
  });

  it('al soltar con el documento cambiado en pocos objetos, repinta solo esas zonas', () => {
    const r = renderer();
    const objs = grid(3000);
    const sel = new Set(['o10', 'o11']);
    frame(r, scene(objs, { selectedIds: sel }));
    frame(r, scene(objs, { selectedIds: sel }, [1, 0, 0, 1, 5, 2]));
    const moved = objs.slice();
    for (const i of [10, 11]) moved[i] = { ...moved[i]!, transform: [1, 0, 0, 1, 5, 2] };
    const done = frame(r, scene(moved, { selectedIds: sel }, null));
    expect(done.strokes).toBeLessThan(150);
    expect(done.strokes).toBeGreaterThan(0);
    // y deshacer (volver al arreglo anterior) también es un parche
    const undone = frame(r, scene(objs, { selectedIds: sel }, null));
    expect(undone.strokes).toBeLessThan(150);
  });

  it('un cambio que no es parchable (se quitó un objeto) o que toca media vista repinta todo', () => {
    const r = renderer();
    const objs = grid(3000);
    frame(r, scene(objs));
    const sinUno = objs.slice(1);
    expect(frame(r, scene(sinUno)).strokes).toBeGreaterThanOrEqual(2999);
    const todos = sinUno.map((o) => ({ ...o, transform: [1, 0, 0, 1, 1, 0] as Affine }));
    expect(frame(r, scene(todos)).strokes).toBeGreaterThanOrEqual(2999);
  });

  it('seleccionar todo y arrastrar con todo a la vista copia el bitmap en vez de volver a trazar 3000 objetos', () => {
    const r = renderer();
    const objs = grid(3000);
    const all = new Set(objs.map((o) => o.id));
    frame(r, scene(objs, { selectedIds: all }));
    const start = frame(r, scene(objs, { selectedIds: all }, [1, 0, 0, 1, 3, 3]));
    expect(start.strokes).toBeLessThan(50); // nada de los 3000 objetos
    const next = frame(r, scene(objs, { selectedIds: all }, [1, 0, 0, 1, 8, 6]));
    expect(next.strokes).toBeLessThan(50);
    expect(next.draws).toBeGreaterThanOrEqual(2); // principal y selección compuestos
  });

  it('con una selección grande y otros objetos a la vista, el bitmap propio se traza una vez y luego solo se compone', () => {
    const r = renderer();
    const objs = grid(3000);
    const sel = new Set(objs.slice(0, 600).map((o) => o.id)); // 6000 segmentos > MOVING_BITMAP_SEGMENTS
    frame(r, scene(objs, { selectedIds: sel }));
    const start = frame(r, scene(objs, { selectedIds: sel }, [1, 0, 0, 1, 3, 3]));
    expect(start.strokes).toBeGreaterThan(0); // se traza la selección en su bitmap...
    const next = frame(r, scene(objs, { selectedIds: sel }, [1, 0, 0, 1, 20, 9]));
    expect(next.strokes).toBeLessThan(10); // ...y después solo se compone con la matriz viva
    const rotated = frame(r, scene(objs, { selectedIds: sel }, [0.9, 0.3, -0.3, 0.9, 20, 9]));
    expect(rotated.strokes).toBeLessThan(10);
  });

  it('al mover la vista el bitmap viejo se reutiliza y se repinta nítido cuando se asienta', () => {
    vi.useFakeTimers();
    const r = renderer();
    let invalidated = 0;
    r.setInvalidate(() => invalidated++);
    const objs = grid(3000);
    frame(r, scene(objs));
    const panned = scene(objs, { view: { zoom: 3, panX: 25, panY: 590 } });
    expect(frame(r, panned).strokes).toBeLessThan(10);
    vi.advanceTimersByTime(200);
    expect(invalidated).toBe(1);
    expect(frame(r, panned).strokes).toBeGreaterThanOrEqual(3000);
    expect(frame(r, panned).strokes).toBeLessThan(10);
  });

  it('un documento ligero sigue trazándose directo en cada cuadro, con o sin gesto', () => {
    const r = renderer();
    const objs = grid(100);
    expect(frame(r, scene(objs)).strokes).toBeGreaterThanOrEqual(100);
    expect(frame(r, scene(objs, { selectedIds: new Set(['o1']) }, [1, 0, 0, 1, 2, 2])).strokes).toBeGreaterThanOrEqual(100);
  });
});
