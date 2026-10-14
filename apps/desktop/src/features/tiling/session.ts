import { readPref, writePref } from '@/lib/platform';
import type { Bounds, TileOrder, TileSpec } from './plan';

/** Qué se hizo con un tramo. */
export type TileStatus = 'pending' | 'done' | 'skipped';

/** Opciones que el usuario elige en el asistente. */
export interface TilingConfig {
  /** Tamaño del tramo (ancho, alto), mm. */
  size: [number, number];
  overlap: number;
  order: TileOrder;
  /** Cruces de alineación en las zonas de solape. */
  marks: boolean;
  /** Margen entre el borde de la cama y el tramo, mm. */
  margin: number;
}

/**
 * Avance de un trabajo dividido. Se guarda en el almacenamiento local para retomarlo si la app se
 * cierra o la conexión se pierde a medias; `key` identifica el diseño y las opciones, y solo se
 * ofrece retomar si siguen siendo los mismos.
 */
export interface TileSession {
  key: string;
  specs: TileSpec[];
  status: TileStatus[];
  /** Tramo que toca (o se está enviando). */
  current: number;
  /** Último tramo que se terminó o saltó antes del actual (para guiar el movimiento del material). */
  last?: number;
  /** Se manda el avance del alimentador entre tramos sin pedir confirmación. */
  feeder: boolean;
  design: Bounds;
  config: TilingConfig;
}

const STORAGE_KEY = 'tiling.session';

/** Firma estable del diseño + opciones + máquina. Cambia si cualquiera de ellos cambia. */
export function sessionKey(design: Bounds, config: TilingConfig, deviceId: string): string {
  const r = (n: number) => Math.round(n * 1000) / 1000;
  return JSON.stringify([
    deviceId,
    design.flat().map(r),
    config.size.map(r),
    r(config.overlap),
    config.order,
    config.marks,
    r(config.margin),
  ]);
}

export function newSession(specs: TileSpec[], design: Bounds, config: TilingConfig, deviceId: string, feeder: boolean): TileSession {
  return { key: sessionKey(design, config, deviceId), specs, status: specs.map(() => 'pending'), current: 0, feeder, design, config };
}

/** Marca el tramo `index` y devuelve la sesión nueva. */
export function mark(s: TileSession, index: number, status: TileStatus): TileSession {
  return { ...s, status: s.status.map((v, i) => (i === index ? status : v)) };
}

/** Siguiente tramo pendiente después de `from` (dando la vuelta al final), o `null` si no queda ninguno. */
export function nextPending(s: TileSession, from: number = s.current): number | null {
  const n = s.status.length;
  for (let k = 1; k <= n; k++) {
    const i = (from + k) % n;
    if (s.status[i] === 'pending') return i;
  }
  return null;
}

export function isComplete(s: TileSession): boolean {
  return s.status.every((v) => v !== 'pending');
}

export function counts(s: TileSession): { done: number; skipped: number; pending: number } {
  return {
    done: s.status.filter((v) => v === 'done').length,
    skipped: s.status.filter((v) => v === 'skipped').length,
    pending: s.status.filter((v) => v === 'pending').length,
  };
}

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isPair = (v: unknown): v is [number, number] => Array.isArray(v) && v.length === 2 && v.every(isNum);

function isSpec(v: unknown): v is TileSpec {
  const s = v as TileSpec | null;
  return !!s && Array.isArray(s.rect) && s.rect.length === 2 && isPair(s.rect[0]) && isPair(s.rect[1]) && isPair(s.offset);
}

/** Lee una sesión guardada; `null` si el texto no es una sesión válida. */
export function parseSession(text: string | null): TileSession | null {
  if (!text) return null;
  try {
    const s = JSON.parse(text) as TileSession;
    const ok =
      typeof s.key === 'string' &&
      Array.isArray(s.specs) &&
      s.specs.length > 0 &&
      s.specs.every(isSpec) &&
      Array.isArray(s.status) &&
      s.status.length === s.specs.length &&
      s.status.every((v) => v === 'pending' || v === 'done' || v === 'skipped') &&
      Number.isInteger(s.current) &&
      s.current >= 0 &&
      s.current < s.specs.length &&
      typeof s.feeder === 'boolean' &&
      Array.isArray(s.design) &&
      isPair(s.design[0]) &&
      isPair(s.design[1]) &&
      !!s.config &&
      isPair(s.config.size);
    return ok ? s : null;
  } catch {
    return null;
  }
}

export function saveSession(s: TileSession | null): void {
  writePref(STORAGE_KEY, s ? JSON.stringify(s) : '');
}

export function loadSession(): TileSession | null {
  return parseSession(readPref(STORAGE_KEY));
}
