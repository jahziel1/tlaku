import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { ellipseBez, flattenBez } from '@/lib/bezpath';
import { applyAffine, objectBounds, objectPolylines } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { lineObj, rectObj } from '@/features/canvas/testUtil';
import { createAppStore } from '@/store';
import { buildAlongPath, contentCenter, pathOfUnit, planAlongPath, samplePath, stationsOf, DEFAULT_ALONG } from './alongPath';
import { circleObject, flattenArrays, operandsOf, replaceObjects, requestHull, requestWarp } from './designOps';
import { arrayOf, draftOf, useDesignUi, DEFAULT_VIRTUAL } from './designUi';
import { DEFAULT_DOCK, planDock, dockDoc } from './dock';
import { selectionUnits } from './units';
import { envelopeOf, isNoWarp, NO_WARP, presetParams, WARP_PRESETS } from './warpPresets';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

function load(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
  s().clearHistory();
}
const find = (id: string) => s().doc.objects.find((o) => o.id === id)!;
const near = (a: readonly number[], b: readonly number[], d = 6) => a.forEach((v, i) => expect(v).toBeCloseTo(b[i]!, d));

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  setApi(createMockApi({ latencyMs: 0 }));
  useDesignUi.setState({ dialog: null, busy: false });
});

describe('deformar: presets y envolvente', () => {
  const box: [[number, number], [number, number]] = [[0, 0], [40, 20]];

  it('sin parámetros no deforma; todos los presets sí', () => {
    expect(isNoWarp(NO_WARP)).toBe(true);
    for (const p of WARP_PRESETS) expect(isNoWarp(presetParams(p, 30, box)), p).toBe(false);
    expect(isNoWarp(presetParams('archUp', 0, box))).toBe(true);
  });

  it('el arco hacia arriba curva ambos bordes horizontales hacia arriba la misma flecha', () => {
    const p = presetParams('archUp', 50, box);
    const env = envelopeOf(p, box);
    // flecha = 50 % de la mitad del alto = 5 mm: el punto medio de cada borde sube 5 mm.
    const mid = (e: typeof env.bottom) => 0.125 * e[0][1] + 0.375 * e[1][1] + 0.375 * e[2][1] + 0.125 * e[3][1];
    expect(mid(env.bottom)).toBeCloseTo(5, 9);
    expect(mid(env.top)).toBeCloseTo(25, 9);
    expect(env.left[0]).toEqual([0, 0]);
    expect(env.right[3]).toEqual([40, 20]);
  });

  it('el abombado y el pellizco son opuestos; la perspectiva estrecha el borde de arriba', () => {
    expect(presetParams('bulge', 40, box).bulge.map((v) => -v)).toEqual(presetParams('pinch', 40, box).bulge);
    const env = envelopeOf(presetParams('perspective', 50, box), box);
    expect(env.top[0][0]).toBeCloseTo(10, 9);
    expect(env.top[3][0]).toBeCloseTo(30, 9);
    expect(env.bottom[0][0]).toBe(0);
    expect(env.bottom[3][0]).toBe(40);
  });

  it('la normal "hacia afuera" de cada borde apunta afuera', () => {
    const env = envelopeOf({ corners: NO_WARP.corners, bulge: [3, 3, 3, 3] }, box);
    expect(env.bottom[1][1]).toBeLessThan(0);
    expect(env.top[1][1]).toBeGreaterThan(20);
    expect(env.left[1][0]).toBeLessThan(0);
    expect(env.right[1][0]).toBeGreaterThan(40);
  });

  it('el motor simulado deforma y conserva ids y grupos', async () => {
    load([rectObj('a', 0, 0, 40, 20, { group: 'g1' }), rectObj('b', 5, 5, 10, 10, { group: 'g1' })]);
    s().select(['a', 'b']);
    const units = selectionUnits(s().doc, s().selectedIds);
    const out = await requestWarp(units, presetParams('perspective', 50, box));
    expect(out.map((o) => o.id)).toEqual(['a', 'b']);
    expect(out[0]!.group).toBe('g1');
    const top = objectPolylines(out[0]!).flatMap((p) => p.points).filter((p) => p[1] > 19.99);
    expect(Math.min(...top.map((p) => p[0]))).toBeCloseTo(10, 6);
    expect(Math.max(...top.map((p) => p[0]))).toBeCloseTo(30, 6);
  });

  it('deformar con el diálogo sustituye los objetos y se deshace', async () => {
    load([rectObj('a', 0, 0, 40, 20)]);
    s().select(['a']);
    const before = find('a');
    const units = selectionUnits(s().doc, s().selectedIds);
    const results = await requestWarp(units, presetParams('slant', 50, box));
    s().applyDocChange('design.history.warp', (doc) => replaceObjects(doc, results));
    expect(find('a')).not.toEqual(before);
    s().undo();
    expect(find('a')).toEqual(before);
  });

  it('las imágenes no se pueden deformar', async () => {
    const img: DesignObject = { ...rectObj('img', 0, 0, 10, 10), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [10, 10] } };
    load([img]);
    s().select(['img']);
    await expect(requestWarp(selectionUnits(s().doc, s().selectedIds), presetParams('bulge', 30, box))).rejects.toThrow(/imágenes/);
  });
});

describe('contorno de banda elástica y círculo tangente (motor simulado)', () => {
  it('el contorno convexo envuelve dos cuadrados', async () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 30, 10)]);
    s().select(['a', 'b']);
    const [hull] = await requestHull(selectionUnits(s().doc, s().selectedIds), null, null);
    expect(hull!.kind.type).toBe('vector');
    const b = objectBounds(hull!)!;
    near(b[0], [0, 0]);
    near(b[1], [30, 10]);
  });

  it('el contorno cóncavo con arista corta se mete en el hueco de una L', async () => {
    load([rectObj('a', 0, 0, 30, 5), rectObj('b', 0, 5, 5, 30)]);
    s().select(['a', 'b']);
    const units = selectionUnits(s().doc, s().selectedIds);
    const area = (o: DesignObject) => Math.abs(objectPolylines(o)[0]!.points.reduce((acc, p, i, all) => acc + (p[0] * all[(i + 1) % all.length]![1] - all[(i + 1) % all.length]![0] * p[1]), 0)) / 2;
    const [convex] = await requestHull(units, null, null);
    const [concave] = await requestHull(units, 12, null);
    expect(area(concave!)).toBeLessThan(area(convex!) - 1);
  });

  it('círculo tangente a dos círculos y a tres', async () => {
    const circle = (id: string, cx: number, cy: number, r: number): DesignObject => circleObject(id, '#ff0000', { center: [cx, cy], radius: r });
    const api = createMockApi({ latencyMs: 0 });
    const two = await api.tangentCircles({ objects: [circle('a', 0, 0, 10), circle('b', 30, 0, 10)], radius: 10 });
    expect(two).toHaveLength(2);
    for (const c of two) {
      expect(Math.hypot(c.center[0], c.center[1])).toBeCloseTo(20, 6);
      expect(Math.hypot(c.center[0] - 30, c.center[1])).toBeCloseTo(20, 6);
    }
    const h = (40 * Math.sqrt(3)) / 2;
    const three = await api.tangentCircles({ objects: [circle('a', 0, 0, 10), circle('b', 40, 0, 10), circle('c', 20, h, 10)] });
    const inner = three.find((c) => Math.hypot(c.center[0] - 20, c.center[1] - h / 3) < 1e-6);
    expect(inner?.radius).toBeCloseTo(40 / Math.sqrt(3) - 10, 6);
    await expect(api.tangentCircles({ objects: [circle('a', 0, 0, 10), circle('b', 30, 0, 10)] })).rejects.toThrow(/radio/);
  });

  it('circleObject crea una elipse con sus parámetros, centrada en el punto', () => {
    const o = circleObject('c', '#ff0000', { center: [12, 7], radius: 3 });
    expect(o.shape).toEqual({ type: 'ellipse', rx: 3, ry: 3 });
    const b = objectBounds(o)!;
    near(b[0], [9, 4]);
    near(b[1], [15, 10]);
    expect(flattenBez(ellipseBez(0, 0, 3, 3)).length).toBeGreaterThan(0);
  });
});

describe('copiar a lo largo de un trazo', () => {
  it('reparte N copias de extremo a extremo en un trayecto abierto y gira según la tangente', () => {
    load([rectObj('cuadro', 0, 0, 2, 2), lineObj('trazo', [10, 0], [10, 40])]);
    s().select(['cuadro', 'trazo']);
    const units = selectionUnits(s().doc, s().selectedIds);
    const path = samplePath(...(Object.values(pathOfUnit(units[1]!)!) as [[number, number][], boolean]))!;
    expect(path.length).toBeCloseTo(40, 9);
    const ref = contentCenter([units[0]!])!;
    near(ref, [1, 1]);
    const plan = planAlongPath(ref, path, { ...DEFAULT_ALONG, count: 5, rotate: true });
    expect(plan.issue).toBeNull();
    expect(plan.matrices).toHaveLength(5);
    expect(plan.anchors.map((p) => p[1])).toEqual([0, 10, 20, 30, 40]);
    // La tangente apunta hacia arriba (90°): el centro del cuadrado cae en el ancla y el cuadrado gira 90°.
    near(applyAffine(plan.matrices[2]!, [1, 1]), [10, 20]);
    near(applyAffine(plan.matrices[2]!, [2, 1]), [10, 21]);
  });

  it('sin girar solo traslada; por distancia incluye el inicio y no se pasa del final', () => {
    const path = samplePath([[0, 0], [100, 0]], false)!;
    expect(stationsOf(path, { mode: 'spacing', count: 0, spacing: 30 })).toEqual([0, 30, 60, 90]);
    const plan = planAlongPath([1, 1], path, { ...DEFAULT_ALONG, rotate: false, count: 3 });
    expect(plan.matrices.map((m) => m.slice(0, 4))).toEqual([[1, 0, 0, 1], [1, 0, 0, 1], [1, 0, 0, 1]]);
    near(applyAffine(plan.matrices[1]!, [1, 1]), [50, 0]);
  });

  it('en un trayecto cerrado la última copia no repite la primera', () => {
    const path = samplePath([[0, 0], [10, 0], [10, 10], [0, 10]], true)!;
    expect(path.length).toBeCloseTo(40, 9);
    expect(stationsOf(path, { mode: 'count', count: 4, spacing: 1 })).toEqual([0, 10, 20, 30]);
  });

  it('entradas inválidas dan un motivo y ninguna copia', () => {
    const path = samplePath([[0, 0], [10, 0]], false)!;
    expect(planAlongPath([0, 0], path, { ...DEFAULT_ALONG, count: 0 }).issue).toBe('design.along.err.count');
    expect(planAlongPath([0, 0], path, { ...DEFAULT_ALONG, mode: 'spacing', spacing: 0 }).issue).toBe('design.along.err.spacing');
    expect(planAlongPath([0, 0], path, { ...DEFAULT_ALONG, count: 5000 }).issue).toBe('design.along.err.tooMany');
    expect(planAlongPath([0, 0], null, DEFAULT_ALONG).issue).toBe('design.along.err.noPath');
    expect(samplePath([[1, 1], [1, 1]], false)).toBeNull();
  });

  it('construye las copias con ids nuevos, quita los originales y se deshace', () => {
    load([rectObj('cuadro', 0, 0, 2, 2), lineObj('trazo', [10, 0], [10, 40])]);
    s().select(['cuadro', 'trazo']);
    const units = selectionUnits(s().doc, s().selectedIds);
    const path = samplePath(...(Object.values(pathOfUnit(units[1]!)!) as [[number, number][], boolean]))!;
    const plan = planAlongPath([1, 1], path, { ...DEFAULT_ALONG, count: 3, rotate: false });
    const content = [units[0]!];
    const made = buildAlongPath(s().doc, content, plan.matrices, false);
    expect(made.created).toHaveLength(3);
    expect(made.doc.objects.map((o) => o.id)).toEqual(['trazo', ...made.created]);
    const kept = buildAlongPath(s().doc, content, plan.matrices, true);
    expect(kept.doc.objects).toHaveLength(2 + 3);
    s().applyDocChange('design.history.along', () => made.doc);
    s().undo();
    expect(s().doc.objects.map((o) => o.id)).toEqual(['cuadro', 'trazo']);
  });
});

describe('acoplar y mover juntos', () => {
  const setup = () => {
    load([rectObj('ref', 100, 0, 120, 20), rectObj('a', 0, 5, 10, 15), rectObj('b', 30, 8, 40, 18)]);
    s().select(['ref', 'a', 'b']);
    const units = selectionUnits(s().doc, s().selectedIds);
    return { ref: units[0]!, movers: [units[1]!, units[2]!], units };
  };

  it('deslizar a la derecha lleva cada forma a tocar el borde izquierdo de la referencia', () => {
    const { ref, movers } = setup();
    const plan = planDock(ref, movers, { ...DEFAULT_DOCK, direction: 'right' });
    expect(plan.moves.map((m) => [m.ids[0], m.dx, m.dy])).toEqual([['a', 90, 0], ['b', 60, 0]]);
    expect(plan.skipped).toBe(0);
  });

  it('con hueco, en grupo y en la otra dirección', () => {
    const { ref, movers } = setup();
    const padded = planDock(ref, movers, { ...DEFAULT_DOCK, direction: 'right', padding: 2 });
    expect(padded.moves.map((m) => m.dx)).toEqual([88, 58]);
    const group = planDock(ref, movers, { ...DEFAULT_DOCK, direction: 'right', asGroup: true });
    expect(group.moves.map((m) => m.dx)).toEqual([60, 60]);
    // Hacia la izquierda no hay nada que tocar: ninguna avanza.
    const wrong = planDock(ref, movers, { ...DEFAULT_DOCK, direction: 'left' });
    expect(wrong.moves).toEqual([]);
    expect(wrong.skipped).toBe(2);
  });

  it('una forma que no queda frente a la referencia no llega a tocarla', () => {
    load([rectObj('ref', 100, 0, 120, 20), rectObj('alta', 0, 50, 10, 60)]);
    s().select(['ref', 'alta']);
    const [ref, alta] = selectionUnits(s().doc, s().selectedIds);
    const plan = planDock(ref!, [alta!], { ...DEFAULT_DOCK, direction: 'right' });
    expect(plan.moves).toEqual([]);
    expect(plan.skipped).toBe(1);
  });

  it('juntar hacia la referencia avanza en línea recta hasta que las cajas se tocan', () => {
    load([rectObj('ref', 0, 0, 10, 10), rectObj('lejos', 50, 0, 60, 10)]);
    s().select(['ref', 'lejos']);
    const [ref, far] = selectionUnits(s().doc, s().selectedIds);
    const plan = planDock(ref!, [far!], { ...DEFAULT_DOCK, mode: 'together' });
    near([plan.moves[0]!.dx, plan.moves[0]!.dy], [-40, 0]);
    const doc = dockDoc(s().doc, plan.moves);
    expect(objectBounds(doc.objects.find((o) => o.id === 'lejos')!)![0][0]).toBeCloseTo(10, 6);
  });

  it('el comando y el diálogo existen y piden al menos dos formas', () => {
    load([rectObj('a', 0, 0, 10, 10)]);
    s().select(['a']);
    expect(registry.availability('arrange.dock', s())).toBe('design.reason.needTwo');
  });
});

describe('arreglo virtual', () => {
  it('el formulario y el contrato se corresponden', () => {
    const grid = arrayOf({ ...DEFAULT_VIRTUAL, kind: 'grid', cols: 4, rows: 2, dx: 1, dy: 2 });
    expect(grid).toEqual({ type: 'grid', cols: 4, rows: 2, dx: 1, dy: 2 });
    expect(arrayOf(draftOf(grid))).toEqual(grid);
    const circ = arrayOf({ ...DEFAULT_VIRTUAL, kind: 'circular', count: 5, centerX: 1, centerY: 2, sweep: 180, rotate: false });
    expect(circ).toEqual({ type: 'circular', count: 5, center: [1, 2], sweep_deg: 180, rotate: false });
    expect(draftOf(circ).sweep).toBe(180);
  });

  it('convertir en objetos agrega las copias, quita el arreglo y se deshace', () => {
    load([rectObj('a', 0, 0, 10, 10, { array: { type: 'grid', cols: 3, rows: 1, dx: 5, dy: 0 } })]);
    s().select(['a']);
    expect(registry.availability('arrange.virtualFlatten', s())).toBe(true);
    void registry.run('arrange.virtualFlatten');
    expect(s().doc.objects).toHaveLength(3);
    expect(s().doc.objects.every((o) => !o.array)).toBe(true);
    const xs = s().doc.objects.map((o) => objectBounds(o)![0][0]).sort((x, y) => x - y);
    expect(xs).toEqual([0, 15, 30]);
    expect(s().selectedIds).toHaveLength(3);
    s().undo();
    expect(s().doc.objects).toHaveLength(1);
    expect(find('a').array).toEqual({ type: 'grid', cols: 3, rows: 1, dx: 5, dy: 0 });
  });

  it('flattenArrays no toca lo que no tiene arreglo', () => {
    load([rectObj('a', 0, 0, 10, 10)]);
    const r = flattenArrays(s().doc, new Set(['a']));
    expect(r.doc).toBe(s().doc);
    expect(r.created).toEqual([]);
  });

  it('quitar el arreglo y la disponibilidad de los comandos', () => {
    load([rectObj('a', 0, 0, 10, 10, { array: { type: 'circular', count: 4, center: [0, -20], sweep_deg: 360, rotate: true } }), rectObj('b', 50, 0, 60, 10)]);
    s().select(['b']);
    expect(registry.availability('arrange.virtualRemove', s())).toBe('design.reason.noVirtual');
    expect(registry.availability('arrange.virtualArray', s())).toBe(true);
    s().select(['a', 'b']);
    expect(registry.availability('arrange.virtualArray', s())).toBe('design.reason.virtualOne');
    s().select(['a']);
    void registry.run('arrange.virtualRemove');
    expect(find('a').array).toBeNull();
    s().undo();
    expect(find('a').array).not.toBeNull();
  });

  it('un objeto de un grupo no admite arreglo virtual', () => {
    load([rectObj('a', 0, 0, 10, 10, { group: 'g1' }), rectObj('b', 20, 0, 30, 10, { group: 'g1' })]);
    s().select(['a']);
    // Seleccionar un miembro selecciona su grupo entero: son dos objetos.
    expect(registry.availability('arrange.virtualArray', s())).not.toBe(true);
  });
});

describe('agrupar automáticamente (comando)', () => {
  it('agrupa la selección por contención y se deshace', async () => {
    load([rectObj('pieza', 0, 0, 100, 100), rectObj('hueco', 10, 10, 30, 30), rectObj('otra', 200, 0, 220, 20)]);
    s().select(['pieza', 'hueco', 'otra']);
    expect(registry.availability('edit.autoGroup', s())).toBe(true);
    await registry.run('edit.autoGroup');
    expect(find('pieza').group).not.toBeNull();
    expect(find('hueco').group).toBe(find('pieza').group);
    expect(find('otra').group).toBeNull();
    s().undo();
    expect(find('pieza').group).toBeNull();
  });

  it('avisa si no hay nada que agrupar', async () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 50, 0, 60, 10)]);
    s().select(['a', 'b']);
    await registry.run('edit.autoGroup');
    expect(s().notices.at(-1)?.text).toContain('contenidas dentro de otras');
    expect(s().canUndo()).toBe(false);
  });
});

describe('asistente booleano y comandos', () => {
  it('abre el diálogo con dos formas y lo bloquea con una', () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 5, 0, 15, 10)]);
    s().select(['a']);
    expect(registry.availability('arrange.boolAssistant', s())).not.toBe(true);
    s().select(['a', 'b']);
    expect(registry.availability('arrange.boolAssistant', s())).toBe(true);
    void registry.run('arrange.boolAssistant');
    expect(useDesignUi.getState().dialog).toBe('boolAssistant');
  });

  it('las cinco operaciones del asistente dan resultado con el motor simulado', async () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 5, 0, 15, 10)]);
    s().select(['a', 'b']);
    const objects = operandsOf(selectionUnits(s().doc, s().selectedIds));
    const api = createMockApi({ latencyMs: 0 });
    for (const op of ['union', 'difference', 'intersection', 'xor', 'weld'] as const) {
      expect((await api.geometryOp({ op, objects })).length, op).toBeGreaterThan(0);
    }
  });

  it('el círculo tangente pide dos o tres formas; deformar y contorno, una', () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 5, 0, 15, 10), rectObj('c', 50, 0, 60, 10), rectObj('d', 80, 0, 90, 10)]);
    s().select(['a']);
    expect(registry.availability('arrange.tangent', s())).toBe('design.reason.needTwo');
    expect(registry.availability('arrange.warp', s())).toBe(true);
    expect(registry.availability('arrange.hull', s())).toBe(true);
    s().select(['a', 'b', 'c', 'd']);
    expect(registry.availability('arrange.tangent', s())).toBe('design.reason.tooMany');
    s().select([]);
    expect(registry.availability('arrange.warp', s())).toBe('arrange.reason.noSelection');
  });
});
