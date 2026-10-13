import { describe, expect, it } from 'vitest';
import type { CanvasPalette } from '@/features/canvas/renderer';
import { BlockCache, type PathLike } from './blocks';
import { buildLayerPaths, drawPreviewModel, qualityLevel, SEGMENT_BUDGET } from './draw';
import { buildModel, type PreviewData, type PreviewViewOptions } from './model';

class FakePath implements PathLike {
  segs = 0;
  moveTo() {}
  lineTo() {
    this.segs++;
  }
}
const factory = () => new FakePath();

const palette = Object.fromEntries(
  ['workspace', 'bed', 'bedEdge', 'gridMinor', 'gridMajor', 'origin', 'selection', 'selectionFill', 'travel', 'cut', 'engrave', 'overscan', 'head', 'ink'].map((k) => [k, k]),
) as unknown as CanvasPalette;

function fakeCtx() {
  const log: string[] = [];
  const strokes: { style: unknown; alpha: number; width: number; segs: number }[] = [];
  const state = { strokeStyle: '', globalAlpha: 1, lineWidth: 1, fillStyle: '', lineJoin: '', lineCap: '' };
  const ctx = new Proxy(state, {
    get(target, prop: string) {
      if (prop in target) return (target as Record<string, unknown>)[prop];
      if (prop === 'stroke') return (p?: FakePath) => strokes.push({ style: state.strokeStyle, alpha: state.globalAlpha, width: state.lineWidth, segs: p?.segs ?? 0 });
      return (...args: unknown[]) => log.push(`${prop}(${args.join(',')})`);
    },
    set(target, prop: string, v) {
      (target as Record<string, unknown>)[prop] = v;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, strokes, log };
}

/** Cortes de 1 mm a lo largo de X; el movimiento i termina en el segundo i + 1. */
function job(n: number): PreviewData {
  const moves: number[] = [];
  for (let i = 1; i <= n; i++) moves.push(1, i, 0, 100);
  return { layers: [], moves, move_times: Array.from({ length: n }, (_, i) => i + 1), total_s: n, bounds: [[0, 0], [n, 0]], order_note: null, warnings: [] };
}
const opts: PreviewViewOptions = { showTravel: true, showOverscan: true, highlightLayer: null };
const view = { zoom: 1, panX: 0, panY: 100 };

function run(data: PreviewData, scrub: number, o: PreviewViewOptions = opts, v = view, size = { w: 200, h: 200 }) {
  const model = buildModel(data);
  const cache = new BlockCache(model, factory, 10);
  const { ctx, strokes, log } = fakeCtx();
  const stats = drawPreviewModel({ ctx, model, cache, factory, view: v, width: size.w, height: size.h, palette, scrub, options: o, highlight: null });
  return { stats, strokes, log, cache };
}

describe('drawPreviewModel', () => {
  it('dibuja solo hasta el punto de la barra de tiempo', () => {
    const half = run(job(40), 0.5); // 20 movimientos: 2 bloques completos
    expect(half.cache.built).toBe(2);
    const segs = half.strokes.reduce((a, s) => a + s.segs, 0);
    expect(segs).toBe(20);
  });

  it('a mitad de un movimiento dibuja el tramo recorrido y el cabezal', () => {
    const r = run(job(40), 0.5 + 0.5 / 40); // 20.5 movimientos
    expect(r.strokes.reduce((a, s) => a + s.segs, 0)).toBe(21);
    expect(r.log.some((l) => l.startsWith('arc('))).toBe(true);
  });

  it('con el recorrido terminado no dibuja cabezal', () => {
    const r = run(job(40), 1);
    expect(r.log.some((l) => l.startsWith('arc('))).toBe(false);
  });

  it('no reconstruye los bloques ya hechos al avanzar la barra', () => {
    const model = buildModel(job(100));
    const cache = new BlockCache(model, factory, 10);
    const draw = (scrub: number) => {
      const { ctx } = fakeCtx();
      drawPreviewModel({ ctx, model, cache, factory, view, width: 200, height: 200, palette, scrub, options: opts, highlight: null });
    };
    draw(0.3);
    const afterFirst = cache.built;
    draw(0.35);
    draw(0.3);
    expect(cache.built - afterFirst).toBeLessThanOrEqual(1);
    draw(1);
    expect(cache.built).toBe(10);
  });

  it('descarta los bloques fuera de la vista', () => {
    // Vista de 30 px de ancho sobre un recorrido de 100 mm: solo 3 o 4 bloques de 10 se cruzan.
    const r = run(job(100), 1, opts, { zoom: 1, panX: -40, panY: 100 }, { w: 30, h: 50 });
    expect(r.stats.blocksCulled).toBeGreaterThanOrEqual(6);
    expect(r.stats.blocksDrawn).toBeLessThanOrEqual(4);
  });

  it('oculta rápidos y overscan según las opciones', () => {
    const d: PreviewData = {
      ...job(0),
      moves: [0, 5, 0, 0, 1, 10, 0, 0, 1, 15, 0, 50],
      move_times: [1, 2, 3],
      total_s: 3,
    };
    const colors = (o: PreviewViewOptions) => new Set(run(d, 1, o).strokes.map((s) => s.style));
    expect(colors(opts)).toEqual(new Set(['travel', 'overscan', 'engrave']));
    expect(colors({ ...opts, showTravel: false })).toEqual(new Set(['overscan', 'engrave']));
    expect(colors({ ...opts, showOverscan: false })).toEqual(new Set(['travel', 'engrave']));
  });

  it('la opacidad del trazo depende de la potencia', () => {
    const d: PreviewData = {
      ...job(0),
      moves: [1, 5, 0, 10, 1, 10, 0, 100],
      move_times: [1, 2],
      total_s: 2,
    };
    const alphas = run(d, 1).strokes.map((s) => s.alpha);
    expect(Math.min(...alphas)).toBeLessThan(Math.max(...alphas));
    expect(Math.max(...alphas)).toBe(1);
  });

  it('el grosor se compensa con el zoom (el trazo mide lo mismo en pantalla)', () => {
    const w1 = run(job(10), 1, opts, { zoom: 1, panX: 0, panY: 100 }).strokes[0]!.width;
    const w4 = run(job(10), 1, opts, { zoom: 4, panX: 0, panY: 100 }).strokes[0]!.width;
    expect(w1 / w4).toBeCloseTo(4);
  });

  it('resalta una capa sin reconstruir el resto', () => {
    const layer = (seconds: number) => ({ color: '#fff', mode: 'line', seconds, summary: '', travel_saved_pct: null, explanations: [] });
    const d = { ...job(10), layers: [layer(5), layer(5)] };
    const model = buildModel(d);
    const hl = buildLayerPaths(model, 1, factory);
    expect(hl).not.toBeNull();
    expect(buildLayerPaths(model, 7, factory)).toBeNull();
    const cache = new BlockCache(model, factory, 10);
    const { ctx, strokes } = fakeCtx();
    drawPreviewModel({ ctx, model, cache, factory, view, width: 200, height: 200, palette, scrub: 1, options: { ...opts, highlightLayer: 1 }, highlight: hl });
    expect(strokes.some((s) => s.style === 'head')).toBe(true);
    expect(strokes.find((s) => s.style === 'cut')!.alpha).toBeLessThan(0.5); // lo demás se atenúa
  });
});

describe('nivel de detalle', () => {
  it('qualityLevel baja con el zoom alto y sube al alejar', () => {
    expect(qualityLevel(100)).toBe(0);
    let prev = qualityLevel(50);
    for (const z of [10, 2, 0.5, 0.1, 0.05]) {
      const l = qualityLevel(z);
      expect(l).toBeGreaterThanOrEqual(prev);
      prev = l;
    }
    expect(qualityLevel(0.0001)).toBeGreaterThan(5);
  });

  it('con pocos segmentos se queda en el nivel que no se nota', () => {
    const r = run(job(100), 1);
    expect(r.stats.level).toBe(qualityLevel(1));
  });

  it('si hay demasiados segmentos baja el detalle hasta entrar en el presupuesto', () => {
    // 300 mil tramos de 0.01 mm en zigzag: a zoom 1 no caben en el presupuesto con el detalle fino.
    const n = SEGMENT_BUDGET * 2.5;
    const moves: number[] = [];
    for (let i = 1; i <= n; i++) moves.push(1, (i % 2) * 0.5, i * 0.01 + (i % 2) * 0.3, 20 + (i % 4) * 25);
    const layers = [{ color: '#fff', mode: 'fill', seconds: n, summary: '', travel_saved_pct: null, explanations: [] }]; // todo es grabado
    const d: PreviewData = { layers, moves, move_times: Array.from({ length: n }, (_, i) => i + 1), total_s: n, bounds: [[0, 0], [1, 1]], order_note: null, warnings: [] };
    const model = buildModel(d);
    const cache = new BlockCache(model, factory);
    const { ctx, strokes } = fakeCtx();
    const stats = drawPreviewModel({ ctx, model, cache, factory, view: { zoom: 1, panX: 0, panY: 4000 }, width: 600, height: 5000, palette, scrub: 1, options: opts, highlight: null });
    expect(stats.level).toBeGreaterThan(qualityLevel(1));
    const segs = strokes.reduce((a, s) => a + s.segs, 0);
    expect(segs).toBeLessThanOrEqual(SEGMENT_BUDGET);
  });
});
