import type { BezPath, DesignObject, Document, LayerSettings, Point } from '@/api/types';
import { uniqueId } from '@/store/document';
import { ensureLayerIn } from '@/lib/layers';
import { pickDrawLayer } from '@/features/canvas/shapes';

export interface PlaceCodeOptions {
  paths: BezPath[];
  /** Tamaño del conjunto (mm); las formas llegan con la esquina inferior izquierda en (0, 0). */
  size: Point;
  /** Centro del código en el documento (mm). */
  center: Point;
  layer: string;
}

/**
 * Centro donde se coloca un código nuevo: el del área visible si cae dentro de la cama; si no, el
 * centro de la cama.
 */
export function codeCenter(view: { zoom: number; panX: number; panY: number; width: number; height: number }, bed: Point): Point {
  const cx = (view.width / 2 - view.panX) / view.zoom;
  const cy = (view.panY - view.height / 2) / view.zoom;
  const inside = Number.isFinite(cx) && Number.isFinite(cy) && cx >= 0 && cy >= 0 && cx <= bed[0] && cy <= bed[1];
  return inside ? [cx, cy] : [bed[0] / 2, bed[1] / 2];
}

/** Objeto vectorial con las formas del código, centrado en `center`. */
export function codeObject(id: string, o: PlaceCodeOptions): DesignObject {
  return {
    id,
    layer: o.layer,
    transform: [1, 0, 0, 1, o.center[0] - o.size[0] / 2, o.center[1] - o.size[1] / 2],
    kind: { type: 'vector', paths: o.paths },
    locked: false,
    hidden: false,
    group: null,
  };
}

/** Documento con el código agregado (y su capa, si hace falta). Es un solo cambio. */
export function applyCodeToDoc(doc: Document, o: PlaceCodeOptions): { doc: Document; id: string | null } {
  if (o.paths.length === 0) return { doc, id: null };
  const id = uniqueId('code', new Set(doc.objects.map((x) => x.id)));
  const withLayer = ensureLayerIn(doc, o.layer);
  return { doc: { ...withLayer, objects: [...withLayer.objects, codeObject(id, o)] }, id };
}

/** Capa de destino: la activa si admite formas; si no, la primera usable. */
export function codeLayerFor(layers: readonly LayerSettings[], activeLayer: string | null): string {
  return pickDrawLayer(layers, activeLayer);
}
