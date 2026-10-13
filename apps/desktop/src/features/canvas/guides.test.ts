import { beforeEach, describe, expect, it } from 'vitest';
import type { Point } from '@/api/types';
import { createAppStore, useStore } from '@/store';
import { createGuideTool, createRulerDrag, rulerToArea } from './guideTool';
import { guideAt, guideAxisForRuler, guideScreenPos, guideSnapTargets, guideValueFromScreen, outsideCanvas, snapGuidePos, useGuideUi } from './guides';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import { collectSnapTargets } from './snap';
import { rectObj } from './testUtil';
import type { Mods, ToolDeps } from './toolDeps';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const ALT: Mods = { shift: false, alt: true, ctrl: false };
const ev = (id = 1) => ({ pointerId: id }) as PointerEvent;
const VIEW = { zoom: 2, panX: 10, panY: 500 };

let store: ReturnType<typeof createAppStore>;
let tips: string[];

function deps(): ToolDeps {
  return {
    el: {} as HTMLElement,
    st: () => store.getState(),
    viewOf: () => VIEW,
    world: (sx, sy) => [(sx - VIEW.panX) / VIEW.zoom, (VIEW.panY - sy) / VIEW.zoom] as Point,
    capture: () => {},
    release: () => {},
    snapOpts: () => null,
    targetsFor: () => ({ xs: [], ys: [] }),
    setTip: (_x, _y, text) => void tips.push(text),
    snapTo: (p) => p,
  };
}

beforeEach(() => {
  store = createAppStore();
  store.setState({ zoom: VIEW.zoom, panX: VIEW.panX, panY: VIEW.panY, viewport: { width: 800, height: 600 }, snapEnabled: false });
  tips = [];
  clearGesture();
  patchInteraction({ guideHover: null });
  useGuideUi.getState().close();
});

describe('guías: geometría de pantalla', () => {
  it('pasa de mm a pantalla y de vuelta, y cada regla crea el eje contrario', () => {
    expect(guideScreenPos({ axis: 'x', pos: 30 }, VIEW)).toBe(70);
    expect(guideScreenPos({ axis: 'y', pos: 30 }, VIEW)).toBe(440);
    expect(guideValueFromScreen('x', VIEW, 70, 0)).toBe(30);
    expect(guideValueFromScreen('y', VIEW, 0, 440)).toBe(30);
    expect(guideAxisForRuler('x')).toBe('y');
    expect(guideAxisForRuler('y')).toBe('x');
  });

  it('agarra la guía más cercana dentro de pocos píxeles', () => {
    const guides = [{ axis: 'x' as const, pos: 30 }, { axis: 'y' as const, pos: 30 }];
    expect(guideAt(guides, VIEW, 73, 0)).toBe(0);
    expect(guideAt(guides, VIEW, 100, 443)).toBe(1);
    expect(guideAt(guides, VIEW, 100, 300)).toBe(-1);
    expect(outsideCanvas(800, 600, -1, 10)).toBe(true);
    expect(outsideCanvas(800, 600, 400, 300)).toBe(false);
  });

  it('las coordenadas del evento en una regla son relativas al área del lienzo', () => {
    const rect = { left: 30, right: 52, top: 120, bottom: 142 } as DOMRect;
    expect(rulerToArea('x', rect, { clientX: 130, clientY: 192 })).toEqual([100, 50]);
    expect(rulerToArea('y', { left: 0, right: 22, top: 120, bottom: 720 } as DOMRect, { clientX: 72, clientY: 150 })).toEqual([50, 30]);
  });
});

describe('guías en el documento', () => {
  it('agregar, mover y borrar son pasos de deshacer', () => {
    const s = store.getState();
    expect(s.addGuide({ axis: 'x', pos: 12 })).toBe(0);
    expect(s.addGuide({ axis: 'y', pos: 5 })).toBe(1);
    expect(store.getState().addGuide({ axis: 'x', pos: Number.NaN })).toBe(-1);
    store.getState().updateGuide(0, { pos: 15 });
    expect(store.getState().doc.guides).toEqual([{ axis: 'x', pos: 15 }, { axis: 'y', pos: 5 }]);
    store.getState().removeGuide(0);
    expect(store.getState().doc.guides).toEqual([{ axis: 'y', pos: 5 }]);
    store.getState().undo();
    expect(store.getState().doc.guides).toHaveLength(2);
    store.getState().undo();
    expect(store.getState().doc.guides?.[0]).toEqual({ axis: 'x', pos: 12 });
    // Sin cambio real no hay paso nuevo.
    const n = store.getState().past.length;
    store.getState().updateGuide(0, { pos: 12 });
    store.getState().removeGuide(9);
    expect(store.getState().past).toHaveLength(n);
  });

  it('las guías viajan con el documento al guardar y abrir', () => {
    store.getState().addGuide({ axis: 'x', pos: 12 });
    const saved = JSON.parse(JSON.stringify(store.getState().doc));
    const other = createAppStore();
    other.getState().replaceDocument(saved, null);
    expect(other.getState().doc.guides).toEqual([{ axis: 'x', pos: 12 }]);
  });
});

describe('ajuste magnético a las guías', () => {
  it('guideSnapTargets da una referencia por guía y collectSnapTargets la admite', () => {
    const t = guideSnapTargets([{ axis: 'x', pos: 30 }, { axis: 'y', pos: 8 }], { width: 100, height: 60 });
    expect(t.xs).toEqual([{ v: 30, kind: 'guide', lo: 0, hi: 60 }]);
    expect(t.ys).toEqual([{ v: 8, kind: 'guide', lo: 0, hi: 100 }]);
    const set = collectSnapTargets([rectObj('r', 0, 0, 10, 10)], new Set(), { width: 100, height: 60 });
    set.xs.push(...t.xs);
    expect(set.xs.some((x) => x.kind === 'guide')).toBe(true);
  });

  it('una guía nueva se pega a la cuadrícula, a los objetos y a otra guía; Alt la suelta', () => {
    store.setState({ snapEnabled: true, snapGrid: true, snapObjects: true });
    store.getState().applyImport({ objects: [rectObj('r', 0, 0, 33, 20)], new_layers: [], warnings: [] });
    store.getState().addGuide({ axis: 'x', pos: 70.2 });
    const s = store.getState();
    // Junto al borde de un objeto (x = 33, tolerancia 8 px / zoom 2 = 4 mm).
    expect(snapGuidePos(s, 'x', 32.4, null, false)).toBe(33);
    // Junto a otra guía.
    expect(snapGuidePos(s, 'x', 71.5, null, false)).toBe(70.2);
    // La guía que se arrastra no se pega a sí misma.
    expect(snapGuidePos(s, 'x', 71.5, 0, false)).not.toBe(70.2);
    // Suspendido: la posición tal cual.
    expect(snapGuidePos(s, 'x', 32.4, null, true)).toBe(32.4);
  });
});

describe('guías sobre el lienzo (herramienta Seleccionar)', () => {
  function withGuides() {
    store.getState().addGuide({ axis: 'x', pos: 30 });
    store.getState().addGuide({ axis: 'y', pos: 40 });
    store.getState().clearHistory();
  }

  it('pasar el puntero resalta la guía y cambia el cursor', () => {
    withGuides();
    const tool = createGuideTool(deps());
    expect(tool.hover(71, 10)).toBe(true);
    expect(useInteraction.getState().guideHover).toBe(0);
    expect(useInteraction.getState().cursor).toBe('ew-resize');
    expect(tool.hover(300, 300)).toBe(false);
    expect(useInteraction.getState().guideHover).toBeNull();
  });

  it('arrastrarla la mueve y es un solo paso de deshacer', () => {
    withGuides();
    const tool = createGuideTool(deps());
    expect(tool.down(ev(), 70, 100)).toBe(true);
    tool.move(ev(), 80, 100, NONE);
    expect(useInteraction.getState().guideDrag).toMatchObject({ index: 0, axis: 'x', pos: 35, remove: false });
    expect(tips.at(-1)).toContain('35');
    tool.move(ev(), 90, 100, NONE);
    tool.up(ev(), 90, 100, NONE);
    expect(store.getState().doc.guides?.[0]).toEqual({ axis: 'x', pos: 40 });
    expect(store.getState().past).toHaveLength(1);
    expect(useInteraction.getState().guideDrag).toBeNull();
    expect(useInteraction.getState().gesture).toBeNull();
  });

  it('soltarla fuera del lienzo (sobre la regla) la borra', () => {
    withGuides();
    const tool = createGuideTool(deps());
    tool.down(ev(), 70, 100);
    tool.move(ev(), -5, 100, NONE);
    expect(useInteraction.getState().guideDrag?.remove).toBe(true);
    expect(tips.at(-1)).toMatch(/borrar/i);
    tool.up(ev(), -5, 100, NONE);
    expect(store.getState().doc.guides).toEqual([{ axis: 'y', pos: 40 }]);
  });

  it('un clic sin mover no cambia nada; Esc o cancelar deja la guía donde estaba', () => {
    withGuides();
    const tool = createGuideTool(deps());
    tool.down(ev(), 70, 100);
    tool.up(ev(), 70, 100, NONE);
    expect(store.getState().past).toHaveLength(0);
    tool.down(ev(), 70, 100);
    tool.move(ev(), 120, 100, NONE);
    tool.cancel();
    expect(useInteraction.getState().guideDrag).toBeNull();
    expect(store.getState().doc.guides?.[0]?.pos).toBe(30);
  });

  it('solo actúa con Seleccionar y con las guías visibles', () => {
    withGuides();
    const tool = createGuideTool(deps());
    store.getState().setTool('rect');
    expect(tool.down(ev(), 70, 100)).toBe(false);
    store.getState().setTool('select');
    store.getState().toggleGuides();
    expect(tool.down(ev(), 70, 100)).toBe(false);
    expect(tool.hover(70, 100)).toBe(false);
  });

  it('el doble clic abre el diálogo de posición de esa guía', () => {
    withGuides();
    const tool = createGuideTool(deps());
    expect(tool.doubleClick(300, 300)).toBe(false);
    expect(useGuideUi.getState().open).toBe(false);
    expect(tool.doubleClick(70, 100)).toBe(true);
    expect(useGuideUi.getState()).toMatchObject({ open: true, focus: 0 });
  });

  it('Alt mientras se arrastra suspende el ajuste', () => {
    store.setState({ snapEnabled: true, snapGrid: true, snapObjects: false });
    withGuides();
    const tool = createGuideTool(deps());
    tool.down(ev(), 70, 100);
    tool.move(ev(), 73, 100, NONE);
    const snapped = useInteraction.getState().guideDrag!.pos;
    expect(Number.isInteger(snapped)).toBe(true);
    tool.move(ev(), 73.4, 100, ALT);
    expect(useInteraction.getState().guideDrag!.pos).toBe(31.7);
    tool.cancel();
  });
});

describe('guías creadas desde la regla', () => {
  const rect = { left: 22, right: 822, top: 40, bottom: 62 } as DOMRect;

  it('arrastrar hacia el lienzo y soltar crea la guía en la posición del puntero', () => {
    useStore.setState({ zoom: VIEW.zoom, panX: VIEW.panX, panY: VIEW.panY, viewport: { width: 800, height: 600 }, snapEnabled: false, showGuides: true, doc: { ...useStore.getState().doc, guides: [] } });
    const drag = createRulerDrag('x'); // la regla de arriba crea guías horizontales
    // y de pantalla 100 dentro del área = clientY 62 + 100
    const live = drag.move(rect, { clientX: 322, clientY: 162, altKey: false });
    expect(live.cancel).toBe(false);
    expect(live.pos).toBe(200);
    expect(useInteraction.getState().guideDrag).toEqual({ index: null, axis: 'y', pos: 200, remove: false });
    drag.finish(rect, { clientX: 322, clientY: 162, altKey: false });
    expect(useStore.getState().doc.guides).toEqual([{ axis: 'y', pos: 200 }]);
    expect(useInteraction.getState().guideDrag).toBeNull();
  });

  it('soltar sobre la propia regla cancela y no deja guía', () => {
    useStore.setState({ zoom: VIEW.zoom, panX: VIEW.panX, panY: VIEW.panY, viewport: { width: 800, height: 600 }, snapEnabled: false, showGuides: true, doc: { ...useStore.getState().doc, guides: [] } });
    const drag = createRulerDrag('x');
    expect(drag.move(rect, { clientX: 100, clientY: 63, altKey: false }).cancel).toBe(true);
    drag.finish(rect, { clientX: 100, clientY: 63, altKey: false });
    expect(useStore.getState().doc.guides ?? []).toEqual([]);
  });

  it('la regla de la izquierda crea guías verticales', () => {
    useStore.setState({ zoom: VIEW.zoom, panX: VIEW.panX, panY: VIEW.panY, viewport: { width: 800, height: 600 }, snapEnabled: false, showGuides: true, doc: { ...useStore.getState().doc, guides: [] } });
    const left = { left: 0, right: 22, top: 62, bottom: 662 } as DOMRect;
    const drag = createRulerDrag('y');
    drag.finish(left, { clientX: 22 + 70, clientY: 200, altKey: false });
    expect(useStore.getState().doc.guides).toEqual([{ axis: 'x', pos: 30 }]);
  });
});
