import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Point } from '@/api/types';
import { objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import { createMeasureTool } from './measureTool';
import { createPickTool, startPickMode } from './pickTool';
import { rectObj } from './testUtil';
import type { Mods, ToolDeps } from './toolDeps';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const SHIFT: Mods = { shift: true, alt: false, ctrl: false };
const ev = (id = 1) => ({ pointerId: id }) as PointerEvent;

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

function load(objects: DesignObject[]) {
  store.getState().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  store.getState().clearHistory();
}

beforeEach(() => {
  store = createAppStore();
  tips = [];
  clearGesture();
  patchInteraction({ measure: null });
});

describe('herramienta Medir', () => {
  it('arrastrar deja la medición y la muestra en vivo; sigue tras soltar', () => {
    const tool = createMeasureTool(deps());
    tool.down(ev(), 10, 10, NONE);
    expect(tool.move(ev(), 11, 10, NONE)).toBe(true); // bajo el umbral: aún no mide
    expect(useInteraction.getState().measure).toBeNull();
    tool.move(ev(), 13, 14, NONE);
    expect(useInteraction.getState().measure).toEqual({ a: [10, 10], b: [13, 14], objectId: null });
    expect(tips.at(-1)).toContain('5');
    expect(useInteraction.getState().gesture).toBe('measure');
    tool.up(ev(), 13, 14, NONE);
    expect(useInteraction.getState().measure).toEqual({ a: [10, 10], b: [13, 14], objectId: null });
    expect(useInteraction.getState().gesture).toBeNull();
    expect(tool.active()).toBe(false);
  });

  it('Mayús fija el ángulo a 0, 45 o 90 grados', () => {
    const tool = createMeasureTool(deps());
    tool.down(ev(), 0, 0, NONE);
    tool.move(ev(), 100, 8, SHIFT);
    const m = useInteraction.getState().measure!;
    expect(m.b[1]).toBeCloseTo(0, 6);
    expect(m.b[0]).toBeCloseTo(Math.hypot(100, 8), 6);
  });

  it('un clic sobre una forma la mide; un clic en el vacío limpia', () => {
    load([rectObj('r', 0, 0, 50, 50)]);
    const tool = createMeasureTool(deps());
    tool.down(ev(), 0, 25, NONE); // sobre el borde del rectángulo
    tool.up(ev(), 0, 25, NONE);
    expect(useInteraction.getState().measure?.objectId).toBe('r');
    tool.down(ev(), 300, 300, NONE);
    tool.up(ev(), 300, 300, NONE);
    expect(useInteraction.getState().measure).toBeNull();
  });

  it('los eventos de otro puntero no son suyos', () => {
    const tool = createMeasureTool(deps());
    expect(tool.move(ev(), 5, 5, NONE)).toBe(false);
    expect(tool.up(ev(), 5, 5, NONE)).toBe(false);
    tool.down(ev(1), 0, 0, NONE);
    expect(tool.move(ev(2), 50, 50, NONE)).toBe(false);
  });
});

describe('espejo sobre una línea dibujada', () => {
  it('arrastrar de A a B crea la copia reflejada y la selecciona', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    const tool = createPickTool(deps());
    startPickMode('mirror');
    tool.down(ev(), 0, -10, NONE);
    tool.move(ev(), 0, 40, NONE);
    expect(useInteraction.getState().draft?.paths).toHaveLength(1);
    tool.up(ev(), 0, 40, NONE);
    const s = store.getState();
    expect(s.doc.objects).toHaveLength(2);
    const b = objectBounds(s.doc.objects[1]!)!;
    expect(b[0][0]).toBeCloseTo(-20, 9);
    expect(b[1][0]).toBeCloseTo(-10, 9);
    expect(s.selectedIds).toEqual([s.doc.objects[1]!.id]);
    expect(useInteraction.getState().pick).toBeNull(); // el modo terminó
    expect(s.past).toHaveLength(1);
  });

  it('dos clics también definen la línea', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    const tool = createPickTool(deps());
    startPickMode('mirror');
    tool.down(ev(), 0, 0, NONE);
    tool.up(ev(), 0, 0, NONE); // clic simple: espera el segundo punto
    expect(useInteraction.getState().pick?.points).toEqual([[0, 0]]);
    expect(store.getState().doc.objects).toHaveLength(1);
    tool.down(ev(), 0, 30, NONE);
    expect(store.getState().doc.objects).toHaveLength(2);
  });

  it('dos puntos iguales no hacen nada', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    const tool = createPickTool(deps());
    startPickMode('mirror');
    tool.down(ev(), 5, 5, NONE);
    tool.up(ev(), 5, 5, NONE);
    tool.down(ev(), 5, 5, NONE);
    expect(store.getState().doc.objects).toHaveLength(1);
  });
});

describe('girar y escalar por dos puntos', () => {
  it('pivote, luego arrastrar la referencia: gira la selección; con Mayús también escala', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    const tool = createPickTool(deps());
    startPickMode('twoPoint');
    tool.down(ev(), 0, 0, NONE); // pivote
    expect(useInteraction.getState().pick?.points).toEqual([[0, 0]]);
    tool.down(ev(), 10, 0, NONE); // referencia
    tool.move(ev(), 0, 10, NONE); // la lleva a 90 grados
    expect(useInteraction.getState().live).not.toBeNull();
    tool.up(ev(), 0, 10, NONE);
    let b = objectBounds(store.getState().doc.objects[0]!)!;
    expect(b[0][0]).toBeCloseTo(-10, 6);
    expect(b[1][0]).toBeCloseTo(0, 6);
    expect(b[0][1]).toBeCloseTo(10, 6);
    expect(b[1][1]).toBeCloseTo(20, 6);
    expect(useInteraction.getState().pick).toBeNull();

    // Con Mayús: referencia a 10 mm del pivote llevada a 20 mm -> el doble de tamaño.
    store.getState().select(['a']);
    startPickMode('twoPoint');
    tool.down(ev(), 100, 100, NONE);
    tool.down(ev(), 110, 100, NONE);
    tool.move(ev(), 120, 100, SHIFT);
    tool.up(ev(), 120, 100, SHIFT);
    b = objectBounds(store.getState().doc.objects[0]!)!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(20, 6);
  });

  it('presionar la referencia sin arrastrar la vuelve a pedir', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    const tool = createPickTool(deps());
    startPickMode('twoPoint');
    tool.down(ev(), 0, 0, NONE);
    tool.down(ev(), 10, 0, NONE);
    tool.up(ev(), 10, 0, NONE);
    expect(useInteraction.getState().pick?.points).toEqual([[0, 0]]);
    expect(store.getState().past).toHaveLength(0);
  });

  it('respeta el interruptor de girar', () => {
    load([rectObj('a', 10, 0, 20, 10)]);
    store.getState().select(['a']);
    store.getState().setTransformAllow({ rotate: false });
    const tool = createPickTool(deps());
    startPickMode('twoPoint');
    tool.down(ev(), 0, 0, NONE);
    tool.down(ev(), 10, 0, NONE);
    tool.move(ev(), 0, 10, NONE);
    tool.up(ev(), 0, 10, NONE);
    expect(store.getState().doc.objects[0]!.transform).toEqual([1, 0, 0, 1, 0, 0]);
  });
});
