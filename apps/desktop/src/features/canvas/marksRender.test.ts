import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { Canvas2DRenderer } from './canvas2d';
import type { SceneMarks } from './marks';
import type { CanvasPalette, Scene } from './renderer';
import { newVectorObject, rectPoints } from './shapes';
import { drawVirtualCopies, MAX_GHOSTS } from './virtualDraw';

/** Contexto 2D que anota las llamadas con sus argumentos y no dibuja nada. */
function fakeCanvas() {
  const calls: [string, unknown[]][] = [];
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (t, k: string) => {
      if (k === 'canvas') return canvas;
      if (!(k in t)) t[k] = (...args: unknown[]) => void calls.push([k, args]);
      return t[k];
    },
    set: (t, k: string, v) => {
      t[k] = v;
      return true;
    },
  });
  const canvas = { width: 0, height: 0, getContext: () => ctx } as unknown as HTMLCanvasElement;
  return { canvas, ctx: ctx as unknown as CanvasRenderingContext2D, calls };
}

const PALETTE: CanvasPalette = {
  workspace: '#000', bed: '#111', bedEdge: '#222', gridMinor: '#333', gridMajor: '#444', origin: '#555', selection: '#666',
  selectionFill: '#777', travel: '#888', cut: '#999', engrave: '#aaa', overscan: '#bbb', head: '#ccc', ink: '#ddd', guide: '#eee', crossing: '#fff',
};

const emptyMarks = (patch: Partial<SceneMarks> = {}): SceneMarks => ({ guides: [], tabs: [], starts: [], hover: null, ...patch });

function scene(patch: Partial<Scene> = {}): Scene {
  const obj = newVectorObject('a', '#ff0000', [polylineBez(rectPoints(10, 10, 50, 40), true)]);
  return {
    width: 400, height: 300, dpr: 1, view: { zoom: 2, panX: 20, panY: 280 },
    bed: { width: 200, height: 130, origin: 'bottom_left' }, objects: [obj], layers: new Map([['#ff0000', makeLayer('#ff0000', 'Rojo')]]),
    selectedIds: new Set(), wireframe: false, preview: null, palette: PALETTE,
    overlay: { hoverId: null, live: null, rotating: false, rotateMode: false, canRotate: true, marquee: null, guides: [], draft: null, pivot: null, nodes: null },
    head: null, backdrop: null, ...patch,
  };
}

beforeEach(() => {
  vi.stubGlobal('Path2D', class { moveTo() {} lineTo() {} bezierCurveTo() {} quadraticCurveTo() {} closePath() {} });
  vi.stubGlobal('document', { createElement: () => fakeCanvas().canvas });
});
afterEach(() => vi.unstubAllGlobals());

function render(s: Scene) {
  const { canvas, calls } = fakeCanvas();
  const r = new Canvas2DRenderer();
  r.attach(canvas);
  r.resize(400, 300, 1);
  r.render(s);
  return calls;
}

const lineTos = (calls: [string, unknown[]][]) => calls.filter(([k]) => k === 'lineTo').map(([, a]) => a as number[]);

describe('guías del documento en el lienzo', () => {
  it('una guía vertical y una horizontal cruzan todo el lienzo', () => {
    const base = render(scene({ marks: emptyMarks() }));
    const calls = render(scene({ marks: emptyMarks({ guides: [{ axis: 'x', pos: 30, hover: false, remove: false }, { axis: 'y', pos: 20, hover: true, remove: false }] }) }));
    const extra = lineTos(calls).filter((a) => !lineTos(base).some((b) => b[0] === a[0] && b[1] === a[1]));
    // x = 20 + 30 * 2 = 80 (medio píxel de alineación); y = 280 - 20 * 2 = 240.
    expect(extra).toContainEqual([80.5, 300]);
    expect(extra).toContainEqual([400, 240.5]);
  });

  it('sin marcas el renderer no falla (escena sin el campo)', () => {
    expect(() => render(scene())).not.toThrow();
  });
});

describe('puentes manuales y flechas de inicio', () => {
  it('cada puente se traza como un tramo grueso del contorno, en dos pasadas', () => {
    const base = render(scene({ marks: emptyMarks() }));
    const calls = render(scene({ marks: emptyMarks({ tabs: [{ span: [[19, 10], [21, 10]], active: true }] }) }));
    expect(calls.filter(([k]) => k === 'stroke').length - base.filter(([k]) => k === 'stroke').length).toBe(2);
  });

  it('la flecha de inicio dibuja un disco en el punto y una punta de flecha', () => {
    const calls = render(scene({ marks: emptyMarks({ starts: [{ point: [10, 10], dir: [1, 0], fixed: true }] }) }));
    const arcs = calls.filter(([k]) => k === 'arc');
    expect(arcs.some(([, a]) => (a as number[])[0] === 40 && (a as number[])[1] === 260)).toBe(true);
    // La punta apunta hacia +x: su vértice más lejano queda a la derecha del disco.
    const tip = lineTos(calls).filter((a) => a[0] > 40 + 20);
    expect(tip.length).toBeGreaterThan(0);
  });

  it('el punto bajo el puntero se marca con un anillo', () => {
    const base = render(scene({ marks: emptyMarks() }));
    const calls = render(scene({ marks: emptyMarks({ hover: [10, 10] }) }));
    expect(calls.filter(([k]) => k === 'arc').length).toBe(base.filter(([k]) => k === 'arc').length + 1);
  });
});

describe('copias del arreglo virtual', () => {
  const withArray = (array: NonNullable<Scene['objects'][number]['array']>) => {
    const s = scene();
    s.objects = [{ ...s.objects[0]!, array }];
    return s;
  };

  it('un arreglo en cuadrícula dibuja las copias sin la original', () => {
    const s = withArray({ type: 'grid', cols: 3, rows: 2, dx: 5, dy: 5 });
    const { ctx, calls } = fakeCanvas();
    expect(drawVirtualCopies(ctx, s, 1)).toBe(5);
    expect(calls.filter(([k]) => k === 'stroke')).toHaveLength(5);
  });

  it('un objeto sin arreglo, oculto o con la vista previa encendida no dibuja nada', () => {
    const { ctx } = fakeCanvas();
    expect(drawVirtualCopies(ctx, scene(), 1)).toBe(0);
    const s = withArray({ type: 'grid', cols: 2, rows: 2, dx: 5, dy: 5 });
    expect(drawVirtualCopies(ctx, { ...s, objects: [{ ...s.objects[0]!, hidden: true }] }, 1)).toBe(0);
    expect(drawVirtualCopies(ctx, { ...s, preview: {} as Scene['preview'] }, 1)).toBe(0);
  });

  it('descarta las copias fuera de la vista y respeta el tope', () => {
    const { ctx } = fakeCanvas();
    // Paso de 140 mm (320 px): solo la primera copia a la derecha cae dentro de los 400 px de la vista.
    const far = withArray({ type: 'grid', cols: 4, rows: 1, dx: 100, dy: 0 });
    expect(drawVirtualCopies(ctx, far, 1)).toBe(1);
    const many = withArray({ type: 'grid', cols: 2000, rows: 1, dx: -39.9, dy: 0 });
    expect(drawVirtualCopies(ctx, many, 1)).toBeLessThanOrEqual(MAX_GHOSTS);
  });

  it('el renderer completo las pinta encima de los objetos', () => {
    const s = withArray({ type: 'grid', cols: 2, rows: 1, dx: 10, dy: 0 });
    const withoutGhost = render(scene());
    const calls = render(s);
    expect(calls.filter(([k]) => k === 'stroke').length).toBeGreaterThan(withoutGhost.filter(([k]) => k === 'stroke').length);
  });
});
