/** Gestos de la herramienta Seleccionar: mover, escalar y rotar. */
import type { DesignObject, Point } from '@/api/types';
import { expandGroups } from '@/lib/groups';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { handleSpots, hitHandle, type HandleSpot } from './handles';
import { HIT_PX, layerLookup, pickObject } from './hit';
import { patchInteraction, useInteraction } from './interaction';
import { constrainDelta } from './shapes';
import type { Mods } from './toolDeps';
import { NO_SNAP, snapFeatures, snapMove, snapToleranceMM, snapToPoints, type Feature } from './snap';
import { nearestSnapPoint, objectSnapPoints, type SnapPoint } from './snapPoints';
import { anchorPoint, boundsCenter, boundsOf, boundsSize, canRotate, handlePoint, rotateByPointer, scaleByHandle, selectionRotation, translateBy, type HandleId } from './transform';
import { fmtMM, dragKey, type Common, type GestureCtx, type Gesture } from './gestureState';

/** Seleccionar, mover, escalar y rotar: abrir, actualizar y confirmar el gesto. */
export function createTransformGesture(ctx: GestureCtx) {
  const { st, viewOf, world, capture, release, snapOpts, targetsFor, setTip, KIND_LABEL, pickCtx, state } = ctx;

  const selectedObjects = (): DesignObject[] => {
    const s = st();
    const ids = new Set(s.selectedIds);
    return s.doc.objects.filter((o) => ids.has(o.id) && !o.locked);
  };

  /** Ids del objeto y de su grupo (solo los seleccionables). */
  const groupIds = (id: string): string[] => expandGroups(st().doc.objects, [id], isSelectableId);

  const isSelectableId = (o: DesignObject) => !o.hidden && !o.locked && (layerLookup(st().doc.layers)(o.layer)?.visible ?? true);

  /** Avisa (sin repetirse) que una transformación está apagada con su interruptor. */
  const blocked = (kind: 'move' | 'scale' | 'rotate'): boolean => {
    if (st().transformAllow[kind]) return false;
    st().notifyKey('info', `canvas.xform.off.${kind}`);
    return true;
  };

  const beginTransform = (spot: HandleSpot, e: PointerEvent, sx: number, sy: number) => {
    const objs = selectedObjects();
    const frame = boundsOf(objs);
    if (!frame || objs.length === 0) return;
    const ids = objs.map((o) => o.id);
    if (blocked(spot.kind === 'rotate' ? 'rotate' : 'scale')) return;
    const startWorld = world(sx, sy);
    const common: Common = { pointerId: e.pointerId, startScreen: [sx, sy], startWorld, active: false };
    if (spot.kind === 'rotate') {
      state.g = { ...common, kind: 'rotate', ids, frame, baseDeg: selectionRotation(objs), matrix: [1, 0, 0, 1, 0, 0] };
    } else {
      const handle = spot.id as HandleId;
      const hp = handlePoint(frame, handle);
      state.g = {
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
      state.g = { ...common, kind: 'press', hit, wasSelected, shift: mods.shift };
    } else {
      state.g = { ...common, kind: 'marquee', shift: mods.shift };
    }
    patchInteraction({ hoverId: null });
    capture(e);
  };

  const updateMove = (gs: Extract<Gesture, { kind: 'move' }>, sx: number, sy: number, mods: Mods) => {
    const w = world(sx, sy);
    let dx = w[0] - gs.startWorld[0];
    let dy = w[1] - gs.startWorld[1];
    if (mods.shift) [dx, dy] = constrainDelta(dx, dy);
    let guides = NO_SNAP.guides;
    let mark: ReturnType<typeof ctx.markOf> = null;
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

  const startActive = (gs: Gesture) => {
    gs.active = true;
    if (gs.kind === 'press') {
      // Un clic sobre un objeto seleccionado que se arrastra: mueve toda la selección.
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (!frame) {
        state.g = null;
        return;
      }
      if (blocked('move')) {
        state.g = null;
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
      state.g = { pointerId: gs.pointerId, startScreen: gs.startScreen, startWorld: gs.startWorld, active: true, kind: 'move', ids, frame, targets: targetsFor(new Set(ids)), dx: 0, dy: 0, grab };
      patchInteraction({ gesture: 'move', hoverId: null });
    } else if (gs.kind === 'move') patchInteraction({ gesture: 'move' });
    else if (gs.kind === 'scale') patchInteraction({ gesture: 'scale' });
    else if (gs.kind === 'rotate') patchInteraction({ gesture: 'rotate', pivot: boundsCenter(gs.frame) });
    else if (gs.kind === 'marquee') patchInteraction({ gesture: 'marquee' });
    else if (gs.kind === 'draw') patchInteraction({ gesture: 'draw' });
  };

  const commitMove = (gs: Extract<Gesture, { kind: 'move' }>) => {
    if (gs.dx === 0 && gs.dy === 0) return;
    st().moveObjects(gs.ids, gs.dx, gs.dy, dragKey());
  };

  return { selectedObjects, groupIds, isSelectableId, blocked, downSelect, updateMove, updateScale, updateRotate, startActive, commitMove };
}
