import type { Point } from '@/api/types';
import { mirrorCopiesDoc } from '@/features/arrange/transformOps';
import { t } from '@/i18n';
import { reflectAcrossLine, twoPointSimilarity } from '@/lib/geometry';
import { fmt } from '@/lib/platform';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import { constrainAngle } from './shapes';
import type { SnapSet } from './snap';
import { DRAG_PX, type Mods, type ToolDeps } from './toolDeps';
import { polylineBez } from '@/lib/bezpath';

/** Inicia un modo de elegir puntos sobre la selección actual (lo llaman los comandos). */
export function startPickMode(mode: 'mirror' | 'twoPoint'): void {
  useInteraction.setState({ gesture: 'pick', pick: { mode, points: [] }, cursor: 'crosshair', draft: null, measure: null });
}

interface Press {
  pointerId: number;
  startScreen: Point;
  active: boolean;
  targets: SnapSet;
}

/**
 * Modos de dos pasos del lienzo:
 *  - Espejo sobre una línea: clic en A y clic en B (o arrastrar de A a B). Se crean copias reflejadas.
 *  - Rotar y escalar por dos puntos: clic en el pivote; luego arrastrar desde un punto de
 *    referencia hasta donde debe quedar. Gira la selección; con Mayús también la escala.
 * Esc o clic derecho lo cancelan (el llamador limpia el estado).
 */
export function createPickTool(d: ToolDeps) {
  let press: Press | null = null;

  const pick = () => useInteraction.getState().pick;

  const setPoints = (points: Point[]) => {
    const p = pick();
    if (p) patchInteraction({ pick: { ...p, points } });
  };

  /** Punto bajo el cursor, ajustado (Mayús fija ángulos de 45 grados respecto al punto anterior). */
  const point = (sx: number, sy: number, mods: Mods, targets: SnapSet, from?: Point): Point => {
    let p = d.world(sx, sy);
    if (mods.shift && from && pick()?.mode === 'mirror') p = constrainAngle(from, p, 45);
    return d.snapTo(p, targets, mods);
  };

  const finishMirror = (a: Point, b: Point) => {
    const m = reflectAcrossLine(a, b);
    clearGesture();
    if (!m) return;
    const s = d.st();
    let created: string[] = [];
    s.applyDocChange('arrange.history.mirror', (doc) => {
      const r = mirrorCopiesDoc(doc, s.selectedIds, m);
      created = r.created;
      return r.doc;
    });
    if (created.length) s.select(created);
  };

  const preview = (a: Point, b: Point) => patchInteraction({ draft: { paths: [polylineBez([a, b], false)] } });

  return {
    active: () => press !== null,

    down(e: PointerEvent, sx: number, sy: number, mods: Mods) {
      const state = pick();
      if (!state) return;
      const targets = d.targetsFor(new Set(d.st().selectedIds));
      const pts = state.points;
      if (state.mode === 'mirror') {
        if (pts.length === 0) {
          const a = point(sx, sy, mods, targets);
          setPoints([a]);
          press = { pointerId: e.pointerId, startScreen: [sx, sy], active: false, targets };
          d.capture(e);
        } else {
          finishMirror(pts[0]!, point(sx, sy, mods, targets, pts[0]));
        }
        return;
      }
      if (pts.length === 0) {
        setPoints([point(sx, sy, mods, targets)]);
        patchInteraction({ pivot: null });
        return;
      }
      // Rotar/escalar: se presiona el punto de referencia y se arrastra hasta su destino.
      const r = point(sx, sy, mods, targets);
      setPoints([pts[0]!, r]);
      press = { pointerId: e.pointerId, startScreen: [sx, sy], active: false, targets };
      d.capture(e);
    },

    move(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      const state = pick();
      if (!state) return false;
      const pts = state.points;
      if (press && e.pointerId !== press.pointerId) return true;
      const targets = press?.targets ?? d.targetsFor(new Set(d.st().selectedIds));
      if (press && !press.active && Math.hypot(sx - press.startScreen[0], sy - press.startScreen[1]) >= DRAG_PX) press.active = true;
      if (state.mode === 'mirror') {
        if (pts.length === 1 && (press?.active || !press)) {
          const b = point(sx, sy, mods, targets, pts[0]);
          preview(pts[0]!, b);
          d.setTip(sx, sy, t('canvas.pick.mirror.line', { len: fmt(Math.hypot(b[0] - pts[0]![0], b[1] - pts[0]![1]), 2) }));
        } else {
          d.setTip(sx, sy, t(pts.length === 0 ? 'canvas.pick.mirror.a' : 'canvas.pick.mirror.b'));
          point(sx, sy, mods, targets);
        }
        return true;
      }
      if (pts.length === 0) {
        point(sx, sy, mods, targets);
        d.setTip(sx, sy, t('canvas.pick.twoPoint.pivot'));
      } else if (pts.length === 1) {
        const p = point(sx, sy, mods, targets);
        patchInteraction({ pivot: pts[0]! });
        preview(pts[0]!, p);
        d.setTip(sx, sy, t('canvas.pick.twoPoint.ref'));
      } else if (press?.active) {
        const to = point(sx, sy, mods, targets);
        const m = twoPointSimilarity(pts[0]!, pts[1]!, to, mods.shift);
        patchInteraction({ live: m, pivot: pts[0]!, draft: { paths: [polylineBez([pts[0]!, to], false)] } });
        const deg = ((Math.atan2(to[1] - pts[0]![1], to[0] - pts[0]![0]) - Math.atan2(pts[1]![1] - pts[0]![1], pts[1]![0] - pts[0]![0])) * 180) / Math.PI;
        const norm = ((deg + 540) % 360) - 180;
        const k = m ? Math.hypot(m[0], m[1]) : 1;
        d.setTip(sx, sy, mods.shift ? t('canvas.pick.twoPoint.both', { deg: fmt(norm, 1), pct: Math.round(k * 100) }) : t('canvas.tip.rotate', { deg: fmt(norm, 1) }));
      }
      return true;
    },

    up(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      const state = pick();
      if (!state || !press || e.pointerId !== press.pointerId) return false;
      const gs = press;
      press = null;
      d.release(gs.pointerId);
      const pts = state.points;
      if (state.mode === 'mirror') {
        // Arrastre de A a B: termina aquí. Un clic simple espera el segundo punto.
        if (gs.active) finishMirror(pts[0]!, point(sx, sy, mods, gs.targets, pts[0]));
        return true;
      }
      if (gs.active && pts.length === 2) {
        const to = point(sx, sy, mods, gs.targets);
        const m = twoPointSimilarity(pts[0]!, pts[1]!, to, mods.shift);
        clearGesture();
        if (m) d.st().transformObjects(d.st().selectedIds, m, `twopoint:${Date.now()}`);
        return true;
      }
      // Sin arrastre: el punto de referencia no sirve; se vuelve a pedir.
      setPoints([pts[0]!]);
      patchInteraction({ live: null, draft: null });
      return true;
    },

    cancel() {
      if (press) d.release(press.pointerId);
      press = null;
    },
  };
}
