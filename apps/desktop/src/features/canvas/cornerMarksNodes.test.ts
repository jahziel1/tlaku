import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { emptyDocument } from '@/store/document';
import { replacePaths } from './nodeActions';

const chamfered = polylineBez([[0, 0], [10, 0], [10, 8], [8, 10], [0, 10]], true);

function withMark(): DesignObject {
  return {
    id: 'a',
    layer: '#ff0000',
    transform: [1, 0, 0, 1, 0, 0],
    kind: { type: 'vector', paths: [chamfered], corners: [{ vertex: [10, 10], from: [10, 8], to: [8, 10] }] },
    locked: false,
    hidden: false,
    group: null,
  };
}

describe('editar nodos y las esquinas guardadas (D60)', () => {
  it('mover un nodo lejos del chaflán conserva la marca', () => {
    const doc = { ...emptyDocument(), objects: [withMark()] };
    const moved = polylineBez([[0, 0], [10, 0], [10, 8], [8, 10], [-2, 10]], true);
    const out = replacePaths(doc, new Map([['a', [moved]]]));
    const kind = out.objects[0]!.kind;
    expect(kind.type === 'vector' && kind.corners).toHaveLength(1);
  });

  it('mover un nodo del chaflán borra la marca del objeto', () => {
    const doc = { ...emptyDocument(), objects: [withMark()] };
    const moved = polylineBez([[0, 0], [10, 0], [10, 7], [8, 10], [0, 10]], true);
    const out = replacePaths(doc, new Map([['a', [moved]]]));
    const kind = out.objects[0]!.kind;
    expect(kind.type).toBe('vector');
    expect(kind.type === 'vector' && 'corners' in kind).toBe(false);
  });
});
