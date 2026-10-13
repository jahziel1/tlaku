import type { DesignObject, Point } from '@/api/types';
import { objectBounds, objectCurves, objectPolylines, polylineLength } from '@/lib/geometry';
import { circleDiameter } from '@/features/arrange/special';

/** Medidas entre dos puntos (mm y grados; Y hacia arriba, ángulo antihorario desde +X). */
export interface Distance {
  distance: number;
  /** Ángulo de A hacia B en grados, en (-180, 180]. */
  angle: number;
  dx: number;
  dy: number;
}

export function measureBetween(a: Point, b: Point): Distance {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const angle = dx === 0 && dy === 0 ? 0 : (Math.atan2(dy, dx) * 180) / Math.PI;
  return { distance: Math.hypot(dx, dy), angle: angle === -180 ? 180 : angle, dx, dy };
}

/** Área con signo de un contorno (fórmula del cordón). */
function ringArea(points: readonly Point[]): number {
  let sum = 0;
  for (let i = 0; i < points.length; i++) {
    const [x1, y1] = points[i]!;
    const [x2, y2] = points[(i + 1) % points.length]!;
    sum += x1 * y2 - x2 * y1;
  }
  return Math.abs(sum) / 2;
}

function inside(point: Point, ring: readonly Point[]): boolean {
  let hit = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i]!;
    const [xj, yj] = ring[j]!;
    if (yi > point[1] !== yj > point[1] && point[0] < ((xj - xi) * (point[1] - yi)) / (yj - yi) + xi) hit = !hit;
  }
  return hit;
}

/** Datos de una forma, en mm y mm². */
export interface ShapeInfo {
  /** Nodos (extremos de segmentos), sin contar el que cierra un contorno. */
  nodes: number;
  /** Trazos: segmentos rectos y curvas (cada tramo entre dos nodos). */
  lines: number;
  curves: number;
  perimeter: number;
  /** Área de los contornos cerrados con los huecos descontados; 0 si no hay ninguno cerrado. */
  area: number;
  /** Todos los contornos son cerrados. */
  closed: boolean;
  /** Algún contorno es abierto. */
  open: boolean;
  size: [number, number];
  /** Diámetro y centro si la forma es un círculo. */
  circle: { diameter: number; center: Point } | null;
}

export function measureShape(obj: DesignObject): ShapeInfo | null {
  const b = objectBounds(obj);
  if (!b) return null;
  let nodes = 0;
  let lines = 0;
  let curves = 0;
  for (const path of objectCurves(obj)) {
    let start: Point | null = null;
    let last: Point | null = null;
    let count = 0;
    // Un contorno cuyo último punto vuelve al primero cuenta ese nodo una sola vez.
    const endSub = () => {
      if (start && last && count > 1 && Math.hypot(start[0] - last[0], start[1] - last[1]) < 1e-9) count--;
      nodes += count;
      count = 0;
    };
    for (const s of path) {
      if (s.type === 'move_to') {
        endSub();
        count = 1;
        start = last = s.to;
      } else if (s.type === 'close') {
        // El cierre es una recta hasta el inicio, salvo que ya estemos encima.
        if (start && last && Math.hypot(start[0] - last[0], start[1] - last[1]) > 1e-9) lines++;
      } else {
        if (s.type === 'line_to') lines++;
        else curves++;
        count++;
        last = s.to;
      }
    }
    endSub();
  }
  const polys = objectPolylines(obj);
  const rings = polys.filter((p) => p.closed).map((p) => p.points);
  // Par-impar: un contorno dentro de otro resta (agujero), dentro de ese suma, y así.
  let area = 0;
  rings.forEach((ring, i) => {
    const depth = rings.reduce((d, other, j) => (i !== j && inside(ring[0]!, other) ? d + 1 : d), 0);
    area += (depth % 2 === 0 ? 1 : -1) * ringArea(ring);
  });
  const diameter = circleDiameter(obj);
  return {
    nodes,
    lines,
    curves,
    perimeter: polys.reduce((sum, p) => sum + polylineLength(p.points, p.closed), 0),
    area: Math.max(0, area),
    closed: polys.length > 0 && polys.every((p) => p.closed),
    open: polys.some((p) => !p.closed),
    size: [b[1][0] - b[0][0], b[1][1] - b[0][1]],
    circle: diameter ? { diameter, center: [(b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2] } : null,
  };
}
