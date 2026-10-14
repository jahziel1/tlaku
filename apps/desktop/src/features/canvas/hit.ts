import type { DesignObject, LayerSettings, Point } from '@/api/types';
import { boxContains, boxesIntersect, eachSegment, getGeom, type Box, type ObjectGeom } from './geom';
import { INDEX_MIN_OBJECTS, spatialIndexFor } from './spatialIndex';

/** Tolerancia de clic en píxeles de pantalla (no depende del zoom). */
export const HIT_PX = 6;

export type LayerLookup = (color: string) => LayerSettings | undefined;

/** Búsqueda de capa por color sin distinguir mayúsculas (el motor normaliza a minúsculas). */
export function layerLookup(layers: readonly LayerSettings[]): LayerLookup {
  const map = new Map(layers.map((l) => [l.color.toLowerCase(), l]));
  return (color) => map.get(color.toLowerCase());
}

/** Un objeto oculto, bloqueado o de una capa invisible no se puede seleccionar. */
export function isSelectable(obj: DesignObject, layer: LayerSettings | undefined): boolean {
  return !obj.hidden && !obj.locked && (layer?.visible ?? true);
}

/** ¿Se rellena el interior (clic dentro lo selecciona)? Capas de relleno, imágenes y textos (se agarran por las letras). */
export function isFilled(obj: DesignObject, layer: LayerSettings | undefined): boolean {
  return obj.kind.type === 'image' || obj.kind.type === 'text' || layer?.mode === 'fill';
}

function distSq(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  let t = len2 === 0 ? 0 : ((px - ax) * dx + (py - ay) * dy) / len2;
  t = t < 0 ? 0 : t > 1 ? 1 : t;
  const cx = ax + t * dx - px;
  const cy = ay + t * dy - py;
  return cx * cx + cy * cy;
}

/** Distancia (mm) del punto al trazo del objeto; `Infinity` si no hay segmentos cerca de `limit`. */
export function distanceToStroke(g: ObjectGeom, px: number, py: number, limit: number): number {
  if (!g.box) return Infinity;
  if (px < g.box[0] - limit || px > g.box[2] + limit || py < g.box[1] - limit || py > g.box[3] + limit) return Infinity;
  let best = limit * limit;
  let found = false;
  for (const ch of g.chunks) {
    const b = ch.box;
    if (px < b[0] - limit || px > b[2] + limit || py < b[1] - limit || py > b[3] + limit) continue;
    eachSegment(g, ch, (x1, y1, x2, y2) => {
      const d = distSq(px, py, x1, y1, x2, y2);
      if (d <= best) {
        best = d;
        found = true;
      }
    });
  }
  return found ? Math.sqrt(best) : Infinity;
}

/** Regla par-impar sobre los contornos cerrados del objeto. */
export function pointInFill(g: ObjectGeom, px: number, py: number): boolean {
  if (!g.box || px < g.box[0] || px > g.box[2] || py < g.box[1] || py > g.box[3]) return false;
  let inside = false;
  for (const ch of g.chunks) {
    if (!ch.closed) continue;
    const b = ch.box;
    if (py < b[1] || py > b[3] || px > b[2]) continue;
    eachSegment(g, ch, (x1, y1, x2, y2) => {
      if (y1 > py !== y2 > py && px < ((x2 - x1) * (py - y1)) / (y2 - y1) + x1) inside = !inside;
    });
  }
  return inside;
}

export function hitObject(g: ObjectGeom, px: number, py: number, tol: number, filled: boolean): boolean {
  if (distanceToStroke(g, px, py, tol) <= tol) return true;
  return filled && pointInFill(g, px, py);
}

export interface PickContext {
  objects: readonly DesignObject[];
  layer: LayerLookup;
}

/** Objeto seleccionable bajo el punto (mm). El de más arriba (el último del documento) gana. */
export function pickObject(ctx: PickContext, p: Point, tol: number): DesignObject | null {
  if (ctx.objects.length >= INDEX_MIN_OBJECTS) {
    // Solo los objetos cuya caja (con la tolerancia) contiene el punto; ya vienen de arriba hacia abajo.
    for (const i of spatialIndexFor(ctx.objects).atPoint(p[0], p[1], tol)) {
      const obj = ctx.objects[i]!;
      const layer = ctx.layer(obj.layer);
      if (!isSelectable(obj, layer)) continue;
      if (hitObject(getGeom(obj), p[0], p[1], tol, isFilled(obj, layer))) return obj;
    }
    return null;
  }
  for (let i = ctx.objects.length - 1; i >= 0; i--) {
    const obj = ctx.objects[i];
    const layer = ctx.layer(obj.layer);
    if (!isSelectable(obj, layer)) continue;
    if (hitObject(getGeom(obj), p[0], p[1], tol, isFilled(obj, layer))) return obj;
  }
  return null;
}

// ---- selección por caja ---------------------------------------------------------------------------

export type BoxMode = 'window' | 'crossing';

/** Modo según el sentido de arrastre: de izquierda a derecha = dentro; de derecha a izquierda = toca. */
export function boxModeFor(startX: number, endX: number): BoxMode {
  return endX >= startX ? 'window' : 'crossing';
}

export function normalizeBox(a: Point, b: Point): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}

/** Segmento contra rectángulo (Liang-Barsky). */
export function segmentIntersectsBox(x1: number, y1: number, x2: number, y2: number, r: Box): boolean {
  let t0 = 0;
  let t1 = 1;
  const dx = x2 - x1;
  const dy = y2 - y1;
  const p = [-dx, dx, -dy, dy];
  const q = [x1 - r[0], r[2] - x1, y1 - r[1], r[3] - y1];
  for (let i = 0; i < 4; i++) {
    if (p[i] === 0) {
      if (q[i] < 0) return false;
    } else {
      const t = q[i] / p[i];
      if (p[i] < 0) {
        if (t > t1) return false;
        if (t > t0) t0 = t;
      } else {
        if (t < t0) return false;
        if (t < t1) t1 = t;
      }
    }
  }
  return true;
}

function touchesBox(g: ObjectGeom, r: Box, filled: boolean): boolean {
  if (!g.box || !boxesIntersect(g.box, r)) return false;
  for (const ch of g.chunks) {
    if (!boxesIntersect(ch.box, r)) continue;
    if (eachSegment(g, ch, (x1, y1, x2, y2) => segmentIntersectsBox(x1, y1, x2, y2, r))) return true;
  }
  // Objetos de un solo punto o un contorno degenerado.
  if (g.chunks.length === 0) return boxesIntersect(g.box, r);
  // Caja entera dentro de un relleno: no cruza el contorno, pero sí toca el objeto.
  return filled && pointInFill(g, (r[0] + r[2]) / 2, (r[1] + r[3]) / 2);
}

/** Ids de los objetos seleccionables dentro (`window`) o tocados (`crossing`) por la caja. */
export function selectByBox(ctx: PickContext, r: Box, mode: BoxMode): string[] {
  const out: string[] = [];
  // Con un rectángulo pequeño sobre un documento enorme, solo se miran los objetos de sus celdas.
  const list: readonly DesignObject[] = ctx.objects.length >= INDEX_MIN_OBJECTS ? spatialIndexFor(ctx.objects).inBox(r).map((i) => ctx.objects[i]!) : ctx.objects;
  for (const obj of list) {
    const layer = ctx.layer(obj.layer);
    if (!isSelectable(obj, layer)) continue;
    const g = getGeom(obj);
    if (!g.box) continue;
    const hit = mode === 'window' ? boxContains(r, g.box) : touchesBox(g, r, isFilled(obj, layer));
    if (hit) out.push(obj.id);
  }
  return out;
}
