import { useEffect, type RefObject } from 'react';
import type { Affine, BezPath, DesignObject, Point } from '@/api/types';
import { objectCurves } from '@/lib/geometry';
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
import { boxShapePaths, constrainAngle, constrainDelta, dragBox, newVectorObject, pickDrawLayer, type ShapeKind } from './shapes';
import { openContextMenu } from '@/features/contextmenu/uiStore';
import { createMeasureTool } from './measureTool';
import { createNodeTool } from './nodeTool';
import { cornerNode, handleLines, hasCurves, penDraft, penPath, smoothNode, type PenNode } from './pen';
import { createPickTool } from './pickTool';
import { newShapeObject, registerShape, type ShapeParams } from './shapeParams';
import { createTextAt, editText, prefetchFonts } from './textTool';
import { DRAG_PX, type Mods, type ToolDeps } from './toolDeps';
import {
  collectSnapTargets, NO_SNAP, snapFeatures, snapMove, snapPoint, snapToleranceMM, snapToPoints, type Feature, type SnapOptions, type SnapSet,
} from './snap';
import { nearestSnapPoint, objectSnapPoints, type SnapPoint, type SnapPointKind } from './snapPoints';
import {
  anchorPoint, boundsCenter, boundsOf, boundsSize, canRotate, handlePoint, isIdentity, rotateByPointer, scaleByHandle,
  selectionRotation, translateBy, type HandleId,
} from './transform';

/** Doble clic al dibujar polilíneas: tiempo (ms) y distancia (px). */
const DOUBLE_MS = 350;
const DOUBLE_PX = 6;
const CLOSE_PX = 8;
/** Tamaño mínimo (mm) de una forma arrastrada para que se cree. */
const MIN_SHAPE_MM = 0.05;

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
  | (Common & { kind: 'move'; ids: string[]; frame: Bounds2; targets: SnapSet; dx: number; dy: number; grab: Point | null })
  | (Common & { kind: 'scale'; ids: string[]; frame: Bounds2; handle: HandleId; offset: Point; targets: SnapSet; matrix: Affine })
  | (Common & { kind: 'rotate'; ids: string[]; frame: Bounds2; baseDeg: number; matrix: Affine })
  | (Common & { kind: 'marquee'; shift: boolean })
  | (Common & { kind: 'draw'; tool: 'rect' | 'ellipse' | 'polygon' | 'star'; targets: SnapSet; end: Point })
  /** Arrastre de las manijas del nodo recién puesto con la herramienta Línea. */
  | (Common & { kind: 'pen'; index: number });

type Bounds2 = [Point, Point];

interface Poly {
  nodes: PenNode[];
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
    const KIND_LABEL: Record<SnapPointKind, I18nKey> = {
      node: 'canvas.snap.node', mid: 'canvas.snap.mid', center: 'canvas.snap.center', inter: 'canvas.snap.inter',
    };
    const markOf = (r: ReturnType<typeof snapPoint>) =>
      r.point ? { p: r.point.p, kind: r.point.kind, label: t(KIND_LABEL[r.point.kind]) } : null;
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

    // La herramienta de nodos lleva su propio gesto (ver nodeTool.ts); aquí solo se le pasan los eventos.
    const nodeTool = createNodeTool({
      el, st, viewOf, world, capture, release, snapOpts, targetsFor, setTip,
      pick: (p, tol) => pickObject(pickCtx(), p, tol),
    });

    const cancel = () => {
      if (g) release(g.pointerId);
      g = null;
      poly = null;
      nodeTool.cancel();
      measureTool.cancel();
      pickTool.cancel();
      clearGesture();
    };

    /** Avisa (sin repetirse) que una transformación está apagada con su interruptor. */
    const blocked = (kind: 'move' | 'scale' | 'rotate'): boolean => {
      if (st().transformAllow[kind]) return false;
      st().notifyKey('info', `canvas.xform.off.${kind}`);
      return true;
    };

    // ---- abrir gestos ----------------------------------------------------------------------------------

    const beginTransform = (spot: HandleSpot, e: PointerEvent, sx: number, sy: number) => {
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (!frame || objs.length === 0) return;
      const ids = objs.map((o) => o.id);
      if (blocked(spot.kind === 'rotate' ? 'rotate' : 'scale')) return;
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

    const downDraw = (e: PointerEvent, tool: 'rect' | 'ellipse' | 'polygon' | 'star', sx: number, sy: number, mods: Mods) => {
      const targets = targetsFor(new Set());
      const start = snapTo(world(sx, sy), targets, mods);
      g = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld: start, active: false, kind: 'draw', tool, targets, end: start };
      capture(e);
    };

    const applyPointSnap = (p: Point, targets: SnapSet, opts: SnapOptions): Point => {
      const r = snapPoint(p, targets, opts);
      patchInteraction({ guides: r.guides, snapMark: markOf(r) });
      return [p[0] + r.dx, p[1] + r.dy];
    };

    /** Pega `p` a puntos de objetos, líneas y cuadrícula (si el ajuste está activo) y marca a dónde. */
    const snapTo = (p: Point, targets: SnapSet, mods: Mods): Point => {
      const opts = snapOpts(mods);
      if (!opts) {
        patchInteraction({ guides: [], snapMark: null });
        return p;
      }
      return applyPointSnap(p, targets, opts);
    };

    const toolDeps: ToolDeps = { el, st, viewOf, world, capture, release, snapOpts, targetsFor, setTip, snapTo };
    const measureTool = createMeasureTool(toolDeps);
    const pickTool = createPickTool(toolDeps);

    // ---- actualizar gestos -------------------------------------------------------------------------------

    const updateMove = (gs: Extract<Gesture, { kind: 'move' }>, sx: number, sy: number, mods: Mods) => {
      const w = world(sx, sy);
      let dx = w[0] - gs.startWorld[0];
      let dy = w[1] - gs.startWorld[1];
      if (mods.shift) [dx, dy] = constrainDelta(dx, dy);
      let guides = NO_SNAP.guides;
      let mark: ReturnType<typeof markOf> = null;
      const opts = snapOpts(mods);
      // Moverlo desde un punto (nodo, punto medio, centro): ese punto se pega a los de otros objetos.
      const pointHit = opts && gs.grab && !mods.shift ? snapToPoints([gs.grab[0] + dx, gs.grab[1] + dy], gs.targets, opts) : null;
      if (pointHit && gs.grab) {
        dx = pointHit.p[0] - gs.grab[0];
        dy = pointHit.p[1] - gs.grab[1];
        mark = { p: pointHit.p, kind: pointHit.kind, label: t(KIND_LABEL[pointHit.kind]) };
      } else if (opts) {
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
      patchInteraction({ live: translateBy(dx, dy), guides, snapMark: mark });
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
      // Con el candado de proporción las esquinas conservan la forma y Mayús la suelta; sin candado, al revés.
      const s = st();
      const m = scaleByHandle(gs.frame, gs.handle, p, { proportional: s.lockRatio !== mods.shift, fromCenter: mods.alt !== s.scaleFromCenter });
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
      const r = rotateByPointer(center, gs.startWorld, w, { stepDeg: mods.shift ? st().prefs.rotateStep : undefined, baseDeg: gs.baseDeg });
      gs.matrix = r.matrix;
      patchInteraction({ live: r.matrix, pivot: center });
      const total = ((gs.baseDeg + r.degrees + 540) % 360) - 180;
      setTip(sx, sy, t('canvas.tip.rotate', { deg: fmt(total, 1) }));
    };

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
      const params: ShapeParams = tool === 'polygon'
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

    // ---- curvas y polilíneas (herramienta Línea) ------------------------------------------------------------

    /** Punto del siguiente nodo bajo el cursor: Mayús fija 45 grados desde el último y el ajuste magnético lo pega. */
    const polyCursor = (sx: number, sy: number, mods: Mods): Point => {
      if (!poly) return world(sx, sy);
      let p = world(sx, sy);
      const lastPt = poly.nodes[poly.nodes.length - 1]!.p;
      if (mods.shift) p = constrainAngle(lastPt, p);
      return snapTo(p, poly.targets, mods);
    };

    /** Dibuja lo puesto, el tramo elástico hasta `cursor` y las manijas del último nodo. */
    const showPoly = (cursor: Point | null, sx = 0, sy = 0) => {
      if (!poly) return;
      const last = poly.nodes[poly.nodes.length - 1]!;
      patchInteraction({
        draft: { paths: [penDraft(poly.nodes, cursor)], rubber: true, nodes: poly.nodes.map((n) => n.p), handles: handleLines(last) },
        polyPoints: poly.nodes.length,
      });
      if (cursor) setTip(sx, sy, lineTip(last.p, cursor));
    };

    const createShape = (paths: BezPath[], kind: ShapeKind) => {
      const s = st();
      const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
      const id = uniqueId(kind, new Set(s.doc.objects.map((o) => o.id)));
      finishCreate(newVectorObject(id, layer, paths), kind);
    };

    /** Agrega el objeto recién dibujado, lo selecciona y vuelve a Seleccionar. */
    const finishCreate = (obj: DesignObject, kind: ShapeKind) => {
      const s = st();
      s.addObjects([obj], `canvas.draw.${kind}` as I18nKey);
      s.select([obj.id]);
      s.setTool('select');
    };

    const finishPoly = (close: boolean) => {
      const nodes = poly?.nodes ?? [];
      poly = null;
      clearGesture();
      const path = penPath(nodes, close);
      if (!path) return;
      const curved = hasCurves(path);
      createShape([path], curved ? 'curve' : path.length === 2 ? 'line' : 'polyline');
    };

    /** Abre el arrastre de manijas del nodo recién puesto (si no se arrastra, queda de esquina). */
    const beginPenDrag = (e: PointerEvent, sx: number, sy: number, index: number) => {
      g = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld: world(sx, sy), active: false, kind: 'pen', index };
      capture(e);
    };

    const downLine = (e: PointerEvent, sx: number, sy: number, mods: Mods) => {
      if (!poly) {
        const targets = targetsFor(new Set());
        const p = snapTo(world(sx, sy), targets, mods);
        poly = { nodes: [cornerNode(p)], lastAt: performance.now(), lastScreen: [sx, sy], targets };
        patchInteraction({ gesture: 'polyline', polyPoints: 1 });
        showPoly(null);
        beginPenDrag(e, sx, sy, 0);
        return;
      }
      const now = performance.now();
      const first = poly.nodes[0]!.p;
      const dblClick = now - poly.lastAt < DOUBLE_MS && Math.hypot(sx - poly.lastScreen[0], sy - poly.lastScreen[1]) < DOUBLE_PX;
      if (dblClick) {
        finishPoly(false);
        return;
      }
      const p = polyCursor(sx, sy, mods);
      const v = viewOf();
      const fs: Point = [first[0] * v.zoom + v.panX, v.panY - first[1] * v.zoom];
      // Clic sobre el primer nodo: cierra la forma.
      if (poly.nodes.length >= 3 && Math.hypot(sx - fs[0], sy - fs[1]) < CLOSE_PX) {
        finishPoly(true);
        return;
      }
      poly.nodes.push(cornerNode(p));
      poly.lastAt = now;
      poly.lastScreen = [sx, sy];
      showPoly(null);
      beginPenDrag(e, sx, sy, poly.nodes.length - 1);
    };

    // ---- eventos del puntero -------------------------------------------------------------------------------------

    /** Herramienta de texto: un clic sobre un texto lo edita; sobre otra cosa, crea uno nuevo ahí. */
    const downText = (sx: number, sy: number) => {
      const s = st();
      const p = world(sx, sy);
      const hit = pickObject(pickCtx(), p, HIT_PX / s.zoom);
      if (hit?.kind.type === 'text') editText(s, hit.id);
      else createTextAt(s, p);
    };

    const onDown = (e: PointerEvent) => {
      if (e.target !== canvas) return;
      if (e.button === 2 && poly) {
        finishPoly(false);
        return;
      }
      // Clic derecho con un modo de elegir puntos abierto: lo cancela.
      if (e.button === 2 && useInteraction.getState().pick) {
        cancel();
        return;
      }
      if (e.button !== 0) return;
      const s = st();
      if (el.dataset.space === 'true' || s.activeTool === 'pan') return;
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      last = { sx, sy, mods };
      if (useInteraction.getState().pick) {
        pickTool.down(e, sx, sy, mods);
        return;
      }
      switch (s.activeTool) {
        case 'rect':
        case 'ellipse':
        case 'polygon':
        case 'star':
          downDraw(e, s.activeTool, sx, sy, mods);
          break;
        case 'line':
          downLine(e, sx, sy, mods);
          break;
        case 'measure':
          measureTool.down(e, sx, sy, mods);
          break;
        case 'nodes':
          nodeTool.down(e, sx, sy, mods);
          break;
        case 'text':
          downText(sx, sy);
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
        if (blocked('move')) {
          g = null;
          release(gs.pointerId);
          return;
        }
        const ids = objs.map((o) => o.id);
        // Si se agarró cerca de un nodo, punto medio o centro de lo seleccionado, ese punto es el que se pega.
        let grab: Point | null = null;
        if (objs.length <= 50) {
          const own: SnapPoint[] = [];
          for (const o of objs) objectSnapPoints(o, own);
          grab = nearestSnapPoint(own, gs.startWorld, snapToleranceMM(st().zoom))?.p ?? null;
        }
        g = { pointerId: gs.pointerId, startScreen: gs.startScreen, startWorld: gs.startWorld, active: true, kind: 'move', ids, frame, targets: targetsFor(new Set(ids)), dx: 0, dy: 0, grab };
        patchInteraction({ gesture: 'move', hoverId: null });
      } else if (gs.kind === 'move') patchInteraction({ gesture: 'move' });
      else if (gs.kind === 'scale') patchInteraction({ gesture: 'scale' });
      else if (gs.kind === 'rotate') patchInteraction({ gesture: 'rotate', pivot: boundsCenter(gs.frame) });
      else if (gs.kind === 'marquee') patchInteraction({ gesture: 'marquee' });
      else if (gs.kind === 'draw') patchInteraction({ gesture: 'draw' });
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
        case 'pen': {
          if (!poly) break;
          const node = poly.nodes[gs.index];
          if (!node) break;
          let ptr = world(sx, sy);
          if (mods.shift) ptr = constrainAngle(node.p, ptr);
          poly.nodes[gs.index] = smoothNode(node.p, ptr);
          showPoly(null);
          setTip(sx, sy, t('canvas.tip.handle', { len: fmtMM(Math.hypot(ptr[0] - node.p[0], ptr[1] - node.p[1])) }));
          break;
        }
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
      if (tool === 'rect' || tool === 'ellipse' || tool === 'line' || tool === 'polygon' || tool === 'star') {
        patchInteraction({ cursor: 'crosshair', hoverId: null });
        return;
      }
      if (tool === 'measure') {
        // Resalta la forma bajo el cursor: un clic muestra sus datos.
        const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
        patchInteraction({ cursor: 'crosshair', hoverId: hit?.id ?? null });
        return;
      }
      if (tool === 'text') {
        patchInteraction({ cursor: 'text', hoverId: null });
        return;
      }
      if (tool === 'nodes') {
        patchInteraction({ hoverId: null });
        nodeTool.hover(sx, sy);
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
      if (nodeTool.move(e, sx, sy, mods)) return;
      if (pickTool.move(e, sx, sy, mods)) return;
      if (measureTool.move(e, sx, sy, mods)) return;
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
          if (last && !g && !nodeTool.active()) hoverAt(last.sx, last.sy);
        });
      }
    };

    const commitMove = (gs: Extract<Gesture, { kind: 'move' }>) => {
      if (gs.dx === 0 && gs.dy === 0) return;
      st().moveObjects(gs.ids, gs.dx, gs.dy, `drag:${++gestureSeq}`);
    };

    const onUp = (e: PointerEvent) => {
      if (nodeTool.active()) {
        const [nx, ny] = local(e);
        nodeTool.up(e, nx, ny, modsOf(e));
        return;
      }
      if (pickTool.up(e, ...local(e), modsOf(e))) return;
      if (measureTool.up(e, ...local(e), modsOf(e))) return;
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
        case 'draw': finishDraw(cur, mods); return;
        case 'pen':
          // El nodo ya está puesto (con sus manijas si se arrastró): la curva sigue abierta.
          if (poly) showPoly(polyCursor(sx, sy, mods), sx, sy);
          return;
        default: break;
      }
      clearGesture();
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
      const { box, w, h, radius, paths } = dragShape(gs.tool, gs.startWorld, gs.end, mods);
      if (w < MIN_SHAPE_MM || h < MIN_SHAPE_MM) return;
      const obj = newVectorObject(id, layer, paths);
      const params: ShapeParams = gs.tool === 'rect' ? { type: 'rect', w, h, radius } : { type: 'ellipse', w, h };
      registerShape(obj, { params, center: [(box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2] });
      finishCreate(obj, gs.tool);
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
        poly.nodes.pop();
        if (poly.nodes.length === 0) cancel();
        else if (last) showPoly(polyCursor(last.sx, last.sy, last.mods), last.sx, last.sy);
      }
    };

    // Cambiar Mayús / Alt / Ctrl a mitad de un gesto actualiza el resultado sin mover el ratón.
    const onModifier = (e: KeyboardEvent) => {
      if (!['Shift', 'Alt', 'Control', 'Meta'].includes(e.key) || !last) return;
      const mods = modsOf(e);
      last = { ...last, mods };
      if (nodeTool.active()) nodeTool.refresh(last.sx, last.sy, mods);
      else if (g?.active) update(last.sx, last.sy, mods);
      else if (poly) showPoly(polyCursor(last.sx, last.sy, mods), last.sx, last.sy);
    };

    // Clic derecho: la polilínea en curso se termina (lo hace pointerdown); un modo de elegir puntos se
    // cancela; en cualquier otro caso se abre el menú contextual, con el objeto de abajo seleccionado.
    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      if (poly || g || useInteraction.getState().pick || nodeTool.active()) return;
      const [sx, sy] = local(e);
      const s = st();
      const at = world(sx, sy);
      const hit = pickObject(pickCtx(), at, HIT_PX / s.zoom);
      if (hit && !s.selectedIds.includes(hit.id)) s.select(groupIds(hit.id));
      openContextMenu({ x: e.clientX, y: e.clientY, at, onObject: hit !== null || s.selectedIds.length > 0 });
    };

    // Doble clic: con nodos, agrega un nodo en el segmento; con Seleccionar, sobre un texto pasa a editarlo.
    const onDoubleClick = (e: MouseEvent) => {
      // Tras capturar el puntero, el navegador dirige el doble clic al área y no al lienzo.
      if (e.target !== canvas && e.target !== el) return;
      const s = st();
      const [sx, sy] = local(e);
      if (s.activeTool === 'nodes') {
        nodeTool.doubleClick(sx, sy);
      } else if (s.activeTool === 'select') {
        const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
        if (hit?.kind.type === 'text') {
          editText(s, hit.id);
          s.setTool('text');
        }
      }
    };

    el.addEventListener('pointerdown', onDown);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', cancel);
    el.addEventListener('pointerleave', onLeave);
    el.addEventListener('contextmenu', onContextMenu);
    el.addEventListener('dblclick', onDoubleClick);
    window.addEventListener('keydown', onKeyCapture, true);
    window.addEventListener('keydown', onModifier);
    window.addEventListener('keyup', onModifier);
    window.addEventListener('blur', cancel);

    const unhook = registerGestureHooks({ cancel, closePolyline: () => poly && finishPoly(true) });

    // Cambiar de herramienta o de selección deshace lo pendiente.
    const unsub = useStore.subscribe((s, prev) => {
      if (s.activeTool !== prev.activeTool) {
        cancel();
        patchInteraction({ hoverId: null, nodeHover: null, measure: null });
        if (s.activeTool === 'text') prefetchFonts();
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
      el.removeEventListener('dblclick', onDoubleClick);
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
