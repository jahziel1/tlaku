import type { Affine, DesignObject } from '@/api/types';
import { multiplyAffine } from '@/lib/geometry';
import { virtualCopies } from '@/lib/virtualArray';
import { boxesIntersect, chunkPath, getGeom, visibleBox, type Box } from './geom';
import type { Scene } from './renderer';

/** Tope de copias fantasma dibujadas a la vez (un arreglo de 2000 piezas no debe frenar el lienzo). */
export const MAX_GHOSTS = 600;
const STROKE_PX = 1.25;
const GHOST_ALPHA = 0.5;

const cache = new WeakMap<DesignObject, Affine[]>();

/** Copias virtuales del objeto (memoizadas: los objetos del store son inmutables). */
function copiesOf(obj: DesignObject): Affine[] {
  let m = cache.get(obj);
  if (!m) {
    m = virtualCopies(obj);
    cache.set(obj, m);
  }
  return m;
}

/** Caja de `b` después de aplicar `m` (esquinas transformadas; basta para descartar lo que no se ve). */
function movedBox(b: Box, m: Affine): Box {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const [x, y] of [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] as const) {
    xs.push(m[0] * x + m[2] * y + m[4]);
    ys.push(m[1] * x + m[3] * y + m[5]);
  }
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/**
 * Dibuja las copias de los arreglos virtuales (`DesignObject.array`) como fantasmas tenues: no son
 * objetos del documento, pero el trabajo las corta. Cada copia es la geometría del objeto con la
 * matriz que da `virtualCopies`. Con la vista previa visible no se dibujan (la vista previa ya las incluye).
 * Devuelve cuántas se dibujaron.
 */
const withArray = new WeakMap<readonly DesignObject[], boolean>();

/** ¿Algún objeto tiene arreglo? Se calcula una vez por lista (sin él, cada cuadro no recorre los 100 000 objetos). */
function anyArray(objects: readonly DesignObject[]): boolean {
  let any = withArray.get(objects);
  if (any === undefined) {
    any = objects.some((o) => o.array);
    withArray.set(objects, any);
  }
  return any;
}

export function drawVirtualCopies(ctx: CanvasRenderingContext2D, scene: Scene, dpr: number): number {
  if (scene.preview || !anyArray(scene.objects)) return 0;
  const { view, width, height, layers, overlay, selectedIds } = scene;
  let drawn = 0;
  let any = false;
  const vb = visibleBox(view, width, height, STROKE_PX * 2);
  const k = dpr * view.zoom;
  for (const obj of scene.objects) {
    if (!obj.array || obj.hidden) continue;
    const layer = layers.get(obj.layer) ?? layers.get(obj.layer.toLowerCase());
    if (layer && !layer.visible) continue;
    const g = getGeom(obj);
    if (!g.box || g.image) continue;
    const live = overlay.live && selectedIds.has(obj.id) ? overlay.live : null;
    if (!any) {
      ctx.save();
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      any = true;
    }
    ctx.strokeStyle = obj.layer;
    ctx.globalAlpha = GHOST_ALPHA;
    for (const m of copiesOf(obj)) {
      if (drawn >= MAX_GHOSTS) break;
      const full = live ? multiplyAffine(m, live) : m;
      if (!boxesIntersect(movedBox(g.box, full), vb)) continue;
      ctx.setTransform(k, 0, 0, -k, dpr * view.panX, dpr * view.panY);
      ctx.transform(full[0], full[1], full[2], full[3], full[4], full[5]);
      const scale = Math.sqrt(Math.abs(full[0] * full[3] - full[1] * full[2])) || 1;
      ctx.lineWidth = STROKE_PX / (view.zoom * scale);
      ctx.setLineDash([4 / (view.zoom * scale), 3 / (view.zoom * scale)]);
      for (const ch of g.chunks) ctx.stroke(chunkPath(g, ch));
      drawn++;
    }
  }
  if (any) ctx.restore();
  return drawn;
}
