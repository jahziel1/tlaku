/** Geometría plana para la alineación: homografía de 4 puntos y esquinas del fondo. */

export type Vec2 = [number, number];
export type Quad = [Vec2, Vec2, Vec2, Vec2];
/** Matriz 3x3 por filas: `[x', y', w'] = M [x, y, 1]`. */
export type Mat3 = [[number, number, number], [number, number, number], [number, number, number]];

export function applyH(h: Mat3, p: Vec2): Vec2 {
  const w = h[2][0] * p[0] + h[2][1] * p[1] + h[2][2];
  return [(h[0][0] * p[0] + h[0][1] * p[1] + h[0][2]) / w, (h[1][0] * p[0] + h[1][1] * p[1] + h[1][2]) / w];
}

function mul(a: Mat3, b: Mat3): Mat3 {
  const m = [[0, 0, 0], [0, 0, 0], [0, 0, 0]] as Mat3;
  for (let i = 0; i < 3; i++)
    for (let j = 0; j < 3; j++) m[i]![j] = a[i]![0]! * b[0]![j]! + a[i]![1]! * b[1]![j]! + a[i]![2]! * b[2]![j]!;
  return m;
}

export function invertH(m: Mat3): Mat3 | null {
  const [[a, b, c], [d, e, f], [g, h, i]] = m;
  const A = e * i - f * h;
  const B = -(d * i - f * g);
  const C = d * h - e * g;
  const det = a * A + b * B + c * C;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-300) return null;
  return [
    [A / det, -(b * i - c * h) / det, (b * f - c * e) / det],
    [B / det, (a * i - c * g) / det, -(a * f - c * d) / det],
    [C / det, -(a * h - b * g) / det, (a * e - b * d) / det],
  ];
}

/** Normalización de Hartley: centro en el origen y distancia media √2. */
function normalizer(pts: Vec2[]): Mat3 {
  const n = pts.length;
  const cx = pts.reduce((s, p) => s + p[0], 0) / n;
  const cy = pts.reduce((s, p) => s + p[1], 0) / n;
  const mean = pts.reduce((s, p) => s + Math.hypot(p[0] - cx, p[1] - cy), 0) / n;
  const s = mean > 1e-12 ? Math.SQRT2 / mean : 1;
  return [[s, 0, -s * cx], [0, s, -s * cy], [0, 0, 1]];
}

/** Resuelve A·x = b (n x n) por eliminación gaussiana con pivote parcial; `null` si es singular. */
export function solveLinear(a: number[][], b: number[]): number[] | null {
  const n = b.length;
  const m = a.map((row, i) => [...row, b[i]!]);
  for (let col = 0; col < n; col++) {
    let piv = col;
    for (let r = col + 1; r < n; r++) if (Math.abs(m[r]![col]!) > Math.abs(m[piv]![col]!)) piv = r;
    if (Math.abs(m[piv]![col]!) < 1e-10) return null;
    [m[col], m[piv]] = [m[piv]!, m[col]!];
    for (let r = col + 1; r < n; r++) {
      const k = m[r]![col]! / m[col]![col]!;
      for (let c = col; c <= n; c++) m[r]![c]! -= k * m[col]![c]!;
    }
  }
  const x = new Array<number>(n).fill(0);
  for (let r = n - 1; r >= 0; r--) {
    let s = m[r]![n]!;
    for (let c = r + 1; c < n; c++) s -= m[r]![c]! * x[c]!;
    x[r] = s / m[r]![r]!;
  }
  return x;
}

/** ¿Hay tres puntos casi alineados? Con ellos la homografía de 4 puntos no está definida. */
function hasCollinearTriple(q: Quad): boolean {
  const span = Math.max(...q.flatMap((p) => p), 0) - Math.min(...q.flatMap((p) => p), 0);
  const tol = 1e-9 * span * span;
  for (let i = 0; i < 4; i++)
    for (let j = i + 1; j < 4; j++)
      for (let k = j + 1; k < 4; k++) {
        const [a, b, c] = [q[i]!, q[j]!, q[k]!];
        if (Math.abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) <= tol) return true;
      }
  return false;
}

/**
 * Homografía exacta que lleva los 4 puntos `src` a `dst` (DLT con h33 = 1 sobre puntos
 * normalizados). `null` si los puntos son degenerados (tres alineados).
 */
export function homographyFrom4(src: Quad, dst: Quad): Mat3 | null {
  if (hasCollinearTriple(src) || hasCollinearTriple(dst)) return null;
  const ts = normalizer(src);
  const td = normalizer(dst);
  const a: number[][] = [];
  const b: number[] = [];
  for (let k = 0; k < 4; k++) {
    const [x, y] = applyH(ts, src[k]!);
    const [u, v] = applyH(td, dst[k]!);
    a.push([x, y, 1, 0, 0, 0, -u * x, -u * y]);
    b.push(u);
    a.push([0, 0, 0, x, y, 1, -v * x, -v * y]);
    b.push(v);
  }
  const h = solveLinear(a, b);
  if (!h || h.some((v) => !Number.isFinite(v))) return null;
  const hn: Mat3 = [[h[0]!, h[1]!, h[2]!], [h[3]!, h[4]!, h[5]!], [h[6]!, h[7]!, 1]];
  const tdInv = invertH(td);
  if (!tdInv) return null;
  const full = mul(mul(tdInv, hn), ts);
  const s = full[2][2];
  if (!Number.isFinite(s) || Math.abs(s) < 1e-300) return full.every((r) => r.every(Number.isFinite)) ? full : null;
  return full.map((r) => r.map((v) => v / s)) as Mat3;
}

/** Área con signo (positiva en sentido antihorario con Y hacia arriba). */
export function signedArea(q: readonly Vec2[]): number {
  let s = 0;
  for (let i = 0; i < q.length; i++) {
    const [x1, y1] = q[i]!;
    const [x2, y2] = q[(i + 1) % q.length]!;
    s += x1 * y2 - x2 * y1;
  }
  return s / 2;
}

/** Cuadrilátero convexo, sin lados cruzados y con área apreciable (`minArea` en sus unidades²). */
export function isConvexQuad(q: Quad, minArea = 1e-6): boolean {
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = q[i]!;
    const b = q[(i + 1) % 4]!;
    const c = q[(i + 2) % 4]!;
    const cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
    if (!Number.isFinite(cross) || cross === 0) return false;
    if (sign === 0) sign = Math.sign(cross);
    else if (Math.sign(cross) !== sign) return false;
  }
  return Math.abs(signedArea(q)) >= minArea;
}

/** Punto dentro de un polígono (par-impar). */
export function pointInPolygon(p: Vec2, poly: readonly Vec2[]): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]!;
    const [xj, yj] = poly[j]!;
    if (yi > p[1] !== yj > p[1] && p[0] < ((xj - xi) * (p[1] - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/**
 * Esquinas de la imagen corregida (arriba-izq., arriba-der., abajo-der., abajo-izq.) en mm de la
 * cama. Las esquinas de la imagen son bordes de píxel: (0,0) a (w,h).
 */
export function backdropCorners(toBed: Mat3, size: readonly [number, number]): Quad {
  const [w, h] = size;
  return [applyH(toBed, [0, 0]), applyH(toBed, [w, 0]), applyH(toBed, [w, h]), applyH(toBed, [0, h])];
}

/** Las 4 esquinas de un rectángulo de la cama en el orden de la alineación (ver `ALIGN_ORDER`). */
export function rectCorners(x0: number, y0: number, w: number, h: number): Quad {
  return [[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]];
}

/** Esquinas en el orden de la alineación (vistas desde el frente de la máquina). */
export const ALIGN_ORDER = ['bl', 'br', 'tr', 'tl'] as const;
