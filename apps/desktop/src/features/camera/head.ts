/**
 * Cámara montada en el cabezal (M36): matemática pura, sin tocar la máquina ni la cámara.
 *
 * Modelo: el centro óptico de la imagen corregida ve un punto de la cama desplazado del láser por
 * `offset_mm`, y un píxel `p` ve el punto `L + offset + A (p - ref)`, donde `L` es la posición del
 * láser (mm de documento, Y hacia arriba) y `A` la matriz píxel -> mm (incluye la inversión del
 * eje Y de la imagen). Se supone una cámara que mira recto a la cama (con lente ya corregida),
 * así que basta una transformación afín.
 */
import type { HeadCalibration } from '@/api/types';
import { solveLinear, type Mat3, type Quad, type Vec2 } from './geometry';

/**
 * Una medición: la marca vista en el píxel `px` con el láser en `laser`. Con `mark` la observación
 * trae su propia marca (calibración automática con varias marcas en una foto); sin él vale la
 * marca única que se pasa al ajuste.
 */
export interface HeadObservation {
  laser: Vec2;
  px: Vec2;
  /** Posición conocida de esta marca (mm de documento). */
  mark?: Vec2;
}

/** Punto de la cama (mm) que ve el píxel `p` con el láser en `laser`. */
export function headPointMm(cal: HeadCalibration, laser: Vec2, p: Vec2): Vec2 {
  const dx = p[0] - cal.ref_px[0];
  const dy = p[1] - cal.ref_px[1];
  const [[a, b], [c, d]] = cal.px_to_mm;
  return [laser[0] + cal.offset_mm[0] + a * dx + b * dy, laser[1] + cal.offset_mm[1] + c * dx + d * dy];
}

/** Homografía (afín) píxel -> mm de una foto tomada con el láser en `laser`. */
export function headTileToBed(cal: HeadCalibration, laser: Vec2): Mat3 {
  const [[a, b], [c, d]] = cal.px_to_mm;
  const ox = laser[0] + cal.offset_mm[0] - a * cal.ref_px[0] - b * cal.ref_px[1];
  const oy = laser[1] + cal.offset_mm[1] - c * cal.ref_px[0] - d * cal.ref_px[1];
  return [[a, b, ox], [c, d, oy], [0, 0, 1]];
}

/** Esquinas de la imagen (arriba-izq., arriba-der., abajo-der., abajo-izq.) en la cama. */
export function headCorners(cal: HeadCalibration, laser: Vec2): Quad {
  const [w, h] = cal.image_size;
  const at = (p: Vec2): Vec2 => headPointMm(cal, laser, p);
  return [at([0, 0]), at([w, 0]), at([w, h]), at([0, h])];
}

/** Lo que cubre una foto: caja en mm y su centro respecto del punto que ve el píxel de referencia. */
export interface Footprint {
  w: number;
  h: number;
  /** Centro de la caja respecto de `L + offset` (mm). */
  cx: number;
  cy: number;
}

export function headFootprint(cal: HeadCalibration): Footprint {
  const corners = headCorners(cal, [-cal.offset_mm[0], -cal.offset_mm[1]]);
  const xs = corners.map((p) => p[0]);
  const ys = corners.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  return { w: x1 - x0, h: y1 - y0, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2 };
}

/** Escala de la cámara del cabezal (píxeles por mm), media geométrica de los dos ejes. */
export function headPxPerMm(cal: HeadCalibration): number {
  const [[a, b], [c, d]] = cal.px_to_mm;
  const det = Math.abs(a * d - b * c);
  return det > 0 ? 1 / Math.sqrt(det) : 0;
}

export type HeadFitError = 'tooFew' | 'collinear' | 'invalid';

export interface HeadFit {
  calibration: HeadCalibration;
}

/**
 * Calibración a partir de las observaciones de UNA marca de posición conocida `mark` (mm): con
 * el láser en `L_i` la marca se ve en `p_i`, y `mark = L_i + offset + A (p_i - ref)`. Son seis
 * incógnitas (`A` y `offset`) y dos ecuaciones por observación; hacen falta al menos tres con la
 * marca en píxeles distintos (el cabezal se mueve entre una y otra). Mínimos cuadrados.
 */
export function fitHeadCalibration(
  obs: readonly HeadObservation[],
  mark: Vec2 | null,
  imageSize: Vec2,
  refPx: Vec2 = [imageSize[0] / 2, imageSize[1] / 2],
): HeadFit | { error: HeadFitError } {
  if (obs.length < 3) return { error: 'tooFew' };
  const markOf = (o: HeadObservation): Vec2 | null => o.mark ?? mark;
  if (obs.some((o) => [...o.laser, ...o.px, ...(markOf(o) ?? [NaN])].some((v) => !Number.isFinite(v)))) {
    return { error: 'invalid' };
  }
  // Ecuaciones normales de [dx dy 1]·[a b off]ᵀ = mark - L, una vez por eje (misma matriz).
  const rows = obs.map((o) => [o.px[0] - refPx[0], o.px[1] - refPx[1], 1]);
  const ata = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
  for (const r of rows) for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) ata[i]![j]! += r[i]! * r[j]!;
  const solve = (axis: 0 | 1): number[] | null => {
    const atb = [0, 0, 0];
    rows.forEach((r, k) => {
      const y = markOf(obs[k]!)![axis] - obs[k]!.laser[axis];
      for (let i = 0; i < 3; i++) atb[i]! += r[i]! * y;
    });
    return solveLinear(ata.map((r) => [...r]), atb);
  };
  const sx = solve(0);
  const sy = solve(1);
  if (!sx || !sy) return { error: 'collinear' };
  const px_to_mm: [[number, number], [number, number]] = [[sx[0]!, sx[1]!], [sy[0]!, sy[1]!]];
  const offset_mm: [number, number] = [sx[2]!, sy[2]!];
  const det = px_to_mm[0][0] * px_to_mm[1][1] - px_to_mm[0][1] * px_to_mm[1][0];
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return { error: 'collinear' };
  let sq = 0;
  rows.forEach((r, k) => {
    const m = markOf(obs[k]!)!;
    const ex = r[0]! * sx[0]! + r[1]! * sx[1]! + sx[2]! - (m[0] - obs[k]!.laser[0]);
    const ey = r[0]! * sy[0]! + r[1]! * sy[1]! + sy[2]! - (m[1] - obs[k]!.laser[1]);
    sq += ex * ex + ey * ey;
  });
  return {
    calibration: {
      image_size: [imageSize[0], imageSize[1]],
      ref_px: [refPx[0], refPx[1]],
      px_to_mm,
      offset_mm,
      rms_mm: Math.sqrt(sq / obs.length),
      observations: obs.length,
    },
  };
}

// ---------------------------------------------------------------------------
// Recorrido en cuadrícula
// ---------------------------------------------------------------------------

export interface ScanStop {
  /** Dónde poner el láser (mm de documento). */
  laser: Vec2;
  /** Centro del terreno que ve la foto (mm). */
  center: Vec2;
  /** El láser no puede ir al punto ideal (fuera de la cama): quedó recortado. */
  clamped: boolean;
}

export interface ScanPlan {
  stops: ScanStop[];
  cols: number;
  rows: number;
  /** Zona que debería cubrir el mosaico (mm). */
  region: [number, number, number, number];
  /** Tamaño útil de cada foto (mm), sin el margen descartado. */
  tile: Vec2;
  /** Hay zonas de la región que ninguna posición alcanza a ver. */
  partial: boolean;
}

/** Fracción de cada lado de la foto que se descarta (borde borroso y deformado). */
export const SCAN_CROP = 0.06;
/** Traslape mínimo entre fotos vecinas, sobre la parte útil. */
export const SCAN_OVERLAP = 0.2;
/** Máximo de posiciones de un recorrido. */
export const SCAN_MAX_STOPS = 150;

function axisCenters(lo: number, hi: number, tile: number, step: number): number[] {
  const span = hi - lo;
  if (span <= tile) return [(lo + hi) / 2];
  const n = Math.ceil((span - tile) / step) + 1;
  const first = lo + tile / 2;
  const last = hi - tile / 2;
  return Array.from({ length: n }, (_, i) => (n === 1 ? (first + last) / 2 : first + ((last - first) * i) / (n - 1)));
}

/**
 * Posiciones de captura para cubrir `region` (por omisión, toda la cama) con la cámara del cabezal.
 * El orden es de serpiente (fila de ida, fila de vuelta) para recorrer lo menos posible. Devuelve
 * un texto en `error` si el recorrido es imposible (cámara sin calibrar, demasiadas fotos).
 */
export function planScan(
  bed: Vec2,
  cal: HeadCalibration,
  region: [number, number, number, number] = [0, 0, bed[0], bed[1]],
): ScanPlan | { error: 'tooMany' | 'empty' | 'scale' } {
  const fp = headFootprint(cal);
  if (!(fp.w > 0 && fp.h > 0) || !Number.isFinite(fp.w + fp.h)) return { error: 'scale' };
  const [x0, y0, x1, y1] = [Math.max(0, region[0]), Math.max(0, region[1]), Math.min(bed[0], region[2]), Math.min(bed[1], region[3])];
  if (!(x1 - x0 > 1e-6 && y1 - y0 > 1e-6)) return { error: 'empty' };
  const usable: Vec2 = [fp.w * (1 - 2 * SCAN_CROP), fp.h * (1 - 2 * SCAN_CROP)];
  const xs = axisCenters(x0, x1, usable[0], usable[0] * (1 - SCAN_OVERLAP));
  const ys = axisCenters(y0, y1, usable[1], usable[1] * (1 - SCAN_OVERLAP));
  if (xs.length * ys.length > SCAN_MAX_STOPS) return { error: 'tooMany' };
  const stops: ScanStop[] = [];
  ys.forEach((cy, j) => {
    const row = j % 2 === 0 ? xs : [...xs].reverse();
    for (const cx of row) {
      // El láser debe estar en la cama; si no llega, se recorta y esa zona queda a medias.
      const ideal: Vec2 = [cx - fp.cx - cal.offset_mm[0], cy - fp.cy - cal.offset_mm[1]];
      const laser: Vec2 = [Math.min(bed[0], Math.max(0, ideal[0])), Math.min(bed[1], Math.max(0, ideal[1]))];
      stops.push({ laser, center: [cx, cy], clamped: laser[0] !== ideal[0] || laser[1] !== ideal[1] });
    }
  });
  const covered = coveredRegion(stops, fp, cal.offset_mm, usable);
  const partial = covered[0] > x0 + 0.5 || covered[1] > y0 + 0.5 || covered[2] < x1 - 0.5 || covered[3] < y1 - 0.5;
  return { stops, cols: xs.length, rows: ys.length, region: [x0, y0, x1, y1], tile: usable, partial };
}

/** Caja que cubren de verdad las posiciones (con el láser recortado a la cama). */
function coveredRegion(stops: readonly ScanStop[], fp: Footprint, offset: Vec2, usable: Vec2): [number, number, number, number] {
  let box: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const s of stops) {
    const cx = s.laser[0] + offset[0] + fp.cx;
    const cy = s.laser[1] + offset[1] + fp.cy;
    box = [Math.min(box[0], cx - usable[0] / 2), Math.min(box[1], cy - usable[1] / 2), Math.max(box[2], cx + usable[0] / 2), Math.max(box[3], cy + usable[1] / 2)];
  }
  return box;
}

/** Tiempo estimado (s) del recorrido a `speed` mm/min más el reposo de cada foto. */
export function estimateScanSeconds(plan: ScanPlan, speedMmMin: number, settleMs: number, from: Vec2 | null): number {
  let dist = 0;
  let prev: Vec2 | null = from;
  for (const s of plan.stops) {
    if (prev) dist += Math.hypot(s.laser[0] - prev[0], s.laser[1] - prev[1]);
    prev = s.laser;
  }
  return (dist / Math.max(1, speedMmMin)) * 60 + (plan.stops.length * settleMs) / 1000;
}
