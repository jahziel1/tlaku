import type { CanvasPalette } from '@/features/canvas/renderer';
import type { ViewTransform } from '@/lib/viewport';
import { buildPaths, LOD_TOLERANCES_MM, lodSettings, MAX_LOD, slotOf, type BlockCache, type BlockPaths, type PathFactory, type PathLike } from './blocks';
import { headAt, LEVEL_ALPHA, MoveClass, POWER_LEVELS, type PreviewModel, type PreviewViewOptions } from './model';

/** Grosor de los trazos en px de pantalla. */
const WIDTH_PX = { travel: 1, overscan: 1, engrave: 1, cut: 1.6, highlight: 2.6 } as const;
/** Opacidad de lo que no está resaltado cuando se resalta una capa. */
const DIM = 0.22;

export interface DrawArgs<P extends PathLike> {
  ctx: CanvasRenderingContext2D;
  model: PreviewModel;
  cache: BlockCache<P>;
  factory: PathFactory<P>;
  view: ViewTransform;
  /** Tamaño del lienzo en px CSS. */
  width: number;
  height: number;
  palette: CanvasPalette;
  /** Fracción (0..1) del tiempo total. */
  scrub: number;
  options: PreviewViewOptions;
  /** Trazados de la capa resaltada (ver `buildLayerPaths`). */
  highlight: BlockPaths<P> | null;
}

/** Segmentos que se dibujan como máximo por cuadro; si hay más se baja el nivel de detalle. */
export const SEGMENT_BUDGET = 120_000;
/** Error máximo aceptado por la simplificación, en píxeles de pantalla. */
const LOD_QUALITY_PX = 0.3;

/** Nivel de detalle mínimo que no se nota a este zoom (0 = recorrido exacto). */
export function qualityLevel(zoom: number): number {
  const tol = LOD_QUALITY_PX / Math.max(zoom, 1e-6);
  if (tol < LOD_TOLERANCES_MM[1]! / 2) return 0;
  for (let k = 1; k <= MAX_LOD; k++) if (LOD_TOLERANCES_MM[k]! >= tol) return k;
  return MAX_LOD;
}

export interface DrawStats {
  /** Nivel de detalle usado (0 = exacto). */
  level: number;
  blocksDrawn: number;
  blocksCulled: number;
  strokes: number;
}

/** Trazados de todos los movimientos de una capa, para resaltarla. */
export function buildLayerPaths<P extends PathLike>(model: PreviewModel, layer: number, factory: PathFactory<P>): BlockPaths<P> | null {
  const s = model.layerStarts;
  if (!s || layer < 0 || layer + 1 >= s.length) return null;
  return buildPaths(model, s[layer]!, s[layer + 1]!, factory, undefined, lodSettings(3));
}

/**
 * Dibuja el recorrido hasta el instante `scrub * total`: los bloques completos salen de la caché
 * (descartando los que quedan fuera de la vista), el bloque a medias y el tramo del movimiento en
 * curso se arman en el momento. Los trazados están en mm; la transformación del contexto hace el
 * paso a píxeles, así que el zoom y el paneo no invalidan nada.
 */
export function drawPreviewModel<P extends PathLike>(a: DrawArgs<P>): DrawStats {
  const { ctx, model, cache, view, palette: p, options } = a;
  const stats: DrawStats = { level: 0, blocksDrawn: 0, blocksCulled: 0, strokes: 0 };
  if (model.count === 0) return stats;

  const t = Math.min(1, Math.max(0, a.scrub)) * model.total;
  const head = headAt(model, t);

  // Rectángulo visible en mm (Y hacia arriba).
  const x0 = (0 - view.panX) / view.zoom;
  const x1 = (a.width - view.panX) / view.zoom;
  const y0 = (view.panY - a.height) / view.zoom;
  const y1 = (view.panY - 0) / view.zoom;

  const blockSize = cache.blockSize;
  const fullBlocks = Math.floor(head.done / blockSize);
  // Bloques completos que se cruzan con la vista (el recuadro de cada bloque no depende del nivel).
  const floor = qualityLevel(view.zoom);
  const inView: number[] = [];
  for (let b = 0; b < fullBlocks; b++) {
    const [bx0, by0, bx1, by1] = cache.get(b, floor).bbox;
    if (bx1 < x0 || bx0 > x1 || by1 < y0 || by0 > y1) stats.blocksCulled++;
    else inView.push(b);
  }
  // Detalle: el que no se nota; si aun así hay demasiados segmentos, se simplifica más.
  let level = floor;
  const overBudget = (lv: number) => {
    let segs = 0;
    for (const b of inView) segs += cache.get(b, lv).segs;
    return segs > SEGMENT_BUDGET;
  };
  if (overBudget(level)) {
    // Primero se funden los niveles de potencia (mismo detalle), luego se sube la tolerancia.
    level = MAX_LOD + 1 + floor;
    while (level < 2 * MAX_LOD + 1 && overBudget(level)) level++;
  }
  stats.level = level;
  const visible: BlockPaths<P>[] = inView.map((b) => cache.get(b, level));
  stats.blocksDrawn = visible.length;
  // Bloque a medias: lo ya terminado más el tramo recorrido del movimiento en curso.
  const partialFrom = fullBlocks * blockSize;
  if (head.done > partialFrom || !head.finished) {
    const tail = head.finished || head.frac <= 0 ? undefined : { x: head.x, y: head.y };
    visible.push(buildPaths(model, partialFrom, head.done, a.factory, tail, lodSettings(level)));
  }

  const z = view.zoom;
  const dim = a.highlight ? DIM : 1;
  const strokeSlot = (cls: number, level: number) => {
    const slot = slotOf(cls, level);
    for (const blk of visible) {
      const path = blk.paths[slot];
      if (path) {
        ctx.stroke(path as unknown as Path2D);
        stats.strokes++;
      }
    }
  };

  ctx.save();
  ctx.translate(view.panX, view.panY);
  ctx.scale(z, -z);
  ctx.lineJoin = 'round';
  ctx.lineCap = 'butt';

  if (options.showTravel) {
    ctx.strokeStyle = p.travel;
    ctx.lineWidth = WIDTH_PX.travel / z;
    ctx.globalAlpha = dim;
    ctx.setLineDash([3 / z, 4 / z]);
    strokeSlot(MoveClass.Travel, POWER_LEVELS - 1);
    ctx.setLineDash([]);
  }
  if (options.showOverscan) {
    ctx.strokeStyle = p.overscan;
    ctx.lineWidth = WIDTH_PX.overscan / z;
    ctx.globalAlpha = dim;
    strokeSlot(MoveClass.Overscan, POWER_LEVELS - 1);
  }
  ctx.strokeStyle = p.engrave;
  ctx.lineWidth = WIDTH_PX.engrave / z;
  for (let l = 0; l < POWER_LEVELS; l++) {
    ctx.globalAlpha = LEVEL_ALPHA[l]! * dim;
    strokeSlot(MoveClass.Engrave, l);
  }
  ctx.strokeStyle = p.cut;
  ctx.lineWidth = WIDTH_PX.cut / z;
  for (let l = 0; l < POWER_LEVELS; l++) {
    ctx.globalAlpha = LEVEL_ALPHA[l]! * dim;
    strokeSlot(MoveClass.Cut, l);
  }
  if (a.highlight) {
    ctx.strokeStyle = p.head;
    ctx.lineWidth = WIDTH_PX.highlight / z;
    ctx.globalAlpha = 1;
    for (const cls of [MoveClass.Overscan, MoveClass.Engrave, MoveClass.Cut]) {
      for (let l = 0; l < POWER_LEVELS; l++) {
        const path = a.highlight.paths[slotOf(cls, l)];
        if (path) {
          ctx.stroke(path as unknown as Path2D);
          stats.strokes++;
        }
      }
    }
  }
  ctx.restore();

  // Cabezal, en píxeles de pantalla para que no cambie de tamaño con el zoom.
  if (!head.finished) {
    const sx = view.panX + head.x * z;
    const sy = view.panY - head.y * z;
    ctx.save();
    ctx.fillStyle = p.head;
    ctx.beginPath();
    ctx.arc(sx, sy, 4.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = p.head;
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.7;
    ctx.beginPath();
    ctx.arc(sx, sy, 9, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
  }
  return stats;
}
