import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds, objectPolylines } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { knifeCandidates, previewShapeOp, resolveKnife, runShapeOp, runTrimAt, shapeParams, subpathCount } from './shapeOps';
import { SHAPE_DEFAULTS, useShapeOps } from './shapeOpsUi';
import { useArrangeUi } from './uiStore';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

type Pt = [number, number];

function path(id: string, paths: { points: Pt[]; closed: boolean }[], extra: Partial<DesignObject> = {}): DesignObject {
  return { id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: paths.map((p) => polylineBez(p.points, p.closed)) }, locked: false, hidden: false, group: null, ...extra };
}
const line = (id: string, a: Pt, b: Pt, extra: Partial<DesignObject> = {}) => path(id, [{ points: [a, b], closed: false }], extra);
const rect = (x0: number, y0: number, x1: number, y1: number): { points: Pt[]; closed: boolean } => ({ points: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true });
const square = (id: string, x0: number, y0: number, x1: number, y1: number, extra: Partial<DesignObject> = {}) => path(id, [rect(x0, y0, x1, y1)], extra);

function load(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  s().clearHistory();
}
const ids = () => s().doc.objects.map((o) => o.id);
const reason = (id: string) => registry.availability(id, s());

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  setApi(createMockApi({ latencyMs: 0 }));
  useArrangeUi.setState({ dialog: null, busy: false });
  useShapeOps.setState({
    dialog: null,
    lastPoint: null,
    breakApart: { ...SHAPE_DEFAULTS.breakApart },
    optimize: { ...SHAPE_DEFAULTS.optimize },
    fillet: { ...SHAPE_DEFAULTS.fillet },
    cut: { ...SHAPE_DEFAULTS.cut },
    slots: { ...SHAPE_DEFAULTS.slots },
    trim: { ...SHAPE_DEFAULTS.trim },
  });
});

describe('motivos de deshabilitado', () => {
  const all = ['arrange.trim', 'arrange.breakApart', 'arrange.optimize', 'arrange.fillet', 'arrange.cutShapes', 'arrange.resizeSlots'];

  it('todas piden selección, vectores y objetos sin bloquear', () => {
    const image: DesignObject = { ...square('i', 0, 0, 1, 1), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [1, 1] } };
    load([square('a', 0, 0, 10, 10), image, square('k', 0, 0, 5, 5, { locked: true })]);
    for (const id of all) expect(reason(id), id).toBe('arrange.reason.noSelection');
    s().select(['a', 'i']);
    for (const id of all) expect(reason(id), id).toBe('arrange.reason.needVectors');
    s().select(['a', 'k']);
    for (const id of all) expect(reason(id), id).toBe('arrange.reason.locked');
  });

  it('separar pide algo con varios subtrayectos', () => {
    load([square('a', 0, 0, 10, 10), path('m', [rect(0, 0, 1, 1), rect(5, 5, 6, 6)])]);
    s().select(['a']);
    expect(reason('arrange.breakApart')).toBe('shapes.reason.nothingToBreak');
    s().select(['a', 'm']);
    expect(reason('arrange.breakApart')).toBe(true);
  });

  it('cortar formas pide una cuchilla cerrada y otra forma', () => {
    load([square('a', 0, 0, 10, 10), line('l', [0, 0], [5, 5]), line('l2', [1, 1], [9, 1])]);
    s().select(['a']);
    expect(reason('arrange.cutShapes')).toBe('shapes.reason.cutNeedTwo');
    s().select(['l', 'l2']);
    expect(reason('arrange.cutShapes')).toBe('shapes.reason.cutNoKnife');
    s().select(['l', 'a']);
    expect(reason('arrange.cutShapes')).toBe(true);
  });

  it('ranuras pide formas cerradas y recortar pide un clic previo', () => {
    load([line('l', [0, 0], [5, 5]), square('a', 0, 0, 10, 10)]);
    s().select(['l']);
    expect(reason('arrange.resizeSlots')).toBe('shapes.reason.noClosed');
    expect(reason('arrange.trim')).toBe('shapes.reason.noPoint');
    useShapeOps.getState().setLastPoint([1, 1]);
    expect(reason('arrange.trim')).toBe(true);
  });

  it('no se puede mientras hay una operación en curso', () => {
    load([square('a', 0, 0, 10, 10)]);
    s().select(['a']);
    useArrangeUi.setState({ busy: true });
    expect(reason('arrange.optimize')).toBe('arrange.reason.busy');
  });

  it('los comandos de diálogo lo abren', () => {
    load([path('m', [rect(0, 0, 1, 1), rect(5, 5, 6, 6)])]);
    s().select(['m']);
    void registry.run('arrange.breakApart');
    expect(useShapeOps.getState().dialog).toBe('break_apart');
    void registry.run('arrange.resizeSlots');
    expect(useShapeOps.getState().dialog).toBe('resize_slots');
  });
});

describe('ayudas', () => {
  it('cuenta subtrayectos', () => {
    expect(subpathCount(square('a', 0, 0, 1, 1))).toBe(1);
    expect(subpathCount(path('m', [rect(0, 0, 1, 1), rect(5, 5, 6, 6)]))).toBe(2);
    expect(subpathCount(path('d', [{ points: [[0, 0], [1, 0]], closed: false }, { points: [[0, 5], [1, 5]], closed: false }]))).toBe(2);
  });

  it('elige la cuchilla: la pedida, o la última cerrada seleccionada', () => {
    const objs = [square('a', 0, 0, 1, 1), line('l', [0, 0], [1, 1]), square('b', 5, 5, 6, 6)];
    expect(knifeCandidates(objs).map((o) => o.id)).toEqual(['a', 'b']);
    expect(resolveKnife(objs, 'a', ['a', 'l', 'b'])?.id).toBe('a');
    expect(resolveKnife(objs, null, ['a', 'l', 'b'])?.id).toBe('b');
    expect(resolveKnife(objs, 'l', ['b', 'a'])?.id).toBe('a');
    expect(resolveKnife([line('l', [0, 0], [1, 1])], null, ['l'])).toBeNull();
  });

  it('el radio solo manda los nodos seleccionados si se pide', () => {
    load([square('q', 0, 0, 10, 10)]);
    s().select(['q']);
    s().setNodeSel({ q: [2] });
    expect(shapeParams('fillet', s()).nodes).toBeUndefined();
    useShapeOps.getState().patchFillet({ onlyNodes: true, radius: 2 });
    expect(shapeParams('fillet', s())).toEqual({ radius: 2, style: 'round', nodes: [[10, 10]] });
  });
});

describe('recortar', () => {
  it('borra el tramo hasta la intersección y es un paso de deshacer', async () => {
    load([line('a', [0, 0], [10, 0]), line('b', [5, -5], [5, 5])]);
    s().select(['a', 'b']);
    const before = s().doc;
    expect(await runTrimAt(s(), [2, 0])).toBe(true);
    expect(ids()).toEqual(['trim-1', 'b']);
    const bb = objectBounds(s().doc.objects[0]!)!;
    expect(bb[0][0]).toBeCloseTo(5);
    expect(bb[1][0]).toBeCloseTo(10);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc).toBe(before);
  });

  it('con keepRemoved deja el tramo como objeto aparte', async () => {
    load([line('a', [0, 0], [10, 0]), line('b', [5, -5], [5, 5])]);
    s().select(['a', 'b']);
    await runTrimAt(s(), [2, 0], { keepRemoved: true });
    expect(ids()).toEqual(['trim-1', 'trim-2', 'b']);
  });

  it('avisa si no hay trazo cerca y no cambia nada', async () => {
    load([line('a', [0, 0], [10, 0])]);
    s().select(['a']);
    const before = s().doc;
    expect(await runTrimAt(s(), [5, 50], { maxDist: 1 })).toBe(false);
    expect(s().doc).toBe(before);
  });

  it('una línea aislada se borra entera', async () => {
    load([line('a', [0, 0], [10, 0]), line('b', [0, 5], [10, 5])]);
    s().select(['a', 'b']);
    expect(await runTrimAt(s(), [3, 5])).toBe(true);
    expect(ids()).toEqual(['a']);
  });
});

describe('separar', () => {
  it('da un objeto por subtrayecto y selecciona los nuevos', async () => {
    load([path('m', [rect(0, 0, 10, 10), rect(20, 0, 30, 10), rect(40, 0, 50, 10)]), square('otro', 100, 0, 110, 10)]);
    s().select(['m']);
    expect(await runShapeOp(s(), 'break_apart')).toBe(true);
    expect(ids()).toEqual(['break-1', 'break-2', 'break-3', 'otro']);
    expect(s().selectedIds).toEqual(['break-1', 'break-2', 'break-3']);
    expect(s().past).toHaveLength(1);
  });

  it('puede conservar los huecos con su contorno', async () => {
    load([path('m', [rect(0, 0, 20, 20), rect(5, 5, 15, 15), rect(30, 0, 40, 10)])]);
    s().select(['m']);
    useShapeOps.getState().patchBreakApart({ keepHoles: true });
    expect(await runShapeOp(s(), 'break_apart')).toBe(true);
    expect(ids()).toHaveLength(2);
    expect(objectPolylines(s().doc.objects[0]!)).toHaveLength(2);
  });
});

describe('optimizar', () => {
  const dense = (): DesignObject => {
    const points: Pt[] = [];
    for (let i = 0; i <= 100; i++) points.push([i / 10, 0]);
    points.push([10, 10]);
    return path('d', [{ points, closed: true }]);
  };

  it('la vista previa da los nodos antes y después sin cambiar el documento', async () => {
    load([dense()]);
    s().select(['d']);
    const before = s().doc;
    const stats = await previewShapeOp(s(), 'optimize', shapeParams('optimize', s()));
    expect(stats.nodes_before).toBe(102);
    expect(stats.nodes_after).toBe(3);
    expect(s().doc).toBe(before);
  });

  it('aplica y deja una forma con menos nodos', async () => {
    load([dense()]);
    s().select(['d']);
    expect(await runShapeOp(s(), 'optimize')).toBe(true);
    expect(ids()).toEqual(['opt-1']);
    expect(objectPolylines(s().doc.objects[0]!)[0]!.points).toHaveLength(3);
  });

  it('avisa si ya está optimizado', async () => {
    load([square('q', 0, 0, 10, 10)]);
    s().select(['q']);
    expect(await runShapeOp(s(), 'optimize')).toBe(false);
    expect(s().past).toHaveLength(0);
  });
});

describe('redondear esquinas', () => {
  it('redondea las cuatro esquinas de un cuadrado', async () => {
    load([square('q', 0, 0, 10, 10)]);
    s().select(['q']);
    useShapeOps.getState().patchFillet({ radius: 2 });
    expect(await runShapeOp(s(), 'fillet')).toBe(true);
    const o = s().doc.objects[0]!;
    expect(o.id).toBe('fillet-1');
    expect(o.kind.type === 'vector' && o.kind.paths[0]!.some((seg) => seg.type === 'cubic_to')).toBe(true);
    expect(objectBounds(o)![1]).toEqual([10, 10]);
  });

  it('con nodos seleccionados solo redondea esas esquinas', async () => {
    load([square('q', 0, 0, 10, 10)]);
    s().select(['q']);
    s().setNodeSel({ q: [0] });
    useShapeOps.getState().patchFillet({ radius: 2, onlyNodes: true });
    expect(await runShapeOp(s(), 'fillet')).toBe(true);
    const o = s().doc.objects[0]!;
    expect(o.kind.type === 'vector' && o.kind.paths[0]!.filter((seg) => seg.type === 'cubic_to')).toHaveLength(1);
  });

  it('rechaza un radio no válido sin cambiar nada', async () => {
    load([square('q', 0, 0, 10, 10)]);
    s().select(['q']);
    useShapeOps.getState().patchFillet({ radius: 0 });
    const before = s().doc;
    expect(await runShapeOp(s(), 'fillet')).toBe(false);
    expect(s().doc).toBe(before);
  });
});

describe('cortar formas', () => {
  it('parte la forma, consume la cuchilla y es un paso de deshacer', async () => {
    load([square('pieza', 0, 0, 10, 10), square('cuchilla', 5, -5, 15, 15)]);
    s().select(['pieza', 'cuchilla']);
    expect(await runShapeOp(s(), 'cut_shapes')).toBe(true);
    expect(ids()).toEqual(['cut-1', 'cut-2']);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(ids()).toEqual(['pieza', 'cuchilla']);
  });

  it('puede conservar la cuchilla y elegir otra distinta de la última', async () => {
    load([square('cuchilla', 5, -5, 15, 15), square('pieza', 0, 0, 10, 10)]);
    s().select(['cuchilla', 'pieza']);
    useShapeOps.getState().patchCut({ keepKnife: true, knifeId: 'cuchilla' });
    expect(await runShapeOp(s(), 'cut_shapes')).toBe(true);
    expect(ids()).toContain('cuchilla');
    expect(ids()).not.toContain('pieza');
    expect(s().selectedIds[0]).toBe('cuchilla');
  });

  it('si la cuchilla no cruza nada, no cambia nada', async () => {
    load([square('pieza', 0, 0, 10, 10), square('cuchilla', 50, 50, 60, 60)]);
    s().select(['pieza', 'cuchilla']);
    const before = s().doc;
    expect(await runShapeOp(s(), 'cut_shapes')).toBe(false);
    expect(s().doc).toBe(before);
  });
});

describe('redimensionar ranuras', () => {
  it('cambia el ancho de un agujero rectangular y lista los anchos', async () => {
    load([square('marco', 0, 0, 50, 50), square('ranura', 10, 10, 30, 13)]);
    s().select(['marco', 'ranura']);
    useShapeOps.getState().patchSlots({ fromWidth: 3, toWidth: 4 });
    const stats = await previewShapeOp(s(), 'resize_slots', shapeParams('resize_slots', s()));
    expect(stats.count).toBe(1);
    expect(stats.widths.map(([w]) => w)).toEqual([3, 20]);
    expect(await runShapeOp(s(), 'resize_slots')).toBe(true);
    const bb = objectBounds(s().doc.objects.find((o) => o.id === 'slot-1')!)!;
    expect(bb[1][1] - bb[0][1]).toBeCloseTo(4);
    expect(s().past).toHaveLength(1);
  });

  it('avisa si no hay ranuras de ese ancho', async () => {
    load([square('marco', 0, 0, 50, 50), square('ranura', 10, 10, 30, 13)]);
    s().select(['marco', 'ranura']);
    useShapeOps.getState().patchSlots({ fromWidth: 7, toWidth: 8 });
    const before = s().doc;
    expect(await runShapeOp(s(), 'resize_slots')).toBe(false);
    expect(s().doc).toBe(before);
  });
});
