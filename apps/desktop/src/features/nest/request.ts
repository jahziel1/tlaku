import type { DesignObject, Document } from '@/api/types';
import { objectPolylines } from '@/lib/geometry';
import type { Unit } from '@/features/arrange/units';
import type { NestContainer, NestGravity, NestPieceInput, NestRequest, NestRotationMode } from './types';

/** Tope de instancias (piezas con sus copias): más allá, el cálculo y el documento se vuelven inmanejables. */
export const MAX_NEST_INSTANCES = 2000;
export const NEST_TIME_RANGE = [1, 600] as const;
export const NEST_STEP_RANGE = [0.5, 180] as const;

/** Opciones del diálogo que se recuerdan durante la sesión. */
export interface NestOptions {
  /** De dónde sale el contenedor: la cama o un objeto del documento. */
  containerKind: 'bed' | 'object';
  /** Objeto material elegido (si ya no existe o está seleccionado, se ignora). */
  materialId: string | null;
  spacing: number;
  margin: number;
  rotationMode: NestRotationMode;
  stepDeg: number;
  allowMirror: boolean;
  useHoles: boolean;
  gravity: NestGravity;
  timeLimitS: number;
  seed: number;
}

export const DEFAULT_NEST_OPTIONS: NestOptions = {
  containerKind: 'bed',
  materialId: null,
  spacing: 1,
  margin: 0,
  rotationMode: 'quarter',
  stepDeg: 15,
  allowMirror: false,
  useHoles: false,
  gravity: 'gravity',
  timeLimitS: 20,
  seed: 1,
};

const clamp = (n: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, n));
const finiteOr = (n: number, fallback: number): number => (Number.isFinite(n) ? n : fallback);

/** Copias de una pieza: entero entre 1 y el tope. */
export const sanitizeCopies = (n: number): number => clamp(Math.round(finiteOr(n, 1)), 1, MAX_NEST_INSTANCES);

/** Total de instancias a acomodar (cada pieza por sus copias). Una pieza sin entrada cuenta 1. */
export function totalInstances(units: readonly Pick<Unit, 'id'>[], copies: Readonly<Record<string, number>>): number {
  return units.reduce((sum, u) => sum + sanitizeCopies(copies[u.id] ?? 1), 0);
}

/** Contenedor para el motor: la cama (origen en 0,0) o el objeto material. */
export function resolveContainer(
  kind: NestOptions['containerKind'],
  bed: readonly [number, number],
  material: DesignObject | null,
): NestContainer | null {
  if (kind === 'bed') return { type: 'rect', width: bed[0], height: bed[1] };
  return material ? { type: 'object', object: material } : null;
}

/**
 * Objetos vectoriales con al menos un contorno cerrado y que no están entre `excluded` (las
 * piezas): candidatos a material. Se descartan los ocultos.
 */
export function materialCandidates(doc: Pick<Document, 'objects'>, excluded: ReadonlySet<string>): DesignObject[] {
  return doc.objects.filter(
    (o) => o.kind.type === 'vector' && !o.hidden && !excluded.has(o.id) && objectPolylines(o).some((p) => p.closed && p.points.length >= 3),
  );
}

/** Arma la petición al motor con las opciones saneadas (rangos del contrato). */
export function buildNestRequest(args: {
  units: readonly Unit[];
  copies: Readonly<Record<string, number>>;
  container: NestContainer;
  options: NestOptions;
}): NestRequest {
  const { units, copies, container, options: o } = args;
  const pieces: NestPieceInput[] = units.map((u) => ({
    id: u.id,
    objects: u.objects,
    copies: sanitizeCopies(copies[u.id] ?? 1),
  }));
  return {
    pieces,
    container,
    spacing: Math.max(0, finiteOr(o.spacing, 0)),
    margin: Math.max(0, finiteOr(o.margin, 0)),
    rotation: { mode: o.rotationMode, step_deg: clamp(finiteOr(o.stepDeg, 15), NEST_STEP_RANGE[0], NEST_STEP_RANGE[1]) },
    allow_mirror: o.allowMirror,
    use_holes: o.useHoles,
    gravity: o.gravity,
    time_limit_s: clamp(Math.round(finiteOr(o.timeLimitS, 20)), NEST_TIME_RANGE[0], NEST_TIME_RANGE[1]),
    seed: Math.max(0, Math.floor(finiteOr(o.seed, 1))),
  };
}
