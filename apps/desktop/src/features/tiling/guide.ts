import type { TileStatus } from './session';
import type { MarkSegment, TileSpec } from './types';

/**
 * Guía para mover el material entre tramos cuando no lo hace el alimentador (M47, tiling en varias
 * columnas): en qué columna y fila de la cuadrícula está cada tramo, y qué cruces de alineación
 * (marcas de registro) ya grabadas hay que poner bajo el puntero para colocar el material.
 * Todo se deduce de los rectángulos de los tramos y de las cruces que calculó el motor.
 */

const EPS = 1e-6;

export interface TileCell {
  /** Columna y fila (desde 1; la fila 1 es la de arriba) y cuántas hay. */
  col: number;
  row: number;
  cols: number;
  rows: number;
}

/** Valores distintos (con tolerancia) en orden. */
function distinct(values: number[]): number[] {
  const out: number[] = [];
  for (const v of [...values].sort((a, b) => a - b)) if (out.length === 0 || v - out[out.length - 1]! > EPS) out.push(v);
  return out;
}

/** Posición en la cuadrícula del tramo `index` de `specs`. */
export function tileCell(specs: readonly TileSpec[], index: number): TileCell {
  const xs = distinct(specs.map((s) => s.rect[0][0]));
  // Las filas van de arriba (Y mayor) hacia abajo.
  const ys = distinct(specs.map((s) => s.rect[0][1])).reverse();
  const spec = specs[index]!;
  const at = (list: number[], v: number) => Math.max(0, list.findIndex((x) => Math.abs(x - v) <= EPS));
  return { col: at(xs, spec.rect[0][0]) + 1, row: at(ys, spec.rect[0][1]) + 1, cols: xs.length, rows: ys.length };
}

/** Qué hay que mover entre el tramo `from` y el `to`. */
export type MoveKind = 'column' | 'row' | 'both' | 'none';

export function moveBetween(specs: readonly TileSpec[], from: number, to: number): MoveKind {
  const a = tileCell(specs, from);
  const b = tileCell(specs, to);
  const col = a.col !== b.col;
  const row = a.row !== b.row;
  return col && row ? 'both' : col ? 'column' : row ? 'row' : 'none';
}

/** Centros de las cruces (dos segmentos por cruz): el punto medio del primero. */
export function crossCenters(marks: readonly MarkSegment[]): [number, number][] {
  const out: [number, number][] = [];
  for (let i = 0; i + 1 < marks.length; i += 2) {
    const [a, b] = marks[i]!;
    out.push([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]);
  }
  return out;
}

const inside = (spec: TileSpec, p: readonly [number, number]) =>
  p[0] >= spec.rect[0][0] - EPS && p[0] <= spec.rect[1][0] + EPS && p[1] >= spec.rect[0][1] - EPS && p[1] <= spec.rect[1][1] + EPS;

/**
 * Cruces que sirven para colocar el material del tramo `index`: las que caen dentro de ese tramo y
 * ya se grabaron en algún tramo terminado (están en la zona de solape). Ordenadas de izquierda a
 * derecha y de abajo hacia arriba. Vacío si no hay marcas, o si nada de lo hecho las contiene.
 */
export function registrationMarks(
  specs: readonly TileSpec[],
  status: readonly TileStatus[],
  marks: readonly MarkSegment[],
  index: number,
): [number, number][] {
  const target = specs[index];
  if (!target) return [];
  const engraved = specs.filter((_, i) => i !== index && status[i] === 'done');
  return crossCenters(marks)
    .filter((c) => inside(target, c) && engraved.some((s) => inside(s, c)))
    .sort((a, b) => a[0] - b[0] || a[1] - b[1]);
}

/**
 * Dónde cae el punto `p` del documento en la cama con el material colocado para el tramo `spec`
 * (el desplazamiento del tramo ya lleva el margen). Es adonde se lleva el puntero.
 */
export function bedPoint(spec: TileSpec, p: readonly [number, number]): [number, number] {
  return [Math.round((p[0] + spec.offset[0]) * 1000) / 1000, Math.round((p[1] + spec.offset[1]) * 1000) / 1000];
}
