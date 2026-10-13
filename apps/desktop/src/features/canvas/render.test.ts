import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Point } from '@/api/types';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import type { CameraBackdrop } from '@/store/camera';
import { Canvas2DRenderer } from './canvas2d';
import type { CanvasPalette, Scene } from './renderer';
import { buildNodeOverlay } from './nodeview';
import { newVectorObject, rectPoints } from './shapes';

/** Contexto 2D que anota las llamadas y no dibuja nada. */
function fakeCanvas() {
  const calls: string[] = [];
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (t, k: string) => {
      if (k === 'canvas') return canvas;
      if (!(k in t)) t[k] = () => void calls.push(k);
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

const PALETTE: CanvasPalette = {
  workspace: '#000', bed: '#111', bedEdge: '#222', gridMinor: '#333', gridMajor: '#444', origin: '#555', selection: '#666',
  selectionFill: '#777', travel: '#888', cut: '#999', engrave: '#aaa', overscan: '#bbb', head: '#ccc', ink: '#ddd', guide: '#eee', crossing: '#fff',
};

function scene(patch: Partial<Scene> = {}): Scene {
  const obj = newVectorObject('a', '#ff0000', [polylineBez(rectPoints(10, 10, 50, 40), true), ellipseBez(80, 30, 15, 10)]);
  const layers = new Map([['#ff0000', makeLayer('#ff0000', 'Rojo')]]);
  return {
    width: 400, height: 300, dpr: 1, view: { zoom: 2, panX: 20, panY: 280 },
    bed: { width: 200, height: 130, origin: 'bottom_left' }, objects: [obj], layers, selectedIds: new Set(['a']), wireframe: false,
    preview: null, palette: PALETTE,
    overlay: {
      hoverId: null, live: null, rotating: false, rotateMode: false, canRotate: true, marquee: null, guides: [], draft: null, pivot: null, nodes: null,
    },
    head: null, backdrop: null, ...patch,
  };
}

beforeEach(() => {
  vi.stubGlobal('Path2D', class { moveTo() {} lineTo() {} bezierCurveTo() {} quadraticCurveTo() {} closePath() {} });
  vi.stubGlobal('document', { createElement: () => fakeCanvas().canvas });
});
afterEach(() => vi.unstubAllGlobals());

function renderer() {
  const { canvas, calls } = fakeCanvas();
  const r = new Canvas2DRenderer();
  r.attach(canvas);
  r.resize(400, 300, 1);
  return { r, calls };
}

describe('renderer: nodos y fondo de cámara', () => {
  it('sin cámara no se dibuja ninguna imagen ni se crea ninguna capa auxiliar', () => {
    const { r, calls } = renderer();
    r.render(scene());
    expect(calls).not.toContain('drawImage');
    expect((r as unknown as { backdropLayer: unknown }).backdropLayer).toBeNull();
  });

  it('con cámara proyecta la imagen y la compone con su opacidad; un cuadro nuevo la rehace y el mismo no', () => {
    const { r } = renderer();
    const backdrop: CameraBackdrop = {
      source: {} as CanvasImageSource, width: 640, height: 480, opacity: 0.5, frame: 1,
      corners: [[10, 120], [190, 110], [200, 10], [0, 0]] as [Point, Point, Point, Point],
    };
    const layerOf = () => (r as unknown as { backdropLayer: { key: unknown; ctx: { drawImage: () => void } } | null }).backdropLayer;
    r.render(scene({ backdrop }));
    const first = layerOf()!;
    expect(first.key).not.toBeNull();
    const key1 = first.key;
    r.render(scene({ backdrop })); // nada cambió: se reutiliza la capa proyectada
    expect(layerOf()!.key).toBe(key1);
    r.render(scene({ backdrop: { ...backdrop, frame: 2 } })); // cuadro nuevo
    expect(layerOf()!.key).not.toBe(key1);
    r.render(scene()); // se quitó la cámara: se libera la capa
    expect(layerOf()).toBeNull();
  });

  it('dibuja los nodos y manejadores del overlay sin errores, también con una vista previa', () => {
    const { r, calls } = renderer();
    const s = scene();
    const nodes = buildNodeOverlay({
      objects: s.objects, selectedIds: ['a'], nodeSel: { a: [0, 5] }, nodeCorners: {}, preview: null, hover: { id: 'a', k: 5, handle: null },
    });
    expect(nodes.nodes.length).toBe(8);
    expect(nodes.handles.length).toBeGreaterThan(0);
    r.render({ ...s, overlay: { ...s.overlay, nodes } });
    expect(calls).toContain('arc');
    expect(calls.filter((c) => c === 'rect').length).toBeGreaterThan(0);
    // con la herramienta de nodos no hay marco de selección con manijas de escala
    const frame = calls.filter((c) => c === 'strokeRect').length;
    r.render(scene());
    const withFrame = calls.filter((c) => c === 'strokeRect').length - frame;
    expect(withFrame).toBeGreaterThan(0);
  });
});

describe('overlay de nodos', () => {
  it('muestra los manejadores de los nodos elegidos y de sus vecinos que miran hacia ellos', () => {
    const obj = newVectorObject('e', '#ff0000', [ellipseBez(0, 0, 10, 10)]);
    const none = buildNodeOverlay({ objects: [obj], selectedIds: ['e'], nodeSel: {}, nodeCorners: {}, preview: null, hover: null });
    expect(none.handles).toHaveLength(0);
    expect(none.nodes).toHaveLength(4);
    const one = buildNodeOverlay({ objects: [obj], selectedIds: ['e'], nodeSel: { e: [0] }, nodeCorners: {}, preview: null, hover: null });
    // 2 del nodo elegido + 1 de cada vecino (el que mira hacia él)
    expect(one.handles).toHaveLength(4);
    expect(one.nodes.filter((n) => n.selected)).toHaveLength(1);
    expect(one.nodes[0].type).toBe('symmetric');
  });

  it('se reutiliza mientras no cambien los insumos y construir 30 000 nodos es barato', () => {
    const pts: Point[] = Array.from({ length: 30_000 }, (_, i) => [i * 0.01, (i % 7) * 0.5]);
    const obj = newVectorObject('big', '#ff0000', [polylineBez(pts, false)]);
    const input = { objects: [obj], selectedIds: ['big'], nodeSel: {}, nodeCorners: {}, preview: null, hover: null };
    const t0 = performance.now();
    const a = buildNodeOverlay(input);
    const ms = performance.now() - t0;
    expect(a.nodes).toHaveLength(30_000);
    expect(buildNodeOverlay(input)).toBe(a);
    expect(ms).toBeLessThan(500);
  });
});
