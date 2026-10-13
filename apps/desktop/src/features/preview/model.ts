import type { JobPreview } from '@/api/types';

/**
 * Modelo de datos de la vista previa: convierte el `JobPreview` del motor (bloques
 * `[kind, x, y, power]` en un arreglo plano) en arreglos tipados que el dibujo y la reproducción
 * recorren sin asignar memoria por cuadro. No depende de React, del DOM ni del store.
 */

/** `move_times` lo agrega el motor: segundos acumulados AL TERMINAR cada movimiento. */
export type PreviewData = JobPreview & { move_times?: number[] | null };

/** Clase visual de un movimiento. El valor es el índice en los arreglos de estilo. */
export const MoveClass = { Travel: 0, Cut: 1, Engrave: 2, Overscan: 3 } as const;
export type MoveClassId = (typeof MoveClass)[keyof typeof MoveClass];
export const CLASS_COUNT = 4;

/** Niveles de opacidad por potencia (0 = muy baja ... 3 = alta). */
export const POWER_LEVELS = 4;
/** Opacidad del trazo por nivel de potencia. */
export const LEVEL_ALPHA: readonly number[] = [0.38, 0.55, 0.76, 1];

/** Sin datos de modo de capa, desde esta potencia (%) un trazo con láser se trata como corte. */
export const CUT_POWER_THRESHOLD = 60;
/** Velocidad en vacío si no se conoce la del perfil (mm/min). */
export const DEFAULT_RAPID_MM_MIN = 10000;

export interface PreviewViewOptions {
  showTravel: boolean;
  showOverscan: boolean;
  /** Índice en `preview.layers` de la capa resaltada, o null. */
  highlightLayer: number | null;
}

export const PLAY_SPEEDS = [1, 2, 5, 10, 50] as const;
export type PlaySpeed = (typeof PLAY_SPEEDS)[number];

export interface ClassTotals {
  mm: number;
  seconds: number;
}
/** Distancia y tiempo por clase de movimiento. */
export type Distances = Record<'travel' | 'cut' | 'engrave' | 'overscan', ClassTotals>;

export interface PreviewModel {
  /** El arreglo `moves` original: sirve de llave de la caché. */
  source: ArrayLike<number>;
  count: number;
  /** n + 1 puntos (x, y): el inicio del movimiento i es el punto i y su final el punto i + 1. */
  pos: Float32Array;
  cls: Uint8Array;
  level: Uint8Array;
  /** Segundos acumulados al terminar cada movimiento. */
  times: Float64Array;
  /** Duración del recorrido en la línea de tiempo (s). */
  total: number;
  /** true si los tiempos se estimaron por distancia porque el motor no los entregó. */
  approximate: boolean;
  /**
   * Primer movimiento de cada capa (+ el final): `layers.length + 1` valores, o null si no hay capas.
   * Solo son fiables con `approximate === false`.
   */
  layerStarts: Int32Array | null;
  distances: Distances;
  /** Lo mismo por capa (solo con `layerStarts`). */
  layerDistances: Distances[] | null;
}

export function powerLevel(power: number): number {
  if (!(power > 0)) return POWER_LEVELS - 1;
  return Math.min(POWER_LEVELS - 1, Math.max(0, Math.ceil(power / 25) - 1));
}

/** Clase de un movimiento. Con el modo de la capa, 'line' es corte y el resto grabado. */
export function classify(kind: number, power: number, layerMode?: string): MoveClassId {
  if (kind === 0) return MoveClass.Travel;
  if (!(power > 0)) return MoveClass.Overscan;
  if (layerMode) return layerMode === 'line' ? MoveClass.Cut : MoveClass.Engrave;
  return power >= CUT_POWER_THRESHOLD ? MoveClass.Cut : MoveClass.Engrave;
}

// ---- tiempos ----------------------------------------------------------------------------------

/** Valida `move_times`: un valor finito por movimiento y sin retrocesos. Si no, null. */
export function sanitizeMoveTimes(raw: ArrayLike<number> | null | undefined, count: number): Float64Array | null {
  if (!raw || count === 0 || raw.length !== count) return null;
  const out = new Float64Array(count);
  let prev = 0;
  for (let i = 0; i < count; i++) {
    const v = raw[i]!;
    if (!Number.isFinite(v) || v < prev - 1e-9) return null;
    out[i] = v;
    prev = v;
  }
  return out;
}

/**
 * Tiempos aproximados cuando el motor no entrega `move_times`: cada movimiento dura
 * distancia / velocidad (vacío a `rapidMmMin`; el resto a la velocidad que cuadra con `totalS`)
 * y la curva se reescala para que termine en `totalS`.
 */
export function fallbackTimes(
  pos: Float32Array,
  isTravel: (i: number) => boolean,
  count: number,
  totalS: number,
  rapidMmMin: number = DEFAULT_RAPID_MM_MIN,
): Float64Array {
  const out = new Float64Array(count);
  if (count === 0) return out;
  const dist = new Float64Array(count);
  let travelMm = 0;
  let workMm = 0;
  for (let i = 0; i < count; i++) {
    const dx = pos[2 * i + 2]! - pos[2 * i]!;
    const dy = pos[2 * i + 3]! - pos[2 * i + 1]!;
    const d = Math.sqrt(dx * dx + dy * dy);
    dist[i] = d;
    if (isTravel(i)) travelMm += d;
    else workMm += d;
  }
  const rapid = rapidMmMin > 0 ? rapidMmMin / 60 : DEFAULT_RAPID_MM_MIN / 60; // mm/s
  const travelS = travelMm / rapid;
  // Velocidad de trabajo que hace que el total cuadre; si no hay total, una por defecto.
  const workSpeed =
    workMm > 0 ? (totalS > travelS + 1e-6 ? workMm / (totalS - travelS) : Math.max(rapid / 10, 1)) : 1;
  let t = 0;
  for (let i = 0; i < count; i++) {
    t += dist[i]! / (isTravel(i) ? rapid : workSpeed);
    out[i] = t;
  }
  if (totalS > 0 && t > 0) {
    const k = totalS / t;
    for (let i = 0; i < count; i++) out[i] = out[i]! * k;
  }
  return out;
}

/** Cantidad de movimientos terminados en el instante `t` (los que tienen times[i] <= t). */
export function completedMoves(times: ArrayLike<number>, t: number): number {
  let lo = 0;
  let hi = times.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (times[mid]! <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Primer índice con times[i] >= t (búsqueda binaria). */
export function lowerBound(times: ArrayLike<number>, t: number): number {
  let lo = 0;
  let hi = times.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (times[mid]! < t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

export interface HeadState {
  /** Movimientos terminados. */
  done: number;
  /** Posición del cabezal (mm). */
  x: number;
  y: number;
  /** Fracción (0..1) del movimiento en curso; 1 si ya terminó todo. */
  frac: number;
  /** El recorrido terminó. */
  finished: boolean;
  /** Clase del movimiento en curso (null al terminar). */
  cls: MoveClassId | null;
}

/** Posición del cabezal en el instante `t`, interpolando linealmente dentro del movimiento. */
export function headAt(model: Pick<PreviewModel, 'count' | 'pos' | 'times' | 'cls'>, t: number): HeadState {
  const n = model.count;
  if (n === 0) return { done: 0, x: 0, y: 0, frac: 1, finished: true, cls: null };
  const k = completedMoves(model.times, t);
  if (k >= n) {
    return { done: n, x: model.pos[2 * n]!, y: model.pos[2 * n + 1]!, frac: 1, finished: true, cls: null };
  }
  const t0 = k > 0 ? model.times[k - 1]! : 0;
  const t1 = model.times[k]!;
  const frac = t1 > t0 ? Math.min(1, Math.max(0, (t - t0) / (t1 - t0))) : 1;
  const x0 = model.pos[2 * k]!;
  const y0 = model.pos[2 * k + 1]!;
  const x1 = model.pos[2 * k + 2]!;
  const y1 = model.pos[2 * k + 3]!;
  return { done: k, x: x0 + (x1 - x0) * frac, y: y0 + (y1 - y0) * frac, frac, finished: false, cls: model.cls[k] as MoveClassId };
}

/** Avanza la reproducción `dtSeconds` de reloj a `speed`x. */
export function advanceScrub(scrub: number, dtSeconds: number, speed: number, total: number): { scrub: number; ended: boolean } {
  if (!(total > 0)) return { scrub: 1, ended: true };
  const next = scrub + (dtSeconds * speed) / total;
  return next >= 1 ? { scrub: 1, ended: true } : { scrub: Math.max(0, next), ended: false };
}

// ---- construcción -------------------------------------------------------------------------------

function emptyDistances(): Distances {
  const z = (): ClassTotals => ({ mm: 0, seconds: 0 });
  return { travel: z(), cut: z(), engrave: z(), overscan: z() };
}

const CLASS_KEYS = ['travel', 'cut', 'engrave', 'overscan'] as const;

/** Suma distancia y tiempo por clase en los movimientos [from, to). */
export function sumDistances(
  model: Pick<PreviewModel, 'pos' | 'cls' | 'times'>,
  from: number,
  to: number,
): Distances {
  const out = emptyDistances();
  for (let i = from; i < to; i++) {
    const dx = model.pos[2 * i + 2]! - model.pos[2 * i]!;
    const dy = model.pos[2 * i + 3]! - model.pos[2 * i + 1]!;
    const slot = out[CLASS_KEYS[model.cls[i]!]!];
    slot.mm += Math.sqrt(dx * dx + dy * dy);
    slot.seconds += model.times[i]! - (i > 0 ? model.times[i - 1]! : 0);
  }
  return out;
}

/**
 * Primer movimiento de cada capa, deducido de los segundos de cada bloque: los bloques se
 * ejecutan en orden, así que el bloque k empieza donde el tiempo acumulado llega a la suma de
 * los anteriores. Devuelve `layers.length + 1` valores (el último es `count`).
 */
export function layerStartIndices(times: Float64Array, layerSeconds: number[], total: number): Int32Array {
  const starts = new Int32Array(layerSeconds.length + 1);
  const sum = layerSeconds.reduce((a, b) => a + b, 0);
  let acc = 0;
  for (let k = 1; k < layerSeconds.length; k++) {
    acc += layerSeconds[k - 1]!;
    // El movimiento que termina justo en el límite todavía es de la capa anterior.
    starts[k] = sum > 0 ? completedMoves(times, (acc / sum) * total + 1e-9) : 0;
    if (starts[k]! < starts[k - 1]!) starts[k] = starts[k - 1]!;
  }
  starts[layerSeconds.length] = times.length;
  return starts;
}

/** Construye el modelo (sin caché). `rapidMmMin` solo se usa si hay que aproximar tiempos. */
export function buildModel(data: PreviewData, rapidMmMin: number = DEFAULT_RAPID_MM_MIN): PreviewModel {
  const m = data.moves;
  const n = Math.floor(m.length / 4);
  const pos = new Float32Array(2 * (n + 1));
  const cls = new Uint8Array(n);
  const level = new Uint8Array(n);
  const travel = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    const kind = m[4 * i]!;
    const power = m[4 * i + 3]!;
    pos[2 * i + 2] = m[4 * i + 1]!;
    pos[2 * i + 3] = m[4 * i + 2]!;
    cls[i] = classify(kind, power);
    level[i] = kind === 0 ? POWER_LEVELS - 1 : powerLevel(power);
    travel[i] = kind === 0 ? 1 : 0;
  }

  let times = sanitizeMoveTimes(data.move_times, n);
  const approximate = times === null;
  if (!times) times = fallbackTimes(pos, (i) => travel[i] === 1, n, data.total_s, rapidMmMin);
  const total = n > 0 ? Math.max(times[n - 1]!, 0) : 0;

  // Los límites de cada capa salen de los segundos de cada bloque (exactos con `move_times`, aproximados
  // si no): el modo de la capa decide si un trazo con láser es corte (line) o grabado (fill, image).
  let layerStarts: Int32Array | null = null;
  if (data.layers.length > 0 && n > 0) {
    layerStarts = layerStartIndices(times, data.layers.map((l) => l.seconds), total);
    for (let k = 0; k < data.layers.length; k++) {
      const mode = data.layers[k]!.mode;
      for (let i = layerStarts[k]!; i < layerStarts[k + 1]!; i++) {
        if (cls[i] === MoveClass.Cut || cls[i] === MoveClass.Engrave) cls[i] = classify(1, m[4 * i + 3]!, mode);
      }
    }
  }

  const partial = { pos, cls, times };
  const distances = sumDistances(partial, 0, n);
  const layerDistances = layerStarts
    ? data.layers.map((_, k) => sumDistances(partial, layerStarts![k]!, layerStarts![k + 1]!))
    : null;

  return { source: m, count: n, pos, cls, level, times, total, approximate, layerStarts, distances, layerDistances };
}

const cache = new WeakMap<object, PreviewModel>();

/** Modelo de un `JobPreview`, calculado una vez por arreglo de movimientos. */
export function getModel(data: PreviewData, rapidMmMin?: number): PreviewModel {
  const hit = cache.get(data.moves);
  if (hit) return hit;
  const model = buildModel(data, rapidMmMin);
  cache.set(data.moves, model);
  return model;
}
