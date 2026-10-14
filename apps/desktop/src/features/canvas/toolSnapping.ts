/** Utilidades de ajuste magnético (snapping) de `useCanvasTools`. */
import type { Point } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { gridSteps, type ViewTransform } from '@/lib/viewport';
import { selectActiveDevice } from '@/store';
import { visibleBox } from './geom';
import { patchInteraction } from './interaction';
import { guideSnapTargets } from './guides';
import { type Mods } from './toolDeps';
import { collectSnapTargets, snapPoint, snapToleranceMM, type SnapOptions, type SnapSet } from './snap';
import { type SnapPointKind } from './snapPoints';
import type { AppState } from '@/store';

/** Ajuste magnético del lienzo: opciones vigentes, blancos de ajuste y marca del punto pegado. */
export function createSnapping(st: () => AppState, viewOf: () => ViewTransform) {
  /** Opciones de ajuste vigentes, o null si está apagado o suspendido (Alt / Ctrl-Cmd). */
  const snapOpts = (mods: Mods, suspendWithAlt = true): SnapOptions | null => {
    const s = st();
    if (!s.snapEnabled || mods.ctrl || (suspendWithAlt && mods.alt)) return null;
    if (!s.snapGrid && !s.snapObjects) return null;
    return {
      tol: snapToleranceMM(s.zoom),
      grid: s.snapGrid ? gridSteps(s.zoom, 7, s.prefs.units).minor : null,
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
    const bed = { width: dev.bed[0], height: dev.bed[1] };
    const set = collectSnapTargets(s.doc.objects, skip, bed, visibleBox(viewOf(), s.viewport.width, s.viewport.height, 40));
    // Las guías visibles del documento también atraen.
    if (s.showGuides && s.doc.guides?.length) {
      const g = guideSnapTargets(s.doc.guides, bed);
      set.xs.push(...g.xs);
      set.ys.push(...g.ys);
    }
    return set;
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

  return { snapOpts, KIND_LABEL, markOf, targetsFor, applyPointSnap, snapTo };
}

export type Snapping = ReturnType<typeof createSnapping>;
