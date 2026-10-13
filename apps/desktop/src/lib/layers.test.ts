import { afterEach, describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { sampleDocument } from '@/api/mock/sample';
import {
  DEFAULT_IMAGE_SETTINGS,
  DEFAULT_TAB_SETTINGS,
  layerFeatures,
  normalizeLayer,
  normalizeLayers,
  usesZ,
  assignObjectsToLayer,
  countByLayer,
  dpiToInterval,
  ensureLayerIn,
  intervalToDpi,
  factoryLayer,
  makeLayer,
  newLayerFor,
  applyTemplate,
  layerTemplate,
  sanitizeLayerTemplate,
  setLayerDefaults,
  newSubLayer,
  MAX_SUB_LAYERS,
} from './layers';
import { GUIDE_COLOR } from './palette';
import { polylineBez } from '@/lib/bezpath';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';

function obj(id: string, layer: string, over: Partial<DesignObject> = {}): DesignObject {
  return {
    id,
    layer,
    transform: [1, 0, 0, 1, 0, 0],
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [1, 1]], false)] },
    locked: false,
    hidden: false,
  group: null,
    ...over,
  };
}

const doc = (objects: DesignObject[], layers = [makeLayer(RED, 'Rojo')]): Document => ({ version: 2, objects, layers, device_id: null });

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

describe('capas de documentos viejos (Fase 4)', () => {
  const old = (): Record<string, unknown> => {
    const l: Record<string, unknown> = { ...makeLayer('#ff0000', 'Rojo') };
    for (const k of ['fill_style', 'tabs', 'z_offset', 'z_step']) delete l[k];
    l.image = { mode: 'floyd', threshold: 0.4, brightness: 0, contrast: 0, gamma: 1, invert: false, dot_width: 0 };
    return l;
  };

  it('completa lo que falta con los valores por defecto y conserva lo que había', () => {
    const l = normalizeLayer(old() as never);
    expect(l.fill_style).toBe('scan');
    expect(l.tabs).toEqual(DEFAULT_TAB_SETTINGS);
    expect(l.z_offset).toBe(0);
    expect(l.z_step).toBe(0);
    expect(l.image.halftone_cell).toBe(0.5);
    expect(l.image.halftone_angle).toBe(45);
    expect(l.image.mode).toBe('floyd');
    expect(l.image.threshold).toBe(0.4);
  });

  it('una capa completa vuelve como el mismo objeto', () => {
    const l = makeLayer('#00f', 'Azul');
    expect(normalizeLayer(l)).toBe(l);
  });

  it('normalizeLayers solo crea un documento nuevo si hizo falta', () => {
    const doc = { ...sampleDocument(), layers: [makeLayer('#00f', 'Azul')] };
    expect(normalizeLayers(doc)).toBe(doc);
    const viejo = { ...doc, layers: [old() as never] };
    const fixed = normalizeLayers(viejo);
    expect(fixed).not.toBe(viejo);
    expect(fixed.layers[0]!.tabs.count).toBe(4);
  });
});

describe('rasgos de una capa para la lista', () => {
  it('puentes solo en line, contornos solo en fill, Z solo si la máquina lo tiene', () => {
    const tabs = { ...DEFAULT_TAB_SETTINGS, enabled: true };
    expect(layerFeatures(makeLayer('#f00', 'a', { tabs }), false)).toEqual(['tabs']);
    expect(layerFeatures(makeLayer('#f00', 'a', { mode: 'fill', tabs }), false)).toEqual([]);
    expect(layerFeatures(makeLayer('#f00', 'a', { mode: 'fill', fill_style: 'offset' }), false)).toEqual(['offset']);
    expect(layerFeatures(makeLayer('#f00', 'a', { mode: 'line', fill_style: 'offset' }), false)).toEqual([]);
    const z = makeLayer('#f00', 'a', { z_step: 0.5 });
    expect(layerFeatures(z, false)).toEqual([]);
    expect(layerFeatures(z, true)).toEqual(['z']);
    expect(usesZ(z)).toBe(true);
    expect(usesZ(makeLayer('#f00', 'a'))).toBe(false);
  });
});

describe('valores por defecto del usuario (C35)', () => {
  afterEach(() => setLayerDefaults(null));

  const custom = () => layerTemplate(makeLayer('#000', 'x', { speed: 2400, power: 33, mode: 'fill', ramp: { enabled: true, length_mm: 3, start_pct: 20, outer_only: false } }));

  it('sin valores propios, makeLayer da los de fábrica', () => {
    expect(makeLayer('#ff0000', 'Rojo')).toEqual(factoryLayer('#ff0000', 'Rojo'));
  });

  it('makeLayer y newLayerFor los usan, conservando color y nombre', () => {
    setLayerDefaults(custom());
    const l = makeLayer('#ff0000', 'Rojo');
    expect(l).toMatchObject({ color: '#ff0000', name: 'Rojo', speed: 2400, power: 33, mode: 'fill', output: true, visible: true, order: null });
    expect(l.ramp).toEqual({ enabled: true, length_mm: 3, start_pct: 20, outer_only: false });
    expect(newLayerFor('#2f7bff')).toMatchObject({ speed: 2400, mode: 'fill' });
  });

  it('el patch manda sobre los valores por defecto', () => {
    setLayerDefaults(custom());
    expect(makeLayer('#f00', 'x', { speed: 100 })).toMatchObject({ speed: 100, power: 33 });
  });

  it('la capa de guías sigue naciendo sin salida', () => {
    setLayerDefaults(custom());
    expect(newLayerFor(GUIDE_COLOR).output).toBe(false);
  });

  it('completar una capa vieja usa los valores de fábrica, no los del usuario', () => {
    setLayerDefaults(custom());
    const vieja: Record<string, unknown> = { ...factoryLayer('#00f', 'Vieja') };
    for (const k of ['ramp', 'perforation', 'sub_layers', 'overcut']) delete vieja[k];
    const n = normalizeLayer(vieja as never);
    expect(n.ramp.enabled).toBe(false);
    expect(n.speed).toBe(1000);
    expect(n.sub_layers).toEqual([]);
  });

  it('las capas que ya existen no cambian al guardar los valores', () => {
    const before = makeLayer('#f00', 'x');
    setLayerDefaults(custom());
    expect(before.speed).toBe(1000);
  });
});

describe('plantilla de ajustes de una capa', () => {
  it('lleva todo menos color, nombre, posición, visibilidad y salida', () => {
    const l = makeLayer('#f00', 'Rojo', { order: 3, output: false, visible: false, speed: 777 });
    const t = layerTemplate(l);
    expect(t.speed).toBe(777);
    for (const k of ['color', 'name', 'order', 'visible', 'output']) expect(k in t, k).toBe(false);
  });

  it('applyTemplate conserva la identidad de la capa y no comparte objetos con la plantilla', () => {
    const t = layerTemplate(makeLayer('#f00', 'Rojo', { speed: 777, sub_layers: [makeLayer('#f00', '', { mode: 'fill' })] }));
    const dst = makeLayer('#00f', 'Azul', { order: 2, output: false });
    const out = applyTemplate(dst, t);
    expect(out).toMatchObject({ color: '#00f', name: 'Azul', order: 2, output: false, speed: 777 });
    expect(out.sub_layers[0]).toMatchObject({ mode: 'fill', color: '#00f' });
    expect(out.image).not.toBe(t.image);
    expect(out.sub_layers[0]).not.toBe(t.sub_layers[0]);
  });
});

describe('saneado de valores por defecto leídos del disco', () => {
  it('lo que no es un objeto no sirve', () => {
    for (const bad of [null, 5, 'x', [], undefined]) expect(sanitizeLayerTemplate(bad)).toBeNull();
  });

  it('completa lo que falta y descarta lo que no es del tipo esperado', () => {
    const t = sanitizeLayerTemplate({ speed: 1500, power: '50', mode: 'laser', fill_style: 'flood', ramp: { enabled: true, length_mm: Number.NaN }, perforation: 7, inventado: 1 })!;
    expect(t.speed).toBe(1500);
    expect(t.power).toBe(50); // texto: toma el valor de fábrica
    expect(t.mode).toBe('line'); // modo desconocido
    expect(t.fill_style).toBe('flood');
    expect(t.ramp).toEqual({ enabled: true, length_mm: 1, start_pct: 0, outer_only: false });
    expect(t.perforation).toEqual(factoryLayer('#000', '').perforation);
    expect('inventado' in t).toBe(false);
  });

  it('una plantilla válida pasa sin cambios, también al ir y volver por JSON', () => {
    const t = layerTemplate(makeLayer('#f00', 'x', { speed: 321, lead_in: { enabled: true, kind: 'arc', length: 2, angle: 30 }, overscan: 3 }));
    expect(sanitizeLayerTemplate(JSON.parse(JSON.stringify(t)))).toEqual(t);
  });

  it('limita las operaciones adicionales y las deja sin las suyas', () => {
    const sub = makeLayer('#f00', '', { sub_layers: [makeLayer('#f00', '')] });
    const t = sanitizeLayerTemplate({ sub_layers: Array.from({ length: MAX_SUB_LAYERS + 5 }, () => sub) })!;
    expect(t.sub_layers).toHaveLength(MAX_SUB_LAYERS);
    expect(t.sub_layers.every((s) => s.sub_layers.length === 0)).toBe(true);
  });
});

describe('operaciones adicionales y documentos viejos', () => {
  it('una operación nueva hereda velocidad, potencia e intervalo de la capa y empieza limpia', () => {
    const parent = makeLayer('#f00', 'x', { speed: 321, power: 12, interval: 0.2, ramp: { enabled: true, length_mm: 1, start_pct: 0, outer_only: false } });
    const sub = newSubLayer(parent, 'fill');
    expect(sub).toMatchObject({ mode: 'fill', speed: 321, power: 12, interval: 0.2, color: '#f00', sub_layers: [] });
    expect(sub.ramp.enabled).toBe(false);
  });

  it('normalizeLayer completa los grupos de la ola 2 y las operaciones adicionales incompletas', () => {
    const vieja: Record<string, unknown> = { ...makeLayer('#f00', 'x'), lead_in: { enabled: true }, sub_layers: [{ color: '#f00', mode: 'fill' }] };
    const n = normalizeLayer(vieja as never);
    expect(n.lead_in).toEqual({ enabled: true, kind: 'straight', length: 1, angle: 45 });
    expect(n.sub_layers[0]).toMatchObject({ mode: 'fill', speed: 1000, ramp: { enabled: false } });
  });
});
