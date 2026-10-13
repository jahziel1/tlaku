import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Document, Point } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import { buildMarks, tabSpans } from './marks';
import { anyStart, anyTabs, clearStart, clearTabs, reverseDirection, setStartPoint, tabIndexNear, toggleTab } from './marksOps';
import { createMarksTool } from './marksTool';
import { lineObj, rectObj } from './testUtil';
import type { Mods, ToolDeps } from './toolDeps';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const ALT: Mods = { ...NONE, alt: true };
const SHIFT: Mods = { ...NONE, shift: true };
const CTRL: Mods = { ...NONE, ctrl: true };

let store: ReturnType<typeof createAppStore>;
let tips: string[];

function deps(): ToolDeps {
  return {
    el: {} as HTMLElement,
    st: () => store.getState(),
    viewOf: () => ({ zoom: 1, panX: 0, panY: 0 }),
    world: (sx, sy) => [sx, sy] as Point,
    capture: () => {},
    release: () => {},
    snapOpts: () => null,
    targetsFor: () => ({ xs: [], ys: [] }),
    setTip: (_x, _y, text) => void tips.push(text),
    snapTo: (p) => p,
  };
}

function load(objects: DesignObject[], lineTabs = true) {
  const layer = makeLayer('#ff0000', 'Rojo');
  store.getState().applyImport({ objects, new_layers: [{ ...layer, mode: 'line', tabs: { ...layer.tabs, enabled: lineTabs, width: 2 } }], warnings: [] });
  store.getState().clearHistory();
}
const obj = (id: string): DesignObject => store.getState().doc.objects.find((o) => o.id === id)!;

beforeEach(() => {
  store = createAppStore();
  tips = [];
  clearGesture();
  patchInteraction({ markHover: null });
});

describe('operaciones de puentes e inicio', () => {
  const doc = (...objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null });

  it('un clic pone un puente y otro clic encima lo quita', () => {
    const d0 = doc(rectObj('r', 0, 0, 40, 40));
    const added = toggleTab(d0, 'r', [20.00004, 0], 1);
    expect(added.action).toBe('add');
    expect(added.doc.objects[0]!.manual_tabs).toEqual([[20, 0]]);
    const removed = toggleTab(added.doc, 'r', [20.5, 0], 1);
    expect(removed.action).toBe('remove');
    expect(removed.doc.objects[0]!.manual_tabs).toEqual([]);
    // Lejos del existente se agrega otro.
    const second = toggleTab(added.doc, 'r', [30, 0], 1);
    expect(second.doc.objects[0]!.manual_tabs).toHaveLength(2);
    expect(tabIndexNear([[1, 1], [5, 5]], [5.2, 5], 0.5)).toBe(1);
  });

  it('no toca objetos bloqueados ni inexistentes', () => {
    const locked = { ...rectObj('r', 0, 0, 40, 40), locked: true };
    expect(toggleTab(doc(locked), 'r', [20, 0], 1).action).toBe('none');
    expect(toggleTab(doc(locked), 'nada', [20, 0], 1).action).toBe('none');
  });

  it('fijar el inicio conserva el sentido; invertir alterna y limpia sin dejar rastro', () => {
    const d0 = doc(rectObj('r', 0, 0, 40, 40));
    const d1 = setStartPoint(d0, 'r', [40, 40]);
    expect(d1.objects[0]!.start).toEqual({ point: [40, 40], reverse: false });
    const d2 = reverseDirection(d1, ['r']);
    expect(d2.objects[0]!.start).toEqual({ point: [40, 40], reverse: true });
    const d3 = setStartPoint(d2, 'r', [0, 0]);
    expect(d3.objects[0]!.start).toEqual({ point: [0, 0], reverse: true });
    // Invertir de nuevo lo deja como estaba.
    expect(reverseDirection(d3, ['r']).objects[0]!.start).toEqual({ point: [0, 0], reverse: false });
    // Solo sentido: sin punto, y al deshacerlo no queda nada guardado.
    const only = reverseDirection(d0, ['r']);
    expect(only.objects[0]!.start).toEqual({ point: null, reverse: true });
    expect(reverseDirection(only, ['r']).objects[0]!.start).toBeNull();
    expect(clearStart(d3, ['r']).objects[0]!.start).toBeNull();
    expect(anyStart(d3.objects)).toBe(true);
    expect(anyStart(clearStart(d3, ['r']).objects)).toBe(false);
  });

  it('invertir con varios: si no todos van invertidos, los invierte a todos; las imágenes se saltan', () => {
    const a = setStartPoint(doc(rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 30, 10)), 'a', [0, 0]);
    const rev = reverseDirection(a, ['a']);
    const all = reverseDirection(rev, ['a', 'b']);
    expect(all.objects.map((o) => o.start?.reverse)).toEqual([true, true]);
    const img: DesignObject = { ...rectObj('i', 0, 0, 5, 5), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [5, 5], mask: [] } };
    const d = doc(img);
    expect(reverseDirection(d, ['i'])).toBe(d);
  });

  it('quitar puentes y devolver el mismo documento si no hay nada que cambiar', () => {
    const d0 = doc(rectObj('r', 0, 0, 40, 40));
    expect(clearTabs(d0, ['r'])).toBe(d0);
    const d1 = toggleTab(d0, 'r', [20, 0], 1).doc;
    expect(anyTabs(d1.objects)).toBe(true);
    expect(anyTabs(clearTabs(d1, ['r']).objects)).toBe(false);
  });
});

describe('herramienta de puentes', () => {
  it('un clic sobre el contorno pone un puente y otro clic lo quita, todo deshacible', () => {
    load([rectObj('r', 0, 0, 40, 40)]);
    store.getState().setTool('tabs');
    const tool = createMarksTool(deps());
    tool.down(20, 0, NONE);
    expect(obj('r').manual_tabs).toEqual([[20, 0]]);
    tool.down(20.4, 0.3, NONE);
    expect(obj('r').manual_tabs).toEqual([]);
    store.getState().undo();
    expect(obj('r').manual_tabs).toEqual([[20, 0]]);
  });

  it('un clic lejos del contorno, o dentro del relleno, no hace nada', () => {
    load([rectObj('r', 0, 0, 40, 40)]);
    store.getState().setTool('tabs');
    const tool = createMarksTool(deps());
    tool.down(20, 20, NONE);
    tool.down(200, 200, NONE);
    expect(obj('r').manual_tabs ?? []).toEqual([]);
    expect(store.getState().past).toHaveLength(0);
  });

  it('en un trazo abierto no hay dónde poner puentes', () => {
    load([lineObj('l', [0, 0], [40, 0])]);
    store.getState().setTool('tabs');
    createMarksTool(deps()).down(20, 0, NONE);
    expect(obj('l').manual_tabs ?? []).toEqual([]);
  });

  it('avisa si la capa tiene los puentes apagados', () => {
    load([rectObj('r', 0, 0, 40, 40)], false);
    store.getState().setTool('tabs');
    createMarksTool(deps()).down(20, 0, NONE);
    expect(store.getState().notices.at(-1)?.text).toContain('Rojo');
  });

  it('el puntero resalta el punto del contorno y dice qué hará el clic', () => {
    load([rectObj('r', 0, 0, 40, 40)]);
    store.getState().setTool('tabs');
    const tool = createMarksTool(deps());
    tool.hover(20.2, 0.2);
    expect(useInteraction.getState().markHover).toEqual([20.2, 0]);
    expect(useInteraction.getState().tip?.text).toMatch(/poner/i);
    tool.down(20, 0, NONE);
    tool.hover(20, 0);
    expect(useInteraction.getState().tip?.text).toMatch(/quitar/i);
    tool.hover(100, 100);
    expect(useInteraction.getState().markHover).toBeNull();
  });
});

describe('herramienta de inicio', () => {
  it('fija el inicio, se pega al vértice más cercano y selecciona el objeto', () => {
    load([rectObj('r', 0, 0, 40, 40)]);
    store.getState().setTool('start');
    const tool = createMarksTool(deps());
    tool.down(39, 1, NONE); // apuntando junto a la esquina, sobre el borde de abajo: (39, 0)
    expect(obj('r').start).toEqual({ point: [40, 0], reverse: false });
    expect(store.getState().selectedIds).toEqual(['r']);
    // Con Alt se queda donde se hizo clic.
    tool.down(20.4, 0, ALT);
    expect(obj('r').start?.point).toEqual([20.4, 0]);
  });

  it('Mayús+clic invierte el sentido del objeto y Ctrl+clic restablece lo fijado', () => {
    load([rectObj('r', 0, 0, 40, 40), rectObj('s', 100, 0, 140, 40)]);
    store.getState().setTool('start');
    const tool = createMarksTool(deps());
    tool.down(40, 0, NONE);
    tool.down(20, 0, SHIFT);
    expect(obj('r').start).toEqual({ point: [40, 0], reverse: true });
    expect(obj('s').start ?? null).toBeNull();
    tool.down(20, 0, CTRL);
    expect(obj('r').start).toBeNull();
    store.getState().undo();
    expect(obj('r').start).toEqual({ point: [40, 0], reverse: true });
  });
});

describe('marcas sobre el lienzo', () => {
  it('los puentes manuales se dibujan como tramos del contorno con el ancho de la capa', () => {
    load([rectObj('r', 0, 0, 40, 40)]);
    store.getState().applyDocChange('canvas.history.tabs', (d) => toggleTab(d, 'r', [20, 0], 1).doc);
    const o = obj('r');
    const spans = tabSpans(o, 2);
    expect(spans).toHaveLength(1);
    expect(spans[0]![0]).toEqual([19, 0]);
    expect(spans[0]!.at(-1)).toEqual([21, 0]);
    expect(tabSpans(o, 2)).toBe(tabSpans(o, 2));
    const marks = buildMarks(store.getState(), useInteraction.getState());
    expect(marks.tabs).toHaveLength(1);
    expect(marks.tabs[0]!.active).toBe(true);
  });

  it('un puente en una capa sin puentes se marca como inactivo', () => {
    load([rectObj('r', 0, 0, 40, 40)], false);
    store.getState().applyDocChange('canvas.history.tabs', (d) => toggleTab(d, 'r', [20, 0], 1).doc);
    expect(buildMarks(store.getState(), useInteraction.getState()).tabs[0]!.active).toBe(false);
  });

  it('la flecha de inicio sale con el objeto seleccionado o con la opción de Ver', () => {
    load([rectObj('a', 0, 0, 40, 40), rectObj('b', 100, 0, 140, 40)]);
    store.getState().applyDocChange('canvas.history.start', (d) => setStartPoint(d, 'b', [140, 40]));
    const ix = useInteraction.getState();
    expect(buildMarks(store.getState(), ix).starts).toHaveLength(0);
    store.getState().select(['a']);
    const sel = buildMarks(store.getState(), ix).starts;
    expect(sel).toHaveLength(1);
    expect(sel[0]!.fixed).toBe(false);
    store.getState().toggleStartMarks();
    const all = buildMarks(store.getState(), ix).starts;
    expect(all.map((s) => s.fixed)).toEqual([false, true]);
    expect(all[1]!.point).toEqual([140, 40]);
  });
});

describe('guías en las marcas', () => {
  it('se dibujan solo si "Mostrar guías" está encendido y la que se arrastra va en vivo', () => {
    store.getState().addGuide({ axis: 'x', pos: 30 });
    store.getState().addGuide({ axis: 'y', pos: 10 });
    const ix = { guideDrag: null, guideHover: null, markHover: null };
    expect(buildMarks(store.getState(), ix).guides).toEqual([
      { axis: 'x', pos: 30, hover: false, remove: false },
      { axis: 'y', pos: 10, hover: false, remove: false },
    ]);
    const moving = { ...ix, guideDrag: { index: 0, axis: 'x' as const, pos: 31, remove: true } };
    expect(buildMarks(store.getState(), moving).guides[0]).toEqual({ axis: 'x', pos: 31, hover: true, remove: true });
    const creating = { ...ix, guideDrag: { index: null, axis: 'y' as const, pos: 5, remove: false } };
    expect(buildMarks(store.getState(), creating).guides).toHaveLength(3);
    store.getState().toggleGuides();
    expect(buildMarks(store.getState(), moving).guides).toEqual([]);
  });
});
