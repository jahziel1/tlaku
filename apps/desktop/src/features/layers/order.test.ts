import { describe, expect, it } from 'vitest';
import type { DesignObject, Document, LayerSettings } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { GUIDE_COLOR } from '@/lib/palette';
import { buildExecution, classifyLayer, executionOf, layerSequence, localOrderNote, moveLayerInDoc, orderConflict } from './order';
import { countByLayer } from '@/lib/layers';

const C = { cut: '#ff3b4a', engrave: '#2f7bff', mark: '#ffb020', photo: '#9aa4b2', extra: '#14c28a' };

const obj = (id: string, layer: string, image = false): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, 0, 0],
  kind: image
    ? { type: 'image', source_path: 'a.png', width_px: 1, height_px: 1, size_mm: [1, 1] }
    : { type: 'vector', paths: [{ points: [[0, 0], [1, 1]], closed: false }] },
  locked: false,
  hidden: false,
});

/** Corte (kerf), grabado (fill), marcas (line) e imagen, todos con objetos. */
function sample(over: Record<string, Partial<LayerSettings>> = {}): Document {
  const l = (c: string, name: string, patch: Partial<LayerSettings>) => makeLayer(c, name, { ...patch, ...over[c] });
  return {
    version: 1,
    device_id: null,
    layers: [
      l(C.cut, 'Corte', { kerf: 0.1 }),
      l(C.engrave, 'Grabado', { mode: 'fill' }),
      l(C.mark, 'Marcas', {}),
      l(C.photo, 'Foto', { mode: 'image' }),
    ],
    objects: [obj('a', C.cut), obj('b', C.engrave), obj('c', C.mark), obj('d', C.photo, true)],
  };
}

const colors = (doc: Document, mode: 'auto' | 'file' = 'auto') =>
  executionOf(doc, mode)
    .filter((e) => e.runs)
    .map((e) => e.layer.color);

describe('clasificación', () => {
  it('relleno e imagen son grabados; la línea con kerf o con mucha potencia y pasadas es corte', () => {
    const c = (patch: Partial<LayerSettings>) => classifyLayer(makeLayer('#000000', 'x', patch));
    expect(c({ mode: 'fill' })).toEqual({ role: 'engrave', reason: null });
    expect(c({ mode: 'image' }).role).toBe('engrave');
    expect(c({})).toEqual({ role: 'line', reason: null });
    expect(c({ kerf: -0.1 })).toEqual({ role: 'cut', reason: 'kerf' });
    expect(c({ power: 80, passes: 2 })).toEqual({ role: 'cut', reason: 'power' });
    expect(c({ power: 80, passes: 1 }).role).toBe('line');
    expect(c({ power: 79, passes: 5 }).role).toBe('line');
  });
});

describe('orden automático', () => {
  it('grabados primero, líneas después y cortes al final, conservando el orden del archivo dentro de cada grupo', () => {
    expect(colors(sample())).toEqual([C.engrave, C.photo, C.mark, C.cut]);
  });

  it('con "orden del archivo" se respeta la lista tal cual', () => {
    expect(colors(sample(), 'file')).toEqual([C.cut, C.engrave, C.mark, C.photo]);
  });

  it('replica layer_sequence del motor (mismos casos que sus pruebas)', () => {
    const roles = ['cut', 'line', 'engrave', 'line', 'engrave', 'cut'] as const;
    const none = Array<number | null>(6).fill(null);
    expect(layerSequence(roles, none, 'auto')).toEqual([2, 4, 1, 3, 0, 5]);
    expect(layerSequence(roles, none, 'file')).toEqual([0, 1, 2, 3, 4, 5]);
    const three = ['cut', 'engrave', 'line'] as const;
    expect(layerSequence(three, [1, null, null], 'auto')).toEqual([0, 1, 2]);
    expect(layerSequence(three, [null, 3, 1], 'auto')).toEqual([2, 0, 1]);
    expect(layerSequence(three, [null, 9, null], 'file')).toEqual([0, 2, 1]);
  });

  it('una posición fijada manda sobre el orden automático', () => {
    const doc = sample({ [C.cut]: { order: 1 } });
    expect(colors(doc)).toEqual([C.cut, C.engrave, C.photo, C.mark]);
  });

  it('las capas sin salida, sin objetos o de guías no se ejecutan y van al final de la lista', () => {
    const doc = sample({ [C.mark]: { output: false } });
    doc.layers.push(makeLayer(C.extra, 'Vacía'), makeLayer(GUIDE_COLOR, 'Guías', { output: false }));
    doc.objects.push(obj('g', GUIDE_COLOR));
    const entries = executionOf(doc, 'auto');
    expect(entries.map((e) => e.layer.color)).toEqual([C.engrave, C.photo, C.cut, C.mark, C.extra, GUIDE_COLOR]);
    expect(entries.map((e) => e.skip)).toEqual([null, null, null, 'off', 'empty', 'guide']);
    expect(entries.map((e) => e.position)).toEqual([1, 2, 3, null, null, null]);
  });

  it('los objetos ocultos no cuentan para que una capa se ejecute', () => {
    const doc = sample();
    doc.objects = doc.objects.map((o) => (o.layer === C.mark ? { ...o, hidden: true } : o));
    expect(colors(doc)).toEqual([C.engrave, C.photo, C.cut]);
  });
});

describe('explicación local del orden', () => {
  const note = (doc: Document, mode: 'auto' | 'file' = 'auto') => localOrderNote(executionOf(doc, mode), mode);

  it('nombra los grupos y por qué una capa es corte', () => {
    expect(note(sample())).toBe('Orden automático: grabados primero, líneas después, cortes al final (Corte: corte porque tiene kerf).');
  });

  it('dos grupos', () => {
    const doc = sample();
    doc.layers = doc.layers.filter((l) => l.color !== C.mark);
    expect(note(doc)).toContain('grabados primero, cortes al final');
  });

  it('capas del mismo tipo', () => {
    const doc = sample({ [C.cut]: { kerf: 0 }, [C.engrave]: { mode: 'line' }, [C.photo]: { mode: 'line' } });
    doc.objects = doc.objects.map((o) => ({ ...o, kind: { type: 'vector', paths: [] } }));
    expect(note(doc)).toContain('mismo tipo');
  });

  it('orden del archivo y posiciones fijadas', () => {
    expect(note(sample(), 'file')).toContain('Orden del archivo');
    expect(note(sample({ [C.mark]: { order: 2 } }))).toContain('Posición fijada: Marcas en la 2');
  });

  it('con una sola capa no hay nota', () => {
    const doc = sample();
    doc.layers = [doc.layers[0]!];
    expect(note(doc)).toBeNull();
  });
});

describe('posiciones fijadas', () => {
  it('detecta otra capa con la misma posición', () => {
    const doc = sample({ [C.cut]: { order: 2 } });
    expect(orderConflict(doc.layers, C.mark, 2)?.color).toBe(C.cut);
    expect(orderConflict(doc.layers, C.cut, 2)).toBeNull();
    expect(orderConflict(doc.layers, C.mark, 3)).toBeNull();
  });
});

describe('mover una capa', () => {
  it('dentro de su grupo solo cambia el orden del archivo (sin fijar nada)', () => {
    const doc = sample();
    // ejecución: engrave, photo, mark, cut -> la foto pasa antes que el grabado
    const moved = moveLayerInDoc(doc, 'auto', C.photo, 0);
    expect(colors(moved)).toEqual([C.photo, C.engrave, C.mark, C.cut]);
    expect(moved.layers.every((l) => l.order === null)).toBe(true);
  });

  it('un corte antes de un grabado exige fijar su posición', () => {
    const moved = moveLayerInDoc(sample(), 'auto', C.cut, 0);
    expect(colors(moved)).toEqual([C.cut, C.engrave, C.photo, C.mark]);
    expect(moved.layers.find((l) => l.color === C.cut)?.order).toBe(1);
    expect(moved.layers.filter((l) => l.order !== null)).toHaveLength(1);
  });

  it('las demás posiciones fijadas se actualizan para no cambiar de sitio', () => {
    const doc = sample({ [C.mark]: { order: 4 } }); // marcas al final
    const moved = moveLayerInDoc(doc, 'auto', C.photo, 0);
    expect(colors(moved)).toEqual([C.photo, C.engrave, C.cut, C.mark]);
    expect(moved.layers.find((l) => l.color === C.mark)?.order).toBe(4);
  });

  it('el resultado siempre es el orden pedido, para cualquier movimiento', () => {
    const doc = sample({ [C.engrave]: { order: 2 } });
    const n = colors(doc).length;
    for (let from = 0; from < n; from++) {
      for (let to = 0; to < n; to++) {
        const start = colors(doc);
        const color = start[from]!;
        const expected = [...start];
        expected.splice(from, 1);
        expected.splice(to, 0, color);
        const moved = moveLayerInDoc(doc, 'auto', color, to);
        expect(colors(moved), `${from}->${to}`).toEqual(expected);
        const pins = moved.layers.map((l) => l.order).filter((o) => o !== null);
        expect(new Set(pins).size, `${from}->${to} sin posiciones repetidas`).toBe(pins.length);
      }
    }
  });

  it('con orden del archivo basta reordenar la lista', () => {
    const moved = moveLayerInDoc(sample(), 'file', C.cut, 3);
    expect(colors(moved, 'file')).toEqual([C.engrave, C.mark, C.photo, C.cut]);
    expect(moved.layers.every((l) => l.order === null)).toBe(true);
  });

  it('sin cambio devuelve el mismo documento; una capa que no se ejecuta no se mueve', () => {
    const doc = sample({ [C.mark]: { output: false } });
    expect(moveLayerInDoc(doc, 'auto', C.engrave, 0)).toBe(doc);
    expect(moveLayerInDoc(doc, 'auto', C.mark, 0)).toBe(doc);
    expect(moveLayerInDoc(doc, 'auto', '#000000', 0)).toBe(doc);
  });

  it('buildExecution no depende de que los colores vengan en mayúsculas', () => {
    const doc = sample();
    doc.layers = doc.layers.map((l) => ({ ...l, color: l.color.toUpperCase() }));
    const e = buildExecution(doc.layers, countByLayer(doc), 'auto');
    expect(e.filter((x) => x.runs)).toHaveLength(4);
  });
});
