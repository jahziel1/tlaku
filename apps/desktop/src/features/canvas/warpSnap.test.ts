import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Point } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { clearGesture, useInteraction } from './interaction';
import { createSnapping } from './toolSnapping';
import type { Mods, ToolDeps } from './toolDeps';
import { rectObj } from './testUtil';
import { resetWarpEdit, useWarpEdit } from './warpEditStore';
import { createWarpTool } from './warpTool';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const ALT: Mods = { ...NONE, alt: true };
const SHIFT: Mods = { ...NONE, shift: true };
const ev = () => ({ pointerId: 1 }) as PointerEvent;

let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();

/** Dependencias con el imán de verdad (el mismo de las demás herramientas). */
function deps(): ToolDeps {
  const view = () => ({ zoom: 1, panX: 0, panY: 0 });
  const snap = createSnapping(() => store.getState(), view);
  return {
    el: {} as HTMLElement,
    st: () => store.getState(),
    viewOf: view,
    world: (sx, sy) => [sx, sy] as Point,
    capture: () => {},
    release: () => {},
    snapOpts: snap.snapOpts,
    targetsFor: snap.targetsFor,
    setTip: () => {},
    snapTo: snap.snapTo,
  };
}

function start(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  s().clearHistory();
  s().select(['r']);
  s().setTool('warp');
  const tool = createWarpTool(deps());
  tool.begin();
  return tool;
}

const corner = (i: number) => useWarpEdit.getState().session!.corners[i]!;

beforeEach(() => {
  store = createAppStore();
  store.setState({ zoom: 1, panX: 0, panY: 0, viewport: { width: 1200, height: 800 }, snapEnabled: true, snapGrid: false, snapObjects: true });
  resetWarpEdit();
  clearGesture();
});

describe('Deformar: imán en las manijas (D58)', () => {
  it('la manija se pega al nodo de otro objeto cercano y marca el punto', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40), rectObj('o', 60, 10, 90, 40)]);
    tool.down(ev(), 40, 40);
    // A 0,4 mm del nodo (60, 10) del otro rectángulo.
    tool.move(ev(), 59.8, 10.3, NONE);
    expect(corner(2)).toEqual([60, 10]);
    expect(useInteraction.getState().snapMark).not.toBeNull();
    tool.up(ev());
    expect(useInteraction.getState().snapMark).toBeNull();
  });

  it('Alt desactiva el imán', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40), rectObj('o', 60, 10, 90, 40)]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 59.8, 10.3, ALT);
    expect(corner(2)).toEqual([59.8, 10.3]);
    expect(useInteraction.getState().snapMark).toBeNull();
  });

  it('sin imán activado la manija queda donde está el puntero', () => {
    store.setState({ snapEnabled: false });
    const tool = start([rectObj('r', 10, 10, 40, 40), rectObj('o', 60, 10, 90, 40)]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 59.8, 10.3, NONE);
    expect(corner(2)).toEqual([59.8, 10.3]);
  });

  it('se pega a una guía del documento', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40)]);
    s().setGuides([{ axis: 'x', pos: 75 }]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 75.3, 30.123, NONE);
    expect(corner(2)[0]).toBe(75);
    expect(corner(2)[1]).toBeCloseTo(30.123);
  });

  it('se pega a la rejilla', () => {
    store.setState({ snapGrid: true, snapObjects: false });
    const tool = start([rectObj('r', 10, 10, 40, 40)]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 55.02, 47.01, NONE);
    const [x, y] = corner(2);
    // Cae sobre una línea de la rejilla (múltiplo entero de milímetro), no donde estaba el puntero.
    expect(x).not.toBe(55.02);
    expect(Math.abs(x - Math.round(x))).toBeLessThan(1e-9);
    expect(Math.abs(y - Math.round(y))).toBeLessThan(1e-9);
  });

  it('lo que se deforma no atrae a su propia manija', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40)]);
    tool.down(ev(), 40, 40);
    // Cerca de la esquina original (10, 10) del propio objeto: no se pega.
    tool.move(ev(), 10.3, 10.3, NONE);
    expect(corner(2)).toEqual([10.3, 10.3]);
  });

  it('con Mayús el imán solo ajusta el eje libre', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40), rectObj('o', 60, 10, 90, 40)]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 59.7, 41, SHIFT);
    expect(corner(2)).toEqual([60, 40]);
  });

  it('Esc a mitad de arrastre limpia la marca del imán', () => {
    const tool = start([rectObj('r', 10, 10, 40, 40), rectObj('o', 60, 10, 90, 40)]);
    tool.down(ev(), 40, 40);
    tool.move(ev(), 59.8, 10.3, NONE);
    expect(useInteraction.getState().snapMark).not.toBeNull();
    tool.cancel();
    expect(useInteraction.getState().snapMark).toBeNull();
    expect(corner(2)).toEqual([40, 40]);
  });
});
