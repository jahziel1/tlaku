/** Gestos de las formas básicas (rectángulo, elipse, polígono, estrella). */
import type { DesignObject, Point } from '@/api/types';
import { objectCurves } from '@/lib/geometry';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { uniqueId } from '@/store/document';
import { clearGesture, patchInteraction } from './interaction';
import { boxShapePaths, dragBox, pickDrawLayer, type ShapeKind } from './shapes';
import { newShapeObject, type ShapeSpec } from '@/lib/paramShape';
import { type Mods } from './toolDeps';
import { boundsSize } from './transform';
import { fmtMM, MIN_SHAPE_MM, type Gesture, type GestureCtx } from './gestureState';

/** Rectángulo, elipse, polígono y estrella: arrastrar para dibujar y crear el objeto al soltar. */
export function createShapeGesture(ctx: GestureCtx) {
  const { st, world, capture, snapOpts, targetsFor, setTip, snapTo, applyPointSnap, state } = ctx;

  /** Caja y medidas de un rectángulo o una elipse arrastrados de `a` a `b`. */
  const dragShape = (tool: 'rect' | 'ellipse', a: Point, b: Point, mods: Mods) => {
    const box = dragBox(a, b, { square: mods.shift, fromCenter: false });
    const [w, h] = boundsSize(box);
    const radius = tool === 'rect' ? Math.min(st().cornerRadius, w / 2, h / 2) : 0;
    return { box, w, h, radius, paths: boxShapePaths(tool, box, radius) };
  };

  /** Polígono o estrella con el centro en `c` y el radio y giro que marca el puntero en `p`. */
  const radialShape = (tool: 'polygon' | 'star', c: Point, p: Point, mods: Mods) => {
    const s = st();
    const radius = Math.hypot(p[0] - c[0], p[1] - c[1]);
    // El primer vértice apunta al puntero; con Mayús queda hacia arriba.
    const rotation = mods.shift ? 0 : Math.atan2(p[1] - c[1], p[0] - c[0]) - Math.PI / 2;
    const params: ShapeSpec = tool === 'polygon'
      ? { type: 'polygon', sides: s.polygonSides, radius, rounding: s.shapeRounding }
      : { type: 'star', points: s.starPoints, outer: radius, ratio: s.starRatio, rounding: s.shapeRounding };
    return { params, radius, rotation };
  };

  const updateDraw = (gs: Extract<Gesture, { kind: 'draw' }>, sx: number, sy: number, mods: Mods) => {
    let p = world(sx, sy);
    const opts = snapOpts(mods);
    if (gs.tool === 'polygon' || gs.tool === 'star') {
      if (opts) p = applyPointSnap(p, gs.targets, opts);
      else patchInteraction({ guides: [], snapMark: null });
      gs.end = p;
      const { params, radius, rotation } = radialShape(gs.tool, gs.startWorld, p, mods);
      if (radius > 1e-9) {
        const preview = newShapeObject('preview', '#000000', params, gs.startWorld, rotation);
        patchInteraction({ draft: { paths: objectCurves(preview) } });
      }
      setTip(sx, sy, t('canvas.tip.radius', { r: fmtMM(radius), d: fmtMM(radius * 2) }));
      return;
    }
    if (opts) p = applyPointSnap(p, gs.targets, opts);
    else patchInteraction({ guides: [], snapMark: null });
    gs.end = p;
    const { paths, w, h } = dragShape(gs.tool, gs.startWorld, p, mods);
    patchInteraction({ draft: { paths } });
    const circle = gs.tool === 'ellipse' && Math.abs(w - h) < 1e-6;
    setTip(sx, sy, circle ? t('canvas.tip.circle', { d: fmtMM(w) }) : t('canvas.tip.size', { w: fmtMM(w), h: fmtMM(h) }));
  };

  const lineTip = (a: Point, b: Point): string => {
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const deg = (Math.atan2(b[1] - a[1], b[0] - a[0]) * 180) / Math.PI;
    return t('canvas.tip.line', { len: fmtMM(len), deg: fmt(deg, 1) });
  };

  const downDraw = (e: PointerEvent, tool: 'rect' | 'ellipse' | 'polygon' | 'star', sx: number, sy: number, mods: Mods) => {
    const targets = targetsFor(new Set());
    const start = snapTo(world(sx, sy), targets, mods);
    state.g = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld: start, active: false, kind: 'draw', tool, targets, end: start };
    capture(e);
  };

  /** Agrega el objeto recién dibujado, lo selecciona y vuelve a Seleccionar. */
  const finishCreate = (obj: DesignObject, kind: ShapeKind) => {
    const s = st();
    s.addObjects([obj], `canvas.draw.${kind}` as I18nKey);
    s.select([obj.id]);
    s.setTool('select');
  };

  const finishDraw = (gs: Extract<Gesture, { kind: 'draw' }>, mods: Mods) => {
    clearGesture();
    if (!gs.active) return;
    const s = st();
    const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
    const id = uniqueId(gs.tool, new Set(s.doc.objects.map((o) => o.id)));
    if (gs.tool === 'polygon' || gs.tool === 'star') {
      const { params, radius, rotation } = radialShape(gs.tool, gs.startWorld, gs.end, mods);
      if (radius < MIN_SHAPE_MM) return;
      finishCreate(newShapeObject(id, layer, params, gs.startWorld, rotation), gs.tool);
      return;
    }
    const { box, w, h, radius } = dragShape(gs.tool, gs.startWorld, gs.end, mods);
    if (w < MIN_SHAPE_MM || h < MIN_SHAPE_MM) return;
    // La forma se guarda centrada en el origen local con sus parámetros en `shape`; la matriz la coloca.
    const params: ShapeSpec = gs.tool === 'rect' ? { type: 'rect', w, h, radius } : { type: 'ellipse', w, h };
    finishCreate(newShapeObject(id, layer, params, [(box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2]), gs.tool);
  };

  return { downDraw, updateDraw, finishDraw, finishCreate, lineTip };
}

export type ShapeGesture = ReturnType<typeof createShapeGesture>;
