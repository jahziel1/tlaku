import { create } from 'zustand';
import type { Guide, GuideAxis, Point } from '@/api/types';
import type { AppState } from '@/store';
import { selectActiveDevice } from '@/store';
import { gridSteps, screenToWorld } from '@/lib/viewport';
import { collectSnapTargets, snapToleranceMM, type SnapTarget } from './snap';

/**
 * Guías del documento (`Document.guides`): líneas de referencia que no se cortan. Se crean
 * arrastrando desde una regla, se mueven arrastrándolas y se borran soltándolas sobre su regla o
 * fuera del lienzo. Atraen al ajuste magnético mientras estén visibles.
 */

/** Distancia en px a la que el puntero "agarra" una guía. */
export const GUIDE_HIT_PX = 5;

/** El eje de la guía que crea cada regla: la regla horizontal (arriba) crea guías horizontales (y fija). */
export const guideAxisForRuler = (ruler: 'x' | 'y'): GuideAxis => (ruler === 'x' ? 'y' : 'x');

/** Posición de pantalla (px, relativa al lienzo) de la guía a lo largo del eje perpendicular a ella. */
export function guideScreenPos(g: Guide, view: { zoom: number; panX: number; panY: number }): number {
  return g.axis === 'x' ? view.panX + g.pos * view.zoom : view.panY - g.pos * view.zoom;
}

/** Guía visible más cercana al punto de pantalla `(sx, sy)` dentro de `GUIDE_HIT_PX`, o `-1`. */
export function guideAt(
  guides: readonly Guide[],
  view: { zoom: number; panX: number; panY: number },
  sx: number,
  sy: number,
): number {
  let best = -1;
  let bestD = GUIDE_HIT_PX + 1e-9;
  guides.forEach((g, i) => {
    const d = Math.abs(guideScreenPos(g, view) - (g.axis === 'x' ? sx : sy));
    if (d <= bestD) {
      best = i;
      bestD = d;
    }
  });
  return best;
}

/** Posición en mm de la guía `axis` para un punto de pantalla del lienzo. */
export function guideValueFromScreen(
  axis: GuideAxis,
  view: { zoom: number; panX: number; panY: number },
  sx: number,
  sy: number,
): number {
  const w: Point = screenToWorld(view, [sx, sy]);
  return axis === 'x' ? w[0] : w[1];
}

/** Redondeo de una posición de guía: el milímetro sobra de precisión, 0,001 basta. */
export const roundGuide = (v: number): number => Math.round(v * 1000) / 1000;

/**
 * Pega la posición de una guía a la cuadrícula visible y a los bordes y centros de los objetos y
 * la cama, según el ajuste magnético activo. `skipIndex` es la guía que se arrastra (no se pega a sí misma).
 */
export function snapGuidePos(s: AppState, axis: GuideAxis, pos: number, skipIndex: number | null, suspend: boolean): number {
  if (!s.snapEnabled || suspend || (!s.snapGrid && !s.snapObjects)) return roundGuide(pos);
  const tol = snapToleranceMM(s.zoom);
  let best = pos;
  let bestD = tol + 1e-9;
  const consider = (v: number) => {
    const d = Math.abs(v - pos);
    if (d < bestD) {
      best = v;
      bestD = d;
    }
  };
  if (s.snapObjects) {
    const dev = selectActiveDevice(s);
    const set = collectSnapTargets(s.doc.objects, new Set(), { width: dev.bed[0], height: dev.bed[1] });
    for (const t of axis === 'x' ? set.xs : set.ys) consider(t.v);
    (s.doc.guides ?? []).forEach((g, i) => {
      if (g.axis === axis && i !== skipIndex) consider(g.pos);
    });
  }
  if (s.snapGrid) {
    const step = gridSteps(s.zoom, 7, s.prefs.units).minor;
    consider(Math.round(pos / step) * step);
  }
  return roundGuide(best);
}

/** Referencias de ajuste magnético de las guías visibles, para agregar a los candidatos de `collectSnapTargets`. */
export function guideSnapTargets(
  guides: readonly Guide[],
  bed: { width: number; height: number },
): { xs: SnapTarget[]; ys: SnapTarget[] } {
  const xs: SnapTarget[] = [];
  const ys: SnapTarget[] = [];
  for (const g of guides) {
    if (g.axis === 'x') xs.push({ v: g.pos, kind: 'guide', lo: 0, hi: bed.height });
    else ys.push({ v: g.pos, kind: 'guide', lo: 0, hi: bed.width });
  }
  return { xs, ys };
}

/** ¿Está el punto de pantalla fuera del lienzo (o sobre sus reglas)? Soltar una guía ahí la borra. */
export function outsideCanvas(width: number, height: number, sx: number, sy: number, margin = 0): boolean {
  return sx < margin || sy < margin || sx > width || sy > height;
}

// ---- diálogo de guías ---------------------------------------------------------------------------

interface GuideUi {
  open: boolean;
  /** Guía a destacar al abrir (doble clic sobre una guía). */
  focus: number | null;
  show: (focus?: number | null) => void;
  close: () => void;
}

export const useGuideUi = create<GuideUi>()((set) => ({
  open: false,
  focus: null,
  show: (focus = null) => set({ open: true, focus }),
  close: () => set({ open: false, focus: null }),
}));
