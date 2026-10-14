import type { DesignObject, DeviceProfile, Document, JobOptions, LayerSettings } from '@/api/types';
import type { FieldCorrection } from '@/bindings/FieldCorrection';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { MAX_NONLINEAR, MAX_SCALE, MIN_SCALE } from './galvoProfile';

/**
 * Asistente de calibración del campo (M43): se marca una rejilla de prueba (dos cuadrados concéntricos
 * y una marca del lado +X), se miden con una regla y las medidas dan la escala y el barril de cada eje.
 *
 * Modelo (propio, primer orden): la rejilla se marca con la corrección ACTUAL del perfil, así que lo que se
 * mide es el error que queda. En cada eje, la razón medida/nominal de un cuadrado de semilado `a` es
 * `r(a) = c (1 + k (a/h)²)` con `h` la mitad del campo. Con dos tamaños se resuelven `c` (escala) y `k` (barril),
 * y la corrección que lo deshace es `escala' = escala / c` y `barril' = barril − k`. Se puede repetir hasta que
 * el error sea pequeño. No corrige trapecio ni sesgo.
 */

/** Lado del cuadrado grande y del chico, como fracción del campo. */
export const OUTER_FRACTION = 0.7;
export const INNER_FRACTION = 0.35;
/** Marca del lado +X: un trazo a la derecha del cuadrado grande, de 0,38 a 0,45 del campo desde el centro. */
const MARK_FROM = 0.38;
const MARK_TO = 0.45;

export const CALIB_COLOR = '#ff0000';

/** Lados nominales (mm) de los dos cuadrados. */
export function calibSizes(fieldMm: number): { outer: number; inner: number } {
  return { outer: fieldMm * OUTER_FRACTION, inner: fieldMm * INNER_FRACTION };
}

/** Parámetros con los que se marca la rejilla. */
export interface CalibMark {
  /** Potencia (%). */
  power: number;
  /** Velocidad (mm/s). */
  speedMmS: number;
  frequencyKhz: number;
  pulseWidthNs: number | null;
}


/** Marca de partida de la rejilla: potencia baja a propósito, frecuencia dentro del rango del perfil. */
export function defaultCalibMark(d: Pick<DeviceProfile, 'galvo'>): CalibMark {
  const g = d.galvo;
  const mid = Math.min(Math.max(30, g.freq_min_khz), g.freq_max_khz);
  return { power: 15, speedMmS: 500, frequencyKhz: mid, pulseWidthNs: g.pulse_widths_ns[0] ?? null };
}

const square = (cx: number, cy: number, side: number) => {
  const h = side / 2;
  return [
    [cx - h, cy - h],
    [cx + h, cy - h],
    [cx + h, cy + h],
    [cx - h, cy + h],
  ] as [number, number][];
};

const vec = (id: string, layer: string, paths: ReturnType<typeof polylineBez>[]): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths },
  locked: false,
  hidden: false,
  group: null,
});

/**
 * Documento con la rejilla de prueba centrada en el campo (que está centrado en la cama). La capa lleva los
 * parámetros pedidos; la velocidad se guarda en mm/min.
 */
export function buildCalibDocument(bed: [number, number], fieldMm: number, mark: CalibMark): Document {
  const cx = bed[0] / 2;
  const cy = bed[1] / 2;
  const { outer, inner } = calibSizes(fieldMm);
  const layer: LayerSettings = makeLayer(CALIB_COLOR, 'calibración', {
    mode: 'line',
    speed: mark.speedMmS * 60,
    power: mark.power,
    passes: 1,
  });
  layer.galvo = { ...layer.galvo, frequency_khz: mark.frequencyKhz, pulse_width_ns: mark.pulseWidthNs };
  const xMark = polylineBez(
    [
      [cx + fieldMm * MARK_FROM, cy],
      [cx + fieldMm * MARK_TO, cy],
    ],
    false,
  );
  return {
    version: 2,
    objects: [
      vec('calib-outer', CALIB_COLOR, [polylineBez(square(cx, cy, outer), true)]),
      vec('calib-inner', CALIB_COLOR, [polylineBez(square(cx, cy, inner), true)]),
      vec('calib-x', CALIB_COLOR, [xMark]),
    ],
    layers: [layer],
    device_id: null,
  };
}

/** Opciones de trabajo de la rejilla: absoluto, sin selección ni tramos ni origen de usuario. */
export function calibJobOptions(base: JobOptions): JobOptions {
  return { ...base, start_from: 'absolute', user_origin: null, selection: null, use_selection_origin: false, transform: null, tile: null };
}

/** Lo que se midió, en mm: ancho (en X, a media altura) y alto (en Y, a mitad del ancho) de cada cuadrado. */
export interface CalibMeasures {
  outer: [number, number];
  inner: [number, number];
}

export type FitError = 'invalid' | 'implausible';

export interface FitResult {
  scale: [number, number];
  bulge: [number, number];
  /** Error medido antes de corregir, mm (medida − nominal) por cuadrado y eje. */
  errorOuter: [number, number];
  errorInner: [number, number];
  /** Algún valor nuevo cae fuera de lo que admite el motor y se recortó. */
  clamped: boolean;
}

const round5 = (n: number) => Math.round(n * 1e5) / 1e5;

/** Un eje: devuelve la escala residual `c`, el barril residual `k` y los errores. */
function fitAxis(fieldMm: number, outerMeasured: number, innerMeasured: number): { c: number; k: number } | null {
  const { outer, inner } = calibSizes(fieldMm);
  const h = fieldMm / 2;
  const ro = outerMeasured / outer;
  const ri = innerMeasured / inner;
  const po = (outer / 2 / h) ** 2;
  const pi = (inner / 2 / h) ** 2;
  const m = (ro - ri) / (po - pi);
  const c = ri - m * pi;
  if (!Number.isFinite(c) || c <= 0) return null;
  return { c, k: m / c };
}

/**
 * Corrección nueva a partir de las medidas y de la corrección actual. `null` si las medidas no sirven (cero,
 * negativas, no numéricas) o dan una escala absurda (fuera de 0,5 a 2 veces: se midió mal o el campo es otro).
 */
export function fitCorrection(fieldMm: number, current: Pick<FieldCorrection, 'scale' | 'bulge'>, m: CalibMeasures): FitResult | FitError {
  const all = [...m.outer, ...m.inner];
  if (!(fieldMm > 0) || !all.every((v) => Number.isFinite(v) && v > 0)) return 'invalid';
  const x = fitAxis(fieldMm, m.outer[0], m.inner[0]);
  const y = fitAxis(fieldMm, m.outer[1], m.inner[1]);
  if (!x || !y || [x.c, y.c].some((c) => c < 0.5 || c > 2)) return 'implausible';
  let clamped = false;
  const clamp = (v: number, lo: number, hi: number) => {
    if (v < lo || v > hi) clamped = true;
    return Math.min(hi, Math.max(lo, v));
  };
  const scale: [number, number] = [
    round5(clamp(current.scale[0] / x.c, MIN_SCALE, MAX_SCALE)),
    round5(clamp(current.scale[1] / y.c, MIN_SCALE, MAX_SCALE)),
  ];
  const bulge: [number, number] = [
    round5(clamp(current.bulge[0] - x.k, -MAX_NONLINEAR, MAX_NONLINEAR)),
    round5(clamp(current.bulge[1] - y.k, -MAX_NONLINEAR, MAX_NONLINEAR)),
  ];
  const { outer, inner } = calibSizes(fieldMm);
  return {
    scale,
    bulge,
    errorOuter: [m.outer[0] - outer, m.outer[1] - outer],
    errorInner: [m.inner[0] - inner, m.inner[1] - inner],
    clamped,
  };
}

/** Medidas que daría una lente con escala `c` y barril `k` por eje (para probar el ajuste). */
export function simulateMeasures(fieldMm: number, lens: { c: [number, number]; k: [number, number] }): CalibMeasures {
  const { outer, inner } = calibSizes(fieldMm);
  const h = fieldMm / 2;
  const side = (n: number, axis: 0 | 1) => n * lens.c[axis] * (1 + lens.k[axis] * (n / 2 / h) ** 2);
  return {
    outer: [side(outer, 0), side(outer, 1)],
    inner: [side(inner, 0), side(inner, 1)],
  };
}
