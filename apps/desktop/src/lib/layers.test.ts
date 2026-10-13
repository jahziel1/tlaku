import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import {
  DEFAULT_IMAGE_SETTINGS,
  assignObjectsToLayer,
  countByLayer,
  dpiToInterval,
  ensureLayerIn,
  intervalToDpi,
  makeLayer,
  newLayerFor,
} from './layers';
import { GUIDE_COLOR } from './palette';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';

function obj(id: string, layer: string, over: Partial<DesignObject> = {}): DesignObject {
  return {
    id,
    layer,
    transform: [1, 0, 0, 1, 0, 0],
    kind: { type: 'vector', paths: [{ points: [[0, 0], [1, 1]], closed: false }] },
    locked: false,
    hidden: false,
    ...over,
  };
}

const doc = (objects: DesignObject[], layers = [makeLayer(RED, 'Rojo')]): Document => ({ version: 1, objects, layers, device_id: null });

describe('valores por defecto', () => {
  it('el umbral de imagen está en 0-1 como pide el motor', () => {
    expect(DEFAULT_IMAGE_SETTINGS.threshold).toBeGreaterThanOrEqual(0);
    expect(DEFAULT_IMAGE_SETTINGS.threshold).toBeLessThanOrEqual(1);
  });
});

describe('asignación por paleta', () => {
  it('mueve los objetos a la capa y la crea con el nombre de la paleta', () => {
    const d = assignObjectsToLayer(doc([obj('a', RED), obj('b', RED)]), ['a'], BLUE);
    expect(d.objects.map((o) => o.layer)).toEqual([BLUE, RED]);
    const layer = d.layers.find((l) => l.color === BLUE);
    expect(layer?.name).toBe('Azul');
    expect(layer?.output).toBe(true);
  });

  it('no toca objetos bloqueados y devuelve el mismo documento si no cambia nada', () => {
    const d = doc([obj('a', RED, { locked: true }), obj('b', RED)]);
    const r = assignObjectsToLayer(d, ['a', 'b'], BLUE);
    expect(r.objects.map((o) => o.layer)).toEqual([RED, BLUE]);
    expect(assignObjectsToLayer(d, ['a'], BLUE)).toBe(d);
    expect(assignObjectsToLayer(d, ['b'], RED)).toBe(d);
    expect(assignObjectsToLayer(d, [], BLUE)).toBe(d);
  });

  it('la capa de guías nace sin salida y con el nombre Guías', () => {
    const d = assignObjectsToLayer(doc([obj('a', RED)]), ['a'], GUIDE_COLOR);
    const guide = d.layers.find((l) => l.color === GUIDE_COLOR);
    expect(guide?.output).toBe(false);
    expect(guide?.name).toBe('Guías');
    expect(newLayerFor(GUIDE_COLOR).output).toBe(false);
  });

  it('ensureLayerIn no duplica capas, sin importar mayúsculas', () => {
    const d = doc([], [makeLayer(RED, 'Rojo')]);
    expect(ensureLayerIn(d, RED.toUpperCase())).toBe(d);
    expect(ensureLayerIn(d, BLUE).layers).toHaveLength(2);
  });
});

describe('conteo por capa', () => {
  it('separa el total de lo que saldría a la máquina', () => {
    const image = obj('img', '#808080', { kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [1, 1] } });
    const d = doc(
      [obj('a', RED), obj('b', RED, { hidden: true }), image, obj('c', '#808080')],
      [makeLayer(RED, 'Rojo'), makeLayer('#808080', 'Foto', { mode: 'image' })],
    );
    const c = countByLayer(d);
    expect(c.get(RED)).toEqual({ total: 2, live: 1 });
    // un vector en una capa de imagen no sale
    expect(c.get('#808080')).toEqual({ total: 2, live: 1 });
  });
});

describe('intervalo y DPI', () => {
  it('son inversos (25,4 mm por pulgada)', () => {
    expect(intervalToDpi(0.1)).toBeCloseTo(254, 9);
    expect(intervalToDpi(0.0847)).toBeCloseTo(299.88, 1);
    expect(dpiToInterval(254)).toBe(0.1);
    expect(dpiToInterval(300)).toBeCloseTo(0.084667, 6);
  });

  it('ida y vuelta casi exacta en el rango útil', () => {
    for (const dpi of [100, 150, 254, 300, 500, 1000]) {
      expect(Math.abs(intervalToDpi(dpiToInterval(dpi)) - dpi) / dpi).toBeLessThan(1e-3);
    }
  });
});
