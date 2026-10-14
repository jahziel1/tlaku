import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Affine } from '@/api/types';
import { screenToWorld, worldToScreen } from '@/lib/viewport';
import type { Box } from './geom';
import { CACHE_MARGIN, LayerBitmap, MAX_REGIONS, REGION_PAD_PX, RING_PX, bitmapMatrix, limitRegions, padBox, sameView, sourceBox, transformBox } from './layerBitmap';

/** Contexto 2D que anota las llamadas con sus argumentos y no dibuja nada. */
function fakeCanvas() {
  const calls: [string, ...unknown[]][] = [];
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (t, k: string) => {
      if (k === 'canvas') return canvas;
      if (!(k in t)) t[k] = (...a: unknown[]) => void calls.push([k, ...a]);
      return t[k];
    },
    set: (t, k: string, v) => {
      t[k] = v;
      return true;
    },
  });
  const canvas = { width: 0, height: 0, getContext: () => ctx } as unknown as HTMLCanvasElement;
  return { canvas, calls };
}

let made: ReturnType<typeof fakeCanvas>[] = [];
beforeEach(() => {
  made = [];
  vi.stubGlobal('document', {
    createElement: () => {
      const f = fakeCanvas();
      made.push(f);
      return f.canvas;
    },
  });
});
afterEach(() => vi.unstubAllGlobals());

const apply = (m: Affine, [x, y]: [number, number]): [number, number] => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];

describe('matriz de composición del bitmap', () => {
  it('sin matriz viva es el escalado y desplazamiento de siempre', () => {
    const view = { zoom: 3, panX: 40, panY: 500 };
    const bm = { zoom: 2, panX: 25 + 60, panY: 300 + 60 }; // pan con el margen sumado
    const m = bitmapMatrix(view, bm, null);
    const k = view.zoom / bm.zoom;
    expect(m[0]).toBeCloseTo(k, 12);
    expect(m[3]).toBeCloseTo(k, 12);
    expect(m[1]).toBeCloseTo(0, 12);
    expect(m[2]).toBeCloseTo(0, 12);
    expect(m[4]).toBeCloseTo(view.panX - k * bm.panX, 9);
    expect(m[5]).toBeCloseTo(view.panY - k * bm.panY, 9);
  });

  it('con la misma vista y sin matriz viva deja cada píxel donde estaba', () => {
    const v = { zoom: 1.7, panX: 33, panY: 410 };
    const m = bitmapMatrix(v, v, null);
    expect(apply(m, [123, 45])[0]).toBeCloseTo(123, 9);
    expect(apply(m, [123, 45])[1]).toBeCloseTo(45, 9);
  });

  it('con una matriz viva, un punto del bitmap va adonde la matriz lleva su punto del documento', () => {
    const view = { zoom: 2.5, panX: 80, panY: 600 };
    const bm = { zoom: 2.5, panX: 80 + 50, panY: 600 + 50 };
    const lives: Affine[] = [
      [1, 0, 0, 1, 12, -7], // mover
      [2, 0, 0, 0.5, 3, 4], // escalar
      [Math.cos(0.7), Math.sin(0.7), -Math.sin(0.7), Math.cos(0.7), 10, 20], // girar
    ];
    for (const live of lives) {
      const m = bitmapMatrix(view, bm, live);
      for (const px of [[0, 0], [100, 250], [333, 17]] as [number, number][]) {
        const world = screenToWorld(bm, px); // el punto del documento que el bitmap pintó ahí
        const moved: [number, number] = [live[0] * world[0] + live[2] * world[1] + live[4], live[1] * world[0] + live[3] * world[1] + live[5]];
        const want = worldToScreen(view, moved);
        const got = apply(m, px);
        expect(got[0]).toBeCloseTo(want[0], 6);
        expect(got[1]).toBeCloseTo(want[1], 6);
      }
    }
  });
});

describe('cajas', () => {
  it('transformBox contiene las cuatro esquinas transformadas', () => {
    expect(transformBox([0, 0, 10, 5], [1, 0, 0, 1, 3, -2])).toEqual([3, -2, 13, 3]);
    const r = transformBox([0, 0, 10, 0], [0, 1, -1, 0, 0, 0]); // giro de 90 grados
    expect(r[0]).toBeCloseTo(0, 9);
    expect(r[2]).toBeCloseTo(0, 9);
    expect(r[3]).toBeCloseTo(10, 9);
  });

  it('sourceBox lleva la zona visible al espacio de lo que se mueve; sin inversa devuelve null', () => {
    expect(sourceBox([0, 0, 100, 100], null)).toEqual([0, 0, 100, 100]);
    expect(sourceBox([10, 10, 20, 20], [1, 0, 0, 1, 5, 5])).toEqual([5, 5, 15, 15]);
    expect(sourceBox([0, 0, 1, 1], [0, 0, 0, 0, 1, 1])).toBeNull();
  });

  it('limitRegions deja pasar pocas zonas y funde muchas en una que las contiene', () => {
    const few: Box[] = [[0, 0, 1, 1], [5, 5, 6, 6]];
    expect(limitRegions(few)).toBe(few);
    const many: Box[] = Array.from({ length: MAX_REGIONS + 1 }, (_, i) => [i, -i, i + 1, -i + 1] as Box);
    const merged = limitRegions(many);
    expect(merged).toHaveLength(1);
    expect(merged[0]).toEqual([0, -MAX_REGIONS, MAX_REGIONS + 1, 1]);
    expect(many[0]!.map(Math.abs)).toEqual([0, 0, 1, 1]); // no modifica las de entrada
  });

  it('padBox ensancha en px de pantalla y sameView compara las tres cifras', () => {
    expect(padBox([0, 0, 10, 10], 4, 2)).toEqual([-2, -2, 12, 12]);
    expect(sameView({ zoom: 1, panX: 2, panY: 3 }, { zoom: 1, panX: 2, panY: 3 })).toBe(true);
    expect(sameView({ zoom: 1, panX: 2, panY: 3 }, { zoom: 1, panX: 2, panY: 4 })).toBe(false);
  });
});

describe('LayerBitmap', () => {
  const view = { zoom: 2, panX: 10, panY: 300 };
  const noPaint = () => {};

  it('crea el lienzo con margen, guarda la vista y las identidades, y las compara una a una', () => {
    const bm = new LayerBitmap();
    const objs = [1];
    expect(bm.hasTag([objs])).toBe(false);
    expect(bm.paintAll(view, 400, 300, 2, [objs, 'a'], noPaint)).toBe(true);
    const margin = Math.round(400 * CACHE_MARGIN);
    expect(bm.margin).toBe(margin);
    expect(bm.canvas!.width).toBe(Math.ceil((400 + margin * 2) * 2));
    expect(bm.canvas!.height).toBe(Math.ceil((300 + margin * 2) * 2));
    expect(bm.hasTag([objs, 'a'])).toBe(true);
    expect(bm.hasTag([objs, 'b'])).toBe(false);
    expect(bm.hasTag([objs])).toBe(false);
    expect(bm.view).toEqual(view);
    expect(bm.paintView).toEqual({ zoom: 2, panX: 10 + margin, panY: 300 + margin });
    // misma medida: reutiliza el lienzo
    const first = bm.canvas;
    bm.paintAll(view, 400, 300, 2, [objs], noPaint);
    expect(bm.canvas).toBe(first);
    expect(made).toHaveLength(1);
  });

  it('coverage es la zona de documento que ve el bitmap con su margen', () => {
    const bm = new LayerBitmap();
    bm.paintAll(view, 400, 300, 1, [], noPaint);
    const m = bm.margin;
    const c = bm.coverage();
    expect(c[0]).toBeCloseTo((-m - view.panX) / view.zoom, 9);
    expect(c[2]).toBeCloseTo((400 + m - view.panX) / view.zoom, 9);
    expect(c[3]).toBeCloseTo((view.panY + m) / view.zoom, 9);
    expect(c[1]).toBeCloseTo((view.panY - 300 - m) / view.zoom, 9);
  });

  it('paintRegions pinta cada zona en un lienzo auxiliar (recortado a la zona con un anillo) y copia solo el interior; ignora las lejanas', () => {
    const bm = new LayerBitmap();
    bm.paintAll(view, 400, 300, 1, ['t'], noPaint);
    const main = made[0]!;
    main.calls.length = 0;
    const seen: { ctx: unknown; v: unknown; r: Box }[] = [];
    const region: Box = [50, 50, 60, 55];
    bm.paintRegions([region, [1e6, 1e6, 1e6 + 1, 1e6 + 1]], ['t2'], (ctx, v, _bw, _bh, r) => seen.push({ ctx, v, r }));
    expect(seen).toHaveLength(1); // la lejana no deja zona de píxeles
    expect(made).toHaveLength(2); // el auxiliar se crea al primer parche
    const scratch = made[1]!;
    expect(scratch.canvas.width).toBe(main.canvas.width); // mismo tamaño y misma vista: cada trazo sale con los mismos números
    expect(seen[0]!.ctx).toBe(scratch.canvas.getContext('2d'));
    expect(seen[0]!.v).toEqual(bm.paintView);
    // la caja que cubre el recorte contiene la zona pedida con la holgura del trazo y el anillo
    const w = seen[0]!.r;
    const pad = (REGION_PAD_PX + RING_PX) / view.zoom;
    expect(w[0]).toBeLessThanOrEqual(region[0] - pad + 1e-9);
    expect(w[2]).toBeGreaterThanOrEqual(region[2] + pad - 1e-9);
    expect(w[1]).toBeLessThanOrEqual(region[1] - pad + 1e-9);
    expect(w[3]).toBeGreaterThanOrEqual(region[3] + pad - 1e-9);
    // en el auxiliar: recortar, borrar lo recortado y pintar dentro
    expect(scratch.calls.map((c) => c[0])).toEqual(['save', 'setTransform', 'beginPath', 'rect', 'clip', 'clearRect', 'restore']);
    expect(scratch.calls.find((c) => c[0] === 'rect')!.slice(1)).toEqual(scratch.calls.find((c) => c[0] === 'clearRect')!.slice(1));
    // en el bitmap: borrar el interior y copiarlo en el mismo sitio
    expect(main.calls.map((c) => c[0])).toEqual(['setTransform', 'clearRect', 'drawImage']);
    const clear = main.calls.find((c) => c[0] === 'clearRect')!;
    const draw = main.calls.find((c) => c[0] === 'drawImage')!;
    expect(draw.slice(2)).toEqual([...clear.slice(1), ...clear.slice(1)]); // origen = destino
    // el anillo del recorte es mayor que el interior que se copia
    const ring = scratch.calls.find((c) => c[0] === 'rect')!;
    expect(ring[3] as number).toBeGreaterThan(clear[3] as number);
    expect(bm.hasTag(['t2'])).toBe(true);
    // un segundo parche reutiliza el auxiliar
    bm.paintRegions([region], ['t3'], () => {});
    expect(made).toHaveLength(2);
  });

  it('copyFrom duplica el contenido, la vista y el margen de otro bitmap', () => {
    const a = new LayerBitmap();
    a.paintAll(view, 400, 300, 2, ['a'], noPaint);
    const b = new LayerBitmap();
    expect(b.copyFrom(a, ['b'])).toBe(true);
    expect(b.canvas).not.toBe(a.canvas);
    expect(b.canvas!.width).toBe(a.canvas!.width);
    expect(b.view).toEqual(a.view);
    expect(b.margin).toBe(a.margin);
    expect(b.hasTag(['b'])).toBe(true);
    expect(made[1]!.calls.map((c) => c[0])).toContain('drawImage');
    expect(new LayerBitmap().copyFrom(new LayerBitmap(), [])).toBe(false);
  });

  it('drawOnto compone con la matriz viva en px físicos', () => {
    const bm = new LayerBitmap();
    bm.paintAll(view, 400, 300, 2, [], noPaint);
    const out = fakeCanvas();
    bm.drawOnto(out.canvas.getContext('2d')!, view, [1, 0, 0, 1, 5, 0]);
    const set = out.calls.find((c) => c[0] === 'setTransform')!;
    const m = bitmapMatrix(view, bm.paintView, [1, 0, 0, 1, 5, 0]);
    expect((set[1] as number)).toBeCloseTo(2 * m[0], 9);
    expect((set[5] as number)).toBeCloseTo(2 * m[4], 9);
    expect(out.calls.some((c) => c[0] === 'drawImage')).toBe(true);
    bm.release();
    out.calls.length = 0;
    bm.drawOnto(out.canvas.getContext('2d')!, view);
    expect(out.calls).toHaveLength(0);
  });
});
