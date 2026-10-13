/**
 * Buscador de centro (M48): el centro de un objeto redondo a partir de puntos de su borde vistos
 * por la cámara. Tres puntos dan el círculo exacto (circuncírculo); con más puntos (los que halla
 * el motor al detectar el círculo) se ajusta por mínimos cuadrados. Todo en mm de la cama: la
 * perspectiva vuelve elipse al círculo de la imagen, así que se ajusta después de pasar a mm.
 */
import { solveLinear, type Vec2 } from './geometry';

export interface Circle {
  center: Vec2;
  radius: number;
  /** Distancia rms de los puntos al círculo (mm). */
  rms: number;
}

/** Círculo por tres puntos. `null` si están casi alineados. */
export function circleFromThree(a: Vec2, b: Vec2, c: Vec2): Circle | null {
  const d = 2 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
  const span = Math.max(Math.hypot(a[0] - b[0], a[1] - b[1]), Math.hypot(b[0] - c[0], b[1] - c[1]), Math.hypot(a[0] - c[0], a[1] - c[1]));
  if (!Number.isFinite(d) || Math.abs(d) <= 1e-9 * span * span) return null;
  const sq = (p: Vec2) => p[0] * p[0] + p[1] * p[1];
  const ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
  const uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
  const center: Vec2 = [ux, uy];
  return { center, radius: Math.hypot(a[0] - ux, a[1] - uy), rms: 0 };
}

/** Ajuste algebraico (Kåsa) de un círculo a 3 o más puntos. `null` si no hay solución. */
export function fitCircle(points: readonly Vec2[]): Circle | null {
  if (points.length < 3) return null;
  if (points.length === 3) return circleFromThree(points[0]!, points[1]!, points[2]!);
  const n = points.length;
  const mx = points.reduce((s, p) => s + p[0], 0) / n;
  const my = points.reduce((s, p) => s + p[1], 0) / n;
  let suu = 0, suv = 0, svv = 0, suuu = 0, svvv = 0, suvv = 0, svuu = 0;
  for (const p of points) {
    const u = p[0] - mx;
    const v = p[1] - my;
    suu += u * u;
    suv += u * v;
    svv += v * v;
    suuu += u * u * u;
    svvv += v * v * v;
    suvv += u * v * v;
    svuu += v * u * u;
  }
  const sol = solveLinear([[suu, suv], [suv, svv]], [0.5 * (suuu + suvv), 0.5 * (svvv + svuu)]);
  if (!sol) return null;
  const center: Vec2 = [sol[0]! + mx, sol[1]! + my];
  const radius = Math.sqrt(sol[0]! ** 2 + sol[1]! ** 2 + (suu + svv) / n);
  if (!Number.isFinite(radius) || radius <= 0) return null;
  const rms = Math.sqrt(points.reduce((s, p) => s + (Math.hypot(p[0] - center[0], p[1] - center[1]) - radius) ** 2, 0) / n);
  return { center, radius, rms };
}

/** Radio máximo que se acepta (mm): descarta puntos casi alineados que dan círculos enormes. */
export const MAX_RADIUS_MM = 2000;

export type CenterProblem = 'collinear' | 'tooFew';

/** Círculo de los puntos (mm) o el motivo por el que no se pudo. */
export function centerFromPoints(points: readonly Vec2[]): { circle: Circle } | { error: CenterProblem } {
  if (points.length < 3) return { error: 'tooFew' };
  const circle = fitCircle(points);
  if (!circle || circle.radius > MAX_RADIUS_MM) return { error: 'collinear' };
  return { circle };
}
