import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { polylineBez } from '@/lib/bezpath';
import { objectPolylines } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { cleanupSummary, replaceCleaned, runCleanup } from './cleanup';
import { useArrangeUi } from './uiStore';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

function line(id: string, points: [number, number][], extra: Partial<DesignObject> = {}): DesignObject {
  return { id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez(points, false)] }, locked: false, hidden: false, group: null, ...extra };
}
const square = (id: string, x0: number, y0: number, x1: number, y1: number, extra: Partial<DesignObject> = {}): DesignObject => ({
  ...line(id, [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], extra),
  kind: { type: 'vector', paths: [polylineBez([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true)] },
});

function load(objects: DesignObject[]) {
  s().applyImport({ objects, new_layers: [makeLayer('#ff0000', 'Rojo'), makeLayer('#0000ff', 'Azul')], warnings: [] });
  s().clearHistory();
}
const ids = () => s().doc.objects.map((o) => o.id);
const reason = (id: string) => registry.availability(id, s());
const length = (o: DesignObject) => objectPolylines(o).reduce((n, p) => n + p.points.slice(1).reduce((m, q, i) => m + Math.hypot(q[0] - p.points[i]![0], q[1] - p.points[i]![1]), 0), 0);

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  setApi(createMockApi({ latencyMs: 0 }));
  useArrangeUi.setState({ dialog: null, busy: false });
});

describe('comandos de limpieza de trazos', () => {
  it('piden selección de vectores y trazos abiertos, con su motivo', () => {
    const image: DesignObject = { ...square('i', 0, 0, 1, 1), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [1, 1] } };
    load([line('a', [[0, 0], [10, 0]]), square('q', 0, 0, 5, 5), image, line('k', [[0, 0], [1, 1]], { locked: true })]);
    for (const id of ['arrange.join', 'arrange.close', 'arrange.dedupe']) expect(reason(id), id).toBe('arrange.reason.noSelection');
    s().select(['q']);
    expect(reason('arrange.join')).toBe('arrange.reason.noOpenPaths');
    expect(reason('arrange.close')).toBe('arrange.reason.noOpenPaths');
    expect(reason('arrange.dedupe')).toBe(true);
    s().select(['a', 'q']);
    for (const id of ['arrange.join', 'arrange.close', 'arrange.dedupe']) expect(reason(id), id).toBe(true);
    s().select(['a', 'i']);
    expect(reason('arrange.join')).toBe('arrange.reason.needVectors');
    s().select(['a', 'k']);
    expect(reason('arrange.dedupe')).toBe('arrange.reason.locked');
  });

  it('abren su diálogo con la tolerancia propia de cada una', () => {
    load([line('a', [[0, 0], [10, 0]])]);
    s().select(['a']);
    void registry.run('arrange.close');
    expect(useArrangeUi.getState().dialog).toEqual({ kind: 'cleanup', op: 'close' });
    expect(useArrangeUi.getState().cleanup).toEqual({ join: 0.05, close: 0.5, dedupe: 0.01 });
  });
});

describe('unir trazos', () => {
  const basic = () => load([line('a', [[0, 0], [10, 0]]), line('b', [[10.02, 0], [10.02, 10]]), line('lejos', [[50, 50], [60, 50]]), square('q', 100, 0, 110, 10)]);

  it('une extremos cercanos en un solo objeto, deja el resto y es un paso de deshacer', async () => {
    basic();
    s().select(['a', 'b', 'lejos']);
    const before = s().doc;
    expect(await runCleanup(s(), 'join', 0.05)).toBe(true);
    expect(ids()).toEqual(['join-1', 'lejos', 'q']);
    expect(objectPolylines(s().doc.objects[0]!)).toHaveLength(1);
    expect(s().selectedIds).toEqual(['lejos', 'join-1']);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc).toBe(before);
  });

  it('avisa con el resultado y, si no hay nada que unir, no cambia nada', async () => {
    basic();
    s().select(['a', 'b']);
    await runCleanup(s(), 'join', 0.05);
    expect(s().notices.at(-1)).toMatchObject({ level: 'success', text: 'Se unieron 2 trazos en 1.' });
    s().select(['join-1', 'lejos']);
    const doc = s().doc;
    expect(await runCleanup(s(), 'join', 0.05)).toBe(false);
    expect(s().doc).toBe(doc);
    expect(s().notices.at(-1)).toMatchObject({ level: 'info' });
  });

  it('con más tolerancia une lo que antes no', async () => {
    basic();
    s().select(['lejos', 'a']);
    expect(await runCleanup(s(), 'join', 0.05)).toBe(false);
    expect(await runCleanup(s(), 'join', 100)).toBe(true);
  });

  it('no mezcla capas', async () => {
    load([line('a', [[0, 0], [10, 0]]), line('b', [[10, 0], [20, 0]], { layer: '#0000ff' })]);
    s().select(['a', 'b']);
    expect(await runCleanup(s(), 'join', 0.05)).toBe(false);
  });

  it('un camino que se cierra queda como forma cerrada y se avisa', async () => {
    load([line('l1', [[0, 0], [10, 0]]), line('l2', [[10, 0], [10, 10]]), line('l3', [[10, 10], [0, 10]]), line('l4', [[0, 10], [0, 0]])]);
    s().select(['l1', 'l2', 'l3', 'l4']);
    await runCleanup(s(), 'join', 0.05);
    expect(ids()).toEqual(['join-1']);
    expect(objectPolylines(s().doc.objects[0]!)[0]!.closed).toBe(true);
    expect(s().notices.at(-1)!.text).toBe('Se unieron 4 trazos en 1; cerrados: 1.');
  });
});

describe('cerrar trayectos', () => {
  it('cierra el que tiene los extremos a menos de la tolerancia y avisa', async () => {
    load([line('u', [[0.3, 0], [0, 0], [0, 10], [10, 10], [10, 0], [0.6, 0]]), line('v', [[0, 20], [10, 20], [10, 30]])]);
    s().select(['u', 'v']);
    expect(await runCleanup(s(), 'close', 0.5)).toBe(true);
    expect(ids()).toEqual(['close-1', 'v']);
    expect(objectPolylines(s().doc.objects[0]!)[0]!.closed).toBe(true);
    expect(s().notices.at(-1)!.text).toBe('Trayectos cerrados: 1.');
    s().undo();
    expect(ids()).toEqual(['u', 'v']);
  });
});

describe('quitar duplicados', () => {
  it('quita el borde que comparten dos piezas y avisa cuánto', async () => {
    load([square('a', 0, 0, 10, 10), square('b', 10, 0, 20, 10)]);
    s().select(['a', 'b']);
    expect(await runCleanup(s(), 'dedupe', 0.01)).toBe(true);
    expect(ids()).toEqual(['a', 'dedupe-1']);
    expect(length(s().doc.objects[1]!)).toBeCloseTo(30);
    expect(s().notices.at(-1)!.text).toContain('10,00 mm');
    expect(s().selectedIds).toEqual(['a', 'dedupe-1']);
  });

  it('una copia exacta desaparece y el solape parcial se parte', async () => {
    load([line('a', [[0, 0], [10, 0]]), line('b', [[10, 0], [0, 0]]), line('c', [[5, 0], [15, 0]])]);
    s().select(['a', 'b', 'c']);
    await runCleanup(s(), 'dedupe', 0.01);
    expect(ids()).toEqual(['a', 'dedupe-1']);
    expect(length(s().doc.objects[1]!)).toBeCloseTo(5);
  });

  it('sin duplicados no cambia nada', async () => {
    load([square('a', 0, 0, 10, 10), square('b', 20, 0, 30, 10)]);
    s().select(['a', 'b']);
    const doc = s().doc;
    expect(await runCleanup(s(), 'dedupe', 0.01)).toBe(false);
    expect(s().doc).toBe(doc);
  });

  it('los ids nuevos no chocan con los del documento fuera de la selección', async () => {
    load([square('a', 0, 0, 10, 10), square('b', 10, 0, 20, 10), square('dedupe-1', 100, 100, 110, 110)]);
    s().select(['a', 'b']);
    await runCleanup(s(), 'dedupe', 0.01);
    expect(ids()).toEqual(['a', 'dedupe-2', 'dedupe-1']);
  });
});

describe('replaceCleaned y resumen', () => {
  it('si los consumidos ya no existen devuelve el mismo documento', () => {
    load([line('a', [[0, 0], [1, 1]])]);
    expect(replaceCleaned(s().doc, new Set(['borrado']), [line('n', [[0, 0], [1, 1]])], 'join', [])).toBe(s().doc);
  });

  it('los nuevos conservan grupo y capa que decidió el motor', () => {
    load([line('a', [[0, 0], [1, 1]]), line('z', [[5, 5], [6, 6]])]);
    const created: string[] = [];
    const doc = replaceCleaned(s().doc, new Set(['a']), [line('n', [[0, 0], [2, 2]], { group: 'g', layer: '#0000ff' })], 'join', created);
    expect(doc.objects.map((o) => o.id)).toEqual(['join-1', 'z']);
    expect(doc.objects[0]).toMatchObject({ group: 'g', layer: '#0000ff' });
    expect(created).toEqual(['join-1']);
  });

  it('el resumen de unir cuenta trazos antes y después', () => {
    const a = line('a', [[0, 0], [1, 0]]);
    const b = line('b', [[1, 0], [2, 0]]);
    const c = line('c', [[2, 0], [3, 0]]);
    expect(cleanupSummary('join', [a, b, c], [line('n', [[0, 0], [3, 0]])])).toBe('Se unieron 3 trazos en 1.');
    expect(cleanupSummary('close', [line('u', [[0, 0], [1, 0], [1, 1]])], [square('q', 0, 0, 1, 1)])).toBe('Trayectos cerrados: 1.');
  });
});
