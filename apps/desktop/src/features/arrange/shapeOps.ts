import { getApi } from '@/api';
import type { DesignObject, Point, ShapeOp, ShapeParams, ShapeStats } from '@/api/types';
import { selectedNodePoints } from '@/features/canvas/nodeActions';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import type { AppState } from '@/store';
import { cleanupOperands, replaceCleaned } from './cleanup';
import { kindInfo } from './geomOps';
import { useShapeOps } from './shapeOpsUi';
import { useArrangeUi } from './uiStore';

/**
 * Operaciones de forma de la ola 3: recortar (D49), separar (D55), optimizar (D56), redondear
 * esquinas (D60), cortar formas (D65) y redimensionar ranuras (D71). Aquí vive la parte de la
 * interfaz: motivos de deshabilitado, parámetros, aplicar el resultado del motor como un solo paso
 * de deshacer y los avisos. Las matemáticas están en `tlaku-geom` y `edit/shape_ops.rs`.
 */

const HISTORY: Record<ShapeOp, I18nKey> = {
  trim: 'shapes.history.trim',
  break_apart: 'shapes.history.breakApart',
  optimize: 'shapes.history.optimize',
  fillet: 'shapes.history.fillet',
  cut_shapes: 'shapes.history.cutShapes',
  resize_slots: 'shapes.history.resizeSlots',
};

/** Prefijo de los ids nuevos en el documento. */
const ID_BASE: Record<ShapeOp, string> = {
  trim: 'trim',
  break_apart: 'break',
  optimize: 'opt',
  fillet: 'fillet',
  cut_shapes: 'cut',
  resize_slots: 'slot',
};

/** Subtrayectos de un objeto vectorial (los `move_to` que dibujan algo). */
export function subpathCount(o: DesignObject): number {
  if (o.kind.type !== 'vector') return 0;
  let n = 0;
  for (const p of o.kind.paths) {
    let drawn = false;
    let open = false;
    for (const s of p) {
      if (s.type === 'move_to') {
        if (open && drawn) n++;
        open = true;
        drawn = false;
      } else if (s.type === 'close') {
        if (open && drawn) n++;
        open = false;
        drawn = false;
      } else {
        drawn = true;
        // Segmentos tras un `close` abren un subtrayecto nuevo.
        open = true;
      }
    }
    if (open && drawn) n++;
  }
  return n;
}

/** Objetos vectoriales cerrados de la selección: los que pueden hacer de cuchilla. */
export function knifeCandidates(objects: readonly DesignObject[]): DesignObject[] {
  return objects.filter((o) => {
    if (o.kind.type !== 'vector') return false;
    const info = kindInfo(o.kind);
    return !info.empty && !info.open;
  });
}

/** La cuchilla que se usaría: la elegida si sigue en la selección; si no, la última cerrada. */
export function resolveKnife(objects: readonly DesignObject[], chosen: string | null, order: readonly string[]): DesignObject | null {
  const candidates = knifeCandidates(objects);
  const picked = candidates.find((o) => o.id === chosen);
  if (picked) return picked;
  for (let i = order.length - 1; i >= 0; i--) {
    const found = candidates.find((o) => o.id === order[i]);
    if (found) return found;
  }
  return candidates[candidates.length - 1] ?? null;
}

/** Motivo por el que una operación de forma no está disponible, o `true`. */
export function shapeReason(s: Pick<AppState, 'doc' | 'selectedIds'>, op: ShapeOp): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const objects = cleanupOperands(s);
  if (objects.length === 0) return 'arrange.reason.noSelection';
  if (objects.some((o) => o.locked)) return 'arrange.reason.locked';
  if (objects.some((o) => o.kind.type !== 'vector')) return 'arrange.reason.needVectors';
  if (objects.every((o) => kindInfo(o.kind).empty)) return 'arrange.reason.noContour';
  switch (op) {
    case 'break_apart':
      if (!objects.some((o) => subpathCount(o) > 1)) return 'shapes.reason.nothingToBreak';
      break;
    case 'cut_shapes':
      if (objects.length < 2) return 'shapes.reason.cutNeedTwo';
      if (knifeCandidates(objects).length === 0) return 'shapes.reason.cutNoKnife';
      break;
    case 'resize_slots':
      if (knifeCandidates(objects).length === 0) return 'shapes.reason.noClosed';
      break;
    default:
      break;
  }
  return useArrangeUi.getState().busy ? 'arrange.reason.busy' : true;
}

/** Recortar necesita además un punto: el último clic en el lienzo. */
export function trimReason(s: Pick<AppState, 'doc' | 'selectedIds'>): true | I18nKey {
  const base = shapeReason(s, 'trim');
  if (base !== true) return base;
  return useShapeOps.getState().lastPoint ? true : 'shapes.reason.noPoint';
}

/** Parámetros que manda cada operación, a partir de los últimos valores del diálogo. */
export function shapeParams(op: ShapeOp, s: Pick<AppState, 'doc' | 'selectedIds' | 'nodeSel'>): ShapeParams {
  const ui = useShapeOps.getState();
  switch (op) {
    case 'trim':
      return { max_dist: ui.trim.maxDist, keep_removed: ui.trim.keepRemoved };
    case 'break_apart':
      return { keep_holes: ui.breakApart.keepHoles };
    case 'optimize':
      return { tolerance: ui.optimize.tolerance, smooth: ui.optimize.smooth, corner_angle: ui.optimize.cornerAngle };
    case 'fillet': {
      const nodes = ui.fillet.onlyNodes ? selectedNodePoints(s) : [];
      return { radius: ui.fillet.radius, style: ui.fillet.style, ...(nodes.length > 0 ? { nodes } : {}) };
    }
    case 'cut_shapes': {
      const knife = resolveKnife(cleanupOperands(s), ui.cut.knifeId, s.selectedIds);
      return { ...(knife ? { knife_id: knife.id } : {}), close_pieces: ui.cut.closePieces, keep_knife: ui.cut.keepKnife };
    }
    case 'resize_slots':
      return {
        from_width: ui.slots.fromWidth,
        to_width: ui.slots.toWidth,
        tolerance: ui.slots.tolerance,
        anchor: ui.slots.anchor,
        notches: ui.slots.notches,
        holes: ui.slots.holes,
        tabs: ui.slots.tabs,
      };
  }
}

/** Calcula la operación sin aplicarla (vista previa de nodos, cuántas ranuras, qué tramo se borra). */
export async function previewShapeOp(s: Pick<AppState, 'doc' | 'selectedIds'>, op: ShapeOp, params: ShapeParams): Promise<ShapeStats> {
  const result = await getApi().shapeOp({ op, objects: cleanupOperands(s), params });
  return result.stats;
}

const pct = (before: number, after: number) => (before > 0 ? Math.round(((before - after) / before) * 100) : 0);

/** Aviso con el resultado de una operación que cambió el documento. */
export function shapeSummary(op: ShapeOp, stats: ShapeStats): string {
  switch (op) {
    case 'trim':
      return t('shapes.msg.trim');
    case 'break_apart':
      return t('shapes.msg.breakApart', { from: stats.count, to: stats.produced });
    case 'optimize':
      return t('shapes.msg.optimize', { before: stats.nodes_before, after: stats.nodes_after, pct: pct(stats.nodes_before, stats.nodes_after) });
    case 'fillet': {
      const parts = [t('shapes.msg.fillet', { n: stats.count })];
      if (stats.clamped > 0) parts.push(t('shapes.msg.filletClamped', { n: stats.clamped }));
      if (stats.skipped > 0) parts.push(t('shapes.msg.filletNoSpace', { n: stats.skipped }));
      return parts.join(' ');
    }
    case 'cut_shapes':
      return t('shapes.msg.cut', { n: stats.count, pieces: stats.produced });
    case 'resize_slots': {
      const parts = [t('shapes.msg.slots', { n: stats.count })];
      if (stats.skipped > 0) parts.push(t('shapes.msg.slotsBlocked', { n: stats.skipped }));
      return parts.join(' ');
    }
  }
}

/** Aviso cuando la operación no cambió nada. */
export function shapeNothing(op: ShapeOp, stats: ShapeStats): string {
  switch (op) {
    case 'trim':
      return t('shapes.none.trim');
    case 'break_apart':
      return t('shapes.none.breakApart');
    case 'optimize':
      return t('shapes.none.optimize', { nodes: stats.nodes_before });
    case 'fillet':
      return stats.skipped > 0 ? t('shapes.none.filletNoSpace', { n: stats.skipped }) : t('shapes.none.fillet');
    case 'cut_shapes':
      return t('shapes.none.cut');
    case 'resize_slots':
      return stats.found > 0 ? t('shapes.none.slotsBlocked', { n: stats.found }) : t('shapes.none.slots', { width: fmt(useShapeOps.getState().slots.fromWidth, 2) });
  }
}

/**
 * Ejecuta una operación de forma sobre la selección: el motor devuelve todos los objetos (los sin
 * cambio con su mismo id, los modificados como objetos nuevos) y la interfaz quita los ids que ya no
 * vuelven e inserta los nuevos donde estaba el primero quitado. Un solo paso de deshacer. Devuelve
 * true si cambió el documento; los errores del motor se avisan en español y no cambian nada.
 */
export async function runShapeOp(s: AppState, op: ShapeOp, params: ShapeParams = shapeParams(op, s)): Promise<boolean> {
  const reason = shapeReason(s, op);
  if (reason !== true) {
    s.notify('warning', t(reason));
    return false;
  }
  const operands = cleanupOperands(s);
  useArrangeUi.getState().setBusy(true);
  let result;
  try {
    result = await getApi().shapeOp({ op, objects: operands, params });
  } catch (e) {
    s.notify('error', errorMessage(e));
    return false;
  } finally {
    useArrangeUi.getState().setBusy(false);
  }

  const sent = new Set(operands.map((o) => o.id));
  const returned = new Set(result.objects.map((r) => r.id));
  const consumed = operands.filter((o) => !returned.has(o.id));
  const fresh = result.objects.filter((r) => !sent.has(r.id));
  if (consumed.length === 0 && fresh.length === 0) {
    s.notify('info', params.revert ? t('shapes.none.unfillet') : shapeNothing(op, result.stats));
    return false;
  }
  const created: string[] = [];
  const removeIds = new Set(consumed.map((o) => o.id));
  s.applyDocChange(params.revert ? 'shapes.history.unfillet' : HISTORY[op], (doc) => {
    created.length = 0;
    return replaceCleaned(doc, removeIds, fresh, ID_BASE[op], created);
  });
  // Quedan seleccionados los objetos nuevos y, si se conservó, la cuchilla.
  const kept = op === 'cut_shapes' ? result.objects.filter((r) => sent.has(r.id) && r.id === params.knife_id).map((r) => r.id) : [];
  s.select([...kept, ...created]);
  s.notify('success', params.revert ? t('shapes.msg.unfillet') : shapeSummary(op, result.stats));
  return true;
}

export interface TrimOptions {
  /** Conservar el tramo borrado como objeto aparte (Mayús+clic). */
  keepRemoved?: boolean;
  /** Distancia máxima (mm) del clic al trazo; por omisión, la del diálogo. */
  maxDist?: number;
}

/**
 * Recorta el tramo de la selección más cercano a `point` (mm, documento). Es lo que llama la
 * herramienta Recortar del lienzo en cada clic, y el comando "Recortar en el último clic".
 */
export function runTrimAt(s: AppState, point: Point, options: TrimOptions = {}): Promise<boolean> {
  const base = shapeParams('trim', s);
  return runShapeOp(s, 'trim', { ...base, point, ...(options.keepRemoved !== undefined ? { keep_removed: options.keepRemoved } : {}), ...(options.maxDist !== undefined ? { max_dist: options.maxDist } : {}) });
}

/** Puntos del tramo que se borraría en `point` (para resaltarlo al pasar el ratón), o `null` si no hay trazo cerca. */
export async function previewTrimAt(s: Pick<AppState, 'doc' | 'selectedIds' | 'nodeSel'>, point: Point, maxDist?: number): Promise<Point[] | null> {
  try {
    const stats = await previewShapeOp(s, 'trim', { ...shapeParams('trim', s), point, ...(maxDist !== undefined ? { max_dist: maxDist } : {}) });
    return stats.removed;
  } catch {
    return null;
  }
}
