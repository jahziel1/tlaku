import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { createAppStore } from '@/store';
import { GUIDE_COLOR, GUIDE_ENTRY, LAYER_PALETTE, paletteEntry } from '@/lib/palette';
import { moveLayer, pickPaletteEntry } from './actions';

const RED = paletteEntry('#ff3b4a')!;
const BLUE = paletteEntry('#2f7bff')!;

const obj = (id: string, layer: string, locked = false): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths: [{ points: [[0, 0], [1, 1]], closed: false }] },
  locked,
  hidden: false,
});

function setup(objects: DesignObject[]) {
  const store = createAppStore();
  store.getState().replaceDocument({ version: 1, objects, layers: [], device_id: null }, null);
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
