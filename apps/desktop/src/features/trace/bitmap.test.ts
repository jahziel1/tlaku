import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { roundedRectBez } from '@/lib/bezpath';
import { applyBitmapToDoc, bitmapEnabled, bitmapSourceIds, imageLayerColor, projectDir, projectStem } from './bitmap';
import type { BitmapResult } from './types';

const vec = (id: string, locked = false): DesignObject => ({
  id,
  layer: '#000000',
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths: [roundedRectBez(0, 0, 10, 10)] },
  locked,
  hidden: false,
  group: null,
});
const doc = (objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null }) as unknown as Document;
const result: BitmapResult = { path: '/p/a-bitmap-1.png', width_px: 100, height_px: 50, size_mm: [20, 10], origin_mm: [5, 7], in_temp_dir: false };

describe('convertir a mapa de bits', () => {
  it('saca la carpeta y el nombre del proyecto', () => {
    expect(projectDir('/home/ana/rotulo.tlaku')).toBe('/home/ana');
    expect(projectDir('C:\\obras\\rotulo.tlaku')).toBe('C:\\obras');
    expect(projectDir(null)).toBeNull();
    expect(projectStem('/home/ana/rotulo.tlaku', 'x')).toBe('rotulo');
    expect(projectStem(null, 'Sin título')).toBe('Sin título');
  });

  it('pide formas vectoriales', () => {
    expect(bitmapEnabled([])).toBe('trace.bitmap.reason.needShapes');
    expect(bitmapEnabled([vec('a')])).toBe(true);
    expect(bitmapSourceIds([vec('a'), vec('b')])).toEqual(['a', 'b']);
  });

  it('pone la imagen en su sitio, en una capa de imagen, y quita los vectores', () => {
    const r = applyBitmapToDoc(doc([vec('a'), vec('b'), vec('c')]), result, { sourceIds: ['a', 'b'], replace: true });
    expect(r.doc.objects.map((o) => o.id)).toEqual(['c', r.id]);
    const img = r.doc.objects.find((o) => o.id === r.id)!;
    expect(img.transform).toEqual([1, 0, 0, 1, 5, 7]);
    expect(img.kind).toMatchObject({ type: 'image', source_path: '/p/a-bitmap-1.png', size_mm: [20, 10] });
    expect(r.doc.layers.find((l) => l.color === img.layer)?.mode).toBe('image');
    expect(imageLayerColor(r.doc)).toBe(img.layer);
  });

  it('sin reemplazar conserva los vectores, y un objeto bloqueado nunca se quita', () => {
    const keep = applyBitmapToDoc(doc([vec('a'), vec('b')]), result, { sourceIds: ['a'], replace: false });
    expect(keep.doc.objects.map((o) => o.id)).toEqual(['a', keep.id, 'b']);
    const locked = applyBitmapToDoc(doc([vec('a', true)]), result, { sourceIds: ['a'], replace: true });
    expect(locked.doc.objects).toHaveLength(2);
  });
});
