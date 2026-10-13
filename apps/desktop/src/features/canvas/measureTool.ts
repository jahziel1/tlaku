import type { Point } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { formatLength } from '@/lib/units';
import { getPref } from '@/store';
import { HIT_PX, layerLookup, pickObject } from './hit';
import { clearGesture, patchInteraction } from './interaction';
import { measureBetween } from './measure';
import { constrainAngle } from './shapes';
import type { SnapSet } from './snap';
import { DRAG_PX, type Mods, type ToolDeps } from './toolDeps';

interface Drag {
  pointerId: number;
  startScreen: Point;
  a: Point;
  targets: SnapSet;
  active: boolean;
}

/**
 * Herramienta Medir: arrastrar entre dos puntos (con ajuste magnético; Mayús fija 0, 45 o 90
 * grados) deja una línea de medida con distancia, ángulo y Δx/Δy; un clic sobre una forma muestra
 * sus datos (nodos, perímetro, área). La medición queda visible hasta Esc o hasta cambiar de herramienta.
 */
export function createMeasureTool(d: ToolDeps) {
  let g: Drag | null = null;

  const endPoint = (gs: Drag, sx: number, sy: number, mods: Mods): Point => {
    let p = d.world(sx, sy);
    if (mods.shift) p = constrainAngle(gs.a, p, 45);
    return d.snapTo(p, gs.targets, mods);
  };

  const tip = (sx: number, sy: number, a: Point, b: Point) => {
    const m = measureBetween(a, b);
    const unit = getPref('units');
    const digits = unit === 'in' ? 4 : 3;
    d.setTip(sx, sy, t('canvas.measure.tip', { dist: formatLength(m.distance, unit, { digits }), deg: fmt(m.angle, 1), dx: formatLength(m.dx, unit, { digits }), dy: formatLength(m.dy, unit, { digits }) }));
  };

  return {
    active: () => g !== null,

    down(e: PointerEvent, sx: number, sy: number, mods: Mods) {
      const targets = d.targetsFor(new Set());
      const a = d.snapTo(d.world(sx, sy), targets, mods);
      g = { pointerId: e.pointerId, startScreen: [sx, sy], a, targets, active: false };
      d.capture(e);
    },

    /** `true` si el evento era de esta herramienta. */
    move(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!g || e.pointerId !== g.pointerId) return false;
      if (!g.active) {
        if (Math.hypot(sx - g.startScreen[0], sy - g.startScreen[1]) < DRAG_PX) return true;
        g.active = true;
        patchInteraction({ gesture: 'measure' });
      }
      const b = endPoint(g, sx, sy, mods);
      patchInteraction({ measure: { a: g.a, b, objectId: null } });
      tip(sx, sy, g.a, b);
      return true;
    },

    up(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!g || e.pointerId !== g.pointerId) return false;
      const gs = g;
      g = null;
      d.release(gs.pointerId);
      if (gs.active) {
        patchInteraction({ measure: { a: gs.a, b: endPoint(gs, sx, sy, mods), objectId: null } });
      } else {
        // Clic sin arrastre: datos de la forma bajo el cursor, o limpiar la medición.
        const s = d.st();
        const p = d.world(sx, sy);
        const hit = pickObject({ objects: s.doc.objects, layer: layerLookup(s.doc.layers) }, p, HIT_PX / s.zoom);
        patchInteraction({ measure: hit ? { a: p, b: p, objectId: hit.id } : null });
      }
      clearGesture();
      return true;
    },

    cancel() {
      if (g) d.release(g.pointerId);
      g = null;
    },
  };
}
