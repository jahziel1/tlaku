import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { roundedRectBez } from '@/lib/bezpath';
import { applyMaskToDoc, invertAffine, maskEnabled, maskPaths, removeMaskFromDoc, splitMaskSelection, unmaskEnabled } from './mask';

const image = (patch: Partial<DesignObject> = {}): DesignObject => ({
  id: 'img',
  layer: '#808080',
  transform: [2, 0, 0, 2, 10, 20],
  kind: { type: 'image', source_path: '/a.png', width_px: 10, height_px: 10, size_mm: [10, 10] },
  locked: false,
  hidden: false,
  group: null,
  ...patch,
});

const shape = (id = 's'): DesignObject => ({
  id,
  layer: '#000000',
  // Cuadrado de 5x5 mm en el documento: de (14, 24) a (19, 29) (local de la imagen: 2..4.5).
  transform: [1, 0, 0, 1, 14, 24],
  kind: { type: 'vector', paths: [roundedRectBez(0, 0, 5, 5)] },
  locked: false,
  hidden: false,
  group: null,
});

const doc = (objects: DesignObject[]): Document => ({ objects, layers: [] }) as unknown as Document;

describe('máscara de imagen', () => {
  it('la inversa deshace la afín', () => {
    const m = invertAffine([2, 0, 0, 2, 10, 20])!;
    expect(m).toEqual([0.5, -0, -0, 0.5, -5, -10]);
    expect(invertAffine([0, 0, 0, 0, 1, 1])).toBeNull();
  });

  it('pasa la forma a coordenadas locales de la imagen', () => {
    const [p] = maskPaths(image(), [shape()]);
    const first = p![0]!;
    expect(first.type).toBe('move_to');
    const pts = p!.flatMap((s) => (s.type === 'close' ? [] : [(s as { to: [number, number] }).to]));
    const xs = pts.map((q) => q[0]);
    const ys = pts.map((q) => q[1]);
    expect(Math.min(...xs)).toBeCloseTo(2);
    expect(Math.max(...xs)).toBeCloseTo(4.5);
    expect(Math.min(...ys)).toBeCloseTo(2);
    expect(Math.max(...ys)).toBeCloseTo(4.5);
  });

  it('exige una imagen y al menos una forma, y explica qué falta', () => {
    expect(maskEnabled([shape()])).toBe('trace.mask.reason.needImage');
    expect(maskEnabled([image()])).toBe('trace.mask.reason.needShape');
    expect(maskEnabled([image(), image({ id: 'b' })])).toBe('trace.mask.reason.oneImage');
    expect(maskEnabled([image({ locked: true }), shape()])).toBe('trace.mask.reason.locked');
    expect(maskEnabled([image(), shape()])).toBe(true);
    expect(splitMaskSelection([image(), shape('a'), shape('b')])?.shapes).toHaveLength(2);
    expect(splitMaskSelection([shape()])).toBeNull();
  });

  it('aplica la máscara, quita las formas y se deshace con quitar', () => {
    const d = doc([image(), shape(), shape('otra')]);
    const masked = applyMaskToDoc(d, 'img', ['s']);
    expect(masked.objects.map((o) => o.id)).toEqual(['img', 'otra']);
    const img = masked.objects[0]!;
    expect(img.kind.type === 'image' && img.kind.mask?.length).toBe(1);
    expect(unmaskEnabled([img])).toBe(true);
    const clean = removeMaskFromDoc(masked, ['img']);
    const back = clean.objects[0]!;
    expect(back.kind.type === 'image' && 'mask' in back.kind).toBe(false);
    expect(unmaskEnabled([back])).toBe('trace.mask.reason.noMask');
  });

  it('no cambia el documento si no hay imagen o forma', () => {
    const d = doc([image()]);
    expect(applyMaskToDoc(d, 'img', ['nada'])).toBe(d);
    expect(applyMaskToDoc(d, 'nada', ['s'])).toBe(d);
  });
});
