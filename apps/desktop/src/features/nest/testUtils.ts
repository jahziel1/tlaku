import type { DesignObject, Document } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';

/** Rectángulo cerrado `w`×`h` con la esquina inferior izquierda en (x, y), como objeto vectorial. */
export function rectObject(id: string, w: number, h: number, at: [number, number] = [0, 0], extra: Partial<DesignObject> = {}): DesignObject {
  return {
    id,
    layer: '#ff0000',
    transform: [1, 0, 0, 1, at[0], at[1]],
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true)] },
    locked: false,
    hidden: false,
    group: null,
    ...extra,
  };
}

export const docOf = (objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null });
