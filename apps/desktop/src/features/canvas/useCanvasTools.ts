import { useEffect, type RefObject } from 'react';
import type { Affine, BezPath, DesignObject, Point } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { expandGroups } from '@/lib/groups';
import { t, type I18nKey } from '@/i18n';
import { isEditableTarget } from '@/commands/shortcuts';
import { fmt } from '@/lib/platform';
import { gridSteps, screenToWorld, type ViewTransform } from '@/lib/viewport';
import { selectActiveDevice, useStore } from '@/store';
import { uniqueId } from '@/store/document';
import { visibleBox } from './geom';
import { cursorForHandle, handleSpots, hitHandle, type HandleSpot } from './handles';
import { boxModeFor, HIT_PX, layerLookup, normalizeBox, pickObject, selectByBox, type PickContext } from './hit';
import { clearGesture, patchInteraction, registerGestureHooks, useInteraction } from './interaction';
import { boxShapePaths, constrainAngle, constrainDelta, dedupePoints, dragBox, newVectorObject, pickDrawLayer, polylinePaths, type ShapeKind } from './shapes';
import { collectSnapTargets, NO_SNAP, snapFeatures, snapMove, snapPoint, snapToleranceMM, type Feature, type SnapOptions, type SnapSet } from './snap';
import {
  anchorPoint, boundsCenter, boundsOf, boundsSize, canRotate, handlePoint, isIdentity, rotateByPointer, scaleByHandle,
  selectionRotation, translateBy, type HandleId,
} from './transform';

/** Distancia en px antes de que un clic se convierta en arrastre. */
const DRAG_PX = 3;
/** Doble clic al dibujar polilíneas: tiempo (ms) y distancia (px). */
const DOUBLE_MS = 350;
const DOUBLE_PX = 6;
const CLOSE_PX = 8;
/** Tamaño mínimo (mm) de una forma arrastrada para que se cree. */
const MIN_SHAPE_MM = 0.05;

interface Mods {
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
}
const modsOf = (e: { shiftKey: boolean; altKey: boolean; ctrlKey: boolean; metaKey: boolean }): Mods => ({
  shift: e.shiftKey,
  alt: e.altKey,
  ctrl: e.ctrlKey || e.metaKey,
});

interface PointerInfo {
  sx: number;
  sy: number;
  mods: Mods;
}

interface Common {
  pointerId: number;
  startScreen: Point;
  startWorld: Point;
  /** Superó el umbral de arrastre. */
  active: boolean;
}

type Gesture =
  | (Common & { kind: 'press'; hit: DesignObject; wasSelected: boolean; shift: boolean })
  | (Common & { kind: 'move'; ids: string[]; frame: Bounds2; targets: SnapSet; dx: number; dy: number })
  | (Common & { kind: 'scale'; ids: string[]; frame: Bounds2; handle: HandleId; offset: Point; targets: SnapSet; matrix: Affine })
  | (Common & { kind: 'rotate'; ids: string[]; frame: Bounds2; baseDeg: number; matrix: Affine })
  | (Common & { kind: 'marquee'; shift: boolean })
  | (Common & { kind: 'draw'; tool: 'rect' | 'ellipse' | 'line'; targets: SnapSet; end: Point });

type Bounds2 = [Point, Point];

interface Poly {
  points: Point[];
  lastAt: number;
  lastScreen: Point;
  targets: SnapSet;
}

let gestureSeq = 0;

/**
 * Interacción del lienzo con el ratón: selección, mover, escalar, rotar y herramientas de dibujo.
 * Cada gesto edita una copia viva (`useInteraction.live`) y escribe en el documento UNA sola vez al
 * soltar, así que un gesto es un solo paso de deshacer y Esc lo cancela sin dejar rastro.
 */
export function useCanvasTools(areaRef: RefObject<HTMLElement | null>, canvasRef: RefObject<HTMLCanvasElement | null>): void {
  useEffect(() => {
    const el = areaRef.current;
    const canvas = canvasRef.current;
    if (!el || !canvas) return;

    const st = () => useStore.getState();
    const viewOf = (): ViewTransform => {
      const s = st();
      return { zoom: s.zoom, panX: s.panX, panY: s.panY };
    };
    const local = (e: { clientX: number; clientY: number }): [number, number] => {
      const r = el.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    const world = (sx: number, sy: number): Point => screenToWorld(viewOf(), [sx, sy]);
    const pickCtx = (): PickContext => ({ objects: st().doc.objects, layer: layerLookup(st().doc.layers) });

    let g: Gesture | null = null;
    let poly: Poly | null = null;
    let last: PointerInfo | null = null;
    let hoverFrame = 0;

    // ---- utilidades de snapping ----------------------------------------------------------------

    /** Opciones de ajuste vigentes, o null si está apagado o suspendido (Alt / Ctrl-Cmd). */
    const snapOpts = (mods: Mods, suspendWithAlt = true): SnapOptions | null => {
      const s = st();
      if (!s.snapEnabled || mods.ctrl || (suspendWithAlt && mods.alt)) return null;
      if (!s.snapGrid && !s.snapObjects) return null;
      return {
        tol: snapToleranceMM(s.zoom),
        grid: s.snapGrid ? gridSteps(s.zoom).minor : null,
        objects: s.snapObjects,
      };
    };
    const targetsFor = (skip: ReadonlySet<string>): SnapSet => {
      const s = st();
      const dev = selectActiveDevice(s);
      return collectSnapTargets(s.doc.objects, skip, { width: dev.bed[0], height: dev.bed[1] }, visibleBox(viewOf(), s.viewport.width, s.viewport.height, 40));
    };

    // ---- selección y datos del gesto ---------------------------------------------------------------

    const selectedObjects = (): DesignObject[] => {
      const s = st();
      const ids = new Set(s.selectedIds);
      return s.doc.objects.filter((o) => ids.has(o.id) && !o.locked);
    };

    /** Ids del objeto y de su grupo (solo los seleccionables). */
    const groupIds = (id: string): string[] => expandGroups(st().doc.objects, [id], isSelectableId);
    const isSelectableId = (o: DesignObject) => !o.hidden && !o.locked && (layerLookup(st().doc.layers)(o.layer)?.visible ?? true);

    const fmtMM = (v: number) => fmt(v, 2);
    const setTip = (sx: number, sy: number, text: string) => patchInteraction({ tip: { x: sx + 16, y: sy + 18, text } });

    const capture = (e: PointerEvent) => {
      try {
        el.setPointerCapture(e.pointerId);
      } catch {
        /* el puntero ya no existe */
      }
    };
    const release = (id: number) => {
      try {
        if (el.hasPointerCapture(id)) el.releasePointerCapture(id);
      } catch {
        /* nada que liberar */
      }
    };

    const cancel = () => {
      if (g) release(g.pointerId);
      g = null;
      poly = null;
      clearGesture();
    };

    // ---- abrir gestos ----------------------------------------------------------------------------------

    const beginTransform = (spot: HandleSpot, e: PointerEvent, sx: number, sy: number) => {
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (!frame || objs.length === 0) return;
      const ids = objs.map((o) => o.id);
      const startWorld = world(sx, sy);
      const common: Common = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld, active: false };
      if (spot.kind === 'rotate') {
        g = { ...common, kind: 'rotate', ids, frame, baseDeg: selectionRotation(objs), matrix: [1, 0, 0, 1, 0, 0] };
      } else {
        const handle = spot.id as HandleId;
        const hp = handlePoint(frame, handle);
        g = {
          ...common, kind: 'scale', ids, frame, handle,
          offset: [hp[0] - startWorld[0], hp[1] - startWorld[1]],
          targets: targetsFor(new Set(ids)), matrix: [1, 0, 0, 1, 0, 0],
        };
      }
      capture(e);
    };

    const downSelect = (e: PointerEvent, sx: number, sy: number, mods: Mods) => {
      const s = st();
      const inter = useInteraction.getState();
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (frame) {
        const spots = handleSpots(frame, viewOf(), { rotateMode: inter.rotateMode, canRotate: canRotate(objs) });
        const spot = hitHandle(spots, sx, sy);
        if (spot) {
          beginTransform(spot, e, sx, sy);
          return;
        }
      }
      const startWorld = world(sx, sy);
      const hit = pickObject(pickCtx(), startWorld, HIT_PX / s.zoom);
      const common: Common = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld, active: false };
      if (hit) {
        const wasSelected = s.selectedIds.includes(hit.id);
        if (!wasSelected) {
          // Un objeto de un grupo selecciona todo el grupo; Alt+clic elige solo ese objeto.
          const picked = mods.alt ? [hit.id] : groupIds(hit.id);
          s.select(mods.shift ? [...s.selectedIds, ...picked] : picked);
          patchInteraction({ rotateMode: false });
        }
        g = { ...common, kind: 'press', hit, wasSelected, shift: mods.shift };
      } else {
        g = { ...common, kind: 'marquee', shift: mods.shift };
      }
      patchInteraction({ hoverId: null });
      capture(e);
    };

    const downDraw = (e: PointerEvent, tool: 'rect' | 'ellipse' | 'line', sx: number, sy: number, mods: Mods) => {
      const targets = targetsFor(new Set());
      const raw = world(sx, sy);
      const opts = snapOpts(mods);
      const start = opts ? applyPointSnap(raw, targets, opts) : raw;
      g = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld: start, active: false, kind: 'draw', tool, targets, end: start };
      capture(e);
    };

    const applyPointSnap = (p: Point, targets: SnapSet, opts: SnapOptions): Point => {
      const r = snapPoint(p, targets, opts);
      patchInteraction({ guides: r.guides });
      return [p[0] + r.dx, p[1] + r.dy];
    };

    // ---- actualizar gestos -------------------------------------------------------------------------------

    const updateMove = (gs: Extract<Gesture, { kind: 'move' }>, sx: number, sy: number, mods: Mods) => {
      const w = world(sx, sy);
      let dx = w[0] - gs.startWorld[0];
      let dy = w[1] - gs.startWorld[1];
      if (mods.shift) [dx, dy] = constrainDelta(dx, dy);
      let guides = NO_SNAP.guides;
      const opts = snapOpts(mods);
      if (opts) {
        const moved: [Point, Point] = [
          [gs.frame[0][0] + dx, gs.frame[0][1] + dy],
          [gs.frame[1][0] + dx, gs.frame[1][1] + dy],
        ];
        const axes = mods.shift ? { x: dy === 0, y: dx === 0 } : { x: true, y: true };
        const r = snapMove(moved, gs.targets, opts, axes);
        dx += r.dx;
        dy += r.dy;
        guides = r.guides;
      }
      gs.dx = dx;
      gs.dy = dy;
      patchInteraction({ live: translateBy(dx, dy), guides });
      setTip(sx, sy, t('canvas.tip.move', { dx: fmtMM(dx), dy: fmtMM(dy) }));
    };

    const updateScale = (gs: Extract<Gesture, { kind: 'scale' }>, sx: number, sy: number, mods: Mods) => {
      const w = world(sx, sy);
      let p: Point = [w[0] + gs.offset[0], w[1] + gs.offset[1]];
      let guides = NO_SNAP.guides;
      const opts = snapOpts(mods, false);
      if (opts) {
        const axisX = !(gs.handle === 'n' || gs.handle === 's');
        const axisY = !(gs.handle === 'e' || gs.handle === 'w');
        const fx: Feature[] = axisX ? [{ v: p[0], grid: true }] : [];
        const fy: Feature[] = axisY ? [{ v: p[1], grid: true }] : [];
        const r = snapFeatures(fx, fy, [gs.frame[0][1], gs.frame[1][1]], [gs.frame[0][0], gs.frame[1][0]], gs.targets, opts);
        p = [p[0] + r.dx, p[1] + r.dy];
        guides = r.guides;
      }
      const m = scaleByHandle(gs.frame, gs.handle, p, { proportional: !mods.shift, fromCenter: mods.alt });
      gs.matrix = m;
      const [fw, fh] = boundsSize(gs.frame);
      const nw = Math.abs(m[0]) * fw;
      const nh = Math.abs(m[3]) * fh;
      patchInteraction({ live: m, guides });
      const pct = fw > 1e-9 ? Math.round(Math.abs(m[0]) * 100) : fh > 1e-9 ? Math.round(Math.abs(m[3]) * 100) : 100;
      setTip(sx, sy, t('canvas.tip.scale', { w: fmtMM(nw), h: fmtMM(nh), pct }));
    };

    const updateRotate = (gs: Extract<Gesture, { kind: 'rotate' }>, sx: number, sy: number, mods: Mods) => {
      const w = world(sx, sy);
      // Alt gira alrededor del punto de referencia elegido; por defecto, el centro.
      const center = mods.alt ? anchorPoint(gs.frame, st().anchor) : boundsCenter(gs.frame);
      const r = rotateByPointer(center, gs.startWorld, w, { stepDeg: mods.shift ? 15 : undefined, baseDeg: gs.baseDeg });
      gs.matrix = r.matrix;
      patchInteraction({ live: r.matrix, pivot: center });
      const total = ((gs.baseDeg + r.degrees + 540) % 360) - 180;
      setTip(sx, sy, t('canvas.tip.rotate', { deg: fmt(total, 1) }));
    };

    const drawPaths = (tool: 'rect' | 'ellipse', a: Point, b: Point, mods: Mods): { paths: BezPath[]; w: number; h: number } => {
      const box = dragBox(a, b, { square: mods.shift, fromCenter: false });
      const [w, h] = boundsSize(box);
      return { paths: boxShapePaths(tool, box, tool === 'rect' ? st().cornerRadius : 0), w, h };
    };

    const updateDraw = (gs: Extract<Gesture, { kind: 'draw' }>, sx: number, sy: number, mods: Mods) => {
      let p = world(sx, sy);
      const opts = snapOpts(mods);
      if (gs.tool === 'line') {
        if (mods.shift) p = constrainAngle(gs.startWorld, p);
        if (opts) p = applyPointSnap(p, gs.targets, opts);
        else patchInteraction({ guides: [] });
        gs.end = p;
        patchInteraction({ draft: { paths: [polylineBez([gs.startWorld, p], false)] } });
        setTip(sx, sy, lineTip(gs.startWorld, p));
        return;
      }
      if (opts) p = applyPointSnap(p, gs.targets, opts);
      else patchInteraction({ guides: [] });
      gs.end = p;
      const { paths, w, h } = drawPaths(gs.tool, gs.startWorld, p, mods);
      patchInteraction({ draft: { paths } });
      const circle = gs.tool === 'ellipse' && Math.abs(w - h) < 1e-6;
      setTip(sx, sy, circle ? t('canvas.tip.circle', { d: fmtMM(w) }) : t('canvas.tip.size', { w: fmtMM(w), h: fmtMM(h) }));
    };

    const lineTip = (a: Point, b: Point): string => {
      const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
      const deg = (Math.atan2(b[1] - a[1], b[0] - a[0]) * 180) / Math.PI;
      return t('canvas.tip.line', { len: fmtMM(len), deg: fmt(deg, 1) });
    };

    // ---- polilínea ------------------------------------------------------------------------------------------

    const polyCursor = (sx: number, sy: number, mods: Mods): Point => {
      if (!poly) return world(sx, sy);
      let p = world(sx, sy);
      const lastPt = poly.points[poly.points.length - 1];
      if (mods.shift) p = constrainAngle(lastPt, p);
      const opts = snapOpts(mods);
      if (opts) p = applyPointSnap(p, poly.targets, opts);
      else patchInteraction({ guides: [] });
      return p;
    };

    const showPoly = (cursor: Point | null, sx = 0, sy = 0) => {
      if (!poly) return;
      const pts = cursor ? [...poly.points, cursor] : poly.points;
      patchInteraction({ draft: { paths: [polylineBez(pts, false)], rubber: true }, polyPoints: poly.points.length });
      if (cursor) setTip(sx, sy, lineTip(poly.points[poly.points.length - 1], cursor));
    };

    const createShape = (paths: BezPath[], kind: ShapeKind) => {
      const s = st();
      const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
      const id = uniqueId(kind, new Set(s.doc.objects.map((o) => o.id)));
      const label = `canvas.draw.${kind}` as I18nKey;
      s.addObjects([newVectorObject(id, layer, paths)], label);
      s.select([id]);
      s.setTool('select');
    };

    const finishPoly = (close: boolean) => {
      const pts = poly?.points ?? [];
      poly = null;
      clearGesture();
      const paths = polylinePaths(pts, close);
      if (!paths) return;
      createShape(paths, dedupePoints(pts).length === 2 ? 'line' : 'polyline');
    };

    const downLine = (e: PointerEvent, sx: number, sy: number, mods: Mods) => {
      if (!poly) {
        downDraw(e, 'line', sx, sy, mods);
        return;
      }
      const now = performance.now();
      const first = poly.points[0];
      const dblClick = now - poly.lastAt < DOUBLE_MS && Math.hypot(sx - poly.lastScreen[0], sy - poly.lastScreen[1]) < DOUBLE_PX;
      if (dblClick) {
        finishPoly(false);
        return;
      }
      const p = polyCursor(sx, sy, mods);
      const v = viewOf();
      const fs: Point = [first[0] * v.zoom + v.panX, v.panY - first[1] * v.zoom];
      if (poly.points.length >= 3 && Math.hypot(sx - fs[0], sy - fs[1]) < CLOSE_PX) {
        finishPoly(true);
        return;
      }
      poly.points.push(p);
      poly.lastAt = now;
      poly.lastScreen = [sx, sy];
      showPoly(p, sx, sy);
    };

    // ---- eventos del puntero -------------------------------------------------------------------------------------

    const onDown = (e: PointerEvent) => {
      if (e.target !== canvas) return;
      if (e.button === 2 && poly) {
        finishPoly(false);
        return;
      }
      if (e.button !== 0) return;
      const s = st();
      if (el.dataset.space === 'true' || s.activeTool === 'pan') return;
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      last = { sx, sy, mods };
      switch (s.activeTool) {
        case 'rect':
        case 'ellipse':
          downDraw(e, s.activeTool, sx, sy, mods);
          break;
        case 'line':
          downLine(e, sx, sy, mods);
          break;
        default:
          downSelect(e, sx, sy, mods);
      }
    };

    const startActive = (gs: Gesture) => {
      gs.active = true;
      if (gs.kind === 'press') {
        // Un clic sobre un objeto seleccionado que se arrastra: mueve toda la selección.
        const objs = selectedObjects();
        const frame = boundsOf(objs);
        if (!frame) {
          g = null;
          return;
        }
        const ids = objs.map((o) => o.id);
        g = { pointerId: gs.pointerId, startScreen: gs.startScreen, startWorld: gs.startWorld, active: true, kind: 'move', ids, frame, targets: targetsFor(new Set(ids)), dx: 0, dy: 0 };
        patchInteraction({ gesture: 'move', hoverId: null });
      } else if (gs.kind === 'move') patchInteraction({ gesture: 'move' });
      else if (gs.kind === 'scale') patchInteraction({ gesture: 'scale' });
      else if (gs.kind === 'rotate') patchInteraction({ gesture: 'rotate', pivot: boundsCenter(gs.frame) });
      else if (gs.kind === 'marquee') patchInteraction({ gesture: 'marquee' });
      else patchInteraction({ gesture: 'draw' });
    };

    const update = (sx: number, sy: number, mods: Mods) => {
      if (!g) return;
      if (!g.active) {
        if (Math.hypot(sx - g.startScreen[0], sy - g.startScreen[1]) < DRAG_PX) return;
        startActive(g);
      }
      const gs = g as Gesture | null;
      if (!gs) return;
      switch (gs.kind) {
        case 'move': updateMove(gs, sx, sy, mods); break;
        case 'scale': updateScale(gs, sx, sy, mods); break;
        case 'rotate': updateRotate(gs, sx, sy, mods); break;
        case 'marquee': {
          const mode = boxModeFor(gs.startScreen[0], sx);
          patchInteraction({ marquee: { a: gs.startWorld, b: world(sx, sy), mode } });
          break;
        }
        case 'draw': updateDraw(gs, sx, sy, mods); break;
        default: break;
      }
    };

    const hoverAt = (sx: number, sy: number) => {
      const s = st();
      const tool = s.activeTool;
      if (tool === 'pan') {
        patchInteraction({ cursor: 'grab', hoverId: null });
        return;
      }
      if (tool === 'rect' || tool === 'ellipse' || tool === 'line') {
        patchInteraction({ cursor: 'crosshair', hoverId: null });
        return;
      }
      const inter = useInteraction.getState();
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (frame) {
        const spot = hitHandle(handleSpots(frame, viewOf(), { rotateMode: inter.rotateMode, canRotate: canRotate(objs) }), sx, sy);
        if (spot) {
          patchInteraction({ cursor: cursorForHandle(spot), hoverId: null });
          return;
        }
      }
      const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
      patchInteraction({ hoverId: hit?.id ?? null, cursor: hit ? (s.selectedIds.includes(hit.id) ? 'move' : 'pointer') : 'default' });
    };

    const onMove = (e: PointerEvent) => {
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      last = { sx, sy, mods };
      if (g) {
        if (e.pointerId === g.pointerId) update(sx, sy, mods);
        return;
      }
      if (poly) {
        const p = polyCursor(sx, sy, mods);
        showPoly(p, sx, sy);
        return;
      }
      if (e.buttons !== 0) return;
      // El hover cuesta un hit-test: como máximo uno por cuadro.
      if (!hoverFrame) {
        hoverFrame = requestAnimationFrame(() => {
          hoverFrame = 0;
          if (last && !g) hoverAt(last.sx, last.sy);
        });
      }
    };

    const commitMove = (gs: Extract<Gesture, { kind: 'move' }>) => {
      if (gs.dx === 0 && gs.dy === 0) return;
      st().moveObjects(gs.ids, gs.dx, gs.dy, `drag:${++gestureSeq}`);
    };

    const onUp = (e: PointerEvent) => {
      if (!g || e.pointerId !== g.pointerId) return;
      const gs = g;
      release(gs.pointerId);
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      if (gs.active) update(sx, sy, mods); // posición final exacta
      g = null;
      const s = st();
      const cur = gs as Gesture;
      switch (cur.kind) {
        case 'press': {
          if (!cur.active) {
            // Clic sin arrastre.
            const picked = mods.alt ? [cur.hit.id] : groupIds(cur.hit.id);
            if (cur.shift && cur.wasSelected) s.select(s.selectedIds.filter((id) => !picked.includes(id)));
            else if (!cur.shift && cur.wasSelected) {
              if (s.selectedIds.length === 1) patchInteraction({ rotateMode: !useInteraction.getState().rotateMode });
              else s.select(picked);
            }
          }
          break;
        }
        case 'move': commitMove(cur); break;
        case 'scale':
          if (cur.active && !isIdentity(cur.matrix)) s.transformObjects(cur.ids, cur.matrix, `drag:${++gestureSeq}`);
          break;
        case 'rotate':
          if (cur.active && !isIdentity(cur.matrix)) s.transformObjects(cur.ids, cur.matrix, `drag:${++gestureSeq}`);
          break;
        case 'marquee':
          if (!cur.active) {
            if (!cur.shift) {
              s.clearSelection();
              patchInteraction({ rotateMode: false });
            }
          } else {
            const mode = boxModeFor(cur.startScreen[0], sx);
            const found = selectByBox(pickCtx(), normalizeBox(cur.startWorld, world(sx, sy)), mode);
            const ids = expandGroups(s.doc.objects, found, isSelectableId);
            s.select(cur.shift ? [...s.selectedIds, ...ids] : ids);
          }
          break;
        case 'draw': finishDraw(cur, sx, sy, mods); return;
        default: break;
      }
      clearGesture();
    };

    const finishDraw = (gs: Extract<Gesture, { kind: 'draw' }>, sx: number, sy: number, mods: Mods) => {
      if (gs.tool === 'line') {
        if (!gs.active) {
          // Clic simple: empieza una polilínea.
          clearGesture();
          poly = { points: [gs.startWorld], lastAt: performance.now(), lastScreen: [sx, sy], targets: gs.targets };
          patchInteraction({ gesture: 'polyline', polyPoints: 1 });
          showPoly(polyCursor(sx, sy, mods), sx, sy);
          return;
        }
        // Arrastre: una línea de dos puntos.
        const paths = polylinePaths([gs.startWorld, gs.end], false);
        clearGesture();
        if (paths) createShape(paths, 'line');
        return;
      }
      clearGesture();
      if (!gs.active) return;
      const { paths, w, h } = drawPaths(gs.tool, gs.startWorld, gs.end, mods);
      if (w < MIN_SHAPE_MM || h < MIN_SHAPE_MM) return;
      createShape(paths, gs.tool);
    };

    const onLeave = () => {
      if (hoverFrame) {
        cancelAnimationFrame(hoverFrame);
        hoverFrame = 0;
      }
      if (!g) patchInteraction({ hoverId: null });
    };

    // ---- teclado ------------------------------------------------------------------------------------------------------

    // Mientras hay una polilínea, Enter la termina y Retroceso quita el último punto, sin importar
    // dónde esté el foco. Va en fase de captura para anticiparse al comando "Eliminar".
    const onKeyCapture = (e: KeyboardEvent) => {
      if (!poly || isEditableTarget(e.target)) return;
      if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        finishPoly(false);
      } else if (e.key === 'Backspace') {
        e.preventDefault();
        poly.points.pop();
        if (poly.points.length === 0) cancel();
        else if (last) showPoly(polyCursor(last.sx, last.sy, last.mods), last.sx, last.sy);
      }
    };

    // Cambiar Mayús / Alt / Ctrl a mitad de un gesto actualiza el resultado sin mover el ratón.
    const onModifier = (e: KeyboardEvent) => {
      if (!['Shift', 'Alt', 'Control', 'Meta'].includes(e.key) || !last) return;
      const mods = modsOf(e);
      last = { ...last, mods };
      if (g?.active) update(last.sx, last.sy, mods);
      else if (poly) showPoly(polyCursor(last.sx, last.sy, mods), last.sx, last.sy);
    };

    const onContextMenu = (e: MouseEvent) => {
      if (poly) e.preventDefault();
    };

    el.addEventListener('pointerdown', onDown);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', cancel);
    el.addEventListener('pointerleave', onLeave);
    el.addEventListener('contextmenu', onContextMenu);
    window.addEventListener('keydown', onKeyCapture, true);
    window.addEventListener('keydown', onModifier);
    window.addEventListener('keyup', onModifier);
    window.addEventListener('blur', cancel);

    const unhook = registerGestureHooks({ cancel, closePolyline: () => poly && finishPoly(true) });

    // Cambiar de herramienta o de selección deshace lo pendiente.
    const unsub = useStore.subscribe((s, prev) => {
      if (s.activeTool !== prev.activeTool) {
        cancel();
        patchInteraction({ hoverId: null });
      }
      if (s.selectedIds !== prev.selectedIds && useInteraction.getState().rotateMode) {
        const same = s.selectedIds.length === prev.selectedIds.length && s.selectedIds.every((id) => prev.selectedIds.includes(id));
        if (!same) patchInteraction({ rotateMode: false });
      }
    });

    // El cursor CSS sigue al estado de la interacción.
    const syncCursor = (c: string) => {
      el.dataset.cursor = c;
    };
    syncCursor(useInteraction.getState().cursor);
    const unsubCursor = useInteraction.subscribe((s, prev) => {
      if (s.cursor !== prev.cursor) syncCursor(s.cursor);
    });

    return () => {
      el.removeEventListener('pointerdown', onDown);
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', cancel);
      el.removeEventListener('pointerleave', onLeave);
      el.removeEventListener('contextmenu', onContextMenu);
      window.removeEventListener('keydown', onKeyCapture, true);
      window.removeEventListener('keydown', onModifier);
      window.removeEventListener('keyup', onModifier);
      window.removeEventListener('blur', cancel);
      if (hoverFrame) cancelAnimationFrame(hoverFrame);
      unhook();
      unsub();
      unsubCursor();
      cancel();
    };
  }, [areaRef, canvasRef]);
}
