import type { Affine, Document, Point } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { groupDoc, groupIdsOf, regroupCopies } from '@/lib/groups';
import { multiplyAffine, type Bounds } from '@/lib/geometry';
import { uniqueId } from '@/store/document';
import { boundsCenter, boundsHeight, boundsWidth, type Unit } from './units';

/** Tope de piezas de un arreglo: más allá, el documento y el historial se vuelven inmanejables. */
export const MAX_ARRAY_PIECES = 2000;

export interface GridParams {
  columns: number;
  rows: number;
  /** Separación horizontal: hueco entre bordes (`gap`) o paso entre centros (`pitch`), según `mode`. */
  spacingX: number;
  spacingY: number;
  mode: 'gap' | 'pitch';
  group: boolean;
}

export interface CircularParams {
  /** Piezas en total, contando la original. */
  copies: number;
  centerX: number;
  centerY: number;
  /** Ángulo total en grados, antihorario (negativo = horario). 360 reparte en círculo completo. */
  angle: number;
  /** Gira cada copia hacia el centro (como las marcas de un reloj); apagado mantiene la orientación. */
  rotate: boolean;
  group: boolean;
}

export const DEFAULT_GRID: GridParams = { columns: 3, rows: 3, spacingX: 5, spacingY: 5, mode: 'gap', group: false };
export const DEFAULT_CIRCULAR: Omit<CircularParams, 'centerX' | 'centerY'> = { copies: 6, angle: 360, rotate: true, group: false };
/** Radio con el que se propone el centro de un arreglo circular nuevo. */
export const DEFAULT_RADIUS = 30;

export interface ArrayPlan {
  /** Una transformación por copia nueva (sin la original), en coordenadas de documento. */
  matrices: Affine[];
  /** Piezas en total, con la original. */
  count: number;
  /** Caja de todo el arreglo (algo mayor que la real si las copias giran). */
  bounds: Bounds;
  size: [number, number];
  outOfBed: boolean;
  /** Contorno de la caja de cada pieza, para la vista previa. */
  outlines: Point[][];
  /** Por qué no se puede aplicar, si no se puede. */
  issue: I18nKey | null;
}

/** Redondea a 1 nm (y evita el -0) para que las posiciones no arrastren ruido de coma flotante. */
const round6 = (n: number) => Math.round(n * 1e6) / 1e6 + 0;

export const translationMatrix = (dx: number, dy: number): Affine => [1, 0, 0, 1, round6(dx), round6(dy)];

/** Giro antihorario de `deg` grados alrededor de (cx, cy). */
export function rotationAbout(cx: number, cy: number, deg: number): Affine {
  const a = (deg * Math.PI) / 180;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy];
}

const corners = (b: Bounds): Point[] => [b[0], [b[1][0], b[0][1]], b[1], [b[0][0], b[1][1]]];
const place = (m: Affine, [x, y]: Point): Point => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];

function summarize(block: Bounds, matrices: Affine[], bed: Bounds, issue: I18nKey | null): ArrayPlan {
  const outlines = [corners(block), ...matrices.map((m) => corners(block).map((p) => place(m, p)))];
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const o of outlines) {
    for (const [x, y] of o) {
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
  }
  const bounds: Bounds = [[minX, minY], [maxX, maxY]];
  const tol = 1e-6;
  return {
    matrices,
    count: matrices.length + 1,
    bounds,
    size: [boundsWidth(bounds), boundsHeight(bounds)],
    outOfBed: minX < bed[0][0] - tol || minY < bed[0][1] - tol || maxX > bed[1][0] + tol || maxY > bed[1][1] + tol,
    outlines,
    issue,
  };
}

/** Paso entre piezas (de origen a origen) según el modo. */
export function gridSteps(p: GridParams, block: Bounds): [number, number] {
  return p.mode === 'pitch'
    ? [p.spacingX, p.spacingY]
    : [boundsWidth(block) + p.spacingX, boundsHeight(block) + p.spacingY];
}

/** Rejilla: la original es la esquina inferior izquierda; columnas hacia la derecha y filas hacia arriba. */
export function planGrid(p: GridParams, block: Bounds, bed: Bounds): ArrayPlan {
  const columns = Math.round(p.columns);
  const rows = Math.round(p.rows);
  const finite = [p.spacingX, p.spacingY].every(Number.isFinite);
  if (!(columns >= 1) || !(rows >= 1) || !finite) return summarize(block, [], bed, 'arrange.array.err.counts');
  if (columns * rows > MAX_ARRAY_PIECES) return summarize(block, [], bed, 'arrange.array.err.tooMany');
  const [sx, sy] = gridSteps(p, block);
  const matrices: Affine[] = [];
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < columns; i++) {
      if (i === 0 && j === 0) continue;
      matrices.push(translationMatrix(i * sx, j * sy));
    }
  }
  return summarize(block, matrices, bed, columns * rows === 1 ? 'arrange.array.err.single' : null);
}

/** Radio: distancia del centro de giro al centro del bloque. */
export function radiusOf(block: Bounds, p: Pick<CircularParams, 'centerX' | 'centerY'>): number {
  const [bx, by] = boundsCenter(block);
  return Math.hypot(bx - p.centerX, by - p.centerY);
}

/** Nuevo centro de giro con el radio dado, en la misma dirección (por defecto, debajo del bloque). */
export function centerForRadius(block: Bounds, p: Pick<CircularParams, 'centerX' | 'centerY'>, radius: number): [number, number] {
  const [bx, by] = boundsCenter(block);
  const d = Math.hypot(bx - p.centerX, by - p.centerY);
  const [ux, uy] = d > 1e-9 ? [(p.centerX - bx) / d, (p.centerY - by) / d] : [0, -1];
  return [round6(bx + ux * radius), round6(by + uy * radius)];
}

/** Arreglo circular alrededor de (centerX, centerY): la pieza original es la primera del reparto. */
export function planCircular(p: CircularParams, block: Bounds, bed: Bounds): ArrayPlan {
  const n = Math.round(p.copies);
  let issue: I18nKey | null = null;
  if (!(n >= 2)) issue = 'arrange.array.err.copies';
  else if (n > MAX_ARRAY_PIECES) issue = 'arrange.array.err.tooMany';
  else if (![p.centerX, p.centerY, p.angle].every(Number.isFinite)) issue = 'arrange.array.err.counts';
  if (issue) return summarize(block, [], bed, issue);
  // 360° (o más) reparte en círculo completo sin repetir la primera pieza; un arco incluye ambos extremos.
  const step = Math.abs(p.angle) >= 360 - 1e-9 ? p.angle / n : p.angle / (n - 1);
  const [bx, by] = boundsCenter(block);
  const matrices: Affine[] = [];
  for (let k = 1; k < n; k++) {
    const rot = rotationAbout(p.centerX, p.centerY, k * step);
    if (p.rotate) matrices.push(rot);
    else {
      const [px, py] = place(rot, [bx, by]);
      matrices.push(translationMatrix(px - bx, py - by));
    }
  }
  return summarize(block, matrices, bed, null);
}

/** Redondea la traslación a 1 nm para que las posiciones no arrastren ruido de coma flotante. */
const tidy = (m: Affine): Affine => [m[0], m[1], m[2], m[3], round6(m[4]), round6(m[5])];

/**
 * Agrega una copia del bloque de `units` por cada matriz. Las copias llevan ids y grupos propios,
 * quedan encima de todo y, con `group`, forman un grupo con las originales. Devuelve el documento
 * nuevo y los ids de las copias.
 */
export function buildArray(doc: Document, units: readonly Unit[], matrices: readonly Affine[], group: boolean): { doc: Document; created: string[] } {
  const source = units.flatMap((u) => u.objects);
  const taken = new Set(doc.objects.map((o) => o.id));
  const groups = groupIdsOf(doc.objects);
  const created: string[] = [];
  const copies = matrices.flatMap((m) =>
    regroupCopies(
      source.map((o) => {
        const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
        taken.add(id);
        created.push(id);
        return { ...o, id, locked: false, transform: tidy(multiplyAffine(m, o.transform)) };
      }),
      groups,
    ),
  );
  if (copies.length === 0) return { doc, created };
  const next = { ...doc, objects: [...doc.objects, ...copies] };
  return { doc: group ? groupDoc(next, [...source.map((o) => o.id), ...created]) : next, created };
}
