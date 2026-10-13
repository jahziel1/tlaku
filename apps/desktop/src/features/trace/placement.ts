import type { BezPath, DesignObject, Document, LayerSettings } from '@/api/types';
import { ensureLayerIn } from '@/lib/layers';
import { LAYER_PALETTE } from '@/lib/palette';
import { uniqueId } from '@/store/document';

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Capa de destino: la activa, o la primera de líneas, o un color libre de la paleta (capa nueva). */
export function traceLayerFor(layers: readonly LayerSettings[], activeLayer: string | null): string {
  if (activeLayer) {
    const l = layers.find((x) => same(x.color, activeLayer));
    if (!l) return activeLayer;
    if (l.mode !== 'image') return l.color;
  }
  const line = layers.find((l) => l.mode === 'line');
  if (line) return line.color;
  const free = LAYER_PALETTE.find((p) => !p.guide && !layers.some((l) => same(l.color, p.color)));
  return (free ?? LAYER_PALETTE[0]!).color;
}

/**
 * Objeto vectorial exactamente sobre la imagen. Las curvas del motor salen en el sistema local de
 * la imagen (mm, 0..size_mm, Y hacia arriba), así que con el mismo `transform` (traslación,
 * escala, giro) coinciden con ella.
 */
export function placeTrace(image: DesignObject, paths: BezPath[], layer: string, id: string): DesignObject {
  return {
    id,
    layer,
    transform: [...image.transform],
    kind: { type: 'vector', paths },
    locked: false,
    hidden: false,
    group: null,
  };
}

export interface ApplyTraceOptions {
  imageId: string;
  paths: BezPath[];
  layer: string;
  deleteImage: boolean;
}

/**
 * Documento con el resultado agregado (y la imagen borrada si se pidió y no está bloqueada).
 * Devuelve el mismo documento si la imagen ya no existe o no hay formas. Es un único cambio:
 * un solo paso de deshacer.
 */
export function applyTraceToDoc(doc: Document, opts: ApplyTraceOptions): { doc: Document; id: string | null } {
  const image = doc.objects.find((o) => o.id === opts.imageId && o.kind.type === 'image');
  if (!image || opts.paths.length === 0) return { doc, id: null };
  const id = uniqueId('trace', new Set(doc.objects.map((o) => o.id)));
  const vector = placeTrace(image, opts.paths, opts.layer, id);
  const withLayer = ensureLayerIn(doc, opts.layer);
  const keep = opts.deleteImage && !image.locked ? withLayer.objects.filter((o) => o !== image) : withLayer.objects;
  // El resultado va justo encima de la imagen (o al final si esta se borra) para no quedar tapado.
  const at = keep.indexOf(image);
  const objects = at < 0 ? [...keep, vector] : [...keep.slice(0, at + 1), vector, ...keep.slice(at + 1)];
  return { doc: { ...withLayer, objects }, id };
}
