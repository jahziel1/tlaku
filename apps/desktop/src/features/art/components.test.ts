import { beforeEach, describe, expect, it } from 'vitest';
import { setApi, getApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { useArtPanel } from './artStore';
import { insertComponent, loadUserComponents, removeUserComponent, renameUserComponent, saveSelectionAsComponent } from './componentActions';
import { buildComponent, componentSize, fitsBed, isComponentData, layerConflicts, planInsert, sizeLabel, validComponents } from './components';
import { usePrompts } from '@/features/history/prompt';

const square = (id: string, layer: string, x: number, y: number, size = 10, group: string | null = null): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, x, y],
  kind: {
    type: 'vector',
    paths: [polylineBez([[0, 0], [size, 0], [size, size], [0, size]], true)],
  },
  locked: false,
  hidden: false,
  group,
});

const s = () => useStore.getState();

async function reset() {
  setApi(createMockApi({ latencyMs: 0 }));
  useArtPanel.setState({ components: [], saving: null, placement: 'auto', error: null });
  useStore.setState({ notices: [] });
  s().newDocument();
  await s().loadDevices();
}

beforeEach(reset);

/** El documento tiene una hoja de 3 cuadros (rojo con ajustes propios) y uno azul. */
function loadSheet() {
  s().applyImport({
    objects: [square('a', '#ff0000', 100, 100), square('b', '#ff0000', 130, 100), square('c', '#0000ff', 160, 100)],
    new_layers: [makeLayer('#ff0000', 'Corte', { speed: 900, power: 80, passes: 2 }), makeLayer('#0000ff', 'Grabado')],
    warnings: [],
  });
}

describe('componentes: construir', () => {
  it('guarda los objetos con las capas que usan, su caja y una miniatura', () => {
    loadSheet();
    const objs = s().doc.objects.filter((o) => o.id !== 'c');
    const data = buildComponent(objs, s().doc.layers)!;
    expect(isComponentData(data)).toBe(true);
    expect(data.objects.map((o) => o.id)).toEqual(['a', 'b']);
    // Solo la capa roja, con sus ajustes de corte.
    expect(data.layers.map((l) => l.color)).toEqual(['#ff0000']);
    expect(data.layers[0]).toMatchObject({ speed: 900, power: 80, passes: 2 });
    expect(data.bounds).toEqual([[100, 100], [140, 110]]);
    expect(sizeLabel(data)).toBe('40 × 10 mm');
    expect(componentSize(data)).toEqual({ width: 40, height: 10 });
    expect(data.thumb).toContain('<svg');
    // Es una copia: cambiar el documento no la cambia.
    s().updateLayer('#ff0000', { speed: 100 });
    expect(data.layers[0]!.speed).toBe(900);
  });

  it('sin objetos o sin área no hay componente', () => {
    expect(buildComponent([], [])).toBeNull();
  });

  it('una hoja enorme usa una miniatura de cajas para no pesar el doble', () => {
    const many = Array.from({ length: 1500 }, (_, i) => square(`p${i}`, '#ff0000', (i % 40) * 14, Math.floor(i / 40) * 14));
    const data = buildComponent(many, [makeLayer('#ff0000', 'Corte')])!;
    expect(data.thumb.length).toBeGreaterThan(0);
    expect(data.thumb.length).toBeLessThan(250_000);
    expect(data.thumb).toContain('<rect');
  });

  it('la caja cabe en la cama solo si está dentro', () => {
    expect(fitsBed([[0, 0], [600, 600]], [610, 610])).toBe(true);
    expect(fitsBed([[10, 10], [620, 600]], [610, 610])).toBe(false);
    expect(fitsBed([[-5, 0], [10, 10]], [610, 610])).toBe(false);
  });

  it('descarta los componentes inválidos de un paquete', () => {
    loadSheet();
    const data = buildComponent(s().doc.objects, s().doc.layers)!;
    const r = validComponents([{ name: 'hoja', data }, { name: '  ', data }, { name: 'x', data: { format: 'otro' } }, null]);
    expect(r.items.map((c) => c.name)).toEqual(['hoja']);
    expect(r.invalid).toBe(3);
  });
});

describe('componentes: insertar (plan)', () => {
  const bed: [number, number] = [610, 610];

  it('por omisión conserva la posición original si cabe en la cama, y si no va al centro de la vista', () => {
    loadSheet();
    const data = buildComponent(s().doc.objects, s().doc.layers)!;
    const doc = { ...s().doc, objects: [], layers: [] };
    const original = planInsert(data, doc, { placement: 'auto', center: [300, 300], bed });
    expect(original.placedAt).toBe('original');
    expect(original.result.objects.map((o) => o.transform)).toEqual(data.objects.map((o) => o.transform));

    const small = planInsert(data, doc, { placement: 'auto', center: [300, 300], bed: [100, 100] });
    expect(small.placedAt).toBe('center');
    const boxes = small.result.objects.map((o) => objectBounds(o)!);
    const mid = (Math.min(...boxes.map((b) => b[0][0])) + Math.max(...boxes.map((b) => b[1][0]))) / 2;
    expect(mid).toBeCloseTo(300);
  });

  it('«original» siempre respeta la posición y avisa si se sale de la cama', () => {
    loadSheet();
    const data = buildComponent(s().doc.objects, s().doc.layers)!;
    const doc = { ...s().doc, objects: [], layers: [] };
    const plan = planInsert(data, doc, { placement: 'original', center: [0, 0], bed: [100, 100] });
    expect(plan.placedAt).toBe('original');
    expect(plan.outsideBed).toBe(true);
  });

  it('los grupos del componente pasan a grupos nuevos y no se unen a los del documento', () => {
    s().applyImport({
      objects: [square('a', '#ff0000', 0, 0, 10, 'grupo-1'), square('b', '#ff0000', 20, 0, 10, 'grupo-1')],
      new_layers: [makeLayer('#ff0000', 'Corte')],
      warnings: [],
    });
    const data = buildComponent(s().doc.objects, s().doc.layers)!;
    const plan = planInsert(data, s().doc, { placement: 'original', center: [0, 0], bed });
    const groups = new Set(plan.result.objects.map((o) => o.group));
    expect(groups.size).toBe(1);
    expect(groups.has('grupo-1')).toBe(false);
  });

  it('detecta las capas que ya existen con otros ajustes y solo trae las que faltan', () => {
    loadSheet();
    const data = buildComponent(s().doc.objects, s().doc.layers)!;
    const same = layerConflicts(data.layers, s().doc.layers);
    expect(same).toEqual([]);
    const other = [makeLayer('#ff0000', 'Corte (otro nombre)', { speed: 900, power: 80, passes: 2 }), makeLayer('#0000ff', 'Grabado', { power: 5 })];
    // El nombre no cuenta; la potencia sí.
    expect(layerConflicts(data.layers, other)).toEqual(['#0000ff']);
    const plan = planInsert(data, { ...s().doc, layers: [other[0]!] }, { placement: 'original', center: [0, 0], bed });
    expect(plan.result.new_layers.map((l) => l.color)).toEqual(['#0000ff']);
    expect(plan.conflicts).toEqual([]);
  });
});

describe('componentes: guardar, listar, renombrar y borrar (modo simulado)', () => {
  it('guarda la selección, la lista y no pisa un nombre existente', async () => {
    loadSheet();
    expect(await saveSelectionAsComponent('hoja 60x60', ['a', 'b', 'c'])).toBe('hoja 60x60');
    expect(await saveSelectionAsComponent('hoja 60x60', ['a'])).toBe('hoja 60x60-2');
    await loadUserComponents();
    const list = useArtPanel.getState().components;
    expect(list.map((c) => c.name)).toEqual(['hoja 60x60', 'hoja 60x60-2']);
    expect(list[0]!.data.objects).toHaveLength(3);
    expect(list[0]!.data.layers.map((l) => l.color).sort()).toEqual(['#0000ff', '#ff0000']);
    expect(s().notices.some((n) => n.level === 'success')).toBe(true);
    expect((await getApi().componentListUser()).length).toBe(2);
  });

  it('un nombre inválido no guarda nada y avisa', async () => {
    loadSheet();
    expect(await saveSelectionAsComponent('   ', ['a'])).toBeNull();
    expect(await getApi().componentListUser()).toEqual([]);
    expect(s().notices.some((n) => n.level === 'error')).toBe(true);
  });

  it('renombra y borra (el borrado pide confirmación)', async () => {
    loadSheet();
    await saveSelectionAsComponent('uno', ['a']);
    await saveSelectionAsComponent('dos', ['b']);
    await renameUserComponent('uno', 'dos');
    expect(useArtPanel.getState().components.map((c) => c.name)).toEqual(['dos', 'dos-2']);

    // Si se cancela, no se borra.
    const cancelled = removeUserComponent('dos');
    expect(usePrompts.getState().current?.message).toContain('dos');
    usePrompts.getState().answer('cancel');
    await cancelled;
    expect(useArtPanel.getState().components).toHaveLength(2);

    const confirmed = removeUserComponent('dos');
    usePrompts.getState().answer('delete');
    await confirmed;
    expect(useArtPanel.getState().components.map((c) => c.name)).toEqual(['dos-2']);
  });
});

describe('componentes: insertar en el documento (modo simulado)', () => {
  it('inserta en su posición original, con sus capas, en un solo paso de Deshacer', async () => {
    loadSheet();
    await saveSelectionAsComponent('hoja', ['a', 'b', 'c']);
    // Otro proyecto: vacío.
    s().newDocument();
    expect(s().doc.objects).toHaveLength(0);
    const before = s().past.length;
    const c = useArtPanel.getState().components[0]!;
    expect(await insertComponent(c)).toBe(true);
    expect(s().doc.objects).toHaveLength(3);
    expect(s().doc.objects.map((o) => o.transform[4]).sort((a, b) => a - b)).toEqual([100, 130, 160]);
    expect(s().doc.layers.find((l) => l.color === '#ff0000')).toMatchObject({ speed: 900, power: 80, passes: 2 });
    expect(s().selectedIds).toHaveLength(3);
    expect(s().past.length).toBe(before + 1);
    s().undo();
    expect(s().doc.objects).toHaveLength(0);
    expect(s().doc.layers.find((l) => l.color === '#ff0000')).toBeUndefined();
  });

  it('al soltarlo en el lienzo se centra en ese punto', async () => {
    loadSheet();
    await saveSelectionAsComponent('hoja', ['a']);
    s().newDocument();
    await insertComponent(useArtPanel.getState().components[0]!, { at: [50, 40] });
    const b = objectBounds(s().doc.objects[0]!)!;
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(50);
    expect((b[0][1] + b[1][1]) / 2).toBeCloseTo(40);
  });

  it('si la capa del mismo color tiene otros ajustes se conservan los del proyecto y se avisa', async () => {
    loadSheet();
    await saveSelectionAsComponent('hoja', ['a', 'b']);
    s().newDocument();
    s().applyImport({ objects: [square('z', '#ff0000', 0, 0)], new_layers: [makeLayer('#ff0000', 'Mi corte', { speed: 300, power: 40 })], warnings: [] });
    s().select([]);
    useStore.setState({ notices: [] });
    await insertComponent(useArtPanel.getState().components[0]!);
    expect(s().doc.objects).toHaveLength(3);
    expect(s().doc.layers.filter((l) => l.color === '#ff0000')).toHaveLength(1);
    expect(s().doc.layers.find((l) => l.color === '#ff0000')).toMatchObject({ speed: 300, power: 40 });
    const warn = s().notices.find((n) => n.level === 'warning');
    expect(warn?.details).toEqual(['#ff0000']);
  });

  it('con «centro de la vista» ignora la posición original', async () => {
    loadSheet();
    await saveSelectionAsComponent('hoja', ['a']);
    s().newDocument();
    useArtPanel.getState().setPlacement('center');
    await insertComponent(useArtPanel.getState().components[0]!);
    const [x, y] = [s().doc.objects[0]!.transform[4], s().doc.objects[0]!.transform[5]];
    expect([x, y]).not.toEqual([100, 100]);
    expect(selectActiveDevice(s()).bed[0]).toBeGreaterThan(0);
  });
});
