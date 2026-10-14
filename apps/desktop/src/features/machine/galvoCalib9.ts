import type { DesignObject, Document, LayerSettings } from '@/api/types';
import type { FieldCorrection } from '@/bindings/FieldCorrection';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { CALIB_COLOR, type CalibMark } from './galvoCalib';
import { MAX_NONLINEAR, MAX_SCALE, MIN_SCALE } from './galvoProfile';

/**
 * Asistente de nueve puntos (M43): se marca una cuadrícula de 3 x 3 cruces, se mide dónde cayó cada una con una
 * regla (en mm, desde la cruz del centro) y se ajusta la corrección paramétrica del perfil (escala, giro, barril y
 * trapecio por eje). El desplazamiento y el sesgo no se tocan.
 *
 * Modelo (propio, sin verificar con una lente real). La cuadrícula se marca con la corrección ACTUAL `F0`, así que lo
 * medido es lo que le falta. Etapa A: se ajusta a las medidas una distorsión `G` de la misma familia (sin
 * desplazamiento), o sea `G(N_i) = M_i` con `N_i` el punto nominal y `M_i` el medido. Etapa B: la corrección buena
 * es `F' = F0 ∘ G⁻¹` (se aplica primero la inversa de lo que la lente hace de más); como esa composición no está en
 * la familia, se ajusta `F'` a la composición en una rejilla de 5 x 5 puntos. Ambas etapas son mínimos cuadrados
 * (Levenberg-Marquardt). `forward` es una copia de `FieldMap::forward` de `tlaku-galvo` (`calib.rs`): la prueba
 * `galvoCalib9.test.ts` fija los mismos números que `the_ts_mirror_numbers_are_pinned` del motor.
 */

/** Lado del cuadrado de las cruces, como fracción del campo (las cruces van a ±0,35 del campo desde el centro). */
export const GRID9_FRACTION = 0.35;
/** Medio largo de los brazos de cada cruz, mm. */
export const CROSS_ARM_MM = 2;
/** Marca del lado +X: un trazo a la derecha de la cruz del medio, de 0,43 a 0,47 del campo desde el centro. */
const XMARK_FROM = 0.43;
const XMARK_TO = 0.47;

export type Pt = [number, number];

/** Los 9 puntos nominales (mm desde el centro) en el orden de lectura: arriba a la izquierda, fila a fila hacia abajo. */
export function grid9Points(fieldMm: number): Pt[] {
  const a = fieldMm * GRID9_FRACTION;
  const out: Pt[] = [];
  for (const y of [a, 0, -a]) for (const x of [-a, 0, a]) out.push([x, y]);
  return out;
}

const vec = (id: string, paths: ReturnType<typeof polylineBez>[]): DesignObject => ({
  id,
  layer: CALIB_COLOR,
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths },
  locked: false,
  hidden: false,
  group: null,
});

/** Documento con las 9 cruces y la marca del lado +X, centrado en la cama (el campo está centrado en la cama). */
export function buildGrid9Document(bed: [number, number], fieldMm: number, mark: CalibMark): Document {
  const cx = bed[0] / 2;
  const cy = bed[1] / 2;
  const layer: LayerSettings = makeLayer(CALIB_COLOR, 'calibración 9', { mode: 'line', speed: mark.speedMmS * 60, power: mark.power, passes: 1 });
  layer.galvo = { ...layer.galvo, frequency_khz: mark.frequencyKhz, pulse_width_ns: mark.pulseWidthNs };
  const arm = CROSS_ARM_MM;
  const objects: DesignObject[] = grid9Points(fieldMm).map(([x, y], i) =>
    vec(`calib9-${i + 1}`, [
      polylineBez([[cx + x - arm, cy + y], [cx + x + arm, cy + y]], false),
      polylineBez([[cx + x, cy + y - arm], [cx + x, cy + y + arm]], false),
    ]),
  );
  objects.push(vec('calib9-x', [polylineBez([[cx + fieldMm * XMARK_FROM, cy], [cx + fieldMm * XMARK_TO, cy]], false)]));
  return { version: 2, objects, layers: [layer], device_id: null };
}

// ---------------------------------------------------------------------------
// Modelo del campo (copia de `FieldMap` de tlaku-galvo)
// ---------------------------------------------------------------------------

type Family = Pick<FieldCorrection, 'scale' | 'offset' | 'rotation_deg' | 'bulge' | 'trapezoid' | 'skew'>;

/** Pasos 1 a 5 de `FieldMap::correct`: del diseño (mm desde el centro) al diseño corregido, antes de los ejes. */
export function correct(fieldMm: number, c: Family, p: Pt): Pt {
  const h = fieldMm / 2;
  const rad = (c.rotation_deg * Math.PI) / 180;
  const [sin, cos] = [Math.sin(rad), Math.cos(rad)];
  let [x, y] = [p[0] * c.scale[0], p[1] * c.scale[1]];
  [x, y] = [x * cos - y * sin, x * sin + y * cos];
  const r2 = (x / h) ** 2 + (y / h) ** 2;
  [x, y] = [x * (1 + c.bulge[0] * r2), y * (1 + c.bulge[1] * r2)];
  [x, y] = [x * (1 + c.trapezoid[0] * (y / h)), y * (1 + c.trapezoid[1] * (x / h))];
  [x, y] = [x + c.skew[0] * (y / h) * h, y + c.skew[1] * (x / h) * h];
  return [x + c.offset[0], y + c.offset[1]];
}

/** Paso 6: intercambio y negación de ejes. */
export function toAxes(c: Pick<FieldCorrection, 'invert' | 'swap_xy'>, p: Pt): Pt {
  let [a, b] = c.swap_xy ? [p[1], p[0]] : [p[0], p[1]];
  if (c.invert[0]) a = -a;
  if (c.invert[1]) b = -b;
  return [a, b];
}

/** Del diseño a mm de los ejes del galvo (sin `.cor`). */
export function forward(fieldMm: number, c: FieldCorrection, p: Pt): Pt {
  return toAxes(c, correct(fieldMm, c, p));
}

const IDENTITY: Family = { scale: [1, 1], offset: [0, 0], rotation_deg: 0, bulge: [0, 0], trapezoid: [0, 0], skew: [0, 0] };

/** Inversa de `correct` por Newton (Jacobiano por diferencias centrales); `null` si no converge. */
function invert(fieldMm: number, c: Family, target: Pt): Pt | null {
  let x: Pt = [target[0] - c.offset[0], target[1] - c.offset[1]];
  const e = 1e-6 * Math.max(fieldMm, 1);
  for (let i = 0; i < 60; i++) {
    const f = correct(fieldMm, c, x);
    const err: Pt = [f[0] - target[0], f[1] - target[1]];
    if (Math.max(Math.abs(err[0]), Math.abs(err[1])) < 1e-12) return x;
    const xp = correct(fieldMm, c, [x[0] + e, x[1]]);
    const xm = correct(fieldMm, c, [x[0] - e, x[1]]);
    const yp = correct(fieldMm, c, [x[0], x[1] + e]);
    const ym = correct(fieldMm, c, [x[0], x[1] - e]);
    const j11 = (xp[0] - xm[0]) / (2 * e);
    const j21 = (xp[1] - xm[1]) / (2 * e);
    const j12 = (yp[0] - ym[0]) / (2 * e);
    const j22 = (yp[1] - ym[1]) / (2 * e);
    const det = j11 * j22 - j12 * j21;
    if (Math.abs(det) < 1e-12) return null;
    x = [x[0] - (j22 * err[0] - j12 * err[1]) / det, x[1] - (-j21 * err[0] + j11 * err[1]) / det];
  }
  const f = correct(fieldMm, c, x);
  return Math.max(Math.abs(f[0] - target[0]), Math.abs(f[1] - target[1])) < 1e-9 ? x : null;
}

// ---------------------------------------------------------------------------
// Mínimos cuadrados (Levenberg-Marquardt) sobre 7 parámetros
// ---------------------------------------------------------------------------

/** Parámetros ajustables: escala X, Y, giro (grados), barril X, Y, trapecio X, Y. */
type Params = [number, number, number, number, number, number, number];
const LOWER: Params = [MIN_SCALE, MIN_SCALE, -45, -MAX_NONLINEAR, -MAX_NONLINEAR, -MAX_NONLINEAR, -MAX_NONLINEAR];
const UPPER: Params = [MAX_SCALE, MAX_SCALE, 45, MAX_NONLINEAR, MAX_NONLINEAR, MAX_NONLINEAR, MAX_NONLINEAR];

const withParams = (base: Family, p: Params): Family => ({
  ...base,
  scale: [p[0], p[1]],
  rotation_deg: p[2],
  bulge: [p[3], p[4]],
  trapezoid: [p[5], p[6]],
});

const paramsOf = (c: Family): Params => [c.scale[0], c.scale[1], c.rotation_deg, c.bulge[0], c.bulge[1], c.trapezoid[0], c.trapezoid[1]];

/** Resuelve `A x = b` (matriz cuadrada pequeña) por eliminación gaussiana con pivote; `null` si es singular. */
function solve(a: number[][], b: number[]): number[] | null {
  const n = b.length;
  const m = a.map((row, i) => [...row, b[i]!]);
  for (let col = 0; col < n; col++) {
    let piv = col;
    for (let r = col + 1; r < n; r++) if (Math.abs(m[r]![col]!) > Math.abs(m[piv]![col]!)) piv = r;
    if (Math.abs(m[piv]![col]!) < 1e-14) return null;
    [m[col], m[piv]] = [m[piv]!, m[col]!];
    for (let r = col + 1; r < n; r++) {
      const f = m[r]![col]! / m[col]![col]!;
      for (let k = col; k <= n; k++) m[r]![k]! -= f * m[col]![k]!;
    }
  }
  const x = new Array<number>(n).fill(0);
  for (let r = n - 1; r >= 0; r--) {
    let s = m[r]![n]!;
    for (let k = r + 1; k < n; k++) s -= m[r]![k]! * x[k]!;
    x[r] = s / m[r]![r]!;
  }
  return x;
}

/**
 * Ajusta los parámetros libres (`free[i]`) para que `residual(params)` (una lista de números) quede lo más cerca de
 * cero. Los demás parámetros se quedan en `start`. Devuelve los parámetros y el residuo cuadrático final.
 */
function levenberg(residual: (p: Params) => number[], start: Params, free: boolean[]): { p: Params; cost: number } {
  const idx = free.flatMap((f, i) => (f ? [i] : []));
  let p = [...start] as Params;
  const costOf = (r: number[]) => r.reduce((s, v) => s + v * v, 0);
  let r = residual(p);
  let cost = costOf(r);
  let lambda = 1e-3;
  for (let iter = 0; iter < 200 && cost > 1e-24; iter++) {
    const jac = idx.map((k) => {
      const h = 1e-6 * Math.max(1, Math.abs(p[k]!));
      const hi = [...p] as Params;
      const lo = [...p] as Params;
      hi[k] += h;
      lo[k] -= h;
      const [rh, rl] = [residual(hi), residual(lo)];
      return rh.map((v, i) => (v - rl[i]!) / (2 * h));
    });
    const n = idx.length;
    const jtj = Array.from({ length: n }, (_, i) => Array.from({ length: n }, (_, j) => jac[i]!.reduce((s, v, k) => s + v * jac[j]![k]!, 0)));
    const jtr = jac.map((col) => col.reduce((s, v, k) => s + v * r[k]!, 0));
    let improved = false;
    for (let tries = 0; tries < 12 && !improved; tries++) {
      const damped = jtj.map((row, i) => row.map((v, j) => (i === j ? v * (1 + lambda) + 1e-12 : v)));
      const step = solve(damped, jtr.map((v) => -v));
      if (step) {
        const next = [...p] as Params;
        idx.forEach((k, i) => {
          next[k] = Math.min(UPPER[k]!, Math.max(LOWER[k]!, p[k]! + step[i]!));
        });
        const rn = residual(next);
        const cn = costOf(rn);
        if (cn < cost) {
          const moved = Math.max(...next.map((v, i) => Math.abs(v - p[i]!)));
          p = next;
          r = rn;
          cost = cn;
          lambda = Math.max(lambda / 3, 1e-12);
          improved = true;
          if (moved < 1e-13) return { p, cost };
        }
      }
      if (!improved) lambda *= 4;
    }
    if (!improved) break;
  }
  return { p, cost };
}

// ---------------------------------------------------------------------------
// El ajuste
// ---------------------------------------------------------------------------

export type Fit9Error = 'invalid' | 'implausible' | 'diverged';

export interface Fit9Result {
  /** Corrección nueva (la actual con escala, giro, barril y trapecio cambiados). */
  correction: FieldCorrection;
  /** Error medido antes de corregir en cada punto (medido − nominal), mm. */
  errorsBefore: Pt[];
  /** Error que quedaría según el modelo con la corrección nueva, mm. */
  errorsAfter: Pt[];
  maxBefore: number;
  maxAfter: number;
  /** Algún parámetro nuevo salió de lo que admite el motor y se recortó. */
  clamped: boolean;
}

/** Cuánto puede alejarse un punto medido del nominal (fracción del lado de la cuadrícula) antes de tomarlo por un error de medida. */
const MAX_ERROR_FRACTION = 0.25;
const round5 = (n: number) => Math.round(n * 1e5) / 1e5;

/**
 * Corrección nueva a partir de las 9 medidas. `measured[i]` es la posición medida (mm) de la cruz `i` (orden de
 * `grid9Points`) respecto de la cruz del centro (la 5, siempre (0, 0)). Con `fitRotation` falso el giro no se toca
 * (la regla no estaba alineada con el montaje). Devuelve un código si las medidas no sirven.
 */
export function fitNinePoints(fieldMm: number, current: FieldCorrection, measured: Pt[], fitRotation: boolean): Fit9Result | Fit9Error {
  const nominal = grid9Points(fieldMm);
  if (!(fieldMm > 0) || measured.length !== 9 || !measured.every((m) => Number.isFinite(m[0]) && Number.isFinite(m[1]))) return 'invalid';
  const spacing = fieldMm * GRID9_FRACTION;
  const meas = measured.map((m, i): Pt => (i === 4 ? [0, 0] : m));
  const errorsBefore = meas.map((m, i): Pt => [m[0] - nominal[i]![0], m[1] - nominal[i]![1]]);
  if (errorsBefore.some((e) => Math.hypot(e[0], e[1]) > MAX_ERROR_FRACTION * spacing)) return 'implausible';
  const free: boolean[] = [true, true, fitRotation, true, true, true, true];

  // Etapa A: la distorsión que la lente deja, G(N_i) = M_i, de la misma familia y sin desplazamiento ni sesgo.
  const a = levenberg(
    (p) => nominal.flatMap((n, i) => {
      const g = correct(fieldMm, withParams(IDENTITY, p), n);
      return [g[0] - meas[i]![0], g[1] - meas[i]![1]];
    }),
    paramsOf(IDENTITY),
    free,
  );
  const distortion = withParams(IDENTITY, a.p);

  // Etapa B: F' ≈ F0 ∘ G⁻¹ sobre una rejilla de 5 x 5 puntos dentro de la cuadrícula.
  const samples: Pt[] = [];
  for (const y of [-1, -0.5, 0, 0.5, 1]) for (const x of [-1, -0.5, 0, 0.5, 1]) samples.push([x * spacing, y * spacing]);
  const targets: Pt[] = [];
  for (const s of samples) {
    const inv = invert(fieldMm, distortion, s);
    if (!inv) return 'diverged';
    targets.push(forward(fieldMm, current, inv));
  }
  const start = paramsOf(current);
  const b = levenberg(
    (p) => {
      const c = { ...current, ...withParams(current, p) };
      return samples.flatMap((s, i) => {
        const f = forward(fieldMm, c, s);
        return [f[0] - targets[i]![0], f[1] - targets[i]![1]];
      });
    },
    start,
    free,
  );
  if (!b.p.every(Number.isFinite)) return 'diverged';
  const clamped = b.p.some((v, i) => v <= LOWER[i]! + 1e-9 || v >= UPPER[i]! - 1e-9);
  const rounded = b.p.map((v, i) => (i === 2 ? Math.round(v * 1e4) / 1e4 : round5(v))) as Params;
  const next: FieldCorrection = { ...current, ...withParams(current, rounded) };

  // Lo que quedaría: la lente es L = G ∘ F0⁻¹ (en el modelo), así que el aterrizaje de N es G(F0⁻¹(F'(N))).
  const errorsAfter: Pt[] = [];
  for (let i = 0; i < 9; i++) {
    const q = forward(fieldMm, next, nominal[i]!);
    const qd = undoAxes(current, q);
    const back = invert(fieldMm, current, qd);
    if (!back) return 'diverged';
    const land = correct(fieldMm, distortion, back);
    errorsAfter.push([land[0] - nominal[i]![0], land[1] - nominal[i]![1]]);
  }
  const worst = (list: Pt[]) => Math.max(...list.map((e) => Math.hypot(e[0], e[1])));
  return { correction: next, errorsBefore, errorsAfter, maxBefore: worst(errorsBefore), maxAfter: worst(errorsAfter), clamped };
}

/** Inversa de `toAxes`. */
function undoAxes(c: Pick<FieldCorrection, 'invert' | 'swap_xy'>, p: Pt): Pt {
  const a = c.invert[0] ? -p[0] : p[0];
  const b = c.invert[1] ? -p[1] : p[1];
  return c.swap_xy ? [b, a] : [a, b];
}

/** Medidas que daría una lente con la distorsión `lens` cuando la rejilla se marcó con `current` (para probar el ajuste). */
export function simulateMeasures9(fieldMm: number, current: FieldCorrection, lens: Family): Pt[] {
  const nominal = grid9Points(fieldMm);
  const landing = (n: Pt): Pt => correct(fieldMm, lens, undoAxes(current, forward(fieldMm, current, n)));
  const center = landing([0, 0]);
  return nominal.map((n): Pt => {
    const l = landing(n);
    return [l[0] - center[0], l[1] - center[1]];
  });
}

export { IDENTITY as IDENTITY_FAMILY };
