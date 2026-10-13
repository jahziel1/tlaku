import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { bezBounds, roundedRectBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { objectCurves } from '@/lib/geometry';
import { createAppStore } from '@/store';
import { applyTraceToDoc, placeTrace, traceLayerFor } from './placement';

/** Imagen de 40 x 20 mm, escalada x2 / x3 y movida a (10, 20). */
const image = (over: Partial<DesignObject> = {}): DesignObject => ({
  id: 'img-1',
  layer: '#808080',
  transform: [2, 0, 0, 3, 10, 20],
  kind: { type: 'image', source_path: '/tmp/a.png', width_px: 400, height_px: 200, size_mm: [40, 20] },
  locked: false,
  hidden: false,
  group: null,
  ...over,
});

const doc = (objects: DesignObject[], layers = [makeLayer('#808080', 'Foto', { mode: 'image' })]): Document => ({
  version: 2,
  objects,
  layers,
  device_id: null,
});

/** Trazo local: la página completa y un cuadrado en la esquina inferior izquierda. */
const fullPage = [roundedRectBez(0, 0, 40, 20)];
const corner = [roundedRectBez(0, 0, 4, 4)];

describe('placeTrace', () => {
  it('copia el transform de la imagen: el trazo cubre exactamente su caja en el documento', () => {
    const img = image();
    const vec = placeTrace(img, fullPage, '#ff3b4a', 'trace-1');
    const imgBox = bezBounds(objectCurves(img));
    const vecBox = bezBounds(objectCurves(vec));
    expect(vecBox).toEqual(imgBox);
    // 40 x 20 mm con escala 2 x 3 en (10, 20): de (10, 20) a (90, 80).
    expect(vecBox).toEqual([[10, 20], [90, 80]]);
  });

  it('un punto local cae donde la imagen lo pondría (escala y traslación)', () => {
    const vec = placeTrace(image(), corner, '#ff3b4a', 'trace-1');
    // Esquina inferior izquierda local (0,0) -> (10, 20); (4,4) -> (2*4+10, 3*4+20) = (18, 32).
    expect(bezBounds(objectCurves(vec))).toEqual([[10, 20], [18, 32]]);
  });

  it('respeta un giro de 90 grados y no comparte el arreglo del transform', () => {
    const img = image({ transform: [0, 1, -1, 0, 50, 0] });
    const vec = placeTrace(img, fullPage, '#ff3b4a', 'trace-1');
    expect(bezBounds(objectCurves(vec))).toEqual(bezBounds(objectCurves(img)));
    expect(vec.transform).not.toBe(img.transform);
  });
});

describe('applyTraceToDoc', () => {
  it('agrega el vector justo encima de la imagen, crea la capa y conserva la imagen', () => {
    const other = { ...image({ id: 'otro' }), transform: [1, 0, 0, 1, 0, 0] as DesignObject['transform'] };
    const before = doc([image(), other]);
    const { doc: after, id } = applyTraceToDoc(before, { imageId: 'img-1', paths: corner, layer: '#ff3b4a', deleteImage: false });
    expect(id).toBe('trace-1');
    expect(after.objects.map((o) => o.id)).toEqual(['img-1', 'trace-1', 'otro']);
    expect(after.layers.some((l) => l.color === '#ff3b4a')).toBe(true);
    expect(after.objects[1]!.layer).toBe('#ff3b4a');
  });

  it('borra la imagen si se pide, salvo que esté bloqueada', () => {
    const opts = { imageId: 'img-1', paths: corner, layer: '#ff3b4a', deleteImage: true };
    expect(applyTraceToDoc(doc([image()]), opts).doc.objects.map((o) => o.id)).toEqual(['trace-1']);
    expect(applyTraceToDoc(doc([image({ locked: true })]), opts).doc.objects.map((o) => o.id)).toEqual(['img-1', 'trace-1']);
  });

  it('no cambia nada sin formas o si la imagen ya no existe', () => {
    const d = doc([image()]);
    expect(applyTraceToDoc(d, { imageId: 'img-1', paths: [], layer: '#ff3b4a', deleteImage: true }).doc).toBe(d);
    expect(applyTraceToDoc(d, { imageId: 'nada', paths: corner, layer: '#ff3b4a', deleteImage: true }).id).toBeNull();
  });

  it('elige un id libre', () => {
    const taken = { ...image({ id: 'trace-1' }), kind: { type: 'vector' as const, paths: [] } };
    const { id } = applyTraceToDoc(doc([image(), taken]), { imageId: 'img-1', paths: corner, layer: '#ff3b4a', deleteImage: false });
    expect(id).toBe('trace-2');
  });

  it('en el store es un solo paso de deshacer, incluso borrando la imagen', () => {
    const store = createAppStore();
    store.getState().replaceDocument(doc([image()]), null);
    const original = store.getState().doc;
    const out: { id: string | null } = { id: null };
    store.getState().applyDocChange('trace.history', (d) => {
      const r = applyTraceToDoc(d, { imageId: 'img-1', paths: corner, layer: '#ff3b4a', deleteImage: true });
      out.id = r.id;
      return r.doc;
    });
    store.getState().select([out.id!]);
    const s = store.getState();
    expect(s.past).toHaveLength(1);
    expect(s.doc.objects.map((o) => o.id)).toEqual(['trace-1']);
    expect(s.selectedIds).toEqual(['trace-1']);
    s.undo();
    expect(store.getState().doc).toBe(original);
    expect(store.getState().selectedIds).toEqual([]);
  });
});

describe('traceLayerFor', () => {
  const line = makeLayer('#2f7bff', 'Azul');
  const img = makeLayer('#808080', 'Foto', { mode: 'image' });
  it('usa la capa activa si no es de imagen', () => {
    expect(traceLayerFor([img, line], '#2f7bff')).toBe('#2f7bff');
    expect(traceLayerFor([img], '#ff3b4a')).toBe('#ff3b4a'); // aún no existe: se crea
  });
  it('si la activa es de imagen o no hay, usa la primera capa de líneas', () => {
    expect(traceLayerFor([img, line], '#808080')).toBe('#2f7bff');
    expect(traceLayerFor([img, line], null)).toBe('#2f7bff');
  });
  it('sin capas de líneas, un color libre de la paleta', () => {
    const color = traceLayerFor([img], null);
    expect(color).not.toBe('#808080');
    expect(color).toMatch(/^#[0-9a-f]{6}$/i);
  });
});
