import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { createAppStore, selectActiveDevice } from '@/store';
import { alignMoves, bedBounds, centerOnBedMoves, distributeMoves, moveDoc } from './align';
import { buildArray, centerForRadius, planCircular, planGrid, radiusOf } from './arrays';
import { analyzeSelection, runGeometry } from './geomOps';
import { useArrangeUi } from './uiStore';
import { blockBounds, selectionUnits } from './units';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

/** Rectángulo cerrado [x0,x1] x [y0,y1] colocado con el transform (así se prueba que cuenta). */
function rect(id: string, x0: number, y0: number, x1: number, y1: number, extra: Partial<DesignObject> = {}): DesignObject {
  return {
    id,
    layer: '#ff0000',
    transform: [1, 0, 0, 1, x0, y0],
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [x1 - x0, 0], [x1 - x0, y1 - y0], [0, y1 - y0]], true)] },
    locked: false,
    hidden: false,
    group: null,
    ...extra,
  };
}

function load(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo'), makeLayer('#00ff00', 'Verde')], warnings: [] });
  s().clearHistory();
}

const find = (id: string) => s().doc.objects.find((o) => o.id === id)!;
const bounds = (id: string) => objectBounds(find(id))!;
const reason = (id: string) => registry.availability(id, s());

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  setApi(createMockApi({ latencyMs: 0 }));
  useArrangeUi.setState({ dialog: null, busy: false, alignReference: 'selection', alignObjectId: null, primaryId: null });
});

describe('alinear', () => {
  beforeEach(() => {
    // a: x 0..10, y 0..10 — b: x 20..30, y 5..25 — c: x 50..70, y 0..10
    load([rect('a', 0, 0, 10, 10), rect('b', 20, 5, 30, 25), rect('c', 50, 0, 70, 10)]);
    s().select(['a', 'b', 'c']);
  });

  it('respecto a la selección: bordes y centros exactos', async () => {
    await registry.run('arrange.align.left');
    expect([bounds('a')[0][0], bounds('b')[0][0], bounds('c')[0][0]]).toEqual([0, 0, 0]);
    await registry.run('arrange.align.top');
    expect([bounds('a')[1][1], bounds('b')[1][1], bounds('c')[1][1]]).toEqual([25, 25, 25]);
    await registry.run('arrange.align.vcenter');
    // Tras alinear arriba la caja total va de y=5 a y=25: su centro es 15.
    for (const id of ['a', 'b', 'c']) {
      const [lo, hi] = bounds(id);
      expect((lo[1] + hi[1]) / 2).toBeCloseTo(15, 9);
    }
  });

  it('derecha y abajo usan el borde correcto (Y hacia arriba)', async () => {
    await registry.run('arrange.align.right');
    expect([bounds('a')[1][0], bounds('b')[1][0], bounds('c')[1][0]]).toEqual([70, 70, 70]);
    await registry.run('arrange.align.bottom');
    expect([bounds('a')[0][1], bounds('b')[0][1], bounds('c')[0][1]]).toEqual([0, 0, 0]);
  });

  it('respecto a un objeto: ese objeto no se mueve y los demás van a su borde', async () => {
    useArrangeUi.getState().setAlign('object', 'b');
    await registry.run('arrange.align.left');
    expect(bounds('b')[0][0]).toBe(20);
    expect(bounds('a')[0][0]).toBe(20);
    expect(bounds('c')[0][0]).toBe(20);
    expect(find('b').transform).toEqual([1, 0, 0, 1, 20, 5]);
  });

  it('respecto a la cama', async () => {
    const [bw, bh] = selectActiveDevice(s()).bed;
    useArrangeUi.getState().setAlign('bed');
    s().select(['a']);
    await registry.run('arrange.align.right');
    await registry.run('arrange.align.top');
    expect(bounds('a')[1][0]).toBe(bw);
    expect(bounds('a')[1][1]).toBe(bh);
    await registry.run('arrange.align.hcenter');
    expect((bounds('a')[0][0] + bounds('a')[1][0]) / 2).toBeCloseTo(bw / 2, 9);
  });

  it('un grupo se alinea como una sola unidad', () => {
    s().applyDocChange('history.group', (d) => ({ ...d, objects: d.objects.map((o) => (o.id === 'b' || o.id === 'c' ? { ...o, group: 'g' } : o)) }));
    const units = selectionUnits(s().doc, ['a', 'b']); // 'b' arrastra a 'c' por el grupo
    expect(units).toHaveLength(2);
    expect(units[1]!.ids).toEqual(['b', 'c']);
    const moves = alignMoves(units, 'left', blockBounds(units)!);
    expect(moves).toEqual([{ ids: ['b', 'c'], dx: -20, dy: 0 }]);
  });

  it('distribuir con espacio igual: los extremos no se mueven', () => {
    const units = selectionUnits(s().doc, s().selectedIds);
    const moves = distributeMoves(units, 'h');
    // anchos 10+10+20, tramo 0..70: huecos de 15 -> b pasa de 20 a 25; c ya está en su sitio.
    expect(moves).toEqual([{ ids: ['b'], dx: 5, dy: 0 }]);
    const doc = moveDoc(s().doc, moves);
    expect(doc.objects.find((o) => o.id === 'b')!.transform).toEqual([1, 0, 0, 1, 25, 5]);
  });

  it('distribuir en vertical', async () => {
    load([rect('p', 0, 0, 10, 10), rect('q', 0, 12, 10, 14), rect('r', 0, 40, 10, 60)]);
    s().select(['p', 'q', 'r']);
    await registry.run('arrange.distribute.v');
    // alturas 10+2+20, tramo 0..60: huecos de 14 -> q empieza en 24.
    expect(bounds('p')[0][1]).toBe(0);
    expect(bounds('q')[0][1]).toBe(24);
    expect(bounds('r')[0][1]).toBe(40);
  });

  it('centrar en la cama mueve el bloque entero', async () => {
    await registry.run('arrange.centerOnBed');
    const [bw, bh] = selectActiveDevice(s()).bed;
    const block = blockBounds(selectionUnits(s().doc, ['a', 'b', 'c']))!;
    expect((block[0][0] + block[1][0]) / 2).toBeCloseTo(bw / 2, 9);
    expect((block[0][1] + block[1][1]) / 2).toBeCloseTo(bh / 2, 9);
    // Las posiciones relativas se conservan.
    expect(bounds('c')[0][0] - bounds('a')[0][0]).toBe(50);
    const moves = centerOnBedMoves(selectionUnits(s().doc, ['a', 'b', 'c']), block, bedBounds([bw, bh]));
    expect(moves).toEqual([]);
  });
});

describe('arreglo en rejilla', () => {
  const bed = bedBounds([400, 400]);

  it('posiciones exactas con separación entre bordes', () => {
    load([rect('a', 0, 0, 10, 10)]);
    const units = selectionUnits(s().doc, ['a']);
    const plan = planGrid({ columns: 3, rows: 2, spacingX: 5, spacingY: 2, mode: 'gap', group: false }, blockBounds(units)!, bed);
    expect(plan.count).toBe(6);
    expect(plan.size).toEqual([40, 22]);
    const { doc, created } = buildArray(s().doc, units, plan.matrices, false);
    expect(created).toHaveLength(5);
    const at = doc.objects.slice(1).map((o) => [o.transform[4], o.transform[5]]);
    expect(at).toEqual([[15, 0], [30, 0], [0, 12], [15, 12], [30, 12]]);
    expect(new Set(doc.objects.map((o) => o.id)).size).toBe(6);
  });

  it('con paso entre centros y agrupando el resultado', () => {
    load([rect('a', 0, 0, 10, 10)]);
    const units = selectionUnits(s().doc, ['a']);
    const plan = planGrid({ columns: 2, rows: 2, spacingX: 25, spacingY: 30, mode: 'pitch', group: true }, blockBounds(units)!, bed);
    expect(plan.size).toEqual([35, 40]);
    const { doc } = buildArray(s().doc, units, plan.matrices, true);
    expect(doc.objects).toHaveLength(4);
    expect(new Set(doc.objects.map((o) => o.group)).size).toBe(1);
    expect(doc.objects[0]!.group).not.toBeNull();
  });

  it('avisa si se sale de la cama y rechaza cuentas inválidas o excesivas', () => {
    load([rect('a', 0, 0, 100, 100)]);
    const block = blockBounds(selectionUnits(s().doc, ['a']))!;
    expect(planGrid({ columns: 5, rows: 1, spacingX: 0, spacingY: 0, mode: 'gap', group: false }, block, bed).outOfBed).toBe(true);
    expect(planGrid({ columns: 4, rows: 1, spacingX: 0, spacingY: 0, mode: 'gap', group: false }, block, bed).outOfBed).toBe(false);
    expect(planGrid({ columns: 0, rows: 1, spacingX: 0, spacingY: 0, mode: 'gap', group: false }, block, bed).issue).toBe('arrange.array.err.counts');
    expect(planGrid({ columns: 1, rows: 1, spacingX: 0, spacingY: 0, mode: 'gap', group: false }, block, bed).issue).toBe('arrange.array.err.single');
    expect(planGrid({ columns: 100, rows: 100, spacingX: 0, spacingY: 0, mode: 'gap', group: false }, block, bed).issue).toBe('arrange.array.err.tooMany');
  });

  it('las copias de un grupo forman grupos propios', () => {
    load([rect('a', 0, 0, 10, 10, { group: 'g' }), rect('b', 20, 0, 30, 10, { group: 'g' })]);
    const units = selectionUnits(s().doc, ['a']);
    expect(units[0]!.ids).toEqual(['a', 'b']);
    const plan = planGrid({ columns: 1, rows: 2, spacingX: 0, spacingY: 5, mode: 'gap', group: false }, blockBounds(units)!, bed);
    const { doc } = buildArray(s().doc, units, plan.matrices, false);
    const groups = doc.objects.map((o) => o.group);
    expect(groups.slice(0, 2)).toEqual(['g', 'g']);
    expect(groups[2]).toBe(groups[3]);
    expect(groups[2]).not.toBe('g');
  });
});

describe('arreglo circular', () => {
  const bed = bedBounds([400, 400]);

  it('4 piezas en círculo completo sin girar: solo se trasladan', () => {
    load([rect('a', -5, 25, 5, 35)]); // centro (0, 30)
    const units = selectionUnits(s().doc, ['a']);
    const plan = planCircular({ copies: 4, centerX: 0, centerY: 0, angle: 360, rotate: false, group: false }, blockBounds(units)!, bed);
    expect(plan.count).toBe(4);
    const { doc } = buildArray(s().doc, units, plan.matrices, false);
    const centers = doc.objects.map((o) => {
      const [[x0, y0], [x1, y1]] = objectBounds(o)!;
      return [(x0 + x1) / 2, (y0 + y1) / 2];
    });
    const expected = [[0, 30], [-30, 0], [0, -30], [30, 0]];
    centers.forEach((c, i) => {
      expect(c[0]).toBeCloseTo(expected[i]![0], 6);
      expect(c[1]).toBeCloseTo(expected[i]![1], 6);
    });
    // Sin girar, la matriz lineal queda intacta.
    expect(doc.objects[1]!.transform.slice(0, 4)).toEqual([1, 0, 0, 1]);
  });

  it('girando las copias, la segunda gira 90 grados', () => {
    load([rect('a', -5, 25, 5, 45)]);
    const units = selectionUnits(s().doc, ['a']);
    const plan = planCircular({ copies: 4, centerX: 0, centerY: 0, angle: 360, rotate: true, group: false }, blockBounds(units)!, bed);
    const { doc } = buildArray(s().doc, units, plan.matrices, false);
    const [a, b] = [doc.objects[1]!.transform[0], doc.objects[1]!.transform[1]];
    expect(a).toBeCloseTo(0, 9);
    expect(b).toBeCloseTo(1, 9);
    // La pieza alta (10 x 20) girada 90° queda ancha (20 x 10), a la izquierda del centro.
    const [[x0, y0], [x1, y1]] = objectBounds(doc.objects[1]!)!;
    expect([x1 - x0, y1 - y0].map((v) => Math.round(v * 1e6) / 1e6)).toEqual([20, 10]);
    expect((x0 + x1) / 2).toBeCloseTo(-35, 6);
    expect((y0 + y1) / 2).toBeCloseTo(0, 6);
  });

  it('un arco reparte con ambos extremos; el radio y el centro van ligados', () => {
    load([rect('a', -5, 25, 5, 35)]);
    const block = blockBounds(selectionUnits(s().doc, ['a']))!;
    const plan = planCircular({ copies: 3, centerX: 0, centerY: 0, angle: 180, rotate: false, group: false }, block, bed);
    expect(plan.matrices).toHaveLength(2);
    // Segunda y tercera: 90° y 180° -> centros (-30, 0) y (0, -30).
    expect(plan.matrices[1]).toEqual([1, 0, 0, 1, 0, -60]);
    expect(radiusOf(block, { centerX: 0, centerY: 0 })).toBeCloseTo(30, 9);
    expect(centerForRadius(block, { centerX: 0, centerY: 0 }, 50)).toEqual([0, -20]);
    expect(planCircular({ copies: 1, centerX: 0, centerY: 0, angle: 360, rotate: true, group: false }, block, bed).issue).toBe('arrange.array.err.copies');
  });
});

describe('habilitación con motivos', () => {
  it('sin selección, todo explica que hay que seleccionar', () => {
    load([rect('a', 0, 0, 10, 10)]);
    for (const id of ['arrange.union', 'arrange.difference', 'arrange.weld', 'arrange.offset', 'arrange.align.left', 'arrange.grid', 'arrange.centerOnBed']) {
      expect(reason(id), id).toBe('arrange.reason.noSelection');
    }
  });

  it('las booleanas piden dos formas cerradas; soldar y offset aceptan una', () => {
    load([rect('a', 0, 0, 10, 10), rect('b', 5, 0, 15, 10)]);
    s().select(['a']);
    expect(reason('arrange.union')).toBe('arrange.reason.needTwoClosed');
    expect(reason('arrange.intersection')).toBe('arrange.reason.needTwoClosed');
    expect(reason('arrange.weld')).toBe(true);
    expect(reason('arrange.offset')).toBe(true);
    s().select(['a', 'b']);
    for (const id of ['arrange.union', 'arrange.difference', 'arrange.intersection', 'arrange.xor', 'arrange.weld', 'arrange.offset']) {
      expect(reason(id), id).toBe(true);
    }
  });

  it('formas abiertas, imágenes y bloqueados dicen cuál es el problema', () => {
    const line: DesignObject = { ...rect('l', 0, 0, 1, 1), kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 10]], false)] } };
    const image: DesignObject = { ...rect('i', 0, 0, 1, 1), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [1, 1] } };
    load([rect('a', 0, 0, 10, 10), line, image, rect('k', 0, 0, 5, 5, { locked: true }), rect('e', 0, 0, 0, 0)]);
    s().select(['a', 'l']);
    expect(reason('arrange.union')).toBe('arrange.reason.open');
    expect(reason('arrange.offset')).toBe('arrange.reason.open');
    s().select(['a', 'i']);
    expect(reason('arrange.weld')).toBe('arrange.reason.image');
    s().select(['a', 'k']);
    expect(reason('arrange.union')).toBe('arrange.reason.locked');
    // Una elipse (curvas con close) y un rectángulo cuyo último punto repite el primero cuentan como cerrados.
    load([{ ...rect('c', 0, 0, 1, 1), kind: { type: 'vector', paths: [ellipseBez(0, 0, 5, 5)] } }]);
    s().select(['a', 'c']);
    expect(analyzeSelection(s()).problem).toBeNull();
  });

  it('alinear y distribuir piden las unidades justas según la referencia', () => {
    load([rect('a', 0, 0, 10, 10), rect('b', 20, 0, 30, 10), rect('c', 40, 0, 50, 10)]);
    s().select(['a']);
    expect(reason('arrange.align.left')).toBe('arrange.reason.alignNeedTwo');
    useArrangeUi.getState().setAlign('bed');
    expect(reason('arrange.align.left')).toBe(true);
    expect(reason('arrange.distribute.h')).toBe('arrange.reason.distributeNeedThree');
    s().select(['a', 'b']);
    expect(reason('arrange.distribute.h')).toBe('arrange.reason.distributeNeedThree');
    s().select(['a', 'b', 'c']);
    expect(reason('arrange.distribute.h')).toBe(true);
  });

  it('todos los comandos deshabilitados traen un motivo con texto', () => {
    load([rect('a', 0, 0, 10, 10)]);
    for (const def of registry.list().filter((d) => d.category === 'arrange')) {
      const r = registry.availability(def.id, s());
      if (r !== true) expect(r.startsWith('arrange.reason.'), def.id).toBe(true);
    }
  });
});

describe('forma principal de la diferencia', () => {
  beforeEach(() => load([rect('a', 0, 0, 10, 10), rect('b', 5, 0, 15, 10), rect('c', 30, 0, 40, 10)]));

  it('la diferencia abre un diálogo con la primera seleccionada como A, pero se puede cambiar', async () => {
    s().select(['b', 'a']);
    await registry.run('arrange.difference');
    expect(useArrangeUi.getState().dialog).toEqual({ kind: 'boolean', op: 'difference' });
    expect(useArrangeUi.getState().primaryId).toBe('b');
    expect(s().past).toHaveLength(0); // abrir el diálogo no toca el documento
  });

  it('A menos B depende de la forma principal elegida, no del orden de selección', async () => {
    s().select(['a', 'b']);
    expect(await runGeometry(s(), { op: 'difference', primaryId: 'a' })).toBe(true);
    const keepA = s().doc.objects.filter((o) => o.id.startsWith('difference'));
    expect(keepA).toHaveLength(1);
    expect(objectBounds(keepA[0]!)).toEqual([[0, 0], [5, 10]]);
    s().undo();
    s().select(['a', 'b']);
    await runGeometry(s(), { op: 'difference', primaryId: 'b' });
    const keepB = s().doc.objects.find((o) => o.id.startsWith('difference'))!;
    expect(objectBounds(keepB)).toEqual([[10, 0], [15, 10]]);
  });

  it('intersección y xor con tres formas piden elegir A; con dos se ejecutan directo', async () => {
    s().select(['a', 'b', 'c']);
    await registry.run('arrange.intersection');
    expect(useArrangeUi.getState().dialog).toEqual({ kind: 'boolean', op: 'intersection' });
    useArrangeUi.getState().close();
    s().select(['a', 'b']);
    await registry.run('arrange.xor');
    expect(useArrangeUi.getState().dialog).toBeNull();
    expect(s().past).toHaveLength(1);
  });
});

describe('cada operación es un solo paso de deshacer', () => {
  beforeEach(() => load([rect('a', 0, 0, 10, 10), rect('b', 5, 0, 15, 10), rect('c', 30, 0, 40, 10)]));

  const oneStep = async (run: () => Promise<unknown>) => {
    const before = s().doc;
    await run();
    expect(s().past).toHaveLength(1);
    expect(s().doc).not.toBe(before);
    s().undo();
    expect(s().doc).toBe(before);
    expect(s().past).toHaveLength(0);
    s().redo();
    expect(s().past).toHaveLength(1);
    s().clearHistory();
  };

  it('unión reemplaza los operandos por el resultado', async () => {
    s().select(['a', 'b']);
    await registry.run('arrange.union');
    expect(s().doc.objects.map((o) => o.id)).toEqual(['union-1', 'c']);
    expect(s().selectedIds).toEqual(['union-1']);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a', 'b', 'c']);
  });

  it('soldar acepta cualquier cantidad de objetos', async () => {
    s().select(['a', 'b', 'c']);
    await oneStep(() => registry.run('arrange.weld'));
    expect(s().doc.objects).toHaveLength(1);
  });

  it('el resultado ocupa el lugar del primer operando en el orden de dibujo', async () => {
    s().select(['b', 'c']);
    await registry.run('arrange.union');
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a', 'union-1']);
  });

  it('offset conservando el original agrega la forma nueva', async () => {
    s().select(['a']);
    await oneStep(() => runGeometry(s(), { op: 'offset', offset: { distance: 2, join: 'miter', keepOriginal: true } }));
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a', 'b', 'c', 'offset-1']);
    expect(objectBounds(find('offset-1'))).toEqual([[-2, -2], [12, 12]]);
  });

  it('offset sin conservar reemplaza al original', async () => {
    s().select(['a']);
    await oneStep(() => runGeometry(s(), { op: 'offset', offset: { distance: 2, join: 'miter', keepOriginal: false } }));
    expect(s().doc.objects.map((o) => o.id)).toEqual(['offset-1', 'b', 'c']);
  });

  it('un offset que borra la forma no cambia el documento y avisa', async () => {
    s().select(['a']);
    const ok = await runGeometry(s(), { op: 'offset', offset: { distance: -8, join: 'round', keepOriginal: true } });
    expect(ok).toBe(false);
    expect(s().past).toHaveLength(0);
    expect(s().notices.at(-1)?.level).toBe('warning');
  });

  it('un error del motor (formas abiertas) se avisa y no cambia nada', async () => {
    const line: DesignObject = { ...rect('l', 0, 0, 1, 1), kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 10]], false)] } };
    s().applyImport({ objects: [line], new_layers: [], warnings: [] });
    s().clearHistory();
    // El comando está deshabilitado; si algo lo llamara igual, el motor explica cuáles son.
    s().select(['a', 'l']);
    expect(await runGeometry(s(), { op: 'union' })).toBe(false);
    expect(s().past).toHaveLength(0);
  });

  it('los arreglos y alinear/distribuir son un paso cada uno', async () => {
    s().select(['a']);
    const units = selectionUnits(s().doc, ['a']);
    const plan = planGrid({ columns: 3, rows: 3, spacingX: 5, spacingY: 5, mode: 'gap', group: true }, blockBounds(units)!, bedBounds([400, 400]));
    await oneStep(async () => s().applyDocChange('arrange.history.grid', (d) => buildArray(d, units, plan.matrices, true).doc));
    s().select(['a', 'b', 'c']);
    await oneStep(() => registry.run('arrange.align.top'));
    await oneStep(() => registry.run('arrange.distribute.h'));
    await oneStep(() => registry.run('arrange.centerOnBed'));
  });
});

describe('atajos', () => {
  it('los atajos de Organizar no chocan con el registro (el constructor lanzaría) y llevan el prefijo del área', () => {
    const withKeys = registry.list().filter((d) => d.category === 'arrange' && d.shortcuts);
    expect(withKeys.length).toBeGreaterThan(0);
    for (const d of withKeys) expect(d.id.startsWith('arrange.')).toBe(true);
  });
});
