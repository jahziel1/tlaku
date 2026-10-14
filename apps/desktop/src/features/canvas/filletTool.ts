import { create } from 'zustand';
import type { BezPath, DesignObject, FilletKind, Point } from '@/api/types';
import { runShapeOp } from '@/features/arrange/shapeOps';
import { useShapeOps } from '@/features/arrange/shapeOpsUi';
import { t } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { objectCurves } from '@/lib/geometry';
import { clearGesture, patchInteraction } from './interaction';
import { tipYClearOf, toolBarBox } from './tipPlacement';
import type { Mods, ToolDeps } from './toolDeps';

/** Distancia (px de pantalla) a la que un clic todavía cuenta como sobre la esquina. */
export const FILLET_HIT_PX = 12;
/** Giro mínimo (grados) para que un vértice sea una esquina. */
const MIN_TURN_DEG = 1;

/** Esquina de recta con recta: el vértice y sus vecinos (mm, documento). */
export interface Corner {
  objectId: string;
  p: Point;
  prev: Point;
  next: Point;
}

/**
 * Esquinas de un objeto vectorial: los nodos donde llegan y salen dos rectas con un giro apreciable
 * (las mismas que redondea el motor). Los extremos de un trazo abierto y los nodos con curvas no cuentan.
 */
export function cornersOf(o: DesignObject): Corner[] {
  if (o.kind.type !== 'vector') return [];
  const out: Corner[] = [];
  for (const path of objectCurves(o)) {
    let nodes: Point[] = [];
    let lines: boolean[] = []; // lines[i]: la arista que llega al nodo i es recta
    let closed = false;
    const flush = () => {
      pushCorners(o.id, nodes, lines, closed, out);
      nodes = [];
      lines = [];
      closed = false;
    };
    for (const seg of path) {
      if (seg.type === 'move_to') {
        flush();
        nodes.push(seg.to);
        lines.push(false);
      } else if (seg.type === 'line_to') {
        nodes.push(seg.to);
        lines.push(true);
      } else if (seg.type === 'quad_to' || seg.type === 'cubic_to') {
        nodes.push(seg.to);
        lines.push(false);
      } else closed = true;
    }
    flush();
  }
  return out;
}

function pushCorners(id: string, nodes: Point[], lines: boolean[], closed: boolean, out: Corner[]): void {
  let n = nodes.length;
  if (n < 3) return;
  const same = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]) < 1e-9;
  // Un trazo que vuelve a su inicio cuenta como cerrado: el último nodo es el primero.
  let wrap = false;
  if (n >= 4 && same(nodes[0]!, nodes[n - 1]!)) {
    wrap = true;
    closed = true;
    n--;
  }
  for (let i = 0; i < n; i++) {
    const isEnd = i === 0 || i === n - 1;
    if (!closed && isEnd) continue;
    const incoming = i === 0 ? (wrap ? lines[n]! : true) : lines[i]!;
    const outgoing = i === n - 1 ? (wrap ? lines[n]! : true) : lines[i + 1]!;
    if (!incoming || !outgoing) continue;
    const p = nodes[i]!;
    const prev = nodes[(i + n - 1) % n]!;
    const next = nodes[(i + 1) % n]!;
    const ux = prev[0] - p[0];
    const uy = prev[1] - p[1];
    const wx = next[0] - p[0];
    const wy = next[1] - p[1];
    const lu = Math.hypot(ux, uy);
    const lw = Math.hypot(wx, wy);
    if (lu < 1e-9 || lw < 1e-9) continue;
    const th = Math.acos(Math.min(1, Math.max(-1, (ux * wx + uy * wy) / (lu * lw))));
    if (th > Math.PI - (MIN_TURN_DEG * Math.PI) / 180 || th < 1e-6) continue;
    out.push({ objectId: id, p, prev, next });
  }
}

/** La esquina más cercana a `p` (a lo más `tol` mm) entre las del objeto. */
export function nearestCorner(o: DesignObject, p: Point, tol: number): Corner | null {
  let best: Corner | null = null;
  let bestD = tol;
  for (const c of cornersOf(o)) {
    const d = Math.hypot(c.p[0] - p[0], c.p[1] - p[1]);
    if (d <= bestD) {
      best = c;
      bestD = d;
    }
  }
  return best;
}

const ARC_STEPS = 16;

/** Forma aproximada que dejaría el redondeo en la esquina (vista previa; el motor es el exacto). */
export function previewCorner(c: Corner, style: FilletKind, radius: number): BezPath {
  const unit = (a: Point, b: Point): Point => {
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    return [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
  };
  const u = unit(c.p, c.prev);
  const w = unit(c.p, c.next);
  const th = Math.acos(Math.min(1, Math.max(-1, u[0] * w[0] + u[1] * w[1])));
  const room = Math.min(Math.hypot(c.prev[0] - c.p[0], c.prev[1] - c.p[1]), Math.hypot(c.next[0] - c.p[0], c.next[1] - c.p[1])) / 2;
  const want = style === 'round' ? radius / Math.tan(th / 2) : radius;
  const d = Math.min(want, room);
  const t1: Point = [c.p[0] + u[0] * d, c.p[1] + u[1] * d];
  const t2: Point = [c.p[0] + w[0] * d, c.p[1] + w[1] * d];
  if (style === 'chamfer') return polylineBez([t1, t2], false);
  // Centro y radio del arco: tangente a las dos rectas (redondeo) o centrado en la esquina (mordisco).
  let center = c.p;
  let r = d;
  if (style === 'round') {
    r = d * Math.tan(th / 2);
    const bx = u[0] + w[0];
    const by = u[1] + w[1];
    const bl = Math.hypot(bx, by);
    const dist = r / Math.sin(th / 2);
    center = [c.p[0] + (bx / bl) * dist, c.p[1] + (by / bl) * dist];
  }
  const a0 = Math.atan2(t1[1] - center[1], t1[0] - center[0]);
  let sweep = Math.atan2(t2[1] - center[1], t2[0] - center[0]) - a0;
  while (sweep > Math.PI) sweep -= 2 * Math.PI;
  while (sweep < -Math.PI) sweep += 2 * Math.PI;
  const pts: Point[] = Array.from({ length: ARC_STEPS + 1 }, (_, k) => {
    const a = a0 + (sweep * k) / ARC_STEPS;
    return [center[0] + r * Math.cos(a), center[1] + r * Math.sin(a)] as Point;
  });
  return polylineBez(pts, false);
}

// ---- estado de la herramienta ---------------------------------------------------------------------------------------

export interface FilletToolState {
  /** Modo «Revertir»: el clic devuelve a punta una esquina redondeada. Alt invierte el modo mientras se mantiene. */
  revert: boolean;
  /** Esquina resaltada bajo el puntero (modo redondear). */
  hover: Corner | null;
}

export const useFilletTool = create<FilletToolState>()(() => ({ revert: false, hover: null }));
export const setFilletRevert = (revert: boolean): void => useFilletTool.setState({ revert });

/** Radio y estilo que usa la herramienta: los mismos del diálogo Redondear esquinas. */
export const filletSettings = () => useShapeOps.getState().fillet;

// ---- herramienta ------------------------------------------------------------------------------------------------------

export interface FilletToolDeps extends ToolDeps {
  /** Objeto bajo el punto (mm) o null. */
  pick: (p: Point, tolMM: number) => DesignObject | null;
}

/**
 * Herramienta Redondear esquinas: con el radio y el estilo elegidos, un clic sobre un vértice
 * redondea solo esa esquina (un paso de deshacer). En modo «Revertir» (o con Alt) un clic sobre una
 * esquina redondeada, o mordida, la devuelve a punta si el arco es exacto; un chaflán no se revierte.
 */
export function createFilletTool(d: FilletToolDeps) {
  const reverting = (mods: Mods) => useFilletTool.getState().revert !== mods.alt;

  const cornerAt = (sx: number, sy: number): Corner | null => {
    const zoom = d.st().zoom;
    const p = d.world(sx, sy);
    const obj = d.pick(p, FILLET_HIT_PX / zoom);
    return obj ? nearestCorner(obj, p, FILLET_HIT_PX / zoom) : null;
  };

  const showHover = (c: Corner | null) => {
    useFilletTool.setState({ hover: c });
    if (!c) {
      patchInteraction({ draft: null });
      return;
    }
    const { radius, style } = filletSettings();
    patchInteraction({ draft: { paths: [previewCorner(c, style, radius)], grips: [c.p], hotGrip: 0 } });
  };

  /** Cartel junto al puntero; si la barra de la herramienta lo taparía, baja hasta quedar debajo de ella. */
  const showTip = (sx: number, sy: number, text: string) =>
    patchInteraction({ tip: { x: sx + 16, y: tipYClearOf(sx + 16, sy + 18, d.st().viewport.width, toolBarBox(d.el)), text } });

  return {
    hover(sx: number, sy: number, mods: Mods): void {
      if (reverting(mods)) {
        showHover(null);
        patchInteraction({ cursor: 'crosshair', hoverId: null });
        showTip(sx, sy, t('canvas.fillet.tipRevert'));
        return;
      }
      const c = cornerAt(sx, sy);
      showHover(c);
      patchInteraction({ cursor: c ? 'pointer' : 'crosshair', hoverId: null });
      showTip(sx, sy, c ? t('canvas.fillet.tipCorner', { r: filletSettings().radius }) : t('canvas.fillet.tip'));
    },

    down(sx: number, sy: number, mods: Mods): void {
      const s = d.st();
      const p = d.world(sx, sy);
      const zoom = s.zoom;
      const obj = d.pick(p, FILLET_HIT_PX / zoom);
      if (!obj) {
        s.notify('info', t('canvas.fillet.tipNone'));
        return;
      }
      const { radius, style } = filletSettings();
      if (reverting(mods)) {
        s.select([obj.id]);
        showHover(null);
        void runShapeOp(d.st(), 'fillet', { revert: true, point: p, max_dist: FILLET_HIT_PX / zoom });
        return;
      }
      const corner = nearestCorner(obj, p, FILLET_HIT_PX / zoom);
      if (!corner) {
        s.notify('info', t('canvas.fillet.noCorner'));
        return;
      }
      s.select([obj.id]);
      showHover(null);
      void runShapeOp(d.st(), 'fillet', { radius, style, nodes: [corner.p] });
    },

    leave(): void {
      showHover(null);
      patchInteraction({ tip: null });
    },

    cancel(): void {
      useFilletTool.setState({ hover: null });
      clearGesture();
    },
  };
}
