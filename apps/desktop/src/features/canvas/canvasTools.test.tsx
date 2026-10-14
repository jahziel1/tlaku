import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject, Point } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { useDesignUi } from '@/features/arrange/designUi';
import { useShapeOps, SHAPE_DEFAULTS } from '@/features/arrange/shapeOpsUi';
import { useArrangeUi } from '@/features/arrange/uiStore';
import { setLanguage } from '@/i18n';
import { objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { cornersOf, createFilletTool, nearestCorner, previewCorner, setFilletRevert, useFilletTool } from './filletTool';
import { clearGesture, useInteraction } from './interaction';
import { rectObj, lineObj } from './testUtil';
import { ToolHintView, toolHintKey } from './ToolHint';
import type { Mods, ToolDeps } from './toolDeps';
import { createWarpTool } from './warpTool';
import { isIdentity } from './warpEdit';
import { registerWarpActions, resetWarpEdit, useWarpEdit, canApplyWarp } from './warpEditStore';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const ALT: Mods = { ...NONE, alt: true };
const SHIFT: Mods = { ...NONE, shift: true };
const ev = (id = 1) => ({ pointerId: id }) as PointerEvent;

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

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
    setTip: () => {},
    snapTo: (p) => p,
  };
}

function load(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  s().clearHistory();
}

beforeEach(() => {
  setLanguage('es');
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  setApi(createMockApi({ latencyMs: 0 }));
  store.setState({ zoom: 1, panX: 0, panY: 0 });
  useArrangeUi.setState({ dialog: null, busy: false });
  useDesignUi.setState({ busy: false });
  useShapeOps.setState({ fillet: { ...SHAPE_DEFAULTS.fillet } });
  setFilletRevert(false);
  resetWarpEdit();
  clearGesture();
});

describe('Deformar en el lienzo', () => {
  function start(objects: DesignObject[] = [rectObj('r', 10, 10, 40, 40)], ids = ['r']) {
    load(objects);
    s().select(ids);
    s().setTool('warp');
    const tool = createWarpTool(deps());
    const unregister = registerWarpActions({ apply: () => tool.apply(), reset: () => tool.reset(), setMode: (m) => tool.setMode(m), canApply: () => tool.canApply() });
    tool.begin();
    return { tool, unregister };
  }

  it('al activarse arma una malla sobre la caja de la selección y dibuja sus manijas', () => {
    const { tool, unregister } = start();
    const session = useWarpEdit.getState().session!;
    expect(session.bounds).toEqual([[10, 10], [40, 40]]);
    expect(session.mode).toBe('bilinear');
    expect(useInteraction.getState().draft?.grips).toHaveLength(4);
    tool.setMode('mesh');
    expect(useInteraction.getState().draft?.grips).toHaveLength(16);
    unregister();
  });

  it('sin selección no hay malla y explica por qué', () => {
    load([rectObj('r', 10, 10, 40, 40)]);
    s().setTool('warp');
    const tool = createWarpTool(deps());
    tool.begin();
    expect(useWarpEdit.getState().session).toBeNull();
    expect(useWarpEdit.getState().reason).toBe('arrange.reason.noSelection');
  });

  it('arrastrar una esquina deforma en vivo sin tocar el documento', () => {
    const { tool, unregister } = start();
    const before = s().doc;
    tool.hover(40, 40);
    expect(useWarpEdit.getState().hot).toBe(2);
    expect(useInteraction.getState().cursor).toBe('pointer');
    tool.down(ev(), 40, 40);
    expect(useInteraction.getState().gesture).toBe('warp');
    expect(tool.move(ev(), 50, 45, NONE)).toBe(true);
    expect(useWarpEdit.getState().session!.corners[2]).toEqual([50, 45]);
    expect(useInteraction.getState().draft?.paths.length).toBeGreaterThan(0);
    expect(s().doc).toBe(before);
    expect(tool.up(ev())).toBe(true);
    expect(useInteraction.getState().gesture).toBeNull();
    expect(useWarpEdit.getState().session!.corners[2]).toEqual([50, 45]);
    unregister();
  });

  it('Mayús mueve solo en horizontal o vertical', () => {
    const { tool, unregister } = start();
    tool.down(ev(), 40, 40);
    tool.move(ev(), 55, 42, SHIFT);
    expect(useWarpEdit.getState().session!.corners[2]).toEqual([55, 40]);
    unregister();
  });

  it('Esc a mitad de un arrastre devuelve la manija a donde estaba', () => {
    const { tool, unregister } = start();
    tool.down(ev(), 10, 10);
    tool.move(ev(), 3, 4, NONE);
    expect(useWarpEdit.getState().session!.corners[0]).toEqual([3, 4]);
    tool.cancel();
    expect(useWarpEdit.getState().session!.corners[0]).toEqual([10, 10]);
    expect(tool.active()).toBe(false);
    unregister();
  });

  it('hacer clic fuera de las manijas no arrastra nada', () => {
    const { tool, unregister } = start();
    tool.down(ev(), 25, 25);
    expect(tool.active()).toBe(false);
    unregister();
  });

  it('Aplicar escribe el resultado en UN paso de deshacer y vuelve a Seleccionar', async () => {
    const { tool, unregister } = start();
    const original = s().doc.objects[0]!;
    tool.down(ev(), 40, 40);
    tool.move(ev(), 55, 52, NONE);
    tool.up(ev());
    expect(tool.canApply()).toBe(true);
    expect(await tool.apply()).toBe(true);
    expect(s().activeTool).toBe('select');
    const warped = s().doc.objects[0]!;
    expect(warped.id).toBe('r');
    expect(warped.kind).not.toEqual(original.kind);
    const box = objectBounds(warped)!;
    expect(box[1][0]).toBeGreaterThan(50);
    expect(s().past.length).toBe(1);
    s().undo();
    expect(s().doc.objects[0]).toEqual(original);
    unregister();
  });

  it('la malla 4×4 se aplica con el motor y también es un solo paso', async () => {
    const { tool, unregister } = start();
    tool.setMode('mesh');
    tool.down(ev(), 20, 20);
    tool.move(ev(), 22, 29, NONE);
    tool.up(ev());
    expect(await tool.apply()).toBe(true);
    expect(s().past.length).toBe(1);
    unregister();
  });

  it('sin cambios no hay nada que aplicar; Restablecer deja la malla en la caja', async () => {
    const { tool, unregister } = start();
    expect(tool.canApply()).toBe(false);
    expect(await tool.apply()).toBe(false);
    expect(s().past.length).toBe(0);
    tool.down(ev(), 10, 10);
    tool.move(ev(), 0, 0, NONE);
    tool.up(ev());
    expect(isIdentity(useWarpEdit.getState().session!)).toBe(false);
    tool.reset();
    expect(isIdentity(useWarpEdit.getState().session!)).toBe(true);
    unregister();
  });

  it('la perspectiva no deja soltar una esquina que vuelve el cuadrilátero cóncavo', () => {
    const { tool, unregister } = start();
    tool.setMode('perspective');
    tool.down(ev(), 40, 40);
    tool.move(ev(), 12, 12, NONE);
    expect(useWarpEdit.getState().session!.corners[2]).toEqual([40, 40]);
    unregister();
  });

  it('cambiar de selección rehace la malla; los comandos reflejan el estado', async () => {
    const { tool, unregister } = start([rectObj('r', 10, 10, 40, 40), rectObj('q', 100, 100, 120, 130)], ['r']);
    s().select(['q']);
    tool.refresh();
    expect(useWarpEdit.getState().session!.bounds).toEqual([[100, 100], [120, 130]]);
    expect(registry.availability('warp.apply', s())).toBe('canvas.reason.noWarpChange');
    expect(registry.availability('warp.reset', s())).toBe(true);
    s().setTool('select');
    expect(registry.availability('warp.reset', s())).toBe('canvas.reason.noWarp');
    expect(canApplyWarp()).toBe(false);
    unregister();
  });
});

describe('Redondear esquinas con clic', () => {
  const tool = () => createFilletTool({ ...deps(), pick: (p, tol) => s().doc.objects.find((o) => {
    const b = objectBounds(o)!;
    return p[0] >= b[0][0] - tol && p[0] <= b[1][0] + tol && p[1] >= b[0][1] - tol && p[1] <= b[1][1] + tol;
  }) ?? null });

  it('un rectángulo tiene cuatro esquinas; una recta o los extremos abiertos, ninguna', () => {
    expect(cornersOf(rectObj('r', 0, 0, 10, 10)).map((c) => c.p)).toEqual([[0, 0], [10, 0], [10, 10], [0, 10]]);
    expect(cornersOf(lineObj('l', [0, 0], [10, 0]))).toEqual([]);
    const text = { ...rectObj('t', 0, 0, 1, 1), kind: { type: 'text' } } as unknown as DesignObject;
    expect(cornersOf(text)).toEqual([]);
  });

  it('elige el vértice más cercano dentro de la tolerancia', () => {
    const r = rectObj('r', 0, 0, 10, 10);
    expect(nearestCorner(r, [9.6, 10.3], 1)?.p).toEqual([10, 10]);
    expect(nearestCorner(r, [5, 5], 1)).toBeNull();
    expect(nearestCorner(r, [10.9, 10], 1)?.p).toEqual([10, 10]);
  });

  it('la vista previa empieza y termina sobre las dos rectas', () => {
    const c = nearestCorner(rectObj('r', 0, 0, 10, 10), [10, 10], 1)!;
    const pts = previewCorner(c, 'round', 2).flatMap((seg) => (seg.type === 'move_to' || seg.type === 'line_to' ? [seg.to] : []));
    // En (10, 10) llegan el lado de (10, 0) y sale el de (0, 10).
    expect(pts[0]![0]).toBeCloseTo(10, 6);
    expect(pts[0]![1]).toBeCloseTo(8, 6);
    const last = pts[pts.length - 1]!;
    expect(last[0]).toBeCloseTo(8, 6);
    expect(last[1]).toBeCloseTo(10, 6);
    expect(previewCorner(c, 'chamfer', 2)).toHaveLength(2);
  });

  it('un clic redondea solo esa esquina, en un solo paso, y deshacer la devuelve', async () => {
    load([rectObj('r', 0, 0, 10, 10)]);
    s().setTool('fillet');
    useShapeOps.setState({ fillet: { radius: 2, style: 'round', onlyNodes: false } });
    const t = tool();
    t.hover(10, 10, NONE);
    expect(useFilletTool.getState().hover?.p).toEqual([10, 10]);
    expect(useInteraction.getState().cursor).toBe('pointer');
    t.down(10, 10, NONE);
    await vi_waitFor(() => s().past.length === 1);
    const out = s().doc.objects;
    expect(out).toHaveLength(1);
    const b = objectBounds(out[0]!)!;
    expect(b[1]).toEqual([10, 10]);
    // Solo se tocó una esquina: las otras tres siguen vivas.
    expect(cornersOf(out[0]!).map((c) => c.p)).toHaveLength(3);
    s().undo();
    expect(s().doc.objects[0]!.id).toBe('r');
  });

  it('en modo Revertir (o con Alt) devuelve a punta la esquina redondeada', async () => {
    load([rectObj('r', 0, 0, 10, 10)]);
    s().setTool('fillet');
    useShapeOps.setState({ fillet: { radius: 2, style: 'round', onlyNodes: false } });
    const t = tool();
    t.down(10, 10, NONE);
    await vi_waitFor(() => s().past.length === 1);
    expect(cornersOf(s().doc.objects[0]!)).toHaveLength(3);
    // Alt invierte el modo sin tocar la preferencia.
    t.down(9.5, 9.5, ALT);
    await vi_waitFor(() => s().past.length === 2);
    expect(cornersOf(s().doc.objects[0]!)).toHaveLength(4);
    // Revertir otra vez: no hay nada que revertir y no se agrega un paso.
    setFilletRevert(true);
    t.down(9.5, 9.5, NONE);
    await new Promise((r) => setTimeout(r, 20));
    expect(s().past.length).toBe(2);
    s().undo();
    expect(cornersOf(s().doc.objects[0]!)).toHaveLength(3);
  });

  it('un chaflán no se revierte: se avisa y queda el deshacer normal', async () => {
    load([rectObj('r', 0, 0, 10, 10)]);
    s().setTool('fillet');
    useShapeOps.setState({ fillet: { radius: 2, style: 'chamfer', onlyNodes: false } });
    const t = tool();
    t.down(10, 10, NONE);
    await vi_waitFor(() => s().past.length === 1);
    setFilletRevert(true);
    t.down(9, 9, NONE);
    await new Promise((r) => setTimeout(r, 20));
    expect(s().past.length).toBe(1);
    expect(s().notices.at(-1)?.text).toContain('chaflán');
  });

  it('hacer clic donde no hay forma avisa y no cambia nada', () => {
    load([rectObj('r', 0, 0, 10, 10)]);
    s().setTool('fillet');
    tool().down(300, 300, NONE);
    expect(s().past.length).toBe(0);
  });
});

describe('ayuda en la barra de estado', () => {
  it('cada herramienta nueva tiene su ayuda y el resto ninguna', () => {
    expect(toolHintKey('warp', 'mesh', false)).toBe('canvas.warp.hint.mesh');
    expect(toolHintKey('warp', null, false)).toBe('canvas.warp.hint.none');
    expect(toolHintKey('fillet', null, true)).toBe('canvas.fillet.hint.revert');
    expect(toolHintKey('select', null, false)).toBeNull();
  });
});

describe('barra de herramientas y comandos', () => {
  it('Mayús+W y Mayús+F activan las herramientas', async () => {
    expect(registry.match('Shift+W')?.id).toBe('tool.warp');
    expect(registry.match('Shift+F')?.id).toBe('tool.fillet');
    await registry.run('tool.warp');
    expect(s().activeTool).toBe('warp');
    await registry.run('tool.fillet');
    expect(s().activeTool).toBe('fillet');
  });

  it('alternar revertir es un comando solo disponible con la herramienta', async () => {
    expect(registry.availability('fillet.revert', s())).toBe('canvas.reason.noFillet');
    s().setTool('fillet');
    expect(registry.availability('fillet.revert', s())).toBe(true);
    await registry.run('fillet.revert');
    expect(useFilletTool.getState().revert).toBe(true);
  });

  it('la ayuda se pinta solo con una herramienta que la tiene', () => {
    expect(renderToString(<ToolHintView tool="select" mode={null} revert={false} />)).toBe('');
    expect(renderToString(<ToolHintView tool="warp" mode="mesh" revert={false} />)).toContain('16 puntos');
    expect(renderToString(<ToolHintView tool="warp" mode={null} revert={false} />)).toContain('Deformar: selecciona');
    expect(renderToString(<ToolHintView tool="fillet" mode={null} revert />)).toContain('Revertir');
    setLanguage('en');
    expect(renderToString(<ToolHintView tool="fillet" mode={null} revert={false} />)).toContain('Fillet: pick the radius');
  });
});

/** Espera a que la condición se cumpla (las operaciones del motor simulado son asíncronas). */
async function vi_waitFor(cond: () => boolean): Promise<void> {
  for (let i = 0; i < 100; i++) {
    if (cond()) return;
    await new Promise((r) => setTimeout(r, 5));
  }
  throw new Error('la condición no se cumplió a tiempo');
}
