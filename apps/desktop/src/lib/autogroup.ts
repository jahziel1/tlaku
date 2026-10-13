import type { DesignObject, Point } from '@/api/types';
import { groupIdsOf } from './groups';
import { objectPolylines, pointsBounds, type Bounds } from './geometry';

/**
 * Agrupar automáticamente por contención (D31): cada pieza con los huecos y grabados que quedan
 * dentro de su contorno exterior.
 *
 * - Solo cuentan los vectores sin grupo y sin bloquear, con algún contorno cerrado. Lo que ya
 *   está agrupado, las imágenes y los textos no se tocan.
 * - A cada forma se le busca su contenedora más pequeña (su caja cabe en la de la otra y un punto
 *   suyo cae dentro del contorno exterior de la otra). La profundidad de anidamiento decide los
 *   papeles con la regla par-impar del motor: profundidad 0 = pieza, 1 = hueco o grabado de esa
 *   pieza, 2 = una pieza nueva dentro del hueco (con sus propios huecos), etc.
 * - Cada pieza con al menos un hijo directo forma un grupo con ellos. Las piezas sueltas quedan
 *   como están.
 */

interface Shape {
  id: string;
  /** Contorno exterior: el cerrado de mayor área. */
  outline: Point[];
  box: Bounds;
  area: number;
}

function polygonArea(points: readonly Point[]): number {
  let sum = 0;
  for (let i = 0; i < points.length; i++) {
    const a = points[i]!;
    const b = points[(i + 1) % points.length]!;
    sum += a[0] * b[1] - b[0] * a[1];
  }
  return Math.abs(sum) / 2;
}

/** ¿Está el punto dentro del polígono? (regla par-impar; el borde cuenta como indefinido). */
function inside(p: Point, poly: readonly Point[]): boolean {
  let hit = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i]!;
    const b = poly[j]!;
    if (a[1] > p[1] !== b[1] > p[1] && p[0] < ((b[0] - a[0]) * (p[1] - a[1])) / (b[1] - a[1]) + a[0]) hit = !hit;
  }
  return hit;
}

function shapeOf(o: DesignObject): Shape | null {
  if (o.kind.type !== 'vector' || o.group !== null || o.locked) return null;
  let best: Point[] | null = null;
  let area = 0;
  for (const p of objectPolylines(o)) {
    if (!p.closed || p.points.length < 3) continue;
    const a = polygonArea(p.points);
    if (a > area) {
      area = a;
      best = p.points;
    }
  }
  if (!best || !(area > 0)) return null;
  const box = pointsBounds(best);
  return box ? { id: o.id, outline: best, box, area } : null;
}

const within = (inner: Bounds, outer: Bounds) =>
  inner[0][0] >= outer[0][0] - 1e-9 && inner[0][1] >= outer[0][1] - 1e-9 && inner[1][0] <= outer[1][0] + 1e-9 && inner[1][1] <= outer[1][1] + 1e-9;

/**
 * Grupos que se formarían: cada uno es la lista de ids (la pieza primero, luego lo que contiene,
 * en el orden de los objetos). Vacío si no hay nada que agrupar.
 */
export function containmentGroups(objects: readonly DesignObject[]): string[][] {
  const shapes = objects.map(shapeOf).filter((s): s is Shape => s !== null);
  // De mayor a menor: el contenedor más pequeño de una forma es el último que la contiene.
  const bySize = [...shapes].sort((a, b) => b.area - a.area);
  const parent = new Map<string, string>();
  bySize.forEach((s, i) => {
    const probe = s.outline[0]!;
    for (let k = i - 1; k >= 0; k--) {
      const c = bySize[k]!;
      if (c.area > s.area && within(s.box, c.box) && inside(probe, c.outline)) {
        parent.set(s.id, c.id);
        break;
      }
    }
  });
  const depth = (id: string): number => {
    let d = 0;
    for (let p = parent.get(id); p !== undefined; p = parent.get(p)) d++;
    return d;
  };
  const order = new Map(objects.map((o, i) => [o.id, i]));
  const members = new Map<string, string[]>();
  for (const s of shapes) {
    const p = parent.get(s.id);
    if (p !== undefined && depth(s.id) % 2 === 1) members.set(p, [...(members.get(p) ?? []), s.id]);
  }
  return [...members.entries()]
    .map(([root, kids]) => [root, ...kids].sort((a, b) => order.get(a)! - order.get(b)!))
    .sort((a, b) => order.get(a[0]!)! - order.get(b[0]!)!);
}

/**
 * Agrupa por contención. Devuelve los objetos con su `group` asignado (los demás, intactos) y
 * cuántos grupos se formaron; la misma lista si no hay nada que agrupar. `taken` son los grupos
 * que ya existen en otro lugar del documento.
 */
export function autoGroupObjects(objects: DesignObject[], taken: ReadonlySet<string> = new Set()): { objects: DesignObject[]; groups: number } {
  const groups = containmentGroups(objects);
  if (groups.length === 0) return { objects, groups: 0 };
  const used = new Set([...taken, ...groupIdsOf(objects)]);
  const assign = new Map<string, string>();
  for (const members of groups) {
    let i = 1;
    while (used.has(`grupo-${i}`)) i++;
    const id = `grupo-${i}`;
    used.add(id);
    for (const m of members) assign.set(m, id);
  }
  return { objects: objects.map((o) => (assign.has(o.id) ? { ...o, group: assign.get(o.id)! } : o)), groups: groups.length };
}
