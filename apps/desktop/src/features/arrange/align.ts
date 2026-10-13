import type { Document } from '@/api/types';
import { boundsCenter, boundsHeight, boundsWidth, type Unit } from './units';
import type { Bounds } from '@/lib/geometry';

export type AlignKind = 'left' | 'hcenter' | 'right' | 'top' | 'vcenter' | 'bottom';
export type DistributeAxis = 'h' | 'v';

/** Desplazamiento (mm) de todos los objetos de una unidad. */
export interface Move {
  ids: string[];
  dx: number;
  dy: number;
}

const EPS = 1e-9;
/** Redondea a 1 nm (y evita el -0) para que las posiciones no arrastren ruido de coma flotante. */
const round6 = (n: number) => Math.round(n * 1e6) / 1e6 + 0;

/** Cama del dispositivo en coordenadas de documento (origen abajo a la izquierda). */
export const bedBounds = (bed: readonly [number, number]): Bounds => [[0, 0], [bed[0], bed[1]]];

/**
 * Mueve cada unidad al borde o centro de `ref`. Eje Y hacia arriba: "arriba" es el máximo en Y.
 * La unidad `fixedId` (el objeto de referencia) no se mueve.
 */
export function alignMoves(units: readonly Unit[], kind: AlignKind, ref: Bounds, fixedId?: string): Move[] {
  const rc = boundsCenter(ref);
  const moves: Move[] = [];
  for (const u of units) {
    if (u.id === fixedId) continue;
    const b = u.bounds;
    const c = boundsCenter(b);
    let dx = 0;
    let dy = 0;
    switch (kind) {
      case 'left': dx = ref[0][0] - b[0][0]; break;
      case 'right': dx = ref[1][0] - b[1][0]; break;
      case 'hcenter': dx = rc[0] - c[0]; break;
      case 'bottom': dy = ref[0][1] - b[0][1]; break;
      case 'top': dy = ref[1][1] - b[1][1]; break;
      case 'vcenter': dy = rc[1] - c[1]; break;
    }
    if (Math.abs(dx) > EPS || Math.abs(dy) > EPS) moves.push({ ids: u.ids, dx, dy });
  }
  return moves;
}

/**
 * Espacio igual entre bordes: los dos extremos se quedan y los demás se reparten para que todos
 * los huecos midan lo mismo. Hacen falta al menos tres unidades.
 */
export function distributeMoves(units: readonly Unit[], axis: DistributeAxis): Move[] {
  if (units.length < 3) return [];
  const i = axis === 'h' ? 0 : 1;
  const size = (u: Unit) => (axis === 'h' ? boundsWidth(u.bounds) : boundsHeight(u.bounds));
  const sorted = [...units].sort((a, b) => a.bounds[0][i] + a.bounds[1][i] - (b.bounds[0][i] + b.bounds[1][i]));
  const first = sorted[0]!;
  const last = sorted[sorted.length - 1]!;
  const span = last.bounds[1][i] - first.bounds[0][i];
  const gap = (span - sorted.reduce((sum, u) => sum + size(u), 0)) / (sorted.length - 1);
  const moves: Move[] = [];
  let cursor = first.bounds[0][i];
  for (const u of sorted) {
    const delta = cursor - u.bounds[0][i];
    cursor += size(u) + gap;
    if (Math.abs(delta) > EPS) moves.push({ ids: u.ids, dx: axis === 'h' ? delta : 0, dy: axis === 'v' ? delta : 0 });
  }
  return moves;
}

/** Lleva el bloque de la selección al centro de la cama (las unidades conservan su posición relativa). */
export function centerOnBedMoves(units: readonly Unit[], block: Bounds, bed: Bounds): Move[] {
  const [bx, by] = boundsCenter(block);
  const [cx, cy] = boundsCenter(bed);
  const dx = cx - bx;
  const dy = cy - by;
  if (Math.abs(dx) <= EPS && Math.abs(dy) <= EPS) return [];
  return units.map((u) => ({ ids: u.ids, dx, dy }));
}

/** Aplica los desplazamientos de una vez. Devuelve el mismo documento si no hay nada que mover. */
export function moveDoc(doc: Document, moves: readonly Move[]): Document {
  if (moves.length === 0) return doc;
  const by = new Map<string, Move>();
  for (const m of moves) for (const id of m.ids) by.set(id, m);
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      const m = by.get(o.id);
      if (!m || o.locked) return o;
      const t = o.transform;
      return { ...o, transform: [t[0], t[1], t[2], t[3], round6(t[4] + m.dx), round6(t[5] + m.dy)] };
    }),
  };
}
