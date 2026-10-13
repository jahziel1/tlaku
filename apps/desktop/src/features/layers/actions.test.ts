import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { createAppStore } from '@/store';
import { GUIDE_COLOR, GUIDE_ENTRY, LAYER_PALETTE, paletteEntry } from '@/lib/palette';
import { usePrompts } from '@/features/history/prompt';
import { MAX_SUB_LAYERS } from '@/lib/layers';
import { addSubLayer, copyLayerSettings, deleteLayerWithObjects, moveLayer, moveSubLayer, pasteLayerSettings, pickPaletteEntry, removeSubLayer } from './actions';
import { useLayerUi } from './layerUi';
import { mapTarget } from './targets';
import { polylineBez } from '@/lib/bezpath';

const RED = paletteEntry('#ff3b4a')!;
const BLUE = paletteEntry('#2f7bff')!;

const obj = (id: string, layer: string, locked = false): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [1, 1]], false)] },
  locked,
  hidden: false,
  group: null,
});

function setup(objects: DesignObject[]) {
  const store = createAppStore();
  store.getState().replaceDocument({ version: 2, objects, layers: [], device_id: null }, null);
  return store;
}

describe('elegir una muestra de la paleta', () => {
  it('con selección mueve los objetos en un solo paso de deshacer y crea la capa', () => {
    const store = setup([obj('a', RED.color), obj('b', RED.color)]);
    store.getState().select(['a', 'b']);
    pickPaletteEntry(store.getState(), BLUE);
    const s = store.getState();
    expect(s.doc.objects.map((o) => o.layer)).toEqual([BLUE.color, BLUE.color]);
    expect(s.doc.layers.map((l) => l.name)).toEqual(['Azul']);
    expect(s.activeLayer).toBe(BLUE.color);
    expect(s.past).toHaveLength(1);
    s.undo();
    expect(store.getState().doc.objects.map((o) => o.layer)).toEqual([RED.color, RED.color]);
    expect(store.getState().doc.layers).toEqual([]);
  });

  it('sin selección solo cambia la capa activa y no ensucia el documento', () => {
    const store = setup([obj('a', RED.color)]);
    pickPaletteEntry(store.getState(), BLUE);
    const s = store.getState();
    expect(s.activeLayer).toBe(BLUE.color);
    expect(s.doc.layers).toEqual([]);
    expect(s.past).toHaveLength(0);
  });

  it('las guías se crean al elegirlas, sin salida, aunque no haya selección', () => {
    const store = setup([]);
    pickPaletteEntry(store.getState(), GUIDE_ENTRY);
    const s = store.getState();
    expect(s.activeLayer).toBe(GUIDE_COLOR);
    expect(s.doc.layers).toHaveLength(1);
    expect(s.doc.layers[0]).toMatchObject({ color: GUIDE_COLOR, name: 'Guías', output: false });
  });

  it('pasar objetos a las guías los saca de la salida', () => {
    const store = setup([obj('a', RED.color)]);
    store.getState().select(['a']);
    pickPaletteEntry(store.getState(), GUIDE_ENTRY);
    const s = store.getState();
    expect(s.doc.objects[0]!.layer).toBe(GUIDE_COLOR);
    expect(s.doc.layers.find((l) => l.color === GUIDE_COLOR)?.output).toBe(false);
  });

  it('si todo lo seleccionado está bloqueado avisa y no cambia nada', () => {
    const store = setup([obj('a', RED.color, true)]);
    store.getState().select(['a']);
    pickPaletteEntry(store.getState(), BLUE);
    const s = store.getState();
    expect(s.doc.objects[0]!.layer).toBe(RED.color);
    expect(s.notices.at(-1)?.level).toBe('warning');
    expect(s.past).toHaveLength(0);
  });

  it('cada muestra de la paleta es elegible y deja su color como capa activa', () => {
    const store = setup([]);
    for (const entry of LAYER_PALETTE) {
      pickPaletteEntry(store.getState(), entry);
      expect(store.getState().activeLayer).toBe(entry.color);
    }
  });
});

describe('reordenar desde la interfaz', () => {
  it('es un solo paso de deshacer, aunque cambie la lista y una posición fijada', () => {
    const store = setup([obj('a', RED.color), obj('b', BLUE.color)]);
    store.getState().ensureLayer(RED.color);
    store.getState().ensureLayer(BLUE.color);
    store.getState().updateLayer(RED.color, { kerf: 0.1 }); // corte: va al final
    const before = store.getState().past.length;
    moveLayer(store.getState(), RED.color, 0);
    const s = store.getState();
    expect(s.past.length).toBe(before + 1);
    expect(s.doc.layers.find((l) => l.color === RED.color)?.order).toBe(1);
    s.undo();
    expect(store.getState().doc.layers.every((l) => l.order === null)).toBe(true);
  });
});

describe('copiar y pegar ajustes de capa (C06)', () => {
  function twoLayers() {
    const store = setup([obj('a', RED.color), obj('b', BLUE.color)]);
    store.getState().ensureLayer(RED.color);
    store.getState().ensureLayer(BLUE.color);
    store.getState().updateLayer(RED.color, { speed: 321, power: 77, name: 'Rojo propio', output: false, visible: false, mode: 'fill', passes: 3 });
    store.getState().updateLayer(RED.color, { ramp: { enabled: true, length_mm: 2, start_pct: 10, outer_only: true } });
    useLayerUi.setState({ copied: null });
    return store;
  }

  it('pega todo menos color, nombre, posición fijada, visibilidad y salida', () => {
    const store = twoLayers();
    store.getState().updateLayer(BLUE.color, { name: 'Azul propio', order: 2 });
    copyLayerSettings(store.getState(), RED.color);
    expect(store.getState().notices.at(-1)?.level).toBe('success');
    const before = store.getState().past.length;
    pasteLayerSettings(store.getState(), BLUE.color);
    const s = store.getState();
    const blue = s.doc.layers.find((l) => l.color === BLUE.color)!;
    expect(blue).toMatchObject({ speed: 321, power: 77, mode: 'fill', passes: 3, ramp: { enabled: true, length_mm: 2, start_pct: 10, outer_only: true } });
    expect(blue).toMatchObject({ color: BLUE.color, name: 'Azul propio', order: 2, output: true, visible: true });
    expect(s.past.length).toBe(before + 1);
    s.undo();
    expect(store.getState().doc.layers.find((l) => l.color === BLUE.color)).toMatchObject({ speed: 1000, mode: 'line' });
  });

  it('lo pegado no comparte objetos con la capa de origen', () => {
    const store = twoLayers();
    copyLayerSettings(store.getState(), RED.color);
    pasteLayerSettings(store.getState(), BLUE.color);
    const [red, blue] = [RED.color, BLUE.color].map((c) => store.getState().doc.layers.find((l) => l.color === c)!);
    expect(blue!.ramp).not.toBe(red!.ramp);
    expect(blue!.image).not.toBe(red!.image);
  });

  it('sin nada copiado no pega', () => {
    const store = twoLayers();
    const before = store.getState().past.length;
    pasteLayerSettings(store.getState(), BLUE.color);
    expect(store.getState().past.length).toBe(before);
  });

  it('las operaciones adicionales también se copian, con el color de la capa de destino', () => {
    const store = twoLayers();
    addSubLayer(store.getState(), RED.color, 'line');
    copyLayerSettings(store.getState(), RED.color);
    pasteLayerSettings(store.getState(), BLUE.color);
    const blue = store.getState().doc.layers.find((l) => l.color === BLUE.color)!;
    expect(blue.sub_layers).toHaveLength(1);
    expect(blue.sub_layers[0]!.color).toBe(BLUE.color);
  });
});

describe('borrar una capa con sus objetos', () => {
  it('pide confirmación, borra capa y objetos en un solo paso de deshacer y suelta la capa activa', async () => {
    const store = setup([obj('a', RED.color), obj('b', RED.color), obj('c', BLUE.color)]);
    store.getState().ensureLayer(RED.color);
    store.getState().ensureLayer(BLUE.color);
    store.getState().setActiveLayer(RED.color);
    const before = store.getState().past.length;
    const done = deleteLayerWithObjects(store.getState, RED.color);
    expect(usePrompts.getState().current?.message).toContain('2 objeto(s)');
    usePrompts.getState().answer('delete');
    expect(await done).toBe(true);
    const s = store.getState();
    expect(s.doc.layers.map((l) => l.color)).toEqual([BLUE.color]);
    expect(s.doc.objects.map((o) => o.id)).toEqual(['c']);
    expect(s.activeLayer).toBeNull();
    expect(s.past.length).toBe(before + 1);
    s.undo();
    expect(store.getState().doc.objects).toHaveLength(3);
    expect(store.getState().doc.layers).toHaveLength(2);
  });

  it('si se cancela no cambia nada', async () => {
    const store = setup([obj('a', RED.color)]);
    store.getState().ensureLayer(RED.color);
    const before = store.getState().past.length;
    const done = deleteLayerWithObjects(store.getState, RED.color);
    usePrompts.getState().answer('cancel');
    expect(await done).toBe(false);
    expect(store.getState().doc.objects).toHaveLength(1);
    expect(store.getState().past.length).toBe(before);
  });

  it('con objetos bloqueados avisa y no pregunta', async () => {
    const store = setup([obj('a', RED.color, true)]);
    store.getState().ensureLayer(RED.color);
    expect(await deleteLayerWithObjects(store.getState, RED.color)).toBe(false);
    expect(usePrompts.getState().current).toBeNull();
    expect(store.getState().notices.at(-1)?.level).toBe('warning');
    expect(store.getState().doc.layers).toHaveLength(1);
  });

  it('una capa vacía también se borra', async () => {
    const store = setup([]);
    store.getState().ensureLayer(RED.color);
    const done = deleteLayerWithObjects(store.getState, RED.color);
    expect(usePrompts.getState().current?.message).toContain('no tiene objetos');
    usePrompts.getState().answer('delete');
    expect(await done).toBe(true);
    expect(store.getState().doc.layers).toEqual([]);
  });
});

describe('operaciones adicionales (sub-capas)', () => {
  function withLayer() {
    const store = setup([obj('a', RED.color)]);
    store.getState().ensureLayer(RED.color);
    store.getState().updateLayer(RED.color, { speed: 800, power: 40 });
    return store;
  }
  const subs = (store: ReturnType<typeof withLayer>) => store.getState().doc.layers[0]!.sub_layers;

  it('agregar elige el modo, hereda velocidad y potencia y es un paso de deshacer', () => {
    const store = withLayer();
    const before = store.getState().past.length;
    addSubLayer(store.getState(), RED.color, 'fill');
    expect(subs(store)).toHaveLength(1);
    expect(subs(store)[0]).toMatchObject({ mode: 'fill', speed: 800, power: 40, color: RED.color, sub_layers: [] });
    expect(store.getState().past.length).toBe(before + 1);
    store.getState().undo();
    expect(subs(store)).toHaveLength(0);
  });

  it('respeta el máximo', () => {
    const store = withLayer();
    for (let i = 0; i < MAX_SUB_LAYERS + 3; i++) addSubLayer(store.getState(), RED.color, 'line');
    expect(subs(store)).toHaveLength(MAX_SUB_LAYERS);
  });

  it('reordenar y borrar', () => {
    const store = withLayer();
    addSubLayer(store.getState(), RED.color, 'line');
    addSubLayer(store.getState(), RED.color, 'fill');
    addSubLayer(store.getState(), RED.color, 'image');
    moveSubLayer(store.getState(), RED.color, 2, 0);
    expect(subs(store).map((s) => s.mode)).toEqual(['image', 'line', 'fill']);
    moveSubLayer(store.getState(), RED.color, 0, 5); // fuera de rango: no hace nada
    expect(subs(store).map((s) => s.mode)).toEqual(['image', 'line', 'fill']);
    removeSubLayer(store.getState(), RED.color, 1);
    expect(subs(store).map((s) => s.mode)).toEqual(['image', 'fill']);
    removeSubLayer(store.getState(), RED.color, 9);
    expect(subs(store)).toHaveLength(2);
  });

  it('editar una operación con la misma lógica que la capa cambia solo esa operación', () => {
    const store = withLayer();
    addSubLayer(store.getState(), RED.color, 'line');
    addSubLayer(store.getState(), RED.color, 'line');
    const edited = mapTarget(store.getState().doc, { color: RED.color, sub: 1 }, (l) => ({ ...l, power: 99, ramp: { ...l.ramp, enabled: true } }));
    const [a, b] = edited.layers[0]!.sub_layers;
    expect(a!.power).toBe(40);
    expect(b).toMatchObject({ power: 99, ramp: { enabled: true } });
    expect(edited.layers[0]!.power).toBe(40);
    // Una operación que no existe deja el documento igual.
    expect(mapTarget(store.getState().doc, { color: RED.color, sub: 7 }, (l) => l)).toBe(store.getState().doc);
  });
});
