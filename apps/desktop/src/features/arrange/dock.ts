import type { Document } from '@/api/types';
import { moveDoc, type Move } from './align';
import { boundsCenter, type Unit } from './units';

/**
 * Acoplar y mover juntos (D74), con las cajas envolventes:
 * - `dock`: cada unidad se desliza en la dirección elegida hasta que su borde de avance toca el
 *   borde opuesto de la referencia, dejando `padding` mm de hueco.
 * - `together`: cada unidad avanza en línea recta hacia el centro de la referencia hasta que sus
 *   cajas se tocan (con `padding` de hueco).
 * Con `asGroup` todas se mueven lo mismo que la primera en llegar y conservan su posición relativa.
 */

export type DockMode = 'dock' | 'together';
export type DockDirection = 'left' | 'right' | 'up' | 'down';

export interface DockParams {
  mode: DockMode;
  direction: DockDirection;
  /** Hueco (mm) que queda entre las cajas al tocarse; negativo = se solapan. */
  padding: number;
  /** Mover todas juntas, sin cambiar su posición relativa. */
  asGroup: boolean;
}

export const DEFAULT_DOCK: DockParams = { mode: 'dock', direction: 'left', padding: 0, asGroup: false };

export interface DockPlan {
  moves: Move[];
  /** Unidades que no avanzan hasta tocar (ya pasaron la referencia o no hay camino). */
  skipped: number;
}

const EPS = 1e-9;

/** Distancia que debe avanzar `box` en `dir` (vector unitario) hasta tocar `ref` (con `pad`), o `null` si no lo alcanza. */
function travel(box: Unit['bounds'], ref: Unit['bounds'], dir: [number, number], pad: number): number | null {
  // Intervalo de t donde las cajas (ref crecida `pad`) se solapan en cada eje: entrada y salida.
  let enter = -Infinity;
  let leave = Infinity;
  for (let axis = 0; axis < 2; axis++) {
    const lo = ref[0][axis]! - pad - box[1][axis]!;
    const hi = ref[1][axis]! + pad - box[0][axis]!;
    const v = dir[axis]!;
    if (Math.abs(v) < EPS) {
      if (lo > EPS || hi < -EPS) return null;
    } else {
      const a = lo / v;
      const b = hi / v;
      enter = Math.max(enter, Math.min(a, b));
      leave = Math.min(leave, Math.max(a, b));
    }
  }
  if (enter > leave + EPS) return null;
  // Ya solapadas con el hueco pedido: no hay que avanzar (entrar quedaría detrás).
  return enter < -EPS ? null : enter;
}

/** Desplazamientos que acoplan o juntan `movers` con `ref`. */
export function planDock(ref: Unit, movers: readonly Unit[], p: DockParams): DockPlan {
  const pad = Number.isFinite(p.padding) ? p.padding : 0;
  const unit: Record<DockDirection, [number, number]> = { left: [-1, 0], right: [1, 0], up: [0, 1], down: [0, -1] };
  const [rcx, rcy] = boundsCenter(ref.bounds);
  const found: { u: Unit; d: [number, number] }[] = [];
  let skipped = 0;
  for (const u of movers) {
    if (u.id === ref.id) continue;
    let dir = unit[p.direction];
    if (p.mode === 'together') {
      const [cx, cy] = boundsCenter(u.bounds);
      const len = Math.hypot(rcx - cx, rcy - cy);
      if (len < EPS) {
        skipped++;
        continue;
      }
      dir = [(rcx - cx) / len, (rcy - cy) / len];
    }
    const t = travel(u.bounds, ref.bounds, dir, pad);
    if (t === null) skipped++;
    else found.push({ u, d: [dir[0] * t, dir[1] * t] });
  }
  if (found.length === 0) return { moves: [], skipped };
  if (p.asGroup) {
    // Todas se mueven lo que avanza la más cercana (en su propia dirección).
    const first = found.reduce((a, b) => (Math.hypot(...b.d) < Math.hypot(...a.d) ? b : a));
    const [dx, dy] = first.d;
    return { moves: movers.filter((u) => u.id !== ref.id).map((u) => ({ ids: u.ids, dx, dy })), skipped: 0 };
  }
  return { moves: found.filter(({ d }) => Math.abs(d[0]) > EPS || Math.abs(d[1]) > EPS).map(({ u, d }) => ({ ids: u.ids, dx: d[0], dy: d[1] })), skipped };
}

export function dockDoc(doc: Document, moves: readonly Move[]): Document {
  return moveDoc(doc, moves);
}
