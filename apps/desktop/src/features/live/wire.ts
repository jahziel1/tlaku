import { EMPTY_SNAPSHOT, type LiveSnapshot } from './source';
import type { LiveReading } from './service';
import type { RouteShape } from './route';

/**
 * Formato de lo que la ventana principal le manda a la ventana aparte de la vista en vivo
 * (`live_window_send`): dos mensajes de texto JSON.
 *
 *  - `route`: la ruta de un trabajo (posiciones, clases, tiempos, capas), con los arreglos en base64;
 *  - `state`: la foto de baja frecuencia (`LiveSnapshot`) y el último reporte de la máquina (`LiveReading`).
 *
 * Todo lo que se decodifica se valida: la ventana aparte no confía en lo que llega.
 */

const VERSION = 1;
/** Rutas más grandes que esto (movimientos) no se mandan: la ventana aparte avisa que no está disponible. */
export const MAX_WIRE_MOVES = 1_500_000;

const CHUNK = 0x8000;

export function bytesToBase64(bytes: Uint8Array): string {
  let out = '';
  for (let i = 0; i < bytes.length; i += CHUNK) out += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  return btoa(out);
}

export function base64ToBytes(text: string): Uint8Array {
  const bin = atob(text);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

const view = (a: ArrayBufferView) => new Uint8Array(a.buffer, a.byteOffset, a.byteLength);

interface WireRoute {
  v: number;
  seq: number;
  count: number;
  total: number;
  bounds: [[number, number], [number, number]];
  layers: string[];
  approximate: boolean;
  layerStarts: number[] | null;
  /** Float32 little-endian, (count + 1) puntos x, y. */
  pos: string;
  /** Uint8, uno por movimiento. */
  cls: string;
  /** Float32 little-endian, segundos acumulados por movimiento. */
  times: string;
}

/** Texto de la ruta del trabajo `seq`, o `null` si es demasiado grande para mandarla. */
export function encodeRoute(seq: number, route: RouteShape): string | null {
  if (route.count > MAX_WIRE_MOVES) return null;
  const m = route.model;
  const times = new Float32Array(route.count);
  for (let i = 0; i < route.count; i++) times[i] = m.times[i]!;
  const w: WireRoute = {
    v: VERSION,
    seq,
    count: route.count,
    total: route.total,
    bounds: route.bounds,
    layers: route.layerColors,
    approximate: m.approximate,
    layerStarts: m.layerStarts ? Array.from(m.layerStarts) : null,
    pos: bytesToBase64(view(Float32Array.from(m.pos.subarray(0, 2 * (route.count + 1))))),
    cls: bytesToBase64(view(Uint8Array.from(m.cls.subarray(0, route.count)))),
    times: bytesToBase64(view(times)),
  };
  return JSON.stringify(w);
}

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isPoint = (v: unknown): v is [number, number] => Array.isArray(v) && v.length === 2 && isNum(v[0]) && isNum(v[1]);

/** Alinea los bytes a un múltiplo para poder leerlos como `Float32Array` (los de `atob` ya vienen alineados, pero se copia por si acaso). */
const f32 = (bytes: Uint8Array): Float32Array => new Float32Array(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));

/** La ruta de un mensaje `route`; `null` si no es válida. */
export function decodeRoute(text: string): { seq: number; route: RouteShape } | null {
  try {
    const w = JSON.parse(text) as Partial<WireRoute>;
    if (w.v !== VERSION || !isNum(w.seq) || !isNum(w.count) || !isNum(w.total) || w.count < 1 || w.count > MAX_WIRE_MOVES) return null;
    if (!(w.total > 0) || !Array.isArray(w.bounds) || !isPoint(w.bounds[0]) || !isPoint(w.bounds[1])) return null;
    if (!Array.isArray(w.layers) || !w.layers.every((c) => typeof c === 'string' && /^#[0-9a-f]{6}$/i.test(c))) return null;
    if (typeof w.pos !== 'string' || typeof w.cls !== 'string' || typeof w.times !== 'string') return null;
    const pos = f32(base64ToBytes(w.pos));
    const cls = base64ToBytes(w.cls);
    const times = f32(base64ToBytes(w.times));
    if (pos.length !== 2 * (w.count + 1) || cls.length !== w.count || times.length !== w.count) return null;
    const starts = w.layerStarts;
    if (starts !== null && starts !== undefined && !(Array.isArray(starts) && starts.every(isNum))) return null;
    return {
      seq: w.seq,
      route: {
        model: { pos, cls, times: Float64Array.from(times), approximate: !!w.approximate, layerStarts: starts ? Int32Array.from(starts) : null },
        count: w.count,
        total: w.total,
        layerColors: w.layers,
        bounds: [[w.bounds[0][0], w.bounds[0][1]], [w.bounds[1][0], w.bounds[1][1]]],
      },
    };
  } catch {
    return null;
  }
}

export interface WireState {
  v: number;
  snap: LiveSnapshot;
  reading: LiveReading | null;
}

export function encodeState(snap: LiveSnapshot, reading: LiveReading | null): string {
  const w: WireState = { v: VERSION, snap, reading };
  return JSON.stringify(w);
}

const PHASES = ['idle', 'running', 'paused', 'finished', 'cancelled', 'error'];
const ROUTES = ['none', 'computing', 'ready', 'unavailable'];
const SOURCES = ['position', 'progress', 'clock'];
const MODES = ['running', 'hold', 'frozen', 'done'];
const numOrNull = (v: unknown): number | null => (isNum(v) ? v : null);

/** El estado de un mensaje `state`; `null` si no es válido. */
export function decodeState(text: string): { snap: LiveSnapshot; reading: LiveReading | null } | null {
  try {
    const w = JSON.parse(text) as { v?: unknown; snap?: Record<string, unknown>; reading?: Record<string, unknown> | null };
    const s = w.snap;
    if (w.v !== VERSION || !s || typeof s !== 'object') return null;
    const bed = s.bed;
    const timing = (s.timing ?? {}) as Record<string, unknown>;
    if (!isPoint(bed) || !(bed[0] > 0) || !(bed[1] > 0)) return null;
    if (!isNum(s.jobSeq) || !ROUTES.includes(s.route as string) || !PHASES.includes(s.job as string) || !isNum(s.fraction)) return null;
    const snap: LiveSnapshot = {
      ...EMPTY_SNAPSHOT,
      jobSeq: s.jobSeq,
      route: s.route as LiveSnapshot['route'],
      source: SOURCES.includes(s.source as string) ? (s.source as LiveSnapshot['source']) : null,
      fraction: Math.min(1, Math.max(0, s.fraction)),
      emitting: s.emitting === true,
      ended: s.ended === 'finished' || s.ended === 'cancelled' || s.ended === 'error' ? s.ended : null,
      job: s.job as LiveSnapshot['job'],
      timing: { jobStartedAt: numOrNull(timing.jobStartedAt), jobPausedAt: numOrNull(timing.jobPausedAt), jobPausedMs: numOrNull(timing.jobPausedMs) ?? 0, jobEndedAt: numOrNull(timing.jobEndedAt) },
      total: numOrNull(s.total),
      bed: [bed[0], bed[1]],
      showTravel: s.showTravel === true,
      theme: s.theme === 'light' ? 'light' : 'dark',
      lang: s.lang === 'en' ? 'en' : 'es',
    };
    const r = w.reading;
    let reading: LiveReading | null = null;
    if (r && isNum(r.jobSeq) && isNum(r.t) && MODES.includes(r.mode as string) && isNum(r.retreatSeq)) {
      reading = { jobSeq: r.jobSeq, t: r.t, mode: r.mode as LiveReading['mode'], retreatSeq: r.retreatSeq, spindle: isNum(r.spindle) ? r.spindle : 0 };
    }
    return { snap, reading };
  } catch {
    return null;
  }
}
