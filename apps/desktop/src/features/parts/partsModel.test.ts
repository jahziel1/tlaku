import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import type { GeneratorRecord } from '@/bindings/GeneratorRecord';
import { mockGenerate } from '@/api/mock/partsEngine';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { emptyDocument } from '@/store/document';
import { applyInsert, engraveLayerFor, freeSpot, freshGroupId, groupOrigin, occupiedBounds, partObjects, recordFor, unusedColor } from './partsModel';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';

const rect = (id: string, x: number, y: number, w: number, h: number, group: string | null = null): DesignObject => ({
  id,
  layer: RED,
  transform: [1, 0, 0, 1, 0, 0],
  locked: false,
  hidden: false,
  group,
  kind: { type: 'vector', paths: [polylineBez([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], true)] },
});

const box = (extra = {}) => mockGenerate('box', { width_mm: 60, depth_mm: 40, height_mm: 30, ...extra }, 300, 5);

describe('hueco libre en la cama', () => {
  const bed = [300, 200] as const;

  it('con la cama vacía va al margen', () => {
    expect(freeSpot([], 100, 50, bed)).toEqual([5, 5]);
  });

  it('esquiva lo que ya hay: primero a la derecha, luego arriba', () => {
    const docBox: Document = { ...emptyDocument(), objects: [rect('a', 5, 5, 120, 60)] };
    const spot = freeSpot(occupiedBounds(docBox), 100, 50, bed)!;
    expect(spot).toEqual([130, 5]);
    // Sin sitio a la derecha: arriba.
    const wide: Document = { ...emptyDocument(), objects: [rect('a', 5, 5, 280, 60)] };
    expect(freeSpot(occupiedBounds(wide), 100, 50, bed)).toEqual([5, 70]);
  });

  it('el resultado nunca solapa y cabe en la cama; si no hay sitio, null', () => {
    const objects = [rect('a', 5, 5, 140, 90), rect('b', 150, 5, 100, 120)];
    const taken = objects.map((o) => objectBounds(o)!);
    const spot = freeSpot(taken, 120, 60, bed)!;
    const [x, y] = spot;
    expect(x + 120).toBeLessThanOrEqual(295);
    expect(y + 60).toBeLessThanOrEqual(195);
    for (const [[x0, y0], [x1, y1]] of taken) {
      expect(x >= x1 + 5 || x + 120 + 5 <= x0 || y >= y1 + 5 || y + 60 + 5 <= y0).toBe(true);
    }
    expect(freeSpot(taken, 400, 60, bed)).toBeNull();
  });
});

describe('piezas como objetos', () => {
  it('un objeto de corte por pieza, en su grupo y capa, con las medidas del motor', () => {
    const r = box();
    const taken = new Set<string>();
    const objs = partObjects(r, { generator: 'box', cutLayer: RED, engraveLayer: null, group: 'grupo-1', origin: [10, 20], taken });
    expect(objs).toHaveLength(6);
    expect(new Set(objs.map((o) => o.group))).toEqual(new Set(['grupo-1']));
    expect(new Set(objs.map((o) => o.layer))).toEqual(new Set([RED]));
    expect(new Set(objs.map((o) => o.id)).size).toBe(6);
    expect(objs.every((o) => o.kind.type === 'vector' && !o.locked)).toBe(true);
    // La caja envolvente del conjunto empieza en el origen pedido.
    const boxes = objs.map((o) => objectBounds(o)!);
    expect(Math.min(...boxes.map((b) => b[0][0]))).toBeCloseTo(10, 6);
    expect(Math.min(...boxes.map((b) => b[0][1]))).toBeCloseTo(20, 6);
    // El frente mide 60 × 30.
    const front = objs.find((o) => o.id.startsWith('box-front'))!;
    const [[x0, y0], [x1, y1]] = objectBounds(front)!;
    expect([x1 - x0, y1 - y0]).toEqual([60, 30]);
  });

  it('los rótulos van en su capa solo si hay capa de grabado', () => {
    const r = box({ labels: true });
    const make = (engraveLayer: string | null) =>
      partObjects(r, { generator: 'box', cutLayer: RED, engraveLayer, group: 'g', origin: [0, 0], taken: new Set() });
    expect(make(null)).toHaveLength(6);
    const withLabels = make(BLUE);
    expect(withLabels).toHaveLength(12);
    expect(withLabels.filter((o) => o.layer === BLUE)).toHaveLength(6);
  });

  it('los ids no chocan con los que ya hay', () => {
    const taken = new Set(['box-front-1']);
    const objs = partObjects(box(), { generator: 'box', cutLayer: RED, engraveLayer: null, group: 'g', origin: [0, 0], taken });
    expect(objs.map((o) => o.id)).not.toContain('box-front-1');
    expect(objs.map((o) => o.id)).toContain('box-front-2');
  });

  it('grupo nuevo con la convención grupo-N', () => {
    expect(freshGroupId([])).toBe('grupo-1');
    expect(freshGroupId([rect('a', 0, 0, 1, 1, 'grupo-1'), rect('b', 0, 0, 1, 1, 'grupo-2/grupo-3')])).toBe('grupo-4');
  });
});

describe('insertar y regenerar en el documento', () => {
  const record: GeneratorRecord = { group: 'grupo-1', generator: 'box', params: { width_mm: 60 }, cut_layer: RED, engrave_layer: null };
  const base: Document = { ...emptyDocument(), layers: [makeLayer(RED, 'Rojo')], objects: [rect('x', 0, 0, 5, 5)] };

  it('agrega objetos, crea la capa que falta y guarda el registro', () => {
    const objects = partObjects(box({ labels: true }), { generator: 'box', cutLayer: RED, engraveLayer: BLUE, group: 'grupo-1', origin: [0, 0], taken: new Set(['x']) });
    const newLayers = [engraveLayerFor(BLUE, 'Rótulos')];
    const doc = applyInsert(base, { objects, record, newLayers }, null);
    expect(doc.objects).toHaveLength(1 + objects.length);
    const blue = doc.layers.find((l) => l.color === BLUE)!;
    expect(blue.name).toBe('Rótulos');
    // La capa de rótulos nueva no atraviesa: poca potencia, de líneas.
    expect(blue.mode).toBe('line');
    expect(blue.power).toBeLessThanOrEqual(20);
    expect(doc.generators).toEqual([record]);
    // El original no se tocó.
    expect(base.objects).toHaveLength(1);
    expect(base.generators ?? []).toEqual([]);
  });

  it('regenerar quita las piezas del grupo y reemplaza su registro', () => {
    const first = partObjects(box(), { generator: 'box', cutLayer: RED, engraveLayer: null, group: 'grupo-1', origin: [10, 10], taken: new Set(['x']) });
    const withBox = applyInsert(base, { objects: first, record, newLayers: [] }, null);
    const origin = groupOrigin(withBox, 'grupo-1')!;
    expect(origin[0]).toBeCloseTo(10, 6);
    expect(origin[1]).toBeCloseTo(10, 6);
    const again = partObjects(box({ width_mm: 80 }), { generator: 'box', cutLayer: RED, engraveLayer: null, group: 'grupo-1', origin, taken: new Set(['x']) });
    const next = { ...record, params: { width_mm: 80 } };
    const doc = applyInsert(withBox, { objects: again, record: next, newLayers: [] }, 'grupo-1');
    expect(doc.objects).toHaveLength(1 + again.length);
    expect(doc.objects.filter((o) => o.group === 'grupo-1')).toHaveLength(6);
    expect(doc.generators).toEqual([next]);
    const [ox, oy] = groupOrigin(doc, 'grupo-1')!;
    expect([Math.round(ox * 1e6) / 1e6, Math.round(oy * 1e6) / 1e6]).toEqual([10, 10]);
  });

  it('sin objetos devuelve el mismo documento', () => {
    expect(applyInsert(base, { objects: [], record, newLayers: [] }, null)).toBe(base);
  });
});

describe('qué registro corresponde a la selección', () => {
  const record: GeneratorRecord = { group: 'grupo-2', generator: 'hinge', params: {}, cut_layer: RED, engrave_layer: null };
  const doc: Document = { ...emptyDocument(), objects: [rect('a', 0, 0, 1, 1, 'grupo-2'), rect('b', 0, 0, 1, 1, 'grupo-9/grupo-2'), rect('c', 0, 0, 1, 1, null)], generators: [record] };

  it('lo halla por la cadena de grupos, aunque esté anidado', () => {
    expect(recordFor(doc, ['a'])).toEqual(record);
    expect(recordFor(doc, ['a', 'b'])).toEqual(record);
  });

  it('no hay registro si la selección incluye algo ajeno o está vacía', () => {
    expect(recordFor(doc, ['a', 'c'])).toBeNull();
    expect(recordFor(doc, ['c'])).toBeNull();
    expect(recordFor(doc, [])).toBeNull();
    expect(recordFor({ ...doc, generators: undefined }, ['a'])).toBeNull();
  });
});

describe('colores de capa', () => {
  it('el color nuevo no lo usa ninguna capa ni la de corte', () => {
    const layers = [makeLayer(RED, 'Rojo')];
    const c = unusedColor(layers, [BLUE]);
    expect([RED, BLUE]).not.toContain(c);
  });
});
