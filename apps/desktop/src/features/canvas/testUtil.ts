import type { DesignObject, LayerSettings, Point } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { layerLookup, type PickContext } from './hit';
import { newVectorObject, rectPoints } from './shapes';

/** Ayudas para las pruebas del lienzo. */
export function rectObj(id: string, x0: number, y0: number, x1: number, y1: number, patch: Partial<DesignObject> = {}): DesignObject {
  return { ...newVectorObject(id, '#ff0000', [{ points: rectPoints(x0, y0, x1, y1), closed: true }]), ...patch };
}

export function lineObj(id: string, a: Point, b: Point, patch: Partial<DesignObject> = {}): DesignObject {
  return { ...newVectorObject(id, '#ff0000', [{ points: [a, b], closed: false }]), ...patch };
}

export function ctxFor(objects: DesignObject[], layers: LayerSettings[] = [makeLayer('#ff0000', 'Rojo')]): PickContext {
  return { objects, layer: layerLookup(layers) };
}
