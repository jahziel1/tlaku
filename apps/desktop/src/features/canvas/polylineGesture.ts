/** Gestos de la herramienta Línea (polilíneas y curvas). */
import type { BezPath, Point } from '@/api/types';
import { uniqueId } from '@/store/document';
import { clearGesture, patchInteraction } from './interaction';
import { constrainAngle, newVectorObject, pickDrawLayer, type ShapeKind } from './shapes';
import { cornerNode, handleLines, hasCurves, penDraft, penPath } from './pen';
import { type Mods } from './toolDeps';
import { CLOSE_PX, DOUBLE_MS, DOUBLE_PX, type GestureCtx } from './gestureState';
import type { ShapeGesture } from './shapeGesture';

/** Herramienta Línea: polilíneas y curvas nodo a nodo. */
export function createPolylineGesture(ctx: GestureCtx, shape: Pick<ShapeGesture, 'finishCreate' | 'lineTip'>) {
  const { st, viewOf, world, capture, targetsFor, setTip, snapTo, state } = ctx;

  const { finishCreate, lineTip } = shape;

  /** Punto del siguiente nodo bajo el cursor: Mayús fija 45 grados desde el último y el ajuste magnético lo pega. */
  const polyCursor = (sx: number, sy: number, mods: Mods): Point => {
    if (!state.poly) return world(sx, sy);
    let p = world(sx, sy);
    const lastPt = state.poly.nodes[state.poly.nodes.length - 1]!.p;
    if (mods.shift) p = constrainAngle(lastPt, p);
    return snapTo(p, state.poly.targets, mods);
  };

  /** Dibuja lo puesto, el tramo elástico hasta `cursor` y las manijas del último nodo. */
  const showPoly = (cursor: Point | null, sx = 0, sy = 0) => {
    if (!state.poly) return;
    const last = state.poly.nodes[state.poly.nodes.length - 1]!;
    patchInteraction({
      draft: { paths: [penDraft(state.poly.nodes, cursor)], rubber: true, nodes: state.poly.nodes.map((n) => n.p), handles: handleLines(last) },
      polyPoints: state.poly.nodes.length,
    });
    if (cursor) setTip(sx, sy, lineTip(last.p, cursor));
  };

  const createShape = (paths: BezPath[], kind: ShapeKind) => {
    const s = st();
    const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
    const id = uniqueId(kind, new Set(s.doc.objects.map((o) => o.id)));
    finishCreate(newVectorObject(id, layer, paths), kind);
  };

  const finishPoly = (close: boolean) => {
    const nodes = state.poly?.nodes ?? [];
    state.poly = null;
    clearGesture();
    const path = penPath(nodes, close);
    if (!path) return;
    const curved = hasCurves(path);
    createShape([path], curved ? 'curve' : path.length === 2 ? 'line' : 'polyline');
  };

  /** Abre el arrastre de manijas del nodo recién puesto (si no se arrastra, queda de esquina). */
  const beginPenDrag = (e: PointerEvent, sx: number, sy: number, index: number) => {
    state.g = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld: world(sx, sy), active: false, kind: 'pen', index };
    capture(e);
  };

  const downLine = (e: PointerEvent, sx: number, sy: number, mods: Mods) => {
    if (!state.poly) {
      const targets = targetsFor(new Set());
      const p = snapTo(world(sx, sy), targets, mods);
      state.poly = { nodes: [cornerNode(p)], lastAt: performance.now(), lastScreen: [sx, sy], targets };
      patchInteraction({ gesture: 'polyline', polyPoints: 1 });
      showPoly(null);
      beginPenDrag(e, sx, sy, 0);
      return;
    }
    const now = performance.now();
    const first = state.poly.nodes[0]!.p;
    const dblClick = now - state.poly.lastAt < DOUBLE_MS && Math.hypot(sx - state.poly.lastScreen[0], sy - state.poly.lastScreen[1]) < DOUBLE_PX;
    if (dblClick) {
      finishPoly(false);
      return;
    }
    const p = polyCursor(sx, sy, mods);
    const v = viewOf();
    const fs: Point = [first[0] * v.zoom + v.panX, v.panY - first[1] * v.zoom];
    // Clic sobre el primer nodo: cierra la forma.
    if (state.poly.nodes.length >= 3 && Math.hypot(sx - fs[0], sy - fs[1]) < CLOSE_PX) {
      finishPoly(true);
      return;
    }
    state.poly.nodes.push(cornerNode(p));
    state.poly.lastAt = now;
    state.poly.lastScreen = [sx, sy];
    showPoly(null);
    beginPenDrag(e, sx, sy, state.poly.nodes.length - 1);
  };

  return { polyCursor, showPoly, finishPoly, downLine };
}
